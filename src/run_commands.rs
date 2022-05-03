use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempdir::TempDir;

/// get a process::Command as String
fn get_cmd_string(cmd: &std::process::Command) -> String {
    let envs: String = cmd
        .get_envs()
        .filter(|(_, y)| y.is_some())
        .map(|(x, y)| format!("{}={}", x.to_string_lossy(), y.unwrap().to_string_lossy()))
        .collect::<Vec<String>>()
        .join(" ");
    let command = format!("{:?}", cmd);
    format!("\"{}\" {}", envs, command).replace('"', "")
}

pub(crate) fn run_rustc(
    executable: &str,
    file: &Path,
    incremental: bool,
    rustc_flags: &[&str],
) -> (Output, String, Vec<OsString>) {
    if incremental {
        // only run incremental compilation tests
        return run_rustc_incremental(executable, file);
    }
    // if the file contains no "main", run with "--crate-type lib"
    let has_main = std::fs::read_to_string(&file)
        .unwrap_or_default()
        .contains("fn main(");

    //let tempdir = TempDir::new("rustc_testrunner_tmpdir").unwrap();
    //let tempdir_path = tempdir.path();
    let output_file = String::from("-o/dev/null");
    let dump_mir_dir = String::from("-Zdump-mir-dir=/dev/null");

    let mut cmd = Command::new(executable);
    cmd.arg(&file)
        .args(rustc_flags)
        // always keep these:
        .arg(&output_file)
        .arg(&dump_mir_dir);
    if !has_main {
        cmd.args(&["--crate-type", "lib"]);
    }
    //dbg!(&cmd);

    let actual_args = cmd
        .get_args()
        .map(|s| s.to_owned())
        .collect::<Vec<OsString>>();

    // run the command
    let output = systemdrun_command(&mut cmd)
        .unwrap_or_else(|_| panic!("Error: {:?}, executable: {:?}", cmd, executable));
    (output, get_cmd_string(&cmd), actual_args)
    // remove tempdir
    //tempdir.close().unwrap();
}

pub(crate) fn run_rustc_incremental(
    executable: &str,
    file: &Path,
) -> (Output, String, Vec<OsString>) {
    let tempdir = TempDir::new("rustc_testrunner_tmpdir").unwrap();
    let tempdir_path = tempdir.path();

    let has_main = std::fs::read_to_string(&file)
        .unwrap_or_default()
        .contains("fn main(");

    let mut cmd = Command::new("DUMMY");
    let mut output = None;
    let mut actual_args = Vec::new();
    for i in &[0, 1] {
        let mut command = Command::new(executable);
        if !has_main {
            command.args(&["--crate-type", "lib"]);
        }
        command
            .arg(&file)
            // avoid error: the generated executable for the input file  .. onflicts with the existing directory..
            .arg(format!("-o{}/{}", tempdir_path.display(), i))
            .arg(format!("-Cincremental={}", tempdir_path.display()))
            .arg("-Zincremental-verify-ich=yes")
            // also enable debuginfo for incremental, since we are codegenning anyway
            .arg("-Cdebuginfo=2")
            .arg("--edition=2021");

        //dbg!(&command);

        output = Some(systemdrun_command(&mut command));
        actual_args = command
            .get_args()
            .map(|s| s.to_owned())
            .collect::<Vec<OsString>>();
        //dbg!(&output);
        cmd = command;
    }

    let output = output.map(|output| output.unwrap()).unwrap();

    tempdir.close().unwrap();
    //dbg!(&output);
    (output, get_cmd_string(&cmd), actual_args)
}

pub(crate) fn run_clippy(executable: &str, file: &Path) -> (Output, String, Vec<OsString>) {
    let has_main = std::fs::read_to_string(&file)
        .unwrap_or_default()
        .contains("pub(crate) fn main(");
    let mut cmd = Command::new(executable);

    if !has_main {
        cmd.args(&["--crate-type", "lib"]);
    }
    cmd.env("RUSTFLAGS", "-Z force-unstable-if-unmarked")
        .env("SYSROOT", "/home/matthias/.rustup/toolchains/master")
        .arg(&file)
        .arg("-Aclippy::cargo") // allow cargo lints
        //.arg("-Wclippy::internal")
        .arg("-Wclippy::pedantic")
        .arg("-Wclippy::nursery")
        .arg("-Wmissing-doc-code-examples")
        .arg("-Wabsolute-paths-not-starting-with-crate")
        .arg("-Wbare-trait-objects")
        .arg("-Wbox-pointers")
        .arg("-Welided-lifetimes-in-paths")
        .arg("-Wellipsis-inclusive-range-patterns")
        .arg("-Wkeyword-idents")
        .arg("-Wmacro-use-extern-crate")
        .arg("-Wmissing-copy-implementations")
        .arg("-Wmissing-debug-implementations")
        .arg("-Wmissing-docs")
        .arg("-Wsingle-use-lifetimes")
        .arg("-Wtrivial-casts")
        .arg("-Wtrivial-numeric-casts")
        .arg("-Wunreachable-pub")
        .arg("-Wunsafe-code")
        .arg("-Wunstable-features")
        .arg("-Wunused-extern-crates")
        .arg("-Wunused-import-braces")
        .arg("-Wunused-labels")
        .arg("-Wunused-lifetimes")
        .arg("-Wunused-qualifications")
        .arg("-Wunused-results")
        .arg("-Wvariant-size-differences")
        .args(&["--cap-lints", "warn"])
        .args(&["-o", "/dev/null"]);

    let output = systemdrun_command(&mut cmd);
    (output.unwrap(), get_cmd_string(&cmd), Vec::new())
}

pub(crate) fn run_rustdoc(executable: &str, file: &Path) -> (Output, String, Vec<OsString>) {
    let mut cmd = Command::new(executable);
    cmd.env("RUSTFLAGS", "-Z force-unstable-if-unmarked")
        .env("SYSROOT", "/home/matthias/.rustup/toolchains/master")
        .arg(&file)
        .arg("-Zunstable-options")
        .arg("--document-private-items")
        .arg("--document-hidden-items")
        .args(&["--cap-lints", "warn"])
        .args(&["-o", "/dev/null"]);
    let output = systemdrun_command(&mut cmd).unwrap();

    (output, get_cmd_string(&cmd), Vec::new())
}

pub(crate) fn run_rust_analyzer(executable: &str, file: &Path) -> (Output, String, Vec<OsString>) {
    let file_content = std::fs::read_to_string(&file).expect("failed to read file ");

    let mut cmd = Command::new(executable)
        .arg("symbols")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let stdin = &mut cmd.stdin.as_mut().unwrap();
    stdin.write_all(file_content.as_bytes()).unwrap();
    (
        cmd.wait_with_output().unwrap(),
        get_cmd_string(Command::new("rust-analyer").arg("symbols")),
        Vec::new(),
    )

    /*
    let output = process.wait_with_output().unwrap();
    println!("\n\n{:?}\n\n", output);
    output
    */
}
pub(crate) fn run_rustfmt(executable: &str, file: &Path) -> (Output, String, Vec<OsString>) {
    let mut cmd = Command::new(executable);
    cmd.env("SYSROOT", "/home/matthias/.rustup/toolchains/master")
        .arg(&file)
        .arg("--check")
        .args(&["--edition", "2018"]);
    let output = systemdrun_command(&mut cmd).unwrap();
    (output, get_cmd_string(&cmd), Vec::new())
}

pub(crate) fn run_miri(
    executable: &str,
    file: &Path,
    miri_flags: &[&str],
) -> (Output, String, Vec<OsString>) {
    let file_stem = &format!("_{}", file.file_stem().unwrap().to_str().unwrap());

    let file_string = std::fs::read_to_string(&file).unwrap_or_default();

    // only check files that have main() as entrypoint
    // assue that if we find "fn main() {\n", the main contains something
    let has_main = file_string.contains("fn main() {\n");

    let has_unsafe = file_string.contains("unsafe ");

    if !has_main || has_unsafe {
        // @FIXME, move this out of run_miri
        // we need some kind main entry point and code should not contain unsafe code
        return (
            std::process::Command::new("true")
                .output()
                .expect("failed to run 'true'"),
            String::new(),
            Vec::new(),
        );
    }

    // running miri is a bit more complicated:
    // first we need a new tempdir

    let tempdir = TempDir::new("icemaker_miri_tempdir").unwrap();
    let tempdir_path = tempdir.path();

    assert!(!has_unsafe, "file should not contain any unsafe code!");
    // create a new cargo project inside the tmpdir
    if !std::process::Command::new("cargo")
        .arg("new")
        .arg(file_stem)
        .current_dir(&tempdir_path)
        .output()
        .expect("failed to exec cargo new")
        .status
        .success()
    {
        eprintln!("ERROR: cargo new failed for: {}", file_stem);
        return (
            std::process::Command::new("true")
                .output()
                .expect("failed to run 'true'"),
            String::new(),
            Vec::new(),
        );
    }
    let source_path = {
        let mut sp = tempdir_path.to_owned();
        sp.push(file_stem);
        sp.push("src/");
        sp.push("main.rs");
        sp
    };

    // write the content of the file we want to check into tmpcrate/src/main.rs
    std::fs::write(source_path, file_string).expect("failed to write to file");

    // we should have everything prepared for the miri invocation now: execute "cargo miri run"

    let mut crate_path = tempdir_path.to_owned();
    crate_path.push(file_stem);

    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("miri")
        .arg("run")
        .current_dir(crate_path)
        .env("MIRIFLAGS", miri_flags.join(" "));

    let out = systemdrun_command(&mut cmd)
        .unwrap_or_else(|_| panic!("Error: {:?}, executable: {:?}", cmd, executable));

    //let stderr = String::from_utf8(out.stderr.clone()).unwrap();
    //eprintln!("{}", stderr);

    (out, get_cmd_string(&cmd), Vec::new())
}

pub(crate) fn systemdrun_command(
    new_command: &mut std::process::Command,
) -> std::result::Result<Output, std::io::Error> {
    if cfg!(feature = "ci") {
        // return as is
        new_command.output()
    } else {
        let program = new_command.get_program();
        let args = new_command.get_args();
        let current_dir = new_command.get_current_dir();
        let envs = new_command
            .get_envs()
            .map(|(k, v)| {
                (
                    k,
                    v.unwrap_or_else(|| panic!("failed to unwrap env {:?}", k.to_str())),
                )
            })
            .collect::<Vec<(&std::ffi::OsStr, &std::ffi::OsStr)>>();
        let mut cmd = Command::new("systemd-run");
        cmd.arg("--user")
            .arg("--scope")
            .arg("-p")
            .arg("MemoryMax=3G")
            .arg("-p")
            .arg("RuntimeMaxSec=300");

        cmd.arg(program);
        cmd.args(args);
        if let Some(dir) = current_dir {
            cmd.current_dir(dir);
        }
        cmd.envs(envs);
        cmd.output()
    }
}

pub(crate) fn incremental_stress_test(
    file_a: &std::path::PathBuf,
    files: &Vec<std::path::PathBuf>,
    executable: &str,
) -> (Output, String, Vec<OsString>, PathBuf, PathBuf) {
    use rand::seq::SliceRandom;

    let file_b = files.choose(&mut rand::thread_rng()).unwrap();
    let files = [&file_a, &file_b];

    let tempdir = TempDir::new("rustc_testrunner_tmpdir").unwrap();
    let tempdir_path = tempdir.path();

    let mut cmd = Command::new("DUMMY");
    let mut output = None;
    let mut actual_args = Vec::new();
    for i in &[0_usize, 1_usize] {
        let file = files[*i];
        let mut command = Command::new(executable);

        let has_main = std::fs::read_to_string(&file)
            .unwrap_or_default()
            .contains("fn main(");

        if !has_main {
            command.args(&["--crate-type", "lib"]);
        }
        command
            .arg(&file)
            // avoid error: the generated executable for the input file  .. onflicts with the existing directory..
            .arg(format!("-o{}/{}", tempdir_path.display(), i))
            .arg(format!("-Cincremental={}", tempdir_path.display()))
            .arg("-Zincremental-verify-ich=yes")
            // also enable debuginfo for incremental, since we are codegenning anyway
            .arg("--edition=2021");

        //dbg!(&command);

        // the output from the second invocation is the interesting one!
        output = Some(systemdrun_command(&mut command));
        actual_args = command
            .get_args()
            .map(|s| s.to_owned())
            .collect::<Vec<OsString>>();
        //dbg!(&output);
        cmd = command;
    }

    let output = output.map(|output| output.unwrap()).unwrap();

    tempdir.close().unwrap();
    //dbg!(&output);

    let mut cmd_str = get_cmd_string(&cmd);
    cmd_str.push_str(&file_a.display().to_string());
    cmd_str.push_str(" | ");
    cmd_str.push_str(&file_b.display().to_string());

    (output, cmd_str, actual_args, file_a.clone(), file_b.clone())
}

use std::ffi::OsString;
use std::io::Write;
use std::path::Path;
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

    let mut output = Command::new(executable);
    output
        .arg(&file)
        .args(rustc_flags)
        // always keep these:
        .arg(&output_file)
        .arg(&dump_mir_dir);
    if !has_main {
        output.args(&["--crate-type", "lib"]);
    }
    //dbg!(&output);

    let actual_args = output
        .get_args()
        .map(|s| s.to_owned())
        .collect::<Vec<OsString>>();

    // run the command
    (
        output
            .output()
            .unwrap_or_else(|_| panic!("Error: {:?}, executable: {:?}", output, executable)),
        get_cmd_string(&output),
        actual_args,
    )
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

        output = Some(command.output());
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
    (cmd.output().unwrap(), get_cmd_string(&cmd), Vec::new())
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
    let output = cmd.output().unwrap();
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
    let output = cmd.output().unwrap();
    (output, get_cmd_string(&cmd), Vec::new())
}

pub(crate) fn run_miri(executable: &str, file: &Path) -> (Output, String, Vec<OsString>) {
    let file_stem = file.file_stem().unwrap();

    // running miri is a bit more complicated:
    // first we need a new tempdir

    let tempdir = TempDir::new("icemaker_miri_tempdir").unwrap();
    let tempdir_path = tempdir.path();

    let file_string = std::fs::read_to_string(&file).unwrap_or_default();

    let has_main = file_string.contains("fn main(");

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

    assert!(!has_unsafe, "file should not contain any unsafe code!");

    // create a new cargo project inside the tmpdir
    std::process::Command::new("cargo")
        .arg("new")
        .arg(file_stem)
        .current_dir(&tempdir_path)
        .status()
        .expect("failed to exec cargo new")
        .success()
        .then(|| 0)
        .expect("cargo new failed");

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

    // check if the file actually compiles, if not, abort
    if !std::process::Command::new("cargo")
        .arg("check")
        .current_dir(&crate_path)
        .output()
        .expect("failed to cargo check")
        .status
        .success()
    {
        return (
            std::process::Command::new("true")
                .output()
                .expect("failed to run 'true'"),
            String::new(),
            Vec::new(),
        );
    }

    let mut output = std::process::Command::new("cargo");
    output.arg("miri").arg("run").current_dir(crate_path);

    let out = output
        .output()
        .unwrap_or_else(|_| panic!("Error: {:?}, executable: {:?}", output, executable));

    eprintln!("{}", String::from_utf8(out.stderr.clone()).unwrap());

    (out, get_cmd_string(&output), Vec::new())
}

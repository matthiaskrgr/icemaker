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
    let has_main = std::fs::read_to_string(file)
        .unwrap_or_default()
        .contains("fn main(");

    let tempdir = TempDir::new("rustc_testrunner_tmpdir").unwrap();
    let tempdir_path = tempdir.path().display();

    // decide whether we want rustc to do codegen (slow!) or not
    let output_file = if rustc_flags.contains(&"-ocodegen") {
        Some(format!("-o{}/outfile", tempdir_path))
    } else {
        // this will prevent rustc from codegening since "nocodegen" is not a dir
        Some("-o/tmp/nocodegen/nocodege".into())
    };
    //  we need to remove the original -o flag from the rustflags because rustc will not accept two -o's
    let rustc_flags = rustc_flags.iter().filter(|flag| **flag != "-ocodegen");

    let dump_mir_dir = format!("-Zdump-mir-dir={}", tempdir_path);

    let mut cmd = Command::new(executable);
    cmd.arg(file)
        // always keep these:
        .arg(&dump_mir_dir);
    cmd.args(output_file);

    if !has_main {
        cmd.arg("--crate-type=lib");
    }
    // be able to override --crate-type=lib/bin
    cmd.args(rustc_flags);

    //dbg!(&cmd);

    let actual_args = cmd
        .get_args()
        .map(|s| s.to_owned())
        .collect::<Vec<OsString>>();

    // run the command
    let output = systemdrun_command(&mut cmd)
        .unwrap_or_else(|_| panic!("Error: {:?}, executable: {:?}", cmd, executable));
    // dbg!(&output);

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

    let has_main = std::fs::read_to_string(file)
        .unwrap_or_default()
        .contains("fn main(");

    let mut cmd = Command::new("DUMMY");
    let mut output = None;
    let mut actual_args = Vec::new();
    for i in &[0, 1] {
        let mut command = Command::new(executable);
        if !has_main {
            command.arg("--crate-type=lib");
        }
        command
            .arg(file)
            // avoid error: the generated executable for the input file  .. onflicts with the existing directory..
            .arg(format!("-o{}/{}", tempdir_path.display(), i))
            .arg(format!("-Cincremental={}", tempdir_path.display()))
            .arg("-Zincremental-verify-ich=yes")
            // also enable debuginfo for incremental, since we are codegenning anyway
            .arg("-Cdebuginfo=2")
            // save-temps creates /tmp/rustc<hash> dirs that are not cleaned up properly
            //.arg("-Csave-temps=yes")
            .arg("--edition=2021");
        //   .arg("-Cpasses=lint");

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
    // runs clippy-driver, not cargo-clippy!

    let has_main = std::fs::read_to_string(file)
        .unwrap_or_default()
        .contains("pub(crate) fn main(");
    let mut cmd = Command::new(executable);

    if !has_main {
        cmd.arg("--crate-type=lib");
    }
    cmd.env("RUSTFLAGS", "-Z force-unstable-if-unmarked")
        .env("SYSROOT", "/home/matthias/.rustup/toolchains/master")
        .arg(file)
        .arg("-Aclippy::cargo") // allow cargo lints
        //.arg("-Wclippy::internal")
        .arg("-Wclippy::complexity")
        .arg("-Wclippy::correctness")
        .arg("-Wclippy::nursery")
        .arg("-Wclippy::pedantic")
        .arg("-Wclippy::perf")
        .arg("-Wclippy::style")
        .arg("-Wclippy::suspicious")
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
        .arg("--edition=2021")
        .arg("-Zvalidate-mir")
        .args(["--cap-lints", "warn"])
        .args(["-o", "/dev/null"]);

    let output = systemdrun_command(&mut cmd).unwrap();

    (output, get_cmd_string(&cmd), Vec::new())
}

pub(crate) fn run_clippy_fix(executable: &str, file: &Path) -> (Output, String, Vec<OsString>) {
    // we need the "cargo-clippy" executable for --fix
    // s/clippy-driver/cargo-clippy
    let cargo_clippy = executable
        .to_string()
        .replace("clippy-driver", "cargo-clippy");

    let file_stem = &format!("_{}", file.file_stem().unwrap().to_str().unwrap())
        .replace('.', "_")
        .replace(['[', ']'], "_");

    let file_string = std::fs::read_to_string(file).unwrap_or_default();

    let has_main = file_string.contains("pub(crate) fn main(");
    let mut cmd = Command::new(executable);

    if !has_main {
        cmd.arg("--crate-type=lib");
    }
    cmd.env("RUSTFLAGS", "-Z force-unstable-if-unmarked")
        .env("SYSROOT", "/home/matthias/.rustup/toolchains/master")
        .arg(file)
        .arg("-Aclippy::cargo") // allow cargo lints
        //.arg("-Wclippy::internal")
        .arg("-Wclippy::restriction")
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
        .args(["--cap-lints", "warn"]);

    let output = systemdrun_command(&mut cmd).unwrap();
    // ^ this is the original clippy warning output, without --fix.
    // from this we can know what kind of lints were actually applied to the code
    let lint_output = String::from_utf8(output.clone().stderr).unwrap();
    let mut lint_lines = lint_output
        .lines()
        .filter(|l| l.contains("https://rust-lang.github.io/rust-clippy/master/index.html#"))
        .map(|l| l.split('#').last().unwrap())
        .map(|s| s.into())
        .collect::<Vec<OsString>>();
    lint_lines.sort();
    lint_lines.dedup();
    let used_lints = lint_lines;

    dbg!(&used_lints);

    //dbg!(&output);
    // if the snippet "compiles" fine, try to run clippy with --fix
    let exit_status = output.status.code().unwrap_or(42);

    if exit_status != 0 {
        // errors while checking file, abort (file may not build)
        return (
            std::process::Command::new("true")
                .output()
                .expect("failed to run 'true'"),
            String::new(),
            Vec::new(),
        );
    }

    let tempdir = TempDir::new("icemaker_clippyfix_tempdir").unwrap();
    let tempdir_path = tempdir.path();
    // create a new cargo project inside the tmpdir
    if !std::process::Command::new("cargo")
        .env("SYSROOT", "/home/matthias/.rustup/toolchains/master")
        .arg("new")
        .args(["--vcs", "none"])
        .arg(file_stem)
        .current_dir(tempdir_path)
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

    /* if !has_main && has_test {
        cmd.arg("miri")
            .arg("test")
            .current_dir(crate_path)
            .env("MIRIFLAGS", miri_flags.join(" "));
    } else { */

    let mut cmd = Command::new("cargo");

    cmd.arg("+master")
        .arg("clippy")
        .env("RUSTFLAGS", "-Z force-unstable-if-unmarked")
        .env("SYSROOT", "/home/matthias/.rustup/toolchains/master")
        .current_dir(crate_path)
        .arg("--fix")
        .arg("--allow-no-vcs")
        .arg("--")
        .arg("-Aclippy::cargo") // allow cargo lints
        //.arg("-Wclippy::internal")
        .arg("-Wclippy::restriction")
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
        .args(["--cap-lints", "warn"]);

    let output = systemdrun_command(&mut cmd).unwrap();

    //  dbg!(&output);
    //  }

    (output, get_cmd_string(&cmd), used_lints)
}

pub(crate) fn run_rustdoc(executable: &str, file: &Path) -> (Output, String, Vec<OsString>) {
    let mut cmd = Command::new(executable);
    cmd.env("RUSTFLAGS", "-Z force-unstable-if-unmarked")
        .env("SYSROOT", "/home/matthias/.rustup/toolchains/master")
        .arg(file)
        .arg("-Znormalize-docs")
        .arg("-Zunstable-options")
        .arg("--document-private-items")
        .arg("--document-hidden-items")
        .args(["--output-format", "json"])
        .args(["--cap-lints", "warn"])
        .arg("-Wrustdoc::invalid-html-tags")
        .arg("-Wrustdoc::missing-crate-level-docs")
        .arg("-Wrustdoc::missing-doc-code-examples")
        .arg("-Wrustdoc::private-doc-tests")
        .arg("--show-type-layout")
        .args(["-o", "/dev/null"]);
    let output = systemdrun_command(&mut cmd).unwrap();

    (output, get_cmd_string(&cmd), Vec::new())
}

pub(crate) fn run_rust_analyzer(executable: &str, file: &Path) -> (Output, String, Vec<OsString>) {
    let file_content = std::fs::read_to_string(file).expect("failed to read file ");

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
        .arg(file)
        .arg("--check")
        .args(["--edition", "2018"]);
    let output = systemdrun_command(&mut cmd).unwrap();
    (output, get_cmd_string(&cmd), Vec::new())
}

pub(crate) fn run_miri(
    executable: &str,
    file: &Path,
    miri_flags: &[&str],
) -> (Output, String, Vec<OsString>) {
    let file_stem = &format!("_{}", file.file_stem().unwrap().to_str().unwrap())
        .replace('.', "_")
        .replace(['[', ']'], "_");

    let file_string = std::fs::read_to_string(file).unwrap_or_default();
    /*    // only check files that have main() as entrypoint
    // assume that if we find "fn main() {\n", the main contains something
    let has_main = file_string.contains("fn main() {\n");

    // let has_test = file_string.contains("#[test");

    let has_unsafe = file_string.contains("unsafe ");
    if (!has_main/*&& !has_test*/) || has_unsafe {
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
    */

    let has_main = file_string.contains("fn main() {\n");

    // running miri is a bit more complicated:
    // first we need a new tempdir

    let no_std = file_string.contains("#![no_std]");
    let platform_intrinsics = file_string.contains("feature(platform_intrinsics)");
    if no_std || platform_intrinsics || !has_main {
        // miri is know to not really handles this well
        return (
            std::process::Command::new("true")
                .output()
                .expect("failed to run 'true'"),
            String::new(),
            Vec::new(),
        );
    }

    let tempdir = TempDir::new("icemaker_miri_tempdir").unwrap();
    let tempdir_path = tempdir.path();
    // create a new cargo project inside the tmpdir
    if !std::process::Command::new("cargo")
        .arg("new")
        .arg(file_stem)
        .args(["--vcs", "none"])
        .current_dir(tempdir_path)
        .output()
        .expect("failed to exec cargo new")
        .status
        .success()
    {
        eprintln!("ERROR: cargo new failed for: {}", file_stem,);
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
    /* if !has_main && has_test {
        cmd.arg("miri")
            .arg("test")
            .current_dir(crate_path)
            .env("MIRIFLAGS", miri_flags.join(" "));
    } else { */
    cmd.arg("+master")
        .arg("miri")
        .arg("run")
        .current_dir(&crate_path)
        .env("MIRIFLAGS", miri_flags.join(" "))
        .env("RUSTFLAGS", "-Zvalidate-mir")
        .env("MIRI_CWD", &crate_path);

    //  }

    let out = systemdrun_command(&mut cmd)
        .unwrap_or_else(|_| panic!("Error: {:?}, executable: {:?}", cmd, executable));

    //let stderr = String::from_utf8(out.stderr.clone()).unwrap();
    //eprintln!("{}", stderr);
    let out2 = out.clone();
    //let out3 = out.clone(); // hax
    if [out2.stderr, out2.stdout].into_iter().any(|out| {
        let out = String::from_utf8(out).unwrap();
        out.contains("compiler_builtins ")
    }) {
        /* eprintln!("\n\n\n\n");
        eprintln!("STDOUT:\n {}", String::from_utf8(out3.stdout).unwrap());
        eprintln!("STDERR:\n {}", String::from_utf8(out3.stderr).unwrap());
        */

        panic!(
            "miri tried to recompile std!!\n{:?} {:?} {:?} in  {:?}\n\n",
            executable, file, miri_flags, crate_path
        )
    }
    (out, get_cmd_string(&cmd), Vec::new())
}

pub(crate) fn run_cranelift(
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
    let has_main = std::fs::read_to_string(file)
        .unwrap_or_default()
        .contains("fn main(");

    //let tempdir = TempDir::new("rustc_testrunner_tmpdir").unwrap();
    //let tempdir_path = tempdir.path();
    let output_file = String::from("-o/dev/null");
    let dump_mir_dir = String::from("-Zdump-mir-dir=/dev/null");

    let mut cmd = Command::new(executable);
    cmd.arg(file)
        .args(rustc_flags)
        // always keep these:
        .arg(&output_file)
        .arg(&dump_mir_dir);
    if !has_main {
        cmd.arg("--crate-type=lib");
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
        let full_miri = new_command
            .get_args()
            .chain(std::iter::once(program))
            .any(|s| s == std::ffi::OsStr::new("miri"));

        let mut cmd = Command::new("systemd-run");
        cmd.arg("--user")
            .arg("--scope")
            .arg("-p")
            .arg("MemoryMax=3G")
            .arg("-p");
        if full_miri {
            // miri timout: 20 seconds
            cmd.arg("RuntimeMaxSec=20");
        } else {
            // all other timeouts: 90 seconds
            cmd.arg("RuntimeMaxSec=91");
        }

        cmd.arg(program);
        cmd.args(args);
        if let Some(dir) = current_dir {
            cmd.current_dir(dir);
        }
        cmd.envs(envs);
        cmd.output()
    }
}
pub(crate) fn file_compiles(file: &std::path::PathBuf, executable: &str) -> bool {
    let has_main = std::fs::read_to_string(file)
        .unwrap_or_default()
        .contains("fn main(");

    let file = file.canonicalize().unwrap();
    let tempdir = TempDir::new("rustc_testrunner_tmpdir").unwrap();
    let tempdir_path = tempdir.path();

    let mut compile_passes_check_cmd = Command::new(executable);
    if !has_main {
        compile_passes_check_cmd.arg("--crate-type=lib");
    }
    compile_passes_check_cmd.arg(&file).arg("-Zno-codegen");
    compile_passes_check_cmd.current_dir(tempdir_path);
    // if we fail to compile one of the files, return None (abort)
    matches!(
        systemdrun_command(&mut compile_passes_check_cmd)
            .ok()
            .map(|x| x.status.success()),
        Some(true)
    )
}

pub(crate) fn incremental_stress_test(
    file_a: &std::path::PathBuf,
    files: &Vec<std::path::PathBuf>,
    executable: &str,
) -> Option<(Output, String, Vec<OsString>, PathBuf, PathBuf)> {
    use rand::seq::SliceRandom;

    let file_b = files.choose(&mut rand::thread_rng()).unwrap();

    let files = [&file_a, &file_b];

    let tempdir = TempDir::new("rustc_testrunner_tmpdir").unwrap();
    let tempdir_path = tempdir.path();

    let mut cmd = Command::new("DUMMY");
    let mut output = None;
    let mut actual_args = Vec::new();

    // make sure both files compile
    for file in files {
        if !file_compiles(file, executable) {
            return None;
        }
    }
    // both files compile, continue with actual incremental checking
    eprintln!("found possible pair: {:?}", files);
    for i in &[0_usize, 1_usize] {
        let file = files[*i];

        let has_main = std::fs::read_to_string(file)
            .unwrap_or_default()
            .contains("fn main(");

        let mut command = Command::new(executable);

        if !has_main {
            command.arg("--crate-type=lib");
        }
        command
            .arg(file)
            // avoid error: the generated executable for the input file  .. onflicts with the existing directory..
            .arg(format!("-o{}/{}", tempdir_path.display(), i))
            .arg(format!("-Cincremental={}", tempdir_path.display()))
            .arg("-Zincremental-verify-ich=yes")
            .arg("-Csave-temps=yes")
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

    Some((output, cmd_str, actual_args, file_a.clone(), file_b.clone()))
}

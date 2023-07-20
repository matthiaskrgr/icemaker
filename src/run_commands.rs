use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use lazy_static::lazy_static;
use once_cell::sync::Lazy;
use regex::Regex;

use clap::Parser;
use tempdir::TempDir;

use crate::library::Args;
use crate::{find_ICE_string, flags};

lazy_static! {
    static ref HOME_DIR: PathBuf = home::home_dir().unwrap();
}

static LOCAL_DEBUG_ASSERTIONS: Lazy<bool> = Lazy::new(|| Args::parse().local_debug_assertions);

static EXPENSIVE_FLAGS_ACTIVE: Lazy<bool> = Lazy::new(|| Args::parse().expensive_flags);

static SYSROOT_PATH: Lazy<String> = Lazy::new(|| {
    format!(
        "{}",
        HOME_DIR
            .join(format!(
                ".rustup/toolchains/{}/",
                if *LOCAL_DEBUG_ASSERTIONS {
                    "local-debug-assertions"
                } else {
                    "master"
                }
            ))
            .display()
    )
});

pub(crate) const PROCESS_TIMEOUT_S: u8 = 30;
pub(crate) const PROCESS_TIMEOUT_MIRI_S: u8 = 20;

#[allow(unused)]
#[derive(Clone, Debug)]
pub(crate) struct CommandOutput {
    output: std::process::Output,
    cmd_string: String,
    // flags executed by the $Executable that hit the ICE
    flags: Vec<OsString>,
    exec: crate::Executable,
}

impl CommandOutput {
    pub(crate) fn unwrap(self) -> (std::process::Output, String, Vec<OsString>) {
        (self.output, self.cmd_string, self.flags)
    }

    fn new(
        output: std::process::Output,
        cmd_string: String,
        flags: Vec<OsString>,
        exec: crate::Executable,
    ) -> Self {
        Self {
            output,
            cmd_string,
            flags,
            exec,
        }
    }
}

/// get a process::Command as String
fn get_cmd_string(cmd: &std::process::Command) -> String {
    let envs: String = cmd
        .get_envs()
        .filter(|(_, y)| y.is_some())
        .map(|(x, y)| format!("{}={}", x.to_string_lossy(), y.unwrap().to_string_lossy()))
        .collect::<Vec<String>>()
        .join(" ");
    let command = format!("{cmd:?}");
    format!("\"{envs}\" {command}").replace('"', "")
}

pub(crate) fn run_rustc(
    executable: &str,
    file: &Path,
    incremental: bool,
    rustc_flags: &[&str],
    global_tempdir_path: &PathBuf,
) -> CommandOutput {
    if incremental {
        // only run incremental compilation tests
        return run_rustc_incremental(executable, file, global_tempdir_path);
    }
    // if the file contains no "main", run with "--crate-type lib"
    let has_main = std::fs::read_to_string(file)
        .unwrap_or_default()
        .contains("fn main(");

    let tempdir = TempDir::new_in(global_tempdir_path, "rustc_testrunner_tmpdir").unwrap();
    let tempdir_path = tempdir.path().display();

    // decide whether we want rustc to do codegen (slow!) or not
    let output_file = if *EXPENSIVE_FLAGS_ACTIVE || rustc_flags.contains(&"-ocodegen") {
        // do codegen
        Some(format!("-o{tempdir_path}/outfile"))
    } else {
        Some("-Zno-codegen".into())
    };

    //  we need to remove the original -o flag from the rustflags because rustc will not accept two -o's
    let rustc_flags = rustc_flags.iter().filter(|flag| **flag != "-ocodegen");

    let dump_mir_dir = format!("-Zdump-mir-dir={tempdir_path}");

    let mut cmd = Command::new(executable);
    cmd.arg(file)
        // always keep these:
        .arg(dump_mir_dir);
    cmd.args(output_file);

    if !has_main {
        cmd.arg("--crate-type=lib");
    }
    // be able to override --crate-type=lib/bin
    cmd.args(rustc_flags);

    cmd.current_dir(format!("{tempdir_path}"));

    //dbg!(&cmd);

    let actual_args = cmd
        .get_args()
        .map(|s| s.to_owned())
        .collect::<Vec<OsString>>();

    // run the command
    let output = prlimit_run_command(&mut cmd)
        .unwrap_or_else(|_| panic!("Error: {cmd:?}, executable: {executable:?}"));
    // dbg!(&output);

    CommandOutput::new(
        output,
        get_cmd_string(&cmd),
        actual_args,
        crate::Executable::Rustc,
    )
    // remove tempdir
    //tempdir.close().unwrap();
}

pub(crate) fn run_rustc_incremental(
    executable: &str,
    file: &Path,
    global_tempdir_path: &PathBuf,
) -> CommandOutput {
    let tempdir = TempDir::new_in(global_tempdir_path, "rustc_testrunner_tmpdir").unwrap();
    let tempdir_path = tempdir.path();

    let dump_mir_dir = String::from("-Zdump-mir-dir=/dev/null");

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
            .env("SYSROOT", &*SYSROOT_PATH)
            // avoid error: the generated executable for the input file  .. onflicts with the existing directory..
            .arg(format!("-o{}/{}", tempdir_path.display(), i))
            .arg(format!("-Cincremental={}", tempdir_path.display()))
            .arg("-Zincremental-verify-ich=yes")
            .arg(&dump_mir_dir)
            // also enable debuginfo for incremental, since we are codegenning anyway
            .arg("-Cdebuginfo=2");
        // save-temps creates /tmp/rustc<hash> dirs that are not cleaned up properly
        //.arg("-Csave-temps=yes")
        //   .arg("--edition=2021");
        //   .arg("-Cpasses=lint");

        //dbg!(&command);

        output = Some(prlimit_run_command(&mut command));
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
    CommandOutput::new(
        output,
        get_cmd_string(&cmd),
        actual_args,
        crate::Executable::Rustc,
    )
}

// compile 2 variations of a file with shared incr comp cache and try to find crashes
pub(crate) fn run_rustc_incremental_with_two_files(
    executable: &str,
    file_1: &Path,
    // mutation
    file_2: &String,
    global_tempdir_path: &PathBuf,
) -> CommandOutput {
    let tempdir = TempDir::new_in(global_tempdir_path, "rustc_testrunner_tmpdir").unwrap();
    let tempdir_path = tempdir.path();
    let file_2_string = file_2;
    let file_2 = tempdir_path.join("file2");
    let file_2 = file_2.as_path();
    let mut file_2_handle = std::fs::File::create(file_2).unwrap();
    file_2_handle.write_all(file_2_string.as_bytes()).unwrap();

    let dump_mir_dir = String::from("-Zdump-mir-dir=/dev/null");

    let has_main = std::fs::read_to_string(file_1)
        .unwrap_or_default()
        .contains("fn main(");

    let mut cmd = Command::new("DUMMY");
    let mut output = None;
    let mut actual_args = Vec::new();
    for i in &[0, 1] {
        let input = match i {
            0 => file_1,
            1 => file_2,
            _ => unreachable!(),
        };
        let mut command = Command::new(executable);
        if !has_main {
            command.arg("--crate-type=lib");
        }
        command
            .arg(input)
            .env("SYSROOT", &*SYSROOT_PATH)
            // avoid error: the generated executable for the input file  .. onflicts with the existing directory..
            .arg(format!("-o{}/{}", tempdir_path.display(), i))
            .arg(format!("-Cincremental={}", tempdir_path.display()))
            .arg("-Zincremental-verify-ich=yes")
            .arg(&dump_mir_dir)
            // also enable debuginfo for incremental, since we are codegenning anyway
            .arg("-Cdebuginfo=2")
            .arg("--edition=2021");
        // save-temps creates /tmp/rustc<hash> dirs that are not cleaned up properly
        //.arg("-Csave-temps=yes")
        //   .arg("--edition=2021");
        //   .arg("-Cpasses=lint");

        //dbg!(&command);

        output = Some(prlimit_run_command(&mut command));
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
    CommandOutput::new(
        output,
        get_cmd_string(&cmd),
        actual_args,
        crate::Executable::Rustc,
    )
}

pub(crate) fn run_clippy(
    executable: &str,
    file: &Path,
    _global_tempdir_path: &Path,
) -> CommandOutput {
    // runs clippy-driver, not cargo-clippy!

    let has_main = std::fs::read_to_string(file)
        .unwrap_or_default()
        .contains("pub(crate) fn main(");
    let mut cmd = Command::new(executable);

    if !has_main {
        cmd.arg("--crate-type=lib");
    }
    cmd.env("RUSTFLAGS", "-Z force-unstable-if-unmarked")
        .env("SYSROOT", &*SYSROOT_PATH)
        .env("CARGO_TERM_COLOR", "never")
        .arg(file)
        .args(flags::CLIPPYLINTS)
        .args(flags::RUSTC_ALLOW_BY_DEFAULT_LINTS)
        .arg("--edition=2021")
        .arg("-Zvalidate-mir")
        .args(["--cap-lints", "warn"])
        .args(["-o", "/dev/null"]);

    let output = prlimit_run_command(&mut cmd).unwrap();

    CommandOutput::new(
        output,
        get_cmd_string(&cmd),
        Vec::new(),
        crate::Executable::Clippy,
    )
}

pub(crate) fn run_clippy_fix(
    _executable: &str,
    file: &Path,
    global_tempdir_path: &PathBuf,
) -> CommandOutput {
    // we need the "cargo-clippy" executable for --fix
    // s/clippy-driver/cargo-clippy
    /*  let cargo_clippy = executable
    .to_string()
    .replace("clippy-driver", "cargo-clippy"); */

    let file_stem = &format!("_{}", file.file_stem().unwrap().to_str().unwrap())
        .replace('.', "_")
        .replace(['[', ']'], "_");

    let file_string = std::fs::read_to_string(file).unwrap_or_default();

    // we need to get the full path to work with --project
    // https://github.com/matthiaskrgr/icemaker/issues/26
    let file = std::fs::canonicalize(file).unwrap();

    let has_main = file_string.contains("pub(crate) fn main(");

    let tempdir = TempDir::new_in(global_tempdir_path, "icemaker_clippyfix_tempdir").unwrap();
    let tempdir_path = tempdir.path();

    // @FIXME should this actually be clippy to catch clippy ICEs
    if !file_compiles(
        &file,
        &crate::ice::Executable::Rustc.path(),
        global_tempdir_path,
    ) {
        return CommandOutput::new(
            std::process::Command::new("true")
                .output()
                .expect("failed to run 'true'"),
            String::new(),
            Vec::new(),
            crate::Executable::ClippyFix,
        );
    }

    // create a new cargo project inside the tmpdir
    if !std::process::Command::new("cargo")
        .env("SYSROOT", &*SYSROOT_PATH)
        .env("CARGO_TERM_COLOR", "never")
        .arg("new")
        .args(["--vcs", "none"])
        .arg(if has_main { "--bin" } else { "--lib" })
        .arg(file_stem)
        .current_dir(tempdir_path)
        .output()
        .expect("failed to exec cargo new")
        .status
        .success()
    {
        eprintln!(
            "ERROR: cargo new failed for: '{file_stem}', run_clippy_fix() {}:{}:{}",
            file!(),
            line!(),
            column!()
        );

        return CommandOutput::new(
            std::process::Command::new("true")
                .output()
                .expect("failed to run 'true'"),
            String::new(),
            Vec::new(),
            crate::Executable::ClippyFix,
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

    let mut cmd = Command::new("cargo");

    cmd.arg(if *LOCAL_DEBUG_ASSERTIONS {
        "+local-debug-assertions"
    } else {
        "+master"
    })
    .arg("clippy")
    .env("CARGO_TERM_COLOR", "never")
    .env("RUSTFLAGS", "-Z force-unstable-if-unmarked")
    .env("SYSROOT", &*SYSROOT_PATH)
    .current_dir(crate_path)
    .arg("--fix")
    .arg("--allow-no-vcs")
    .arg("--")
    .args(flags::CLIPPYLINTS)
    .args(flags::RUSTC_ALLOW_BY_DEFAULT_LINTS)
    .args(["--cap-lints", "warn"]);
    //dbg!(&cmd);

    let output = prlimit_run_command(&mut cmd).unwrap();

    // grab the output from the clippy-fix command to get the lints that we ran so we can bisect the offending lint later on
    let lint_output = String::from_utf8(output.clone().stderr).unwrap();
    let mut clippy_lint_lines = lint_output
        .lines()
        .filter(|l| l.contains("https://rust-lang.github.io/rust-clippy/master/index.html#"))
        .map(|l| l.split('#').last().unwrap())
        //  .map(|lintname| format!("-Wclippy::{}", lintname.replace('_', "-")))
        .map(|lintname| lintname.replace('_', "-"))
        .map(|lint| format!("--force-warn clippy::{lint}"))
        .map(|l| l.into())
        .collect::<Vec<OsString>>();
    clippy_lint_lines.sort();
    clippy_lint_lines.dedup();

    let rustc_lint_lines_default = lint_output
        .lines()
        .filter(|l| l.contains(" = note: `#[warn(") && l.contains(")]` on by default"))
        .map(|l| l.split('(').last().unwrap())
        .map(|l| l.split(')').next().unwrap());

    let rustc_lint_lints_cmdline = lint_output
        .lines()
        .filter(|l| l.contains(" = note: requested on the command line with `"))
        .map(|l| l.split('`').nth(1).unwrap())
        .map(|l| l.split("-W ").last().unwrap());

    let mut rustc_lints_all = rustc_lint_lines_default
        .chain(rustc_lint_lints_cmdline)
        // added later
        //  .map(|lint| format!("-W{}", lint))
        .map(|lint| format!("--force-warn {lint}"))
        .map(OsString::from)
        .collect::<Vec<OsString>>();

    rustc_lints_all.sort();
    rustc_lints_all.dedup();

    clippy_lint_lines.extend(rustc_lints_all);

    let used_lints = clippy_lint_lines;

    //dbg!(String::from_utf8_lossy(&output.stderr));

    CommandOutput::new(
        output,
        get_cmd_string(&cmd),
        used_lints,
        crate::Executable::ClippyFix,
    )
}

pub(crate) fn run_rustfix(
    _executable: &str,
    file: &Path,
    global_tempdir_path: &PathBuf,
) -> CommandOutput {
    // run cargo fix --broken-code and check if rustc provides any suggestions that do not fix the actual problemy

    let file_stem = &format!("_{}", file.file_stem().unwrap().to_str().unwrap())
        .replace('.', "_")
        .replace(['[', ']'], "_");

    let file_string = std::fs::read_to_string(file).unwrap_or_default();

    let tempdir = TempDir::new_in(global_tempdir_path, "icemaker_rustffix_tempdir").unwrap();
    let tempdir_path = tempdir.path();
    // create a new cargo project inside the tmpdir
    if !std::process::Command::new("cargo")
        .env("SYSROOT", &*SYSROOT_PATH)
        .env("CARGO_TERM_COLOR", "never")
        .arg("new")
        .args(["--vcs", "none"])
        .arg(file_stem)
        .current_dir(tempdir_path)
        .output()
        .expect("failed to exec cargo new")
        .status
        .success()
    {
        eprintln!("ERROR: cargo new failed for: {file_stem}");
        return CommandOutput::new(
            std::process::Command::new("true")
                .output()
                .expect("failed to run 'true'"),
            String::new(),
            Vec::new(),
            crate::Executable::RustFix,
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
    std::fs::write(source_path, &file_string).expect("failed to write to file");

    // we should have everything prepared for the miri invocation now: execute "cargo miri run"

    let mut crate_path = tempdir_path.to_owned();
    crate_path.push(file_stem);

    let mut cmd = Command::new("cargo");

    cmd.arg(if *LOCAL_DEBUG_ASSERTIONS {
        "+local-debug-assertions"
    } else {
        "+master"
    })
    .env("RUSTFLAGS", "-Z force-unstable-if-unmarked -Aunused")
    .env("SYSROOT", &*SYSROOT_PATH)
    .env("CARGO_TERM_COLOR", "never")
    .current_dir(&crate_path)
    .arg("fix")
    .arg("--allow-no-vcs")
    .arg("--broken-code");
    // cargo fix doesnt need this
    // .args(["--", "--cap-lints", "warn"]);

    //dbg!(&cmd);

    let output = prlimit_run_command(&mut cmd).unwrap();

    let fixed_file = &crate_path.join("src").join("main.rs");
    let file_after_fixing = std::fs::read_to_string(fixed_file).unwrap_or_default();
    if file_string == file_after_fixing {
        // we didn't actually apply any changes, ignore
        return CommandOutput::new(
            std::process::Command::new("true")
                .output()
                .expect("failed to run 'true'"),
            String::new(),
            Vec::new(),
            crate::Executable::RustFix,
        );
    }

    //dbg!(&output);

    //  modified the file! BUT if we fixed all errors, we are not intersted in this...
    if find_ICE_string(file, &crate::Executable::RustFix, output.clone()).is_some() {
        let diff = diff::lines(&file_string, &file_after_fixing)
            .iter()
            .map(|diff| match diff {
                diff::Result::Left(l) => format!("-{l}\n"),
                diff::Result::Both(l, _) => format!(" {l}\n"),
                diff::Result::Right(r) => format!("+{r}\n"),
            })
            .collect::<String>();

        let file_str = file.display();
        //  let stderr = String::from_utf8(output.clone().stderr).unwrap();
        eprintln!("\n\n{file_str}\n{diff}\n" /*{stderr}*/);
    }

    CommandOutput::new(
        output,
        get_cmd_string(&cmd),
        Vec::new(),
        crate::Executable::RustFix,
    )
}

pub(crate) fn run_clippy_fix_with_args(
    executable: &str,
    file: &Path,
    args: &[&str],
    global_tempdir_path: &PathBuf,
) -> CommandOutput {
    //  dbg!(&args);
    // we need the "cargo-clippy" executable for --fix
    // s/clippy-driver/cargo-clippy
    //    dbg!(args);
    let _cargo_clippy = executable
        .to_string()
        .replace("clippy-driver", "cargo-clippy");

    let file_stem = &format!("_{}", file.file_stem().unwrap().to_str().unwrap())
        .replace('.', "_")
        .replace(['[', ']'], "_");

    let file_string = std::fs::read_to_string(file).unwrap_or_default();

    // let has_main = file_string.contains("pub(crate) fn main(");

    // since we already run clippy successfully on the file we SHOULD not encounter any errors here.
    // I assume that cargo clippy --fix throws errors somehow and that returns early here

    let tempdir = TempDir::new_in(global_tempdir_path, "icemaker_clippyfix_tempdir").unwrap();
    let tempdir_path = tempdir.path();
    // create a new cargo project inside the tmpdir
    if !std::process::Command::new("cargo")
        .env("SYSROOT", &*SYSROOT_PATH)
        .env("CARGO_TERM_COLOR", "never")
        .arg("new")
        .args(["--vcs", "none"])
        .arg(file_stem)
        .current_dir(tempdir_path)
        .output()
        .expect("failed to exec cargo new")
        .status
        .success()
    {
        eprintln!(
            "ERROR: cargo new failed for: '{file_stem}', run_clippy_fix_with_args() {}:{}:{}",
            file!(),
            line!(),
            column!()
        );

        return CommandOutput::new(
            std::process::Command::new("true")
                .output()
                .expect("failed to run 'true'"),
            String::new(),
            Vec::new(),
            crate::Executable::ClippyFix,
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

    let mut cmd = Command::new("cargo");

    cmd.arg(if *LOCAL_DEBUG_ASSERTIONS {
        "+local-debug-assertions"
    } else {
        "+master"
    })
    .arg("clippy")
    .env("RUSTFLAGS", "-Z force-unstable-if-unmarked")
    .env("SYSROOT", &*SYSROOT_PATH)
    .env("CARGO_TERM_COLOR", "never")
    .current_dir(crate_path)
    .arg("--fix")
    .arg("--allow-no-vcs")
    .arg("--")
    .arg("-Aclippy::all")
    // need to silence all default rustc lints first so we can properly bisect them
    // also add
    .arg("-Awarnings")
    .args(args.iter().flat_map(|a| a.split_whitespace()))
    .args(["--cap-lints", "warn"]);

    //dbg!(&cmd);

    let output = prlimit_run_command(&mut cmd).unwrap();

    //  dbg!(&output);
    //  }

    CommandOutput::new(
        output,
        get_cmd_string(&cmd),
        Vec::new(),
        crate::Executable::ClippyFix,
    )
}

pub(crate) fn run_rustdoc(
    executable: &str,
    file: &Path,
    _global_tempdir_path: &Path,
) -> CommandOutput {
    let mut cmd = Command::new(executable);
    cmd.env("RUSTFLAGS", "-Z force-unstable-if-unmarked")
        .env("SYSROOT", &*SYSROOT_PATH)
        .env("CARGO_TERM_COLOR", "never")
        .arg(file)
        .arg("-Znormalize-docs")
        .arg("--edition=2021")
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
    let output = prlimit_run_command(&mut cmd).unwrap();

    CommandOutput::new(
        output,
        get_cmd_string(&cmd),
        Vec::new(),
        crate::Executable::Rustdoc,
    )
}

pub(crate) fn run_rust_analyzer(
    executable: &str,
    file: &Path,
    _global_tempdir_path: &Path,
) -> CommandOutput {
    let file_content = std::fs::read_to_string(file).expect("failed to read file ");

    let mut cmd = Command::new(executable)
        .arg("symbols")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let stdin = &mut cmd.stdin.as_mut().unwrap();
    stdin.write_all(file_content.as_bytes()).unwrap();
    CommandOutput::new(
        cmd.wait_with_output().unwrap(),
        get_cmd_string(Command::new("rust-analyer").arg("symbols")),
        Vec::new(),
        crate::Executable::RustAnalyzer,
    )

    /*
    let output = process.wait_with_output().unwrap();
    println!("\n\n{:?}\n\n", output);
    output
    */
}
pub(crate) fn run_rustfmt(
    executable: &str,
    file: &Path,
    _global_tempdir_path: &Path,
) -> CommandOutput {
    let mut cmd = Command::new(executable);
    cmd.env("SYSROOT", &*SYSROOT_PATH)
        .arg(file)
        .arg("--check")
        .args(["--color", "never"])
        .args(["--edition", "2021"]);
    let output = prlimit_run_command(&mut cmd).unwrap();
    CommandOutput::new(
        output,
        get_cmd_string(&cmd),
        Vec::new(),
        crate::Executable::Rustfmt,
    )
}

pub(crate) fn run_miri(
    executable: &str,
    file: &Path,
    miri_flags: &[&str],
    rustc_flags: &[&str],
    global_tempdir_path: &PathBuf,
) -> CommandOutput {
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

    let has_test = file_string.contains("#[test]");

    let no_std = file_string.contains("#![no_std]");
    let platform_intrinsics = file_string.contains("platform_intrinsics");
    let custom_mir = file_string.contains("mir!");

    if no_std || platform_intrinsics || (!has_main && !has_test) || custom_mir {
        // miri is know to not really handles this well
        return CommandOutput::new(
            std::process::Command::new("true")
                .output()
                .expect("failed to run 'true'"),
            String::new(),
            Vec::new(),
            crate::Executable::Miri,
        );
    }

    let edition = rustc_flags
        .iter()
        .find(|flag| flag.starts_with("--edition="));

    let tempdir = TempDir::new_in(global_tempdir_path, "icemaker_miri_tempdir").unwrap();
    let tempdir_path = tempdir.path();
    // create a new cargo project inside the tmpdir
    if !std::process::Command::new("cargo")
        .arg("new")
        .arg(file_stem)
        .args(["--vcs", "none"])
        .arg(edition.expect("miri got no --edition passed via its MIRI_RUSTFLAGS"))
        .current_dir(tempdir_path)
        .output()
        .expect("failed to exec cargo new")
        .status
        .success()
    {
        eprintln!(
            "ERROR: cargo new failed for: '{file_stem}', run_miri() {}:{}:{}",
            file!(),
            line!(),
            column!()
        );
        return CommandOutput::new(
            std::process::Command::new("true")
                .output()
                .expect("failed to run 'true'"),
            String::new(),
            Vec::new(),
            crate::Executable::Miri,
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
    cmd.arg(if *LOCAL_DEBUG_ASSERTIONS {
        "+local-debug-assertions"
    } else {
        "+master"
    })
    .arg("miri")
    .arg(if has_test && !has_main { "test" } else { "run" })
    .current_dir(&crate_path)
    .env("MIRIFLAGS", miri_flags.join(" "))
    .env(
        "RUSTFLAGS",
        rustc_flags
            .iter()
            .filter(|f| !f.contains("--edition"))
            .map(|f| format!(" {f}"))
            .collect::<String>(),
    )
    .env("MIRI_CWD", &crate_path);

    let out = prlimit_run_command(&mut cmd)
        .unwrap_or_else(|_| panic!("Error: {cmd:?}, executable: {executable:?}"));

    // dbg!(&out);

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
            "miri tried to recompile std!!\n{executable:?} {file:?} {miri_flags:?} in  {crate_path:?}\n\n"
        )
    }
    CommandOutput::new(
        out,
        get_cmd_string(&cmd),
        Vec::new(),
        crate::Executable::Miri,
    )
}

pub(crate) fn run_kani(
    executable: &str,
    file: &Path,
    _kani_flags: &[&str],
    rustc_flags: &[&str],
    global_tempdir_path: &PathBuf,
) -> CommandOutput {
    let file_stem = &format!("_{}", file.file_stem().unwrap().to_str().unwrap())
        .replace('.', "_")
        .replace(['[', ']'], "_");

    let file_string = &std::fs::read_to_string(file).unwrap_or_default();

    let has_main = file_string.contains("fn main() {\n");

    // let has_test = file_string.contains("#[test]");

    let no_std = file_string.contains("#![no_std]");
    let platform_intrinsics = file_string.contains("feature(platform_intrinsics)");
    if no_std || platform_intrinsics {
        // miri is know to not really handles this well
        return CommandOutput::new(
            std::process::Command::new("true")
                .output()
                .expect("failed to run 'true'"),
            String::new(),
            Vec::new(),
            crate::Executable::Miri,
        );
    }

    let mut out = None;
    let mut cmd_ = None;
    let mut v = Vec::new();
    #[allow(non_snake_case)]
    for RUSTCFLAGS in &["-Zmir-opt-level=0", "-Zmir-opt-level=5"] {
        let mut RUSTCFLAGS = vec![RUSTCFLAGS.to_string()];
        RUSTCFLAGS.extend(
            rustc_flags
                .iter()
                .cloned()
                .map(|x| x.to_string())
                .collect::<Vec<String>>(),
        );
        if !has_main {
            RUSTCFLAGS.push("--crate-type lib".into());
        }

        let RUSTCFLAGS = RUSTCFLAGS.join(" ");

        let tempdir = TempDir::new_in(global_tempdir_path, "icemaker_miri_tempdir").unwrap();
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
            eprintln!(
                "ERROR: cargo new failed for: '{file_stem}', run_kani() {}:{}:{}",
                file!(),
                line!(),
                column!()
            );
            return CommandOutput::new(
                std::process::Command::new("true")
                    .output()
                    .expect("failed to run 'true'"),
                String::new(),
                Vec::new(),
                crate::Executable::Miri,
            );
        }
        let source_path = {
            let mut sp = tempdir_path.to_owned();
            sp.push(file_stem);
            sp.push("src/");
            sp.push("main.rs");
            sp
        };

        //fmt!
        let mut rustfmt = std::process::Command::new("rustfmt")
            .args(["--config", "max_width=2000000", "--edition", "2021"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let fmt_stdin = rustfmt.stdin.as_mut().unwrap();

        fmt_stdin.write_all(file_string.as_bytes()).unwrap();
        // drop(fmt_stdin);
        let fmt_output = rustfmt.wait_with_output().unwrap();

        // if we fail formatting, fall back to unformatted output
        let maybe_formatted = if !fmt_output.status.success() {
            String::from_utf8(fmt_output.stdout).unwrap()
        } else {
            file_string.clone()
        };

        //  https://github.com/model-checking/kani/blob/main/library/kani/src/arbitrary.rs
        const ALLOWED_TYPES: &[&str; 21] = &[
            "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
            "f32", "f64", "()", "bool", "char", "[", "Option", "Result", "Phantom",
        ];

        let file_instrumented = maybe_formatted
            .lines()
            // ignore comments
            .filter(|line| !line.starts_with("//"))
            .map(|line| {
                // note: the kani::proof attr does NOT work on functions that have args :(
                // try to only add it to argless fns
                if line.contains("fn ")
                    && Regex::new(r"[[:word:]]\(\)").unwrap().is_match(line)
                    // dont treat generics
                    && !(line.contains('<') || line.contains('>'))
                {
                    format!("#[kani::proof]\n{}", line)
                    // no ret type
                } else if let Some(shortest_match_initial) =
                    Regex::new(r"fn [[:word:]]\(.+\) ->").unwrap().find(line)
                {
                    let shortest_match = shortest_match_initial.as_str();
                    //   dbg!(&shortest_match);
                    let shortest_match_args = Regex::new(r"\(.*\)")
                        .unwrap()
                        .find(shortest_match)
                        .unwrap()
                        .as_str();
                    //    dbg!(shortest_match_args);
                    // remove the args from the line, since we are going to add instrument them
                    let mut new_line = line.replace(shortest_match_args, "()");
                    // skip first and last char
                    let args = &shortest_match_args[1..shortest_match_args.len() - 1];
                    let args = args.split(',').collect::<Vec<_>>();
                    let new_args = args
                        .iter()
                        .map(|binding_plus_ty| {
                            if ALLOWED_TYPES.iter().any(|ty| binding_plus_ty.contains(ty)) {
                                format!("let {binding_plus_ty} = kani::any(); \n")
                            } else {
                                format!("let {binding_plus_ty} = Default::default();\n")
                            }
                        })
                        .collect::<String>();
                    new_line.insert_str(
                        new_line.rfind('{').unwrap() + 1, /* insert after the '{' */
                        &new_args,
                    );

                    format!("#[kani::proof] {new_line}")
                } else if let Some(shortest_match_initial) =
                    Regex::new(r"fn [[:word:]]\(.*\) \{").unwrap().find(line)
                {
                    let shortest_match = shortest_match_initial.as_str();
                    // dbg!(&shortest_match);
                    let shortest_match_args = Regex::new(r"\(.*\)")
                        .unwrap()
                        .find(shortest_match)
                        .unwrap()
                        .as_str();
                    // dbg!(shortest_match_args);
                    // remove the args from the line, since we are going to add instrument them
                    let mut new_line = line.replace(shortest_match_args, "()");

                    // skip first and last char
                    let args = &shortest_match_args[1..shortest_match_args.len() - 1];
                    let args = args.split(',').collect::<Vec<_>>();

                    let new_args = args
                        .iter()
                        .map(|binding_plus_ty| {
                            if ALLOWED_TYPES.iter().any(|ty| binding_plus_ty.contains(ty)) {
                                format!("let {binding_plus_ty} = kani::any(); \n")
                            } else {
                                format!("let {binding_plus_ty} = Default::default(); ")
                            }
                        })
                        .collect::<String>();
                    new_line.insert_str(
                        new_line.rfind('{').unwrap() + 1, /* insert after the '{' */
                        &new_args,
                    );

                    format!("#[kani::proof] {new_line}")
                    // with ret type
                } else {
                    line.into()
                }
            })
            // Lines iterator by default removes linebreaks, so readd them
            .map(|line| format!("{line}\n"))
            .collect::<String>();

        // eprintln!("\n\n{file_instrumented}\n\n");
        // panic!();
        // write the content of the file we want to check into tmpcrate/src/main.rs
        std::fs::write(source_path, file_instrumented).expect("failed to write to file");

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
        cmd.arg("kani")
            .current_dir(&crate_path)
            .env("RUSTFLAGS", RUSTCFLAGS)
            .env("RUSTC_WRAPPER", "");

        out = Some(
            prlimit_run_command(&mut cmd)
                .unwrap_or_else(|_| panic!("Error: {cmd:?}, executable: {executable:?}")),
        );
        cmd_ = Some(cmd);
        v.push(out.clone());
    }

    let out = out.unwrap();
    let cmd = cmd_.unwrap();

    //  dbg!(&out);
    // eprintln!("{}", String::from_utf8(out.clone().stderr).unwrap());
    // eprintln!("{}", String::from_utf8(out.clone().stdout).unwrap());

    let res = v
        .iter()
        .flatten()
        .map(|output| {
            let mut std = String::from_utf8(output.clone().stdout).unwrap();
            let stderr = String::from_utf8(output.clone().stderr).unwrap();
            std.push_str(&stderr);

            std.lines()
                .filter(|l| l.contains("`kani::Arbitrary` is not implemented"))
                .for_each(|l| eprintln!("{l}"));
            std.lines()
                .filter(|l| l.contains("https://github.com/model-checking/kani/issues/"))
                .for_each(|l| eprintln!("{} {l}", file.display()));
            std.lines()
                .filter(|line| line.contains("Status: FAILURE"))
                .count()
        })
        .collect::<Vec<usize>>();
    if res[0] < res[1] {
        eprintln!(
            "\n\n{} MIR OPT CAUSES PANICS: {} to {}\n\n",
            file.display(),
            res[0],
            res[1]
        );
    }

    CommandOutput::new(
        out,
        get_cmd_string(&cmd),
        Vec::new(),
        crate::Executable::Kani,
    )
}

pub(crate) fn run_cranelift(
    executable: &str,
    file: &Path,
    incremental: bool,
    rustc_flags: &[&str],
    global_tempdir_path: &PathBuf,
) -> CommandOutput {
    if incremental {
        // only run incremental compilation tests
        return run_rustc_incremental(executable, file, global_tempdir_path);
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
        .arg(output_file)
        .arg(dump_mir_dir);
    if !has_main {
        cmd.arg("--crate-type=lib");
    }
    //dbg!(&cmd);

    let actual_args = cmd
        .get_args()
        .map(|s| s.to_owned())
        .collect::<Vec<OsString>>();

    // run the command
    let output = prlimit_run_command(&mut cmd)
        .unwrap_or_else(|_| panic!("Error: {cmd:?}, executable: {executable:?}"));
    CommandOutput::new(
        output,
        get_cmd_string(&cmd),
        actual_args,
        crate::Executable::RustcCGClif,
    )
    // remove tempdir
    //tempdir.close().unwrap();
}

// https://github.com/rust-lang/rustc_codegen_gcc
pub(crate) fn rustc_codegen_gcc_local(
    executable: &str,
    file: &Path,
    incremental: bool,
    rustc_flags: &[&str],
    global_tempdir_path: &PathBuf,
) -> CommandOutput {
    if incremental {
        // only run incremental compilation tests
        return run_rustc_incremental(executable, file, global_tempdir_path);
    }
    // if the file contains no "main", run with "--crate-type lib"
    let has_main = std::fs::read_to_string(file)
        .unwrap_or_default()
        .contains("fn main(");

    //let tempdir = TempDir::new("rustc_testrunner_tmpdir").unwrap();
    //let tempdir_path = tempdir.path();

    let new_tempdir = TempDir::new_in(global_tempdir_path, "newdir").unwrap();
    let new_tempdir = new_tempdir.path();
    let new_tempdir = new_tempdir.join("executablename");

    let dump_mir_dir = String::from("-Zdump-mir-dir=/dev/null");

    let mut cmd = Command::new("rustc");
    cmd
    .args(["+nightly-2023-06-19" , "-Zcodegen-backend=/home/matthias/vcs/github/rustc_codegen_gcc_attempt2/rustc_codegen_gcc/target/release/librustc_codegen_gcc.so", 
    "--sysroot",
     "/home/matthias/vcs/github/rustc_codegen_gcc_attempt2/rustc_codegen_gcc/build_sysroot/sysroot"]).env("LD_LIBRARY_PATH","/home/matthias/vcs/github/rustc_codegen_gcc_attempt2/gcc-build/gcc" )
     .arg(file)
     .env("RUST_COMPILER_RT_ROOT", "/home/matthias/vcs/github/rustc_codegen_gcc_attempt2/llvm/compiler-rt/")
        .args(rustc_flags)
        // always keep these:
        .arg("-o")
        .arg(new_tempdir)
        .arg(dump_mir_dir);
    if !has_main {
        cmd.arg("--crate-type=lib");
    }
    //    dbg!(&cmd);

    let actual_args = cmd
        .get_args()
        .map(|s| s.to_owned())
        .collect::<Vec<OsString>>();

    // run the command
    let output = prlimit_run_command(&mut cmd)
        .unwrap_or_else(|_| panic!("Error: {cmd:?}, executable: {executable:?}"));

    // eprintln!("{:?}", output);
    CommandOutput::new(
        output,
        get_cmd_string(&cmd),
        actual_args,
        crate::Executable::RustcCGClif,
    )
    // remove tempdir
    //tempdir.close().unwrap();
}

pub(crate) fn prlimit_run_command(
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
            .chain(std::iter::once((
                std::ffi::OsStr::new("RUSTC_ICE"),
                std::ffi::OsStr::new("0"),
            )))
            .collect::<Vec<(&std::ffi::OsStr, &std::ffi::OsStr)>>();
        let full_miri = new_command
            .get_args()
            .chain(std::iter::once(program))
            .any(|s| s == std::ffi::OsStr::new("miri"));

        let mut cmd = Command::new("prlimit");
        cmd.arg("--noheadings");
        let runtime_limit = if full_miri {
            // miri timout: 20 seconds
            PROCESS_TIMEOUT_MIRI_S
        } else {
            // all other timeouts: 30 seconds
            PROCESS_TIMEOUT_S
        };
        cmd.arg(format!("--as={}", 3076_u32 * 1000_u32 * 1000_u32)); // 3 GB
        cmd.arg(format!("--cpu={runtime_limit}"));

        cmd.arg(program);
        cmd.args(args);
        if let Some(dir) = current_dir {
            cmd.current_dir(dir);
        }
        cmd.envs(envs);
        cmd.output()
    }
}

pub(crate) fn file_compiles(
    file: &std::path::PathBuf,
    executable: &str,
    global_tempdir_path: &PathBuf,
) -> bool {
    let has_main = std::fs::read_to_string(file)
        .unwrap_or_default()
        .contains("fn main(");

    let file = file.canonicalize().unwrap();
    let tempdir = TempDir::new_in(global_tempdir_path, "rustc_testrunner_tmpdir").unwrap();
    let tempdir_path = tempdir.path();

    ["2015", "2018", "2021"]
        .iter()
        .map(|year| format!("--edition={year}"))
        .any(|edition_flag| {
            let mut cmd = Command::new(executable);
            if !has_main {
                cmd.arg("--crate-type=lib");
            } else {
                cmd.arg("--crate-type=bin");
            }
            cmd.arg(&file)
                .arg("-Zno-codegen")
                .arg("-Zforce-unstable-if-unmarked")
                .arg(edition_flag)
                .args(["--cap-lints", "warn"])
                .env("CARGO_TERM_COLOR", "never")
                .current_dir(tempdir_path)
                .env("CARGO_TERM_COLOR", "never")
                .env("SYSROOT", &*SYSROOT_PATH);

            matches!(
                prlimit_run_command(&mut cmd)
                    .ok()
                    .map(|x| x.status.success()),
                Some(true)
            )
        })
}

pub(crate) fn file_compiles_from_string(
    file_content: &String,
    executable: &str,
    global_tempdir_path: &PathBuf,
) -> bool {
    let has_main = file_content.contains("fn main(");

    let tempdir = TempDir::new_in(global_tempdir_path, "compiles_from_string_tempdir").unwrap();
    let tempdir_path = tempdir.path();

    let file_path = tempdir_path.join("file.rs");

    let mut file_handle =
        std::fs::File::create(&file_path).expect("failed to create file_compiles_from_string file");

    file_handle.write_all(file_content.as_bytes()).unwrap();

    // to path
    let file = file_path;
    ["2015", "2018", "2021"]
        .iter()
        .map(|year| format!("--edition={year}"))
        .any(|edition_flag| {
            let mut cmd = Command::new(executable);
            if !has_main {
                cmd.arg("--crate-type=lib");
            } else {
                cmd.arg("--crate-type=bin");
            }
            cmd.arg(&file)
                .arg("-Zno-codegen")
                .arg("-Zforce-unstable-if-unmarked")
                .arg(edition_flag)
                .args(["--cap-lints", "warn"])
                .env("CARGO_TERM_COLOR", "never")
                .current_dir(tempdir_path)
                .env("CARGO_TERM_COLOR", "never")
                .env("SYSROOT", &*SYSROOT_PATH);

            matches!(
                prlimit_run_command(&mut cmd)
                    .ok()
                    .map(|x| x.status.success()),
                Some(true)
            )
        })
}

pub(crate) fn incremental_stress_test(
    file_a: &std::path::PathBuf,
    files: &Vec<std::path::PathBuf>,
    executable: &str,
    global_tempdir_path: &PathBuf,
) -> Option<(Output, String, Vec<OsString>, PathBuf, PathBuf)> {
    use rand::seq::SliceRandom;

    let file_b = files.choose(&mut rand::thread_rng()).unwrap();

    let files = [&file_a, &file_b];

    let tempdir = TempDir::new_in(global_tempdir_path, "rustc_testrunner_tmpdir").unwrap();
    let tempdir_path = tempdir.path();

    let mut cmd = Command::new("DUMMY");
    let mut output = None;
    let mut actual_args = Vec::new();

    // make sure both files compile
    for file in files {
        if !file_compiles(file, executable, global_tempdir_path) {
            return None;
        }
    }
    // both files compile, continue with actual incremental checking
    eprintln!("found possible pair: {files:?}");
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
        output = Some(prlimit_run_command(&mut command));
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

/// Run rustc its own tests with different parameters
/// If an ICE (internal compiler error/crash/panic) is found, find out
/// the smallest combination of responsible flags and save data about the crash
///
/// The programm is not limited to run rustc, but can also run clippy or rustdoc:
/// rustc:         icemaker
/// clippy:        icemaker -c
/// rustfmt:       icemaker -f
/// rust-analyzer: icemaker -a
/// rustdoc:       icemaker -r
/// incr comp      icemaker -i
use itertools::Itertools;
use pico_args::Arguments;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::Instant;
use tempdir::TempDir;
use walkdir::WalkDir;
// whether we run clippy, rustdoc or rustc (default: rustc)
struct Args {
    clippy: bool,
    rustdoc: bool,
    analyzer: bool, // rla
    rustfmt: bool,
    incremental: bool, // incremental compilation
}

// in what channel a regression is first noticed?
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
enum Regression {
    Stable,
    Beta,
    Nightly,
    Master,
}

impl std::fmt::Display for Regression {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        let s = match self {
            Regression::Stable => "stable",
            Regression::Beta => "beta",
            Regression::Nightly => "nightly",
            Regression::Master => "master",
        };

        write!(f, "{}", s)
    }
}

enum Executable {
    Rustc,
    Clippy,
    Rustdoc,
    RustAnalyzer,
    Rustfmt,
}

impl Executable {
    fn path(&self) -> String {
        match self {
            Executable::Rustc => {
                let mut p = home::rustup_home().unwrap();
                p.push("toolchains");
                p.push("master");
                p.push("bin");
                p.push("rustc");
                p.display().to_string()
            }
            Executable::Clippy => {
                let mut p = home::rustup_home().unwrap();
                p.push("toolchains");
                p.push("master");
                p.push("bin");
                p.push("clippy-driver");
                p.display().to_string()
            }
            Executable::Rustdoc => {
                let mut p = home::rustup_home().unwrap();
                p.push("toolchains");
                p.push("master");
                p.push("bin");
                p.push("rustdoc");
                p.display().to_string()
            }
            Executable::RustAnalyzer => {
                let mut p = home::rustup_home().unwrap();
                p.push("toolchains");
                p.push("master");
                p.push("bin");
                p.push("rust-analyzer");
                p.display().to_string()
            }
            Executable::Rustfmt => {
                let mut p = home::rustup_home().unwrap();
                p.push("toolchains");
                p.push("master");
                p.push("bin");
                p.push("rustfmt");
                p.display().to_string()
            }
        }
    }
}

const RUSTC_FLAGS: &[&str] = &[
    "-Zvalidate-mir",
    "-Zverify-llvm-ir=yes",
    "-Zincremental-verify-ich=yes",
    "-Zmir-opt-level=0",
    "-Zmir-opt-level=1",
    "-Zmir-opt-level=2",
    "-Zmir-opt-level=3",
    "-Zunsound-mir-opts",
    "-Zdump-mir=all",
    "--emit=mir",
    "-Zsave-analysis",
    "-Zprint-mono-items=full",
    "-Zpolymorphize=on",
    //"-Zchalk=yes",
    //"-Zinstrument-coverage",
    //"-Cprofile-generate=/tmp/icemaker_pgo/", // incompatible with Zinstr-cov
];
// -Zvalidate-mir -Zverify-llvm-ir=yes -Zincremental-verify-ich=yes -Zmir-opt-level=0 -Zmir-opt-level=1 -Zmir-opt-level=2 -Zmir-opt-level=3 -Zdump-mir=all --emit=mir -Zsave-analysis -Zprint-mono-items=full

// represents a crash
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
struct ICE {
    // what release channel did we crash on?
    regresses_on: Regression,
    // do we need any special features for that ICE?
    needs_feature: bool,
    // file that reproduces the ice
    file: PathBuf,
    // path to the rustc binary
    //    executable: String,
    // args that are needed to crash rustc
    args: Vec<String>,
    // part of the error message
    error_reason: String,
    // ice message
    ice_msg: String,
}

impl std::fmt::Display for ICE {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(
            f,
            "'rustc {} {}' ICEs on {}, {} with: {} / '{}'",
            self.file.display(),
            self.args.join(" "),
            self.regresses_on,
            if self.needs_feature {
                "and uses features"
            } else {
                "without features!"
            },
            self.error_reason,
            self.ice_msg,
        )
    }
}

fn get_flag_combinations() -> Vec<Vec<String>> {
    // get the power set : [a, b, c] => [a], [b], [c], [a,b], [a,c], [b,c], [a,b,c]
    let mut combs = Vec::new();
    for numb_comb in 0..=RUSTC_FLAGS.len() {
        let combinations = RUSTC_FLAGS
            .iter()
            .map(|s| s.to_string())
            .combinations(numb_comb);
        combs.push(combinations);
    }

    let combs = combs.into_iter().flatten();
    combs.collect()
}

fn main() {
    // read in existing errors
    // read the string INTO Vec<ICE>
    let errors_before: Vec<ICE> = if std::path::PathBuf::from("errors.json").exists() {
        serde_json::from_str(&std::fs::read_to_string("errors.json").unwrap()).unwrap()
    } else {
        Vec::new()
    };

    let flags: Vec<Vec<String>> = get_flag_combinations();
    // println!("flags:\n");
    // flags.iter().for_each(|x| println!("{:?}", x));
    // parse args
    let mut args = Arguments::from_env();

    let args = Args {
        clippy: args.contains(["-c", "--clippy"]),
        rustdoc: args.contains(["-r", "--rustdoc"]),
        analyzer: args.contains(["-a", "--analyzer"]),
        rustfmt: args.contains(["-f", "--rustfmt"]),
        incremental: args.contains(["-i", "--incremental"]),
    };

    let executable: Executable = if args.clippy {
        Executable::Clippy
    } else if args.rustdoc {
        Executable::Rustdoc
    } else if args.analyzer {
        Executable::RustAnalyzer
    } else if args.rustfmt {
        Executable::Rustfmt
    } else {
        Executable::Rustc
    };

    // search for rust files inside CWD
    let mut files = WalkDir::new(".")
        .into_iter()
        .filter(|entry| entry.is_ok())
        .map(|e| e.unwrap())
        .filter(|f| f.path().extension() == Some(&OsStr::new("rs")))
        .map(|f| f.path().to_owned())
        .collect::<Vec<PathBuf>>();

    // sort by path
    files.sort();

    let exec_path = executable.path();

    // "rustc"
    // assume CWD is src/test from rustc repo root
    // "build/x86_64-unknown-linux-gnu/stage1/bin/rustc"

    println!("bin: {}", exec_path);
    println!("checking: {} files\n\n", files.len());

    // files that take too long (several minutes) to check or cause other problems
    #[allow(non_snake_case)]
    let EXCEPTION_LIST: Vec<PathBuf> = [
        // runtime
        "./src/test/ui/closures/issue-72408-nested-closures-exponential.rs",
        "./src/test/ui/issues/issue-74564-if-expr-stack-overflow.rs",
        "./library/stdarch/crates/core_arch/src/mod.rs", //10+ mins
        // memory
        "./src/test/ui/issues/issue-50811.rs",
        "./src/test/ui/issues/issue-29466.rs",
        "./src/tools/miri/tests/run-pass/float.rs",
        "./src/test/ui/numbers-arithmetic/saturating-float-casts-wasm.rs",
        "./src/test/ui/numbers-arithmetic/saturating-float-casts-impl.rs",
        "./src/test/ui/numbers-arithmetic/saturating-float-casts.rs",
        "./src/test/ui/wrapping-int-combinations.rs",
        // glacier/memory/time:
        "./fixed/23600.rs",
        "./23600.rs",
        "./fixed/71699.rs",
        "./71699.rs",
        // runtime
        "./library/stdarch/crates/core_arch/src/x86/avx512bw.rs",
        "./library/stdarch/crates/core_arch/src/x86/mod.rs",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();

    // how long did we take?
    let start_time = Instant::now();

    // collect errors by running on files in parallel
    let mut errors: Vec<ICE> = files
        .par_iter()
        .filter(|file| !EXCEPTION_LIST.contains(file))
        .filter_map(|file| find_crash(&file, &exec_path, &executable, &flags, args.incremental))
        .collect();

    // sort by filename first and then by ice so that identical ICS are grouped up
    errors.sort_by_key(|ice| ice.file.clone());
    errors.sort_by_key(|ice| ice.ice_msg.clone());

    // if we are done, print all errors
    println!("errors:\n");

    errors.iter().for_each(|f| {
        let mut debug = format!("{:?}", f);
        debug.truncate(300);
        println!("{}", debug);
    });

    // in the end, save all the errors to a file
    let errors_new = serde_json::to_string_pretty(&errors).unwrap();
    std::fs::write("errors.json", &errors_new).expect("failed to write to file");

    println!("\ndiff: \n");
    // get the diff
    let diff = diff::lines(
        &serde_json::to_string_pretty(&errors_before).unwrap(),
        &errors_new,
    )
    .iter()
    .map(|diff| match diff {
        diff::Result::Left(l) => format!("-{}\n", l),
        diff::Result::Both(l, _) => format!(" {}\n", l),
        diff::Result::Right(r) => format!("+{}\n", r),
    })
    .collect::<String>();

    println!("{}", diff);

    let new_ices = errors
        .iter()
        .filter(|new_ice| !errors_before.contains(new_ice))
        .collect::<Vec<&ICE>>();
    // TODO do the same for removed ices?
    println!("NEW ICES:\n{:#?}", new_ices);

    // print a warning if a file takes longer than X to process
    let seconds_elapsed = start_time.elapsed().as_secs();
    let files_number = files.len();
    let files_per_second = files_number / seconds_elapsed as usize;
    println!(
        "\nChecked {} files in {:.2} minutes, {} files/second",
        files_number,
        seconds_elapsed as f64 / 60_f64,
        files_per_second
    );
}

fn find_crash(
    file: &PathBuf,
    exec_path: &str,
    executable: &Executable,
    compiler_flags: &[Vec<String>],
    incremental: bool,
) -> Option<ICE> {
    let thread_start = Instant::now();

    let output = file.display().to_string();
    let cmd_output = match executable {
        Executable::Clippy => run_clippy(exec_path, file),
        Executable::Rustc => run_rustc(exec_path, file, incremental),
        Executable::Rustdoc => run_rustdoc(exec_path, file),
        Executable::RustAnalyzer => run_rust_analyzer(exec_path, file),
        Executable::Rustfmt => run_rustfmt(exec_path, file),
    };

    // find out the ice message
    let mut ice_msg = String::from_utf8_lossy(&cmd_output.stderr)
        .lines()
        .find(|line| {
            line.contains("panicked at") || line.contains("error: internal compiler error: ")
        })
        .unwrap_or_default()
        .to_string();

    ice_msg = ice_msg.replace("error: internal compiler error:", "ICE");

    let found_error: Option<String> = find_ICE(cmd_output);
    // check if the file enables any compiler features
    let uses_feature: bool = uses_feature(file);

    // @TODO merge the two  found_error.is_some() branches and print ice reason while checking
    if found_error.is_some() {
        print!("\r");
        println!(
            "ICE: {output: <150} {msg} {feat}",
            output = output,
            msg = found_error.clone().unwrap(),
            feat = if uses_feature { "" } else { "no feat!" },
        );
        print!("\r");
        let _stdout = std::io::stdout().flush();
    } else {
        //@FIXME this only advances the checking once the files has already been checked!

        // let stdout = std::io::stdout().flush();
        print!("\rChecking {output: <150}", output = output);
        let _stdout = std::io::stdout().flush();
    }

    if incremental && found_error.is_some() {
        return Some(ICE {
            regresses_on: Regression::Nightly,

            needs_feature: uses_feature,
            file: file.to_owned(),
            args: vec![
                "-Z incremental-verify-ich=yes".into(),
                "-C incremental   ???".into(),
            ],
            // executable: rustc_path.to_string(),
            error_reason: found_error.clone().unwrap_or_default(),
            ice_msg,
        });
    }

    let mut ret = None;
    if let Some(error_reason) = found_error {
        // rustc or clippy crashed, we have an ice
        // find out which flags are responsible
        // run rustc with the file on several flag combinations, if the first one ICEs, abort
        let mut bad_flags: &Vec<String> = &Vec::new();

        match executable {
            Executable::Rustc => {
                compiler_flags.iter().any(|flags| {
                    let tempdir = TempDir::new("rustc_testrunner_tmpdir").unwrap();
                    let tempdir_path = tempdir.path();
                    let output_file = format!("-o{}/file1", tempdir_path.display());
                    let dump_mir_dir = format!("-Zdump-mir-dir={}", tempdir_path.display());

                    let output = Command::new(exec_path)
                        .arg(&file)
                        .args(&*flags)
                        .arg(output_file)
                        .arg(dump_mir_dir)
                        .output()
                        .unwrap();
                    let found_error2 = find_ICE(output);
                    // remove the tempdir
                    tempdir.close().unwrap();
                    if found_error2.is_some() {
                        // save the flags that the ICE repros with
                        bad_flags = flags;
                        true
                    } else {
                        false
                    }
                });

                // find out if this is a beta/stable/nightly regression
            }
            Executable::Clippy
            | Executable::Rustdoc
            | Executable::RustAnalyzer
            | Executable::Rustfmt => {}
        }
        let regressing_channel = find_out_crashing_channel(&bad_flags, file);

        let ret2 = ICE {
            regresses_on: match executable {
                Executable::Clippy => Regression::Master,
                _ => regressing_channel,
            },

            needs_feature: uses_feature,
            file: file.to_owned(),
            args: bad_flags.to_vec(),
            // executable: rustc_path.to_string(),
            error_reason,
            ice_msg,
        };
        ret = Some(ret2);
    };

    // print a warning if a file takes longer than X to process
    let seconds_elapsed = thread_start.elapsed().as_secs();
    let minutes_elapsed: u64 = seconds_elapsed / 60;
    const MINUTE_LIMIT: u64 = 1;
    if minutes_elapsed > (MINUTE_LIMIT) {
        println!(
            "\n{} running for more ({} minutes) than {} minute\n",
            file.display(),
            seconds_elapsed / 60,
            MINUTE_LIMIT
        );
    }

    ret
}

fn find_out_crashing_channel(bad_flags: &[String], file: &PathBuf) -> Regression {
    // simply check if we crasn on nightly, beta, stable or master
    let toolchain_home: PathBuf = {
        let mut p = home::rustup_home().unwrap();
        p.push("toolchains");
        p
    };

    let bad_but_no_nightly_flags: Vec<&String> = bad_flags
        .iter()
        .filter(|flag| !flag.starts_with("-Z"))
        .collect();

    let tempdir = TempDir::new("rustc_testrunner_tmpdir").unwrap();
    let tempdir_path = tempdir.path();
    let output_file = format!("-o{}/file1", tempdir_path.display());
    let dump_mir_dir = format!("-Zdump-mir-dir={}", tempdir_path.display());

    let mut nightly_path = toolchain_home.clone();
    nightly_path.push("nightly-x86_64-unknown-linux-gnu");
    nightly_path.push("bin");
    nightly_path.push("rustc");
    let mut beta_path = toolchain_home.clone();
    beta_path.push("beta-x86_64-unknown-linux-gnu");
    beta_path.push("bin");
    beta_path.push("rustc");
    let mut stable_path = toolchain_home;
    stable_path.push("stable-x86_64-unknown-linux-gnu");
    stable_path.push("bin");
    stable_path.push("rustc");

    let stable_ice: bool = find_ICE(
        Command::new(stable_path)
            .arg(&file)
            .args(&bad_but_no_nightly_flags)
            .arg(&output_file)
            //.arg(&dump_mir_dir)
            .output()
            .unwrap(),
    )
    .is_some();

    let beta_ice: bool = find_ICE(
        Command::new(beta_path)
            .arg(&file)
            .args(&bad_but_no_nightly_flags)
            .arg(&output_file)
            //.arg(&dump_mir_dir)
            .output()
            .unwrap(),
    )
    .is_some();

    let nightly_ice: bool = find_ICE(
        Command::new(nightly_path)
            .arg(&file)
            .args(bad_flags)
            .arg(&output_file)
            .arg(&dump_mir_dir)
            .output()
            .unwrap(),
    )
    .is_some();
    // remove tempdir
    tempdir.close().unwrap();

    if stable_ice {
        Regression::Stable
    } else if beta_ice {
        Regression::Beta
    } else if nightly_ice {
        Regression::Nightly
    } else {
        Regression::Master
    }
}

fn uses_feature(file: &std::path::Path) -> bool {
    let file: String = std::fs::read_to_string(&file)
        .unwrap_or_else(|_| panic!("Failed to read '{}'", file.display()));
    file.contains("feature(")
}

#[allow(non_snake_case)]
fn find_ICE(output: Output) -> Option<String> {
    // let output = cmd.output().unwrap();
    let _exit_status = output.status;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let ice_keywords = [
        "LLVM ERROR",
        "panicked at:",
        "`delay_span_bug`",
        "query stack during panic:",
        "internal compiler error:",
        "RUST_BACKTRACE=",
    ];

    for kw in &ice_keywords {
        if stderr.contains(kw) || stdout.contains(kw) {
            return Some((*kw).into());
        }
    }

    None
}

fn run_rustc(executable: &str, file: &PathBuf, incremental: bool) -> Output {
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
        .args(RUSTC_FLAGS)
        // always keep these:
        .arg(&output_file)
        .arg(&dump_mir_dir);
    if !has_main {
        output.args(&["--crate-type", "lib"]);
    }
    //dbg!(&output);
    // run the command
    output.output().expect(&format!(
        "Error: {:?}, executable: {:?}",
        output, executable
    ))
    // remove tempdir
    //tempdir.close().unwrap();
}

fn run_rustc_incremental(executable: &str, file: &PathBuf) -> Output {
    let tempdir = TempDir::new("rustc_testrunner_tmpdir").unwrap();
    let tempdir_path = tempdir.path();

    let has_main = std::fs::read_to_string(&file)
        .unwrap_or_default()
        .contains("fn main(");

    let mut output = None;
    for i in &[0, 1] {
        let mut command = Command::new(executable);
        if !has_main {
            command.args(&["--crate-type", "lib"]);
        }
        let command = command
            .arg(&file)
            // avoid error: the generated executable for the input file  .. onflicts with the existing directory..
            .arg(format!("-o{}/{}", tempdir_path.display(), i))
            .arg(format!("-Cincremental={}", tempdir_path.display()))
            .arg("-Zincremental-verify-ich=yes");

        output = Some(command.output());
    }

    let output = output.map(|output| output.unwrap()).unwrap();

    tempdir.close().unwrap();
    //dbg!(&output);
    output
}

fn run_clippy(executable: &str, file: &PathBuf) -> Output {
    Command::new(executable)
        .env("RUSTFLAGS", "-Z force-unstable-if-unmarked")
        .env("SYSROOT", "/home/matthias/.rustup/toolchains/master")
        .arg(&file)
        .arg("-Aclippy::cargo") // allow cargo lints
        .arg("-Wclippy::internal")
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
        .args(&["-o", "/dev/null"])
        .output()
        .unwrap()
}

fn run_rustdoc(executable: &str, file: &PathBuf) -> Output {
    Command::new(executable)
        .env("RUSTFLAGS", "-Z force-unstable-if-unmarked")
        .env("SYSROOT", "/home/matthias/.rustup/toolchains/master")
        .arg(&file)
        .arg("-Zunstable-options")
        .arg("--document-private-items")
        .arg("--document-hidden-items")
        .args(&["--cap-lints", "warn"])
        .args(&["-o", "/dev/null"])
        .output()
        .unwrap()
}

fn run_rust_analyzer(executable: &str, file: &PathBuf) -> Output {
    let file_content = std::fs::read_to_string(&file).expect("failed to read file ");

    let mut process = Command::new(executable)
        .arg("symbols")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let stdin = &mut process.stdin.as_mut().unwrap();
    stdin.write_all(file_content.as_bytes()).unwrap();
    process.wait_with_output().unwrap()

    /*
    let output = process.wait_with_output().unwrap();
    println!("\n\n{:?}\n\n", output);
    output
    */
}

fn run_rustfmt(executable: &str, file: &PathBuf) -> Output {
    Command::new(executable)
        .env("SYSROOT", "/home/matthias/.rustup/toolchains/master")
        .arg(&file)
        .arg("--check")
        .args(&["--edition", "2018"])
        .output()
        .unwrap()
}

use home;
use itertools::Itertools;
use pico_args::Arguments;
use rayon::prelude::*;
use std::ffi::OsStr;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output};
use tempdir::TempDir;
use walkdir::WalkDir;

// if we run clippy or rustc (default: rustc)
struct Args {
    clippy: bool,
}

// in what channel a regression starts occurring
#[derive(Debug)]
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

const RUSTC_PATH: &str =
    "/home/matthias/vcs/github/rust/build/x86_64-unknown-linux-gnu/stage1/bin/rustc";

const RUSTC_FLAGS: &[&str] = &[
    "-Zvalidate-mir",
    "-Zverify-llvm-ir=yes",
    "-Zincremental-verify-ich=yes",
    "-Zmir-opt-level=0",
    "-Zmir-opt-level=1",
    "-Zmir-opt-level=2",
    "-Zmir-opt-level=3",
    "-Zdump-mir=all",
    "--emit=mir",
    "-Zsave-analysis",
];

// data for an ICE
#[derive(Debug)]
struct ICE {
    regresses_on: Regression,
    // do we need any special features for that ICE
    needs_feature: bool,
    // file that reproduces the ice
    file: PathBuf,
    // path to the rustc binary
    executable: String,
    // args that are needed to crash rustc
    args: Vec<String>,
}

fn get_flag_combinations() -> Vec<Vec<String>> {
    // get the power set : [a, b, c] => [a] [b] [c] , [ab], [ac] [ bc] [ a, b, c]
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
    let flags: Vec<Vec<String>> = get_flag_combinations();
    // println!("flags:\n");
    // flags.iter().for_each(|x| println!("{:?}", x));
    // parse args
    let mut args = Arguments::from_env();

    let args = Args {
        clippy: args.contains(["-c", "--clippy"]),
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

    let rustc_path = if args.clippy {
        "clippy-driver"
    } else {
        // "rustc"
        // assume CWD is src/test from rustc repo root
        // "build/x86_64-unknown-linux-gnu/stage1/bin/rustc"
        RUSTC_PATH
    };

    println!("bin: {}\n\n", rustc_path);
    // collect error by running on files in parallel
    let mut errors: Vec<ICE> = files
        .par_iter()
        .filter_map(|file| find_crash(&file, rustc_path, args.clippy, &flags))
        .collect();

    errors.sort_by_key(|ice| ice.file.clone());

    println!("errors:\n");
    errors.iter().for_each(|f| println!("{:?}", f));
}

fn find_crash(
    file: &PathBuf,
    rustc_path: &str,
    clippy: bool,
    compiler_flags: &Vec<Vec<String>>,
) -> Option<ICE> {
    let output = file.display().to_string();
    let cmd_output = if clippy {
        run_rustc(rustc_path, file)
    } else {
        run_clippy(rustc_path, file)
    };

    let found_error: Option<String> = find_ICE(cmd_output);
    let uses_feature: bool = uses_feature(file);

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
        // let stdout = std::io::stdout().flush();
        print!("\rChecking {output: <150}", output = output);
        let _stdout = std::io::stdout().flush();
    }

    if found_error.is_some() {
        // rustc or clippy crashed, we have an ice
        // find out which flags are responsible
        // run rustc with the file on several flag combinations, if the first one ICEs, abort
        let mut bad_flags: &Vec<String> = &Vec::new();

        compiler_flags.iter().any(|flags| {
            let tempdir = TempDir::new("tmpdir").unwrap();
            let tempdir_path = tempdir.path();
            let output_file = format!("-o{}/file1", tempdir_path.display());
            let dump_mir_dir = format!("-Zdump-mir-dir={}", tempdir_path.display());

            let output = Command::new(rustc_path)
                .arg(&file)
                .args(&*flags)
                .arg(output_file)
                .arg(dump_mir_dir)
                .output()
                .unwrap();
            let found_error = find_ICE(output);
            // remove the tempdir
            tempdir.close().unwrap();
            if found_error.is_some() {
                // save the flags that the ICE repros with
                bad_flags = flags;
                true
            } else {
                false
            }
        });

        // find out if this is a beta/stable/nightly regression

        let regressing_channel = find_out_crashing_channel(&bad_flags, file);

        if found_error.is_some() {
            return Some(ICE {
                regresses_on: regressing_channel,
                needs_feature: uses_feature,
                file: file.to_owned(),
                args: bad_flags.to_vec(),
                executable: rustc_path.to_string(),
            });
        }
    }
    None
}

fn find_out_crashing_channel(bad_flags: &Vec<String>, file: &PathBuf) -> Regression {
    let toolchain_home: PathBuf = {
        let mut p = home::rustup_home().unwrap();
        p.push("toolchains");
        p
    };

    let tempdir = TempDir::new("tmpdir").unwrap();
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
    let mut stable_path = toolchain_home.clone();
    stable_path.push("stable-x86_64-unknown-linux-gnu");
    stable_path.push("bin");
    stable_path.push("rustc");

    let stable_ice: bool = find_ICE(
        Command::new(stable_path)
            .arg(&file)
            .args(bad_flags)
            .arg(&output_file)
            .arg(&dump_mir_dir)
            .output()
            .unwrap(),
    )
    .is_some();

    let beta_ice: bool = find_ICE(
        Command::new(beta_path)
            .arg(&file)
            .args(bad_flags)
            .arg(&output_file)
            .arg(&dump_mir_dir)
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
    let file: String = std::fs::read_to_string(&file).unwrap();
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

fn run_clippy(executable: &str, file: &PathBuf) -> Output {
    let tempdir = TempDir::new("tmpdir").unwrap();
    let tempdir_path = tempdir.path();
    let output_file = format!("-o{}/file1", tempdir_path.display());
    let dump_mir_dir = format!("-Zdump-mir-dir={}", tempdir_path.display());

    let output = Command::new(executable)
        .arg(&file)
        .args(RUSTC_FLAGS)
        // always keep these:
        .arg(&output_file)
        .arg(&dump_mir_dir)
        .output()
        .unwrap();
    // remove tempdir
    tempdir.close().unwrap();
    output
}

fn run_rustc(executable: &str, file: &PathBuf) -> Output {
    Command::new(executable)
        .env("RUSTFLAGS", "-Z force-unstable-if-unmarked")
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
        .args(&["-Zdump-mir-dir=/dev/null"])
        .output()
        .unwrap()
}

use pico_args::Arguments;
use rayon::prelude::*;
use std::ffi::OsStr;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output};
use walkdir::WalkDir;

struct Args {
    clippy: bool,
}

#[derive(Debug)]
struct ICE {
    path: PathBuf,
    executable: String,
    args: String,
}

fn main() {
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
        //  "rustc"
        // assume CWD is src/test from rustc repo root
        "build/x86_64-unknown-linux-gnu/stage1/bin/rustc"
    };

    println!("bin: {}\n\n", rustc_path);
    // collect error by running on files in parallel
    let mut errors: Vec<ICE> = files
        .par_iter()
        .filter_map(|file| find_crash(&file, rustc_path, args.clippy))
        .collect();

    errors.sort_by_key(|ice| ice.path.clone());

    println!("errors:\n");
    errors.iter().for_each(|f| println!("{:?}", f));
}

fn find_crash(file: &PathBuf, rustc_path: &str, clippy: bool) -> Option<ICE> {
    let output = file.display().to_string();
    let cmd_output = if clippy {
        run_rustc(rustc_path, file)
    } else {
        run_clippy(rustc_path, file)
    };

    let found_error: Option<String> = find_ICE(cmd_output);

    if found_error.is_some() {
        print!("\r");
        println!(
            "ICE: {output: <150} {msg}",
            output = output,
            msg = found_error.clone().unwrap()
        );
        print!("\r");
        let _stdout = std::io::stdout().flush();
    } else {
        // let stdout = std::io::stdout().flush();
        print!("\rChecking {output: <150}", output = output);
        let _stdout = std::io::stdout().flush();
    }

    if let Some(error_msg) = found_error {
        return Some(ICE {
            path: file.to_owned(),
            args: error_msg,
            executable: rustc_path.to_string(),
        });
    }
    None
}

#[allow(non_snake_case)]
fn find_ICE(output: Output) -> Option<String> {
    // let output = cmd.output().unwrap();
    let _exit_status = output.status;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let ice_keywords = vec![
        "LLVM ERROR",
        "`delay_span_bug`",
        "query stack during panic:",
        "internal compiler error:",
        "RUST_BACKTRACE=",
    ];

    for kw in ice_keywords {
        if stderr.contains(kw) || stdout.contains(kw) {
            return Some(kw.into());
        }
    }

    None
}

fn run_clippy(executable: &str, file: &PathBuf) -> Output {
    Command::new(executable)
        .arg(&file)
        .arg("-Zvalidate-mir")
        .arg("-Zverify-llvm-ir=yes")
        .arg("-Zincremental-verify-ich=yes")
        .args(&["-Zmir-opt-level=3"])
        //.args(&["-Zparse-only"])
        .args(&["-Zdump-mir=all"])
        .args(&["--emit", "mir"])
        .args(&["-Zsave-analysis"])
        // always keep these:
        .args(&["-o", "/dev/null"])
        .args(&["-Zdump-mir-dir=/dev/null"])
        .output()
        .unwrap()
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

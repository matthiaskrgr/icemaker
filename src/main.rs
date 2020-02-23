use rayon::prelude::*;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;

fn main() {
    const CLIPPY: bool = false;

    let mut files = WalkDir::new(".")
        .into_iter()
        .filter(|entry| entry.is_ok())
        .map(|e| e.unwrap())
        .filter(|f| f.path().extension() == Some(&OsStr::new("rs")))
        .map(|f| f.path().to_owned())
        .collect::<Vec<PathBuf>>();

    files.sort();

    let rustc_path = if CLIPPY {
        "clippy-driver"
    } else {
        //"rustc
        // assume CWD is src/test from rustc repo root
        "../../build/x86_64-unknown-linux-gnu/stage2/bin/rustc"
    };

    let mut errors: Vec<_> = files
        .par_iter()
        .filter(|file| find_crashes(&file, rustc_path, CLIPPY))
        .collect();

    errors.sort();

    println!("errors:\n");
    errors.iter().for_each(|f| println!("{:?}", f));
}

fn find_crashes(file: &PathBuf, rustc_path: &str, clippy: bool) -> bool {
    let mut error = false;
    let mut output = file.display().to_string();
    let cmd = if clippy {
        Command::new(rustc_path)
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
    } else {
        Command::new(rustc_path)
            .arg(&file)
            .args(&["-Zmir-opt-level=3"])
            //.args(&["-Zparse-only"])
            //.args(&["-Zdump-mir=all"])
            .args(&["--emit", "mir"])
            .args(&["-Zsave-analysis"])
            // always keep these:
            .args(&["-o", "/dev/null"])
            .args(&["-Zdump-mir-dir=/dev/null"])
            .output()
            .unwrap()
    };

    let cmd_output = cmd;
    let _status = cmd_output.status;
    let stderr = String::from_utf8_lossy(&cmd_output.stderr);
    let stdout = String::from_utf8_lossy(&cmd_output.stdout);

    if clippy {
        if stderr.contains("internal compiler error:")
            || stderr.contains("query stack during panic:")
            || stderr.contains("RUST_BACKTRACE")
        {
            output.push_str("           ERROR! stderr");
            error = true;
        } else if stdout.contains("internal compiler error:")
            || stdout.contains("query stack during panic:")
            || stderr.contains("RUST_BACKTRACE")
        {
            output.push_str("           ERROR! stderr");
            error = true;
        }
    } else {
        if stderr.contains("internal compiler error:")
            || stderr.contains("query stack during panic:")
            || stderr.contains("RUST_BACKTRACE")
        {
            output.push_str("           ERROR! stderr");
            error = true;
        } else if stdout.contains("internal compiler error:")
            || stdout.contains("query stack during panic:")
            || stderr.contains("RUST_BACKTRACE")
        {
            output.push_str("           ERROR! stderr");
            error = true;
        }
    }
    println!("{}", output);

    error
}

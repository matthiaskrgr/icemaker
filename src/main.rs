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
// convert glacier scripts into .rs files:
//
// for i in `rg EOF . | grep -o "^.*.sh"`; do ; CONTENT=` cat $i | pcregrep --no-messages -M  '.*<<.*EOF:*(\n|.)*EOF'  | grep -v ".*EOF.*"` ; echo $CONTENT >| `echo $i | sed -e s/\.sh/\.rs/` ; done
use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use itertools::Itertools;
use pico_args::Arguments;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tempdir::TempDir;
use walkdir::WalkDir;
// whether we run clippy, rustdoc or rustc (default: rustc)
struct Args {
    clippy: bool,
    rustdoc: bool,
    analyzer: bool, // rla
    rustfmt: bool,
    silent: bool,
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

#[derive(PartialEq, Eq)]
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

// -Zvalidate-mir -Zverify-llvm-ir=yes -Zincremental-verify-ich=yes -Zmir-opt-level=0 -Zmir-opt-level=1 -Zmir-opt-level=2 -Zmir-opt-level=3 -Zdump-mir=all --emit=mir -Zsave-analysis -Zprint-mono-items=full

const RUSTC_FLAGS: &[&[&str]] = &[
    &[
        "-Zvalidate-mir",
        "-Zverify-llvm-ir=yes",
        "-Zincremental-verify-ich=yes",
        "-Zmir-opt-level=0",
        "-Zmir-opt-level=1",
        "-Zmir-opt-level=2",
        "-Zmir-opt-level=3",
        "-Zmir-opt-level=4",
        // "-Zunsound-mir-opts",
        "-Zdump-mir=all",
        "--emit=mir",
        "-Zsave-analysis",
        "-Zprint-mono-items=full",
        "-Zpolymorphize=on",
    ],
    // all allow-by-default lints, split into two because otherwise the get_flag_combinations would eat all ram
    // I might fix this at some point by making it work lists of &str instead of String
    &[
        // must_not_suspend and non_exhaustive_omitted_patterns are unstable :(
        "-Wabsolute-paths-not-starting-with-crate",
        "-Wbox-pointers",
        "-Wdeprecated-in-future",
        "-Welided-lifetimes-in-paths",
        "-Wexplicit-outlives-requirements",
        "-Wkeyword-idents",
        "-Wmacro-use-extern-crate",
        "-Wmeta-variable-misuse",
        "-Wmissing-abi",
        "-Wmissing-copy-implementations",
        "-Wmissing-debug-implementations",
        "-Wmissing-docs",
        // "-Wmust-not-suspend",
        "-Wnon-ascii-idents",
        // "-Wnon-exhaustive-omitted-patterns",
        "-Wnoop-method-call",
        "-Wpointer-structural-match",
        "-Wrust-2021-incompatible-closure-captures",
    ],
    &[
        "-Wrust-2021-incompatible-or-patterns",
        "-Wrust-2021-prefixes-incompatible-syntax",
        "-Wrust-2021-prelude-collisions",
        "-Wsingle-use-lifetimes",
        "-Wtrivial-casts",
        "-Wtrivial-numeric-casts",
        "-Wunreachable-pub",
        "-Wunsafe-code",
        "-Wunsafe-op-in-unsafe-fn",
        "-Wunstable-features",
        "-Wunused-crate-dependencies",
        "-Wunused-extern-crates",
        "-Wunused-import-braces",
        "-Wunused-lifetimes",
        "-Wunused-qualifications",
        "-Wunused-results",
        "-Wvariant-size-differences",
    ],
    &["-Zinstrument-coverage"],
    &["-Cprofile-generate=/tmp/icemaker_pgo/"],
    &["-Zunpretty=expanded,hygiene"],
    &["-Zunpretty=everybody_loops"],
    &["-Zunpretty=hir,typed"],
    &["-Zunpretty=mir"],
    &["-Zunpretty=mir-cfg"],
    &["-Zunpretty=ast,expanded"],
    &["-Zunpretty=thir-tree"],
    &["-Zthir-unsafeck=yes"],
    &["INCR_COMP"],
    //&["-Copt-level=z"],
    //&["-Zsanitizer=address"],
    //&["-Zsanitizer=memory"],
    //&["-Zunstable-options", "--edition", "2021"],
];

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

fn get_flag_combination(flags: &[&str]) -> Vec<Vec<String>> {
    // get the power set : [a, b, c] => [a], [b], [c], [a,b], [a,c], [b,c], [a,b,c]

    let mut combs = Vec::new();
    for numb_comb in 0..=flags.len() {
        let combinations = flags.iter().map(|s| s.to_string()).combinations(numb_comb);
        combs.push(combinations);
    }

    let combs: Vec<Vec<String>> = combs.into_iter().flatten().collect();

    // add an empty "" flag so start with, in case an ice does not require any flags
    let mut tmp = vec![vec![String::new()]];
    tmp.extend(combs);
    //dbg!(&x);

    // we may have a lot of    Zmiroptlvl1 .. 2 .. 3 ,   1, 3 ..   1, 4 .. combinations, dedupe these to only keep the last one

    let tmp2 = tmp.iter_mut().map(|vec| {
        // reverse
        let vec_reversed: Vec<String> = {
            let mut v = vec.clone();
            v.reverse();
            v
        };

        // have we seen a mir-opt-level already?
        let mut seen: bool = false;

        // check the reversed vec for the first -Zmir and skip all other -Zmirs afterwards
        let vtemp: Vec<String> = vec_reversed
            .into_iter()
            .filter(|flag| {
                let cond = seen && flag.contains("-Zmir-opt-level");
                if flag.contains("-Zmir-opt-level") {
                    seen = true;
                }
                !cond
            })
            .collect();

        // now reverse again, so in the end we only kept the last -Zmir-opt-level
        let mut vfinal: Vec<String> = vtemp;
        vfinal.reverse();
        vfinal
    });

    let mut tmp2 = tmp2.collect::<Vec<Vec<String>>>();
    tmp2.sort();
    // remove duplicates that occurred due to removed mir opt levels
    tmp2.dedup();
    tmp2
}

fn main() {
    // read in existing errors
    // read the string INTO Vec<ICE>
    let errors_before: Vec<ICE> = if std::path::PathBuf::from("errors.json").exists() {
        serde_json::from_str(&std::fs::read_to_string("errors.json").unwrap())
            .expect("Failed to parse errors.json, is it a json file?")
    } else {
        Vec::new()
    };

    //let flags: Vec<Vec<String>> = get_flag_combinations();
    // println!("flags:\n");
    // flags.iter().for_each(|x| println!("{:?}", x));
    // parse args
    let mut args = Arguments::from_env();

    let args = Args {
        clippy: args.contains(["-c", "--clippy"]),
        rustdoc: args.contains(["-r", "--rustdoc"]),
        analyzer: args.contains(["-a", "--analyzer"]),
        rustfmt: args.contains(["-f", "--rustfmt"]),
        silent: args.contains(["-s", "--silent"]),
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
        .filter(|f| f.path().extension() == Some(OsStr::new("rs")))
        .map(|f| f.path().to_owned())
        .collect::<Vec<PathBuf>>();

    // check biggest files first
    files.sort_by_cached_key(|file| std::fs::metadata(file).unwrap().len());
    files.reverse();

    let exec_path = executable.path();

    // "rustc"
    // assume CWD is src/test from rustc repo root
    // "build/x86_64-unknown-linux-gnu/stage1/bin/rustc"

    println!("bin: {}", exec_path);
    if matches!(executable, Executable::Rustc) {
        println!(
            "checking: {} files x {} flags\n\n",
            files.len(),
            RUSTC_FLAGS.len() + 2 // incremental
        );
    } else {
        println!("checking: {} files\n", files.len(),);
    }

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
        // 3.5 hours when reporting errors :(
        "./library/stdarch/crates/core_arch/src/lib.rs",
        // memory 2.0
        "./src/test/run-make-fulldeps/issue-47551/eh_frame-terminator.rs",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();

    // how long did we take?
    let start_time = Instant::now();

    let counter = std::sync::atomic::AtomicUsize::new(0);

    let mut errors: Vec<ICE> = files
        .par_iter()
        .filter(|file| !EXCEPTION_LIST.contains(file))
        .map(|file| {
            if Executable::Rustc == executable {
                // for each file, run every chunk of RUSTC_FLAGS2 and check it and see if it crahes
                // process flags in parallel as well (can this be dangerous in relation to ram usage?)
                RUSTC_FLAGS
                    // do not par_iter() here in order to reduce peak memory load
                    // if one file needed launch several threads for it at the same time
                    // it also makes the program slower apparently
                    .par_iter()
                    .map(|flag_combination| {
                        find_crash(
                            file,
                            &exec_path,
                            &executable,
                            &flag_combination,
                            false,
                            &counter,
                            files.len() * (RUSTC_FLAGS.len() + 1/* incr */),
                            args.silent,
                        )
                    })
                    .collect::<Vec<Option<ICE>>>()
            } else {
                // if we run clippy/rustfmt/rls .. we dont need to check multiple combinations of RUSTFLAGS
                vec![find_crash(
                    file,
                    &exec_path,
                    &executable,
                    &[],
                    false,
                    &counter,
                    files.len(),
                    args.silent,
                )]
            }
        })
        .flatten()
        .filter(|opt_ice| opt_ice.is_some())
        .map(|ice| ice.unwrap())
        .collect();

    // dedupe equal ICEs, before sorting
    errors.dedup();

    let flagless_ices = errors
        .iter()
        .filter(|ice| ice.args.is_empty())
        .cloned()
        .collect::<Vec<ICE>>();

    flagless_ices.iter().for_each(|flglice| {
        // if we have an ICE where the msg and the file is equal to a flagless ice (but the ice is not the flagless ice), assume that the flags are unrelated
        // remove the ice from "errors"
        errors.retain(|ice| {
            !(ice.file == flglice.file && ice.ice_msg == flglice.ice_msg && !ice.args.is_empty())
        });
    });

    // sort by filename first and then by ice so that identical ICES are grouped up
    errors.sort_by_key(|ice| ice.file.clone());
    errors.dedup();
    errors.sort_by_key(|ice| ice.ice_msg.clone());
    // dedupe equal ICEs
    errors.dedup();

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
    if seconds_elapsed == 0 {
        println!("Checked {} files in <1 second", files_number);
        return;
    }
    let files_per_second = files_number / seconds_elapsed as usize;
    println!(
        "\nChecked {} files in {:.2} minutes, {} files/second",
        files_number,
        seconds_elapsed as f64 / 60_f64,
        files_per_second
    );
}

fn find_crash(
    file: &Path,
    exec_path: &str,
    executable: &Executable,
    compiler_flags: &[&str],
    incremental: bool,
    counter: &AtomicUsize,
    total_number_of_files: usize,
    silent: bool,
) -> Option<ICE> {
    let thread_start = Instant::now();

    let mut incremental = incremental;
    if compiler_flags == &["INCR_COMP"] {
        incremental = true
    }

    let index = counter.fetch_add(1, Ordering::SeqCst);
    let output = file.display().to_string();
    let cmd_output = match executable {
        Executable::Clippy => run_clippy(exec_path, file),
        Executable::Rustc => run_rustc(exec_path, file, incremental, compiler_flags),
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
            "ICE: {output: <150} {msg: <30} {feat}     {flags}",
            output = output,
            msg = found_error.clone().unwrap(),
            feat = if uses_feature { "        " } else { "no feat!" },
            flags = {
                let mut s = format!("{:?}", compiler_flags);
                s.truncate(100);
                s
            }
        );
        print!("\r");
        let _stdout = std::io::stdout().flush();
    } else if !silent {
        //@FIXME this only advances the checking once the files has already been checked!

        // let stdout = std::io::stdout().flush();

        let perc = ((index * 100) as f32 / total_number_of_files as f32) as u8;
        print!(
            "\r[{idx}/{total} {perc}%] Checking {output: <150}",
            output = output,
            idx = index,
            total = total_number_of_files,
            perc = perc
        );
        let _stdout = std::io::stdout().flush();
    }

    if incremental && found_error.is_some() {
        return Some(ICE {
            regresses_on: Regression::Nightly,

            needs_feature: uses_feature,
            file: file.to_owned(),
            args: vec![
                "-Z incremental-verify-ich=yes".into(),
                "-C incremental=<dir>".into(),
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

        let flag_combinations = get_flag_combination(compiler_flags);

        match executable {
            Executable::Rustc => {
                flag_combinations.iter().any(|flag_combination| {
                    let tempdir = TempDir::new("rustc_testrunner_tmpdir").unwrap();
                    let tempdir_path = tempdir.path();
                    let output_file = format!("-o{}/file1", tempdir_path.display());
                    let dump_mir_dir = format!("-Zdump-mir-dir={}", tempdir_path.display());

                    let output = Command::new(exec_path)
                        .arg(&file)
                        .args(flag_combination)
                        .arg(output_file)
                        .arg(dump_mir_dir)
                        .output()
                        .unwrap();
                    let found_error2 = find_ICE(output);
                    // remove the tempdir
                    tempdir.close().unwrap();
                    if found_error2.is_some() {
                        // save the flags that the ICE repros with
                        bad_flags = flag_combination;
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
        let regressing_channel = find_out_crashing_channel(bad_flags, file);

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
        print!("\r");
        println!(
            "{} running for more ({} minutes) than {} minute",
            file.display(),
            seconds_elapsed / 60,
            MINUTE_LIMIT
        );
    }

    ret
}

fn find_out_crashing_channel(bad_flags: &[String], file: &Path) -> Regression {
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
    match std::fs::read_to_string(&file) {
        Ok(file) => file.contains("feature("),
        _ => {
            eprintln!("Failed to read '{}'", file.display());
            false
        }
    }
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

fn run_rustc(executable: &str, file: &Path, incremental: bool, rustc_flags: &[&str]) -> Output {
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
    // run the command
    output
        .output()
        .unwrap_or_else(|_| panic!("Error: {:?}, executable: {:?}", output, executable))
    // remove tempdir
    //tempdir.close().unwrap();
}

fn run_rustc_incremental(executable: &str, file: &Path) -> Output {
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

fn run_clippy(executable: &str, file: &Path) -> Output {
    let has_main = std::fs::read_to_string(&file)
        .unwrap_or_default()
        .contains("fn main(");
    let mut output = Command::new(executable);

    if !has_main {
        output.args(&["--crate-type", "lib"]);
    }
    output
        .env("RUSTFLAGS", "-Z force-unstable-if-unmarked")
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
    output.output().unwrap()
}

fn run_rustdoc(executable: &str, file: &Path) -> Output {
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

fn run_rust_analyzer(executable: &str, file: &Path) -> Output {
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
fn run_rustfmt(executable: &str, file: &Path) -> Output {
    Command::new(executable)
        .env("SYSROOT", "/home/matthias/.rustup/toolchains/master")
        .arg(&file)
        .arg("--check")
        .args(&["--edition", "2018"])
        .output()
        .unwrap()
}

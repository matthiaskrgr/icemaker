mod flags;
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

// check out every single file that exists in a repo:
//
// for object_hash in `git cat-file --batch-all-objects --batch-check | grep blob | cut -d' ' -f1` ; do
// git cat-file $object_hash -p > ${object_hash}.rs
// done

// get the first 275000 smallest files
// git cat-file --batch-all-objects --batch-check  | grep blob | cut -d' ' -f1,3 |  awk '{for(i=NF;i>=1;i--) printf "%s ", $i;print ""}' | sort -n | head -n 275000| cut -d' ' -f2  | parallel -I% "git cat-file % -p > %.rs"

// all the interesting miri findings:
//
//  for file in `cat errors.json | grep file.: | cut -d' ' -f6 | sed s/\"//g | sed s/,//` ; do; echo -n "$file " ; grep "unsafe\|simd\|no_core\|transmute\|Box::\|rustc_variance" -c $file ; done  | grep 0$
//
mod fuzz;
mod fuzz2;
mod ice;
mod lib;
mod run_commands;

use crate::flags::*;
use crate::fuzz::*;
use crate::ice::*;
use crate::lib::*;
use crate::run_commands::*;

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::io::BufRead;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use clap::Parser;
use lazy_static::lazy_static;
use rayon::prelude::*;
use regex::Regex;
use std::sync::Mutex;
use tempdir::TempDir;
use walkdir::WalkDir;

lazy_static! {
    static ref ALL_ICES_WITH_FLAGS: Mutex<Vec<Vec<OsString>>> =
        Mutex::new(vec![vec![OsString::new()]]);
}

#[allow(clippy::if_same_then_else)]
fn executable_from_args(args: &Args) -> Executable {
    if args.clippy {
        Executable::Clippy
    } else if args.clippy_fix {
        Executable::ClippyFix
    } else if args.rustdoc {
        Executable::Rustdoc
    } else if args.analyzer {
        Executable::RustAnalyzer
    } else if args.rustfmt {
        Executable::Rustfmt
    } else if args.miri {
        Executable::Miri
    } else if args.rustc {
        Executable::Rustc
    } else if args.cranelift {
        Executable::RustcCGClif
    } else {
        Executable::Rustc
    }
}

fn main() {
    // how long did we take?
    let start_time = Instant::now();

    // read in existing errors
    // read the string INTO Vec<ICE>
    let errors_before: Vec<ICE> = if std::path::PathBuf::from("errors.json").exists() {
        serde_json::from_str(&std::fs::read_to_string("errors.json").unwrap())
            .expect("Failed to parse errors.json, is it a json file?")
    } else {
        Vec::new()
    };

    let args = Args::parse();

    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build_global()
        .unwrap();

    let executable = executable_from_args(&args);
    let executables = if !matches!(executable, Executable::Rustc) ||  /* may have passed --rustc to disable clippy rustdoc etc */ args.rustc
    {
        // assume that we passed something, do not take the default Rustc
        vec![&executable]
    } else {
        // default Executables
        // dont run cranelift by default, maybe wait until we have an official rustup component
        if cfg!(feature = "ci") {
            // on ci, don't run miri
            vec![
                &Executable::Rustc,
                &Executable::Rustdoc,
                &Executable::Clippy,
                &Executable::Rustfmt,
            ]
        } else {
            vec![
                &Executable::Rustc,
                &Executable::Rustdoc,
                &Executable::Rustfmt,
                &Executable::Clippy,
                &Executable::Miri,
            ]
        }
    };

    if executables.contains(&&Executable::Miri) || matches!(executable, Executable::Miri) {
        println!("Running cargo miri setup");
        let _ = std::process::Command::new("cargo")
            .arg("miri")
            .arg("setup")
            .status()
            .unwrap()
            .success();
    }

    if args.heat {
        let chain_order = args.chain_order;
        let _ = run_space_heater(executable, chain_order);
        return;
    }

    if args.fuzz {
        let _ = run_random_fuzz(executable);
        return;
    } else if args.fuzz2 {
        crate::fuzz2::fuzz2::fuzz2main();
        return;
    }

    if args.codegen {
        codegen_git();
        return;
    }

    // search for rust files inside CWD
    let mut files = WalkDir::new(".")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|f| f.path().extension() == Some(OsStr::new("rs")))
        .map(|f| f.path().to_owned())
        .collect::<Vec<PathBuf>>();

    // check biggest files first
    files.sort_by_cached_key(|file| std::fs::metadata(file).unwrap().len());
    files.reverse();

    //let exec_path = executable.path();
    executables.iter().for_each(|executable| {
        println!("Using executable(s): {}", executable.path());
    });
    if matches!(executable, Executable::Rustc) {
        println!(
            "checking: {} files x {} flags, {} executables\n\n",
            files.len(),
            RUSTC_FLAGS.len() + 2, // incremental
            executables.len()
        );
    } else {
        println!("checking: {} files\n", files.len(),);
    }

    // files that take too long (several minutes) to check or cause other problems
    #[allow(non_snake_case)]
    let EXCEPTION_LIST: Vec<PathBuf> = EXCEPTIONS.iter().map(PathBuf::from).collect();
    #[allow(non_snake_case)]
    let MIRI_EXCEPTION_LIST: Vec<PathBuf> = MIRI_EXCEPTIONS.iter().map(PathBuf::from).collect();

    // count progress
    let counter = std::sync::atomic::AtomicUsize::new(0);

    ctrlc::set_handler(move || {
        println!("Ctrl+C: TERMINATED");

        ALL_ICES_WITH_FLAGS
            .lock()
            .unwrap()
            .iter()
            .for_each(|flags| {
                let flags = flags
                    .iter()
                    .map(|x| x.to_str().unwrap().to_string())
                    .collect::<Vec<String>>();
                if !flags.is_empty() {
                    println!("{}", flags.join(" "))
                }
            });

        std::process::exit(42);
    })
    .expect("Error setting Ctrl-C handler");

    let rustc_exec_path = Executable::Rustc.path();

    if args.incremental_test {
        eprintln!("checking which files compile...");
        let files = files
            .par_iter()
            .filter(|file| !EXCEPTION_LIST.contains(file))
            .filter(|file| file_compiles(file, &rustc_exec_path))
            .cloned()
            .collect::<Vec<_>>();
        eprintln!("checking {} files...", files.len());

        let incr_crashes = files
            .par_iter()
            .filter(|file| !EXCEPTION_LIST.contains(file))
            .panic_fuse()
            .filter(|file| !EXCEPTION_LIST.contains(file))
            .filter_map(|file_a| {
                let (output, _cmd_str, _actual_args, file_a, file_b) =
                    incremental_stress_test(file_a, &files, &rustc_exec_path)?;
                let is_ice = find_ICE_string(&Executable::Rustc, output.clone());
                if is_ice.is_some() {
                    eprintln!("\nINCRCOMP ICE: {},{}", file_a.display(), file_b.display());
                    Some((file_a, file_b, output))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        dbg!(incr_crashes);
        return;
    }

    let mut errors: Vec<ICE> = files
        .par_iter()
        .flat_map(|file| {
            // for each file, increment counter by one
            let _ = counter.fetch_add(1, Ordering::SeqCst);
            executables
                .par_iter()
                .panic_fuse()
                // don't check anything that is contained in the exception list
                .filter(|executable| {
                    !EXCEPTION_LIST.contains(file)
                        || (matches!(executable, Executable::Miri)
                            && !MIRI_EXCEPTION_LIST.contains(file))
                })
                .map(|executable| {
                    let exec_path = executable.path();

                    match executable {
                        Executable::Rustc | Executable::RustcCGClif => {
                            // for each file, run every chunk of RUSTC_FLAGS and check it and see if it crashes
                            RUSTC_FLAGS
                                // note: this can be dangerous in case of max memory usage, if a file needs a lot
                                .par_iter()
                                .panic_fuse()
                                .map(|flag_combination| {
                                    ICE::discover(
                                        file,
                                        &exec_path,
                                        executable,
                                        flag_combination,
                                        &[],
                                        false,
                                        &counter,
                                        files.len(),
                                        args.silent,
                                    )
                                })
                                .collect::<Vec<Option<ICE>>>()
                        }
                        Executable::Miri => MIRIFLAGS
                            .par_iter()
                            .map(|miri_flag_combination| {
                                MIRIRUSTFLAGS
                                    .par_iter()
                                    .panic_fuse()
                                    .map(|miri_rustflag| {
                                        ICE::discover(
                                            file,
                                            &exec_path,
                                            executable,
                                            miri_rustflag,
                                            miri_flag_combination,
                                            false,
                                            &counter,
                                            files.len(),
                                            args.silent,
                                        )
                                    })
                                    .find_any(|ice| ice.is_some())
                            })
                            .flatten()
                            .collect::<Vec<Option<ICE>>>(),
                        _ => {
                            // if we run clippy/rustfmt/rla .. we dont need to check multiple combinations of RUSTFLAGS
                            vec![ICE::discover(
                                file,
                                &exec_path,
                                executable,
                                // run with no flags
                                &[],
                                &[],
                                false,
                                &counter,
                                files.len(),
                                args.silent,
                            )]
                        }
                    }
                })
                .flatten()
                .filter(|opt_ice| opt_ice.is_some())
                .map(|ice| ice.unwrap())
                .collect::<Vec<ICE>>()
        })
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

    // if we have the same file, same error_msg and same error_reason, this also gotta be an identical ICE
    errors.sort_by_key(|ice| {
        format!(
            "file: {} error_reason: {} ice_msg: {}",
            ice.file.display(),
            ice.error_reason,
            ice.ice_msg
        )
    });

    errors.dedup_by_key(|ice| {
        format!(
            "file: {} error_reason: {} ice_msg: {}",
            ice.file.display(),
            ice.error_reason,
            ice.ice_msg
        )
    });
    // original sorting again
    errors.sort_by_key(|ice| ice.file.clone());
    errors.sort_by_key(|ice| ice.ice_msg.clone());

    // sort by command
    // errors.sort_by_key(|ice| ice.cmd.clone());

    // if we are done, print all errors
    println!("errors:\n");

    /*
    errors.iter().for_each(|f| {
        let mut debug = format!("{:?}", f);
        debug.truncate(300);
        println!("{}", debug);
    });
    */

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

    eprintln!("\n\nALL CRASHES\n\n");
    ALL_ICES_WITH_FLAGS
        .lock()
        .unwrap()
        .iter()
        .for_each(|flags| {
            let flags = flags
                .iter()
                .map(|x| x.to_str().unwrap().to_string())
                .collect::<Vec<String>>();
            // miri for example has no flags, don't spam a bunch of empty lines into stdout
            if !flags.is_empty() {
                println!("{}", flags.join(" "))
            }
        });
}

impl ICE {
    /// find out if a file crashes rustc with the given flags
    #[allow(clippy::too_many_arguments)]
    fn discover(
        file: &Path,
        exec_path: &str,
        executable: &Executable,
        compiler_flags: &[&str],
        miri_flags: &[&str],
        incremental: bool,
        counter: &AtomicUsize,
        total_number_of_files: usize,
        silent: bool,
    ) -> Option<Self> {
        let thread_start = Instant::now();

        let incremental = if compiler_flags == ["INCR_COMP"] {
            true
        } else {
            incremental
        };

        // run the command with some flags (actual_args)

        let index = counter.load(Ordering::SeqCst); // the current file number
        let file_name = file.display().to_string();
        let (cmd_output, _cmd, actual_args) = match executable {
            Executable::Clippy => run_clippy(exec_path, file),
            Executable::ClippyFix => run_clippy_fix(exec_path, file),
            Executable::Rustc => run_rustc(exec_path, file, incremental, compiler_flags),
            Executable::Rustdoc => run_rustdoc(exec_path, file),
            Executable::RustAnalyzer => run_rust_analyzer(exec_path, file),
            Executable::Rustfmt => run_rustfmt(exec_path, file),
            Executable::Miri => run_miri(exec_path, file, miri_flags),
            Executable::RustcCGClif => run_cranelift(exec_path, file, incremental, compiler_flags),
        };

        /*if cmd_output.stdout.len() > 10_000_000 || cmd_output.stderr.len() > 10_000_000 {
            eprintln!(
                "\nVERY LONG: stdout {} stderr {} {}\n",
                cmd_output.stdout.len(),
                cmd_output.stderr.len(),
                file.display()
            );
        }*/

        //dbg!(&cmd_output);
        //dbg!(&_cmd);

        // find out the ice message
        let mut ice_msg = String::from_utf8_lossy(&cmd_output.stderr)
            .lines()
            .find(|line| {
                line.contains("panicked at") || line.contains("error: internal compiler error: ")
            })
            .unwrap_or_default()
            .to_string();

        ice_msg = ice_msg.replace("error: internal compiler error:", "ICE");

        // rustc sets 101 if it crashed
        let exit_status = cmd_output.status.code().unwrap_or(0);

        let found_error: Option<String> = find_ICE_string(executable, cmd_output);

        // check if the file enables any compiler features
        let uses_feature: bool = uses_feature(file);

        let exit_code_looks_like_crash =
            exit_status == 101 ||  /* segmentation fault etc */ (132..=139).contains(&exit_status);

        // @TODO merge the two  found_error.is_some() branches and print ice reason while checking
        #[allow(clippy::format_in_format_args)]
        if exit_code_looks_like_crash && found_error.is_some()
    // in miri, "cargo miri run" will return 101 if the run program (not miri!) just panics so ignore that
        || (matches!(executable, Executable::Miri) && found_error.is_some())
        {
            print!("\r");
            println!(
                "ICE: {executable:?} {file_name:<20.80} {msg:<30.200} {feat}     {flags:<.30}",
                msg = {
                    let s = found_error
                        .clone()
                        // we might have None error found but still a suspicious exit status, account, dont panic on None == found_error then
                        .unwrap_or(format!("No error found but exit code: {}", exit_status));
                    s.replace("error: internal compiler error:", "ICE:")
                },
                feat = if uses_feature { "        " } else { "no feat!" },
                flags = format!("{:?}", compiler_flags)
            );
            print!("\r");
            let _stdout = std::io::stdout().flush();
        } else if !silent {
            //@FIXME this only advances the checking once the files has already been checked!

            // let stdout = std::io::stdout().flush();

            let perc = ((index * 100) as f32 / total_number_of_files as f32) as u8;
            print!(
                "\r[{idx}/{total} {perc}%] Checking {output: <150}",
                output = file_name,
                idx = index,
                total = total_number_of_files,
                perc = perc
            );
            let _stdout = std::io::stdout().flush();
        }

        if exit_code_looks_like_crash || found_error.is_some() {
            crate::ALL_ICES_WITH_FLAGS
                .lock()
                .unwrap()
                .push(actual_args.clone());
        }

        // incremental ices don't need to have their flags reduced
        if incremental && found_error.is_some() {
            return Some(ICE {
                regresses_on: Regression::Nightly,

                needs_feature: uses_feature,
                file: file.to_owned(),
                args: vec![
                    "-Z incremental-verify-ich=yes".into(),
                    "-C incremental=<dir>".into(),
                    "-Cdebuginfo=2".into(),
                ],
                // executable: rustc_path.to_string(),
                error_reason: found_error.clone().unwrap_or_default(),
                ice_msg,
                executable: executable.clone(),
                //cmd,
            });
        }

        let mut ret = None;
        if let Some(error_reason) = found_error {
            eprintln!("ICE\n\n\nICE\n\n");
            if !matches!(executable, Executable::Miri) {
                // PRECHECK
                // optimization: check if rustc crashes on the file without needing any flags, if yes, return that as an ICE
                // we might produce several of those if we check different flags but they should all be deduplicated laster on?
                let mut pure_rustc_cmd = Command::new(Executable::Rustc.path());
                pure_rustc_cmd.arg(file);

                let pure_rustc_output = systemdrun_command(&mut pure_rustc_cmd).unwrap();
                let found_error0 = find_ICE_string(&Executable::Rustc, pure_rustc_output);
                if found_error0.is_some() {
                    let ice = ICE {
                        regresses_on: Regression::Master,
                        needs_feature: uses_feature,
                        file: file.to_owned(),
                        args: Vec::new(),
                        error_reason: found_error0.unwrap_or_default(),
                        ice_msg,
                        executable: Executable::Rustc,
                    };
                    return Some(ice);
                }
            }

            // rustc or clippy crashed, we have an ice
            // find out which flags are actually responsible of the many we passed
            // run rustc with the file on several flag combinations, if the first one ICEs, abort
            let mut bad_flags: Vec<&&str> = Vec::new();

            let args2 = actual_args
                .iter()
                .map(|x| x.to_str().unwrap().to_string())
                .collect::<Vec<String>>();
            let args3 = &args2.iter().map(String::as_str).collect::<Vec<&str>>()[..];

            let flag_combinations = get_flag_combination(args3);
            //dbg!(&flag_combinations);

            // the last one should be the full combination of flags
            let last = flag_combinations[&flag_combinations.len() - 1].clone();

            match executable {
                Executable::Rustc | Executable::RustcCGClif => {
                    // if the full set of flags (last) does not reproduce the ICE, bail out immediately (or assert?)
                    let tempdir = TempDir::new("rustc_testrunner_tmpdir").unwrap();

                    // WE ALREADY HAVE filename etc in the args, rustc erros if we pass 2 files etc

                    // let tempdir_path = tempdir.path();
                    // let output_file = format!("-o{}/file1", tempdir_path.display());
                    //let dump_mir_dir = format!("-Zdump-mir-dir={}", tempdir_path.display());
                    let mut cmd = Command::new(exec_path);
                    cmd.args(&last);
                    let output = systemdrun_command(&mut cmd).unwrap();

                    // dbg!(&output);
                    let found_error2 = find_ICE_string(executable, output);
                    // remove the tempdir
                    tempdir.close().unwrap();
                    // the full set of flags did reproduce the ice
                    if found_error2.is_some() {
                        // walk through the flag combinations and save the first (and smallest) one that reproduces the ice
                        flag_combinations.iter().any(|flag_combination| {
                            let tempdir = TempDir::new("rustc_testrunner_tmpdir").unwrap();
                            let tempdir_path = tempdir.path();
                            let output_file = format!("-o{}/file1", tempdir_path.display());
                            let dump_mir_dir = format!("-Zdump-mir-dir={}", tempdir_path.display());

                            let mut cmd = Command::new(exec_path);
                            cmd.arg(&file)
                                .args(flag_combination.iter())
                                .arg(output_file)
                                .arg(dump_mir_dir);
                            let output = systemdrun_command(&mut cmd).unwrap();

                            let found_error3 = find_ICE_string(executable, output);
                            // remove the tempdir
                            tempdir.close().unwrap();
                            if found_error3.is_some() {
                                // save the flags that the ICE repros with
                                bad_flags = flag_combination.clone();
                                true
                            } else {
                                false
                            }
                        });

                        // find out if this is a beta/stable/nightly regression
                    } else {
                        // full set of flags did NOT reproduce the ice...????
                        /*   eprintln!("full set of flags:");
                        dbg!(&last);
                        eprintln!("originl (actual) args:");
                        dbg!(&actual_args);
                        dbg!(file); */
                        debug_assert!(
                            false,
                            "full set of flags on '{}' did not reproduce the ICE??",
                            file.display()
                        );
                    }
                }
                Executable::Clippy
                | Executable::ClippyFix
                | Executable::Rustdoc
                | Executable::RustAnalyzer
                | Executable::Rustfmt
                | Executable::Miri => {}
            }
            let regressing_channel = find_out_crashing_channel(&bad_flags, file);
            // add these for a more accurate representation of what we ran originally
            bad_flags.push(&"-ooutputfile");
            bad_flags.push(&"-Zdump-mir-dir=dir");

            let ret2 = ICE {
                regresses_on: match executable {
                    Executable::Clippy => Regression::Master,
                    _ => regressing_channel,
                },

                needs_feature: uses_feature,
                file: file.to_owned(),
                args: bad_flags
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>(),
                // executable: rustc_path.to_string(),
                error_reason,
                ice_msg,
                executable: executable.clone(),
                //cmd,
            };

            ret = Some(ret2);
        };

        /*
        match executable {
            Executable::Miri => {
                std::fs::remove_file(&file).expect("failed to remove file after running miri");
            }
            _ => {}
        } */

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
}

/// find out if we crash on master, nightly, beta or stable
fn find_out_crashing_channel(bad_flags: &Vec<&&str>, file: &Path) -> Regression {
    // simply check if we crash on nightly, beta, stable or master
    let toolchain_home: PathBuf = {
        let mut p = home::rustup_home().unwrap();
        p.push("toolchains");
        p
    };

    let bad_but_no_nightly_flags = bad_flags
        .iter()
        .filter(|flag| !flag.starts_with("-Z"))
        .collect::<Vec<_>>();

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

    let stable_ice: bool = find_ICE_string(
        &Executable::Rustc,
        systemdrun_command(
            Command::new(stable_path)
                .arg(&file)
                .args(&bad_but_no_nightly_flags)
                .arg(&output_file), //.arg(&dump_mir_dir)
        )
        .unwrap(),
    )
    .is_some();

    let beta_ice: bool = find_ICE_string(
        &Executable::Rustc,
        systemdrun_command(
            Command::new(beta_path)
                .arg(&file)
                .args(&bad_but_no_nightly_flags)
                .arg(&output_file), //.arg(&dump_mir_dir)
        )
        .unwrap(),
    )
    .is_some();

    let nightly_ice: bool = find_ICE_string(
        &Executable::Rustc,
        systemdrun_command(
            Command::new(nightly_path)
                .arg(&file)
                .args(bad_flags)
                .arg(&output_file)
                .arg(&dump_mir_dir),
        )
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

/// check if the given output looks like rustc crashed
#[allow(non_snake_case)]
fn find_ICE_string(executable: &Executable, output: Output) -> Option<String> {
    let ice_keywords = match executable {
        Executable::Miri => vec![
            "error: Undefined Behavior",
            // "the evaluated program leaked memory", // memleaks are save apparently
            "internal compiler error:",
            "this indicates a bug in the program",
        ],
        Executable::ClippyFix => vec![
            "internal compiler error:",
            "indicates a bug in either rustc or cargo itself",
        ],

        _ => vec![
            "^LLVM ERROR",
            "^thread '.*' panicked at:",
            "^query stack during panic:$",
            "^error: internal compiler error: no errors encountered even though `delay_span_bug` issued$",
            "^error: internal compiler error: ",
            "RUST_BACKTRACE=",
            "error: Undefined Behavior",
            //"MIRIFLAGS",
            "segmentation fault",
            "(core dumped)",
            "^fatal runtime error: stack overflow",
            "^Unusual: ",
            "^Undefined behavior:",
        ],
    };

    let ice_keywords = ice_keywords
        .into_iter()
        .map(|kw| Regex::new(kw).unwrap_or_else(|_| panic!("failed to construct regex: {kw}")))
        .collect::<Vec<_>>();

    // let output = cmd.output().unwrap();
    let _exit_status = output.status;

    //stdout
    let line = std::io::Cursor::new(&output.stdout)
        .lines()
        .filter_map(|line| line.ok())
        .find(|line| {
            ice_keywords.iter().any(|regex| {
                if &regex.to_string() == "panicked at:" {
                    // do not warn when the checked .rs file contains something like const A = panic!()
                    regex.is_match(line) && !line.contains("the evaluated program panicked at")
                } else {
                    regex.is_match(line)
                }
            })
        });

    if line.is_some() {
        return line;
    }

    // stderr
    let line = std::io::Cursor::new(&output.stderr)
        .lines()
        .filter_map(|line| line.ok())
        .find(|line| ice_keywords.iter().any(|regex| regex.is_match(line)));
    drop(output);

    if line.is_some() {
        return line;
    }

    None
}

pub(crate) fn run_random_fuzz(executable: Executable) -> Vec<ICE> {
    const LIMIT: usize = 4000;
    let exec_path = executable.path();
    let counter = std::sync::atomic::AtomicUsize::new(0);

    #[allow(non_snake_case)]
    let mut ICEs = (0..LIMIT)
        .into_par_iter()
        .panic_fuse()
        .filter_map(|num| {
            // gen the snippet
            let rust_code = get_random_string();

            let filename = format!("icemaker_{}.rs", num);
            let path = PathBuf::from(&filename);
            let mut file = std::fs::File::create(filename).expect("failed to create file");
            file.write_all(rust_code.as_bytes())
                .expect("failed to write to file");

            // only iterate over flags when using rustc
            let ice = match executable {
                Executable::Rustc => RUSTC_FLAGS.iter().find_map(|compiler_flags| {
                    ICE::discover(
                        &path,
                        &exec_path,
                        &executable,
                        compiler_flags,
                        &[],
                        false,
                        &counter,
                        LIMIT * RUSTC_FLAGS.len(),
                        false,
                    )
                }),
                _ => ICE::discover(
                    &path,
                    &exec_path,
                    &executable,
                    &[""],
                    &[],
                    false,
                    &counter,
                    LIMIT * RUSTC_FLAGS.len(),
                    false,
                ),
            };

            // if there is no ice, remove the file
            if ice.is_none() {
                std::fs::remove_file(path).unwrap();
            } else {
                eprintln!(
                    "\nice: {}, {}",
                    path.display(),
                    ice.as_ref()
                        .unwrap()
                        .args
                        .iter()
                        .map(|s| format!("{} ", s))
                        .collect::<String>(),
                );
            }
            ice
        })
        .collect::<Vec<_>>();

    // dedupe
    ICEs.sort_by_key(|ice| ice.file.clone());
    ICEs.dedup();
    ICEs.sort_by_key(|ice| ice.ice_msg.clone());
    // dedupe equal ICEs
    ICEs.dedup();

    dbg!(&ICEs);
    ICEs
}
pub(crate) fn run_space_heater(executable: Executable, chain_order: usize) -> Vec<ICE> {
    println!("Using executable: {}", executable.path());

    let chain_order: usize = std::num::NonZeroUsize::new(chain_order)
        .expect("no 0 please")
        .get();
    const LIMIT: usize = 100000;
    let counter = std::sync::atomic::AtomicUsize::new(0);
    let exec_path = executable.path();
    #[allow(non_snake_case)]
    let EXCEPTION_LIST: Vec<PathBuf> = EXCEPTIONS.iter().map(PathBuf::from).collect();

    //let mut file_hashset = HashSet::new();

    println!("Reading files...");
    // gather all rust files
    let files = WalkDir::new(".")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|f| f.path().extension() == Some(OsStr::new("rs")))
        .map(|f| f.path().to_owned())
        .collect::<Vec<PathBuf>>();

    println!("Hashing existing files");
    let hashset = files
        .iter()
        .filter(|file| !EXCEPTION_LIST.contains(file))
        .map(|path| std::fs::read_to_string(path).unwrap_or_default())
        .collect::<HashSet<String>>();

    let mut chain = markov::Chain::of_order(chain_order);
    println!("Feeding input to chain");
    // add the file content to the makov model
    files
        .iter()
        .filter(|file| !EXCEPTION_LIST.contains(file))
        .map(|path| std::fs::read_to_string(path).unwrap_or_default())
        .for_each(|file| {
            chain.feed_str(&file);
        });
    println!("Generating code");

    // iterate over markov-model-generated files
    #[allow(non_snake_case)]
    let mut ICEs = (0..LIMIT)
        .into_par_iter()
        .panic_fuse()
        .filter_map(|num| {
            // gen the snippet
            let rust_code = chain.generate_str();

            // if the file is the same a some of our input files, skip it
            if hashset.contains(&rust_code) {
                return None;
            }

            // if we already have an ICE with our generated code, we don't need to check it
            // TODO: check this via hashset
            // this will be had because we have to insert into the same hashset from multiple threads at the same time :/
            let mut already_found_ices = std::fs::read_dir(PathBuf::from("."))
                .expect("failed to read dir")
                .into_iter()
                .map(|f| f.unwrap().path())
                .filter(|path| {
                    let filename = path.file_name();
                    if let Some(name) = filename {
                        let s = name.to_str().expect("failed to_str");
                        s.starts_with("icemaker")
                    } else {
                        false
                    }
                })
                .map(std::fs::read_to_string)
                .map(|s| s.unwrap_or_default());

            if already_found_ices.any(|icemaker_ice_code| icemaker_ice_code == rust_code) {
                //   eprintln!("SKIPPING!!");
                return None;
            }

            let filename = format!("icemaker_{}.rs", num);
            let path = PathBuf::from(&filename);
            let mut file = std::fs::File::create(filename).expect("failed to create file");
            file.write_all(rust_code.as_bytes())
                .expect("failed to write to file");

            // only iterate over flags when using rustc
            let ice = match executable {
                Executable::Rustc => RUSTC_FLAGS.iter().find_map(|compiler_flags| {
                    ICE::discover(
                        &path,
                        &exec_path,
                        &executable,
                        compiler_flags,
                        &[],
                        false,
                        &counter,
                        LIMIT * RUSTC_FLAGS.len(),
                        false,
                    )
                }),
                _ => ICE::discover(
                    &path,
                    &exec_path,
                    &executable,
                    &[""],
                    &[],
                    false,
                    &counter,
                    LIMIT * RUSTC_FLAGS.len(),
                    false,
                ),
            };

            // if there is no ice, remove the file
            if ice.is_none() {
                std::fs::remove_file(path).unwrap();
            } else {
                eprintln!(
                    "\nice: {}, {}",
                    path.display(),
                    ice.as_ref()
                        .unwrap()
                        .args
                        .iter()
                        .map(|s| format!("{} ", s))
                        .collect::<String>()
                );
            }
            ice
        })
        .collect::<Vec<_>>();

    // dedupe
    ICEs.sort_by_key(|ice| ice.file.clone());
    ICEs.dedup();
    ICEs.sort_by_key(|ice| ice.ice_msg.clone());
    // dedupe equal ICEs
    ICEs.dedup();

    dbg!(&ICEs);
    ICEs
}

fn codegen_git() {
    println!("querying blobs");
    let stdout = std::process::Command::new("git")
        .arg("rev-list")
        .arg("--objects")
        .arg("--all")
        .output()
        .expect("git rev-list failed")
        .stdout;

    println!("converting to text");

    let s = String::from_utf8(stdout).unwrap();
    /*
        3a9e68329aa60201fe4eedeed3e1b80cc68926dc regex_macros/src
    eb6c6f8f12a6d6db38bcfa741036d9622fad6c89 regex_macros/src/lib.rs
    a7e1f44f5eae607f1fa51951eff463e62d03bd13
    a6945d655576f7497515d6870f476f45ddd07a33 regex_macros
    fd0fd35ca74b281eb4753bc44d2f36583fefbca0 regex_macros/Cargo.toml
        */
    println!("writing to files");

    let objects = s
        .lines()
        .filter(|line| line.ends_with(".rs"))
        .map(|line| line.split_whitespace().next().unwrap())
        .collect::<Vec<_>>();

    /*
    eb6c6f8f12a6d6db38bcfa741036d9622fad6c89
    fd0fd35ca74b281eb4753bc44d2f36583fefbca0
    */

    objects.par_iter().for_each(|obj| {
        let first = obj.chars().next().unwrap();
        let second = obj.chars().nth(1).unwrap();
        let stdout = std::process::Command::new("git")
            .arg("cat-file")
            .arg("-p")
            .arg(obj)
            .output()
            .expect("git cat-file -p <obj> failed")
            .stdout;
        let text = String::from_utf8(stdout).unwrap();
        std::fs::create_dir_all(format!("{}/{}", first, second))
            .expect("failed to create directories");
        std::fs::write(format!("{}/{}/{}.rs", first, second, obj), text)
            .expect("failed to write file");
    })
}

fn _codegen_git_and_check() {
    println!("querying blobs");
    let stdout = std::process::Command::new("git")
        .arg("rev-list")
        .arg("--objects")
        .arg("--all")
        .output()
        .expect("git rev-list failed")
        .stdout;

    println!("converting to text");

    let s = String::from_utf8(stdout).unwrap();
    /*
        3a9e68329aa60201fe4eedeed3e1b80cc68926dc regex_macros/src
    eb6c6f8f12a6d6db38bcfa741036d9622fad6c89 regex_macros/src/lib.rs
    a7e1f44f5eae607f1fa51951eff463e62d03bd13
    a6945d655576f7497515d6870f476f45ddd07a33 regex_macros
    fd0fd35ca74b281eb4753bc44d2f36583fefbca0 regex_macros/Cargo.toml
        */
    println!("writing to files");

    let objects = s
        .lines()
        .filter(|line| line.ends_with(".rs"))
        .map(|line| line.split_whitespace().next().unwrap())
        .collect::<Vec<_>>();

    /*
    eb6c6f8f12a6d6db38bcfa741036d9622fad6c89
    fd0fd35ca74b281eb4753bc44d2f36583fefbca0
    */

    // return this as an iterator?
    objects.par_iter().for_each(|obj| {
        let first = obj.chars().next().unwrap();
        let second = obj.chars().nth(1).unwrap();
        let stdout = std::process::Command::new("git")
            .arg("cat-file")
            .arg("-p")
            .arg(obj)
            .output()
            .expect("git cat-file -p <obj> failed")
            .stdout;
        let text = String::from_utf8(stdout).unwrap();
        std::fs::create_dir_all(format!("{}/{}", first, second))
            .expect("failed to create directories");
        std::fs::write(format!("{}/{}/{}.rs", first, second, obj), text)
            .expect("failed to write file");
    })
}

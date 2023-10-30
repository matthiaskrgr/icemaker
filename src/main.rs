#![feature(variant_count)]

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
mod fuzz_tree_splicer;
mod ice;
mod library;
mod printing;
mod run_commands;
mod smolfuzz;

use crate::flags::*;
use crate::fuzz::*;
use crate::fuzz_tree_splicer::*;
use crate::ice::*;
use crate::library::*;
use crate::printing::*;
use crate::run_commands::*;
use crate::smolfuzz::*;

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::io::BufRead;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use clap::Parser;
use colored::Colorize;
use lazy_static::lazy_static;
use rayon::prelude::*;
use regex::Regex;
use sha2::{Digest, Sha256};
use tempdir::TempDir;
use walkdir::WalkDir;

// local debug assertions: run with:
// LD_LIBRARY_PATH=/home/matthias/vcs/github/rust_debug_assertions/build/x86_64-unknown-linux-gnu/stage1/lib/rustlib/x86_64-unknown-linux-gnu/lib/

lazy_static! {
    static ref ALL_ICES_WITH_FLAGS: Mutex<Vec<Vec<OsString>>> =
        Mutex::new(vec![vec![OsString::new()]]);
}

static PRINTER: Printer = printing::Printer::new();

impl From<&Args> for Executable {
    #[allow(clippy::if_same_then_else)]
    fn from(args: &Args) -> Self {
        if args.clippy {
            Executable::Clippy
        } else if args.clippy_fix {
            Executable::ClippyFix
        } else if args.rust_fix {
            Executable::RustFix
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
        } else if (args).cranelift {
            // from local-debug-assertions build, -Zcodegen-backend=cranelift
            Executable::Cranelift
        } else if (args).kani {
            Executable::Kani
        } else if args.rustc_codegen_gcc {
            Executable::RustcCodegenGCC
        } else {
            Executable::Rustc
        }
    }
}

/// run on a project, each project having its own errors.json
fn check_dir(
    root_path: &PathBuf,
    args: &Args,
    global_tempdir_path: &PathBuf,
    timer: &Timer,
    errors_json_tmp: &Arc<Mutex<std::fs::File>>,
) -> Vec<PathBuf> {
    // read in existing errors
    // read the string INTO Vec<ICE>

    let errors_json = root_path.join("errors.json");
    let errors_before: Vec<ICE> = if errors_json.exists() {
        let read = match std::fs::read_to_string(&errors_json) {
            Ok(content) => content,
            Err(_) => panic!("failed to read '{}'", errors_json.display()),
        };
        match serde_json::from_str(&read) {
            Ok(previous_errors) => previous_errors,
            Err(e) => {
                // this can happen if we for example change the representation of Ice so that that the previous file is no longer compatible with the new format
                eprintln!("Failed to parse errors.json, is it a json file?");
                eprintln!("origina error: '{e:?}'");
                Vec::new()
            }
        }
    } else {
        // we don't have a file, start blank
        Vec::new()
    };

    let executable = Executable::from(args);
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
        } else if args.local_debug_assertions {
            vec![
                &Executable::Rustc,
                &Executable::Rustdoc,
                &Executable::Rustfmt,
                &Executable::ClippyFix,
                &Executable::Miri,
                &Executable::Cranelift,
            ]
        } else {
            vec![
                &Executable::Rustc,
                &Executable::Rustdoc,
                &Executable::Rustfmt,
                &Executable::ClippyFix,
                //  &Executable::Miri,
            ]
        }
    };

    if args.codegen {
        codegen_git_original_dirs();
        std::process::exit(0);
    }

    if args.smolfuzz {
        codegen_smolfuzz();
        return Vec::new();
    }

    if executables.contains(&&Executable::Miri) || matches!(executable, Executable::Miri) {
        println!("Running cargo miri setup");
        let _ = std::process::Command::new("cargo")
            .arg(if args.local_debug_assertions {
                "+local-debug-assertions"
            } else {
                "+master"
            })
            .arg("miri")
            .arg("setup")
            .status()
            .unwrap()
            .success();
    }

    if args.heat {
        let chain_order = args.chain_order;
        let _ = run_space_heater(executable, chain_order, global_tempdir_path);
        return Vec::new();
    }

    if args.codegen_splice {
        codegen_tree_splicer();
        std::process::exit(0);
    }

    if args.codegen_splice_omni {
        codegen_tree_splicer_omni();
        std::process::exit(0);
    }

    if args.fuzz {
        let _ = run_random_fuzz(executable, global_tempdir_path);
        return Vec::new();
    } else if args.fuzz2 {
        crate::fuzz2::fuzz2::fuzz2main();
        return Vec::new();
    }

    if args.incr_fuzz {
        tree_splice_incr_fuzz(global_tempdir_path);
        std::process::exit(1);
    }

    if (args).reduce {
        reduce_all(global_tempdir_path);
        std::process::exit(0);
    }

    // search for rust files inside CWD
    let mut files = WalkDir::new(root_path)
        .into_iter()
        .par_bridge()
        .filter_map(|e| e.ok())
        .filter(|f| f.path().extension() == Some(OsStr::new("rs")))
        .map(|f| f.path().to_owned())
        .collect::<Vec<PathBuf>>();

    // check biggest files first
    files.par_sort_by_cached_key(|file| std::fs::metadata(file).unwrap().len());
    files.reverse();

    /*
    eprintln!("\n\nSTART\n\n");

    // files that compile
    let files_that_compile = files
        .iter()
        //        filter(|f| EXCEPTIONS.)
        .filter(|f| file_compiles(f, &Executable::Rustc.path()))
        .collect::<Vec<&PathBuf>>();

    eprintln!("\n\nDONE\n\n");
     */

    //let exec_path = executable.path();
    executables.iter().for_each(|executable| {
        println!("Using {:?}: {}", executable, executable.path());
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
    let EXCEPTION_LIST: Vec<PathBuf> = EXCEPTIONS
        .iter()
        .map(PathBuf::from)
        // otherwise we don't match
        .map(|p| root_path.join(p))
        .collect();
    #[allow(non_snake_case)]
    let MIRI_EXCEPTION_LIST: Vec<PathBuf> = MIRI_EXCEPTIONS
        .iter()
        .map(PathBuf::from)
        // otherwise we don't match
        .map(|p| root_path.join(p))
        .collect();

    // count progress
    let counter = std::sync::atomic::AtomicUsize::new(0);

    // type Timer<'a> = (&'a Executable, AtomicUsize);

    let rustc_exec_path = Executable::Rustc.path();

    if args.incremental_test {
        eprintln!("checking which files compile...");
        let files = files
            .par_iter()
            .filter(|file| !EXCEPTION_LIST.contains(file))
            .filter(|file| file_compiles(file, &rustc_exec_path, global_tempdir_path))
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
                    incremental_stress_test(file_a, &files, &rustc_exec_path, global_tempdir_path)?;
                let is_ice = find_ICE_string(&file_a, &Executable::Rustc, output.clone());
                if is_ice.is_some() {
                    eprintln!("\nINCRCOMP ICE: {},{}", file_a.display(), file_b.display());
                    Some((file_a, file_b, output))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        dbg!(incr_crashes);
        return Vec::new();
    }

    // main loop that checks all the files
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
                            || (matches!(executable, Executable::Cranelift))
                                && !MIRI_EXCEPTION_LIST.contains(file))
                })
                .map(|executable| {
                    let executable_start_time = Instant::now();

                    let exec_path = executable.path();

                    match executable {
                        Executable::Rustc
                        /* | Executable::CraneliftLocal */ => {
                            // with expensive flags, run on each of the editions separately
                            let editions = if args.expensive_flags {
                                vec!["--edition=2015", "--edition=2018", "--edition=2021"]
                            } else {
                                // FIXME need to have somehting here to at least iter once :/
                                vec!["-Ccodegen-units=1"]
                            };
                            // for each file, run every chunk of RUSTC_FLAGS and check it and see if it crashes
                            RUSTC_FLAGS
                                // note: this can be dangerous in case of max memory usage, if a file needs a lot
                                .par_iter()
                                .panic_fuse()
                                .map(|flag_combinations| flag_combinations.iter())
                                // need shit to flat map a sequential iter into a par_iter
                                .flat_map_iter(|flag_combinations| {
                                    editions.iter().map(move |x| {
                                        if args.expensive_flags {
                                            flag_combinations
                                                .clone()
                                                .chain(std::iter::once(x).take(1))
                                        } else {
                                            flag_combinations
                                                .clone()
                                                .chain(std::iter::once(&"").take(0))
                                        }
                                    })
                                })
                                .map(|flag_combination| {
                                    let ice = ICE::discover(
                                        file,
                                        &exec_path,
                                        executable,
                                        flag_combination,
                                        &[],
                                        false,
                                        &counter,
                                        files.len(),
                                        args.silent,
                                        global_tempdir_path,
                                    );
                                    let seconds_elapsed =
                                        executable_start_time.elapsed().as_millis() as usize;
                                    timer.update_from_executable(executable, seconds_elapsed);

                                    ice
                                })
                                .collect::<Vec<Option<ICE>>>()
                        }
                        Executable::Miri => MIRIFLAGS
                            .par_iter()
                            .map(|miri_flag_combination| {
                                MIRI_RUSTFLAGS
                                    .par_iter()
                                    .panic_fuse()
                                    .map(|miri_rustflag| {
                                        let ice = ICE::discover(
                                            file,
                                            &exec_path,
                                            executable,
                                            *miri_rustflag,
                                            miri_flag_combination,
                                            false,
                                            &counter,
                                            files.len(),
                                            args.silent,
                                            global_tempdir_path,
                                        );
                                        let seconds_elapsed =
                                            executable_start_time.elapsed().as_millis() as usize;
                                        timer.update_from_executable(executable, seconds_elapsed);

                                        ice
                                    })
                                    .find_any(|ice| ice.is_some())
                            })
                            .flatten()
                            .collect::<Vec<Option<ICE>>>(),
                        _ => {
                            // if we run clippy/rustfmt/rla .. we dont need to check multiple combinations of RUSTFLAGS
                            let ice = vec![ICE::discover(
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
                                global_tempdir_path,
                            )];
                            let seconds_elapsed =
                                executable_start_time.elapsed().as_millis() as usize;
                            timer.update_from_executable(executable, seconds_elapsed);
                            ice
                        }
                    }
                })
                .flatten()
                .filter(|opt_ice| opt_ice.is_some())
                .map(|ice| ice.unwrap())
                .map(|ice| {
                    let ice_json =
                        serde_json::to_string_pretty(&ice).expect("failed ot jsonify ICE");
                    let errors_tmp = Arc::clone(errors_json_tmp);
                    let mut f = errors_tmp.lock().unwrap();
                    writeln!(f, "{}", ice_json)
                        .expect("failed to write to mutex locked errors_tmp.json");
                    ice
                })
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

    // if the ices start with the root path, we need to strip the root path
    errors.iter_mut().for_each(|ice| {
        let mut ice_path = ice.file.clone();
        if ice_path.starts_with(root_path) {
            ice_path = ice_path
                .strip_prefix(root_path)
                .expect("strip_prefix failed, could not fix ice.file")
                .to_owned();
            // readd the leading "./" that was stripped previously
            ice.file = PathBuf::from("./").join(ice_path);
        }
    });

    // errors.iter().for_each(|ice| ice.to_disk());

    // in the end, save all the errors to a file
    let errors_new = serde_json::to_string_pretty(&errors).unwrap();
    std::fs::write(&errors_json, &errors_new)
        .unwrap_or_else(|_| panic!("error: failed to write to {}", errors_json.display()));

    println!("\ndiff: \n");
    // get the diff
    let diff = diff::lines(
        &serde_json::to_string_pretty(&errors_before).unwrap(),
        &errors_new,
    )
    .iter()
    .map(|diff| match diff {
        diff::Result::Left(l) => format!("-{l}\n"),
        diff::Result::Both(l, _) => format!(" {l}\n"),
        diff::Result::Right(r) => format!("+{r}\n"),
    })
    .collect::<String>();

    println!("{diff}");

    // write the diff into report folder
    let reports_dir = crate::ice::REPORTS_DIR.to_owned();
    if !PathBuf::from(&reports_dir).exists() {
        std::fs::create_dir_all(&reports_dir).expect("failed to create icemaker reports dir!");
    }
    let diff_path = reports_dir.join("errors.diff");
    let mut file =
        std::fs::File::create(diff_path).expect("report.to_disk() failed to create file");
    file.write_all(diff.as_bytes())
        .expect("failed to write report");

    let new_ices = errors
        .iter()
        .filter(|new_ice| !errors_before.contains(new_ice))
        .collect::<Vec<&ICE>>();
    // TODO do the same for removed ices?
    println!("NEW ICES:\n{new_ices:#?}");

    if new_ices.len() == 0 {
        eprintln!("No new, ices, skipping reports...");
    } else {
        eprintln!("Generating reports...");
    }
    new_ices
        .into_iter()
        .map(|ice| {
            let ice = ice.clone();
            ice.into_report(global_tempdir_path)
        })
        .for_each(|ice_report| ice_report.to_disk());

    eprintln!("done");

    /*
    let root_path_string = root_path.display().to_string();


    // crashing commands
    ALL_ICES_WITH_FLAGS
        .lock()
        .unwrap()
        .iter()
        .map(|flags| {
            let flags = flags
                .iter()
                .map(|x| x.to_str().unwrap().to_string())
                .collect::<Vec<String>>();
            // miri for example has no flags, don't spam a bunch of empty lines into stdout
            flags
        })
        .filter(|flags| !flags.is_empty())
        .map(|flags| flags.join(" "))
        .filter(|flag| flag.starts_with(&root_path_string))
        .for_each(|_line| {
            // @HERE avoid spam
            // println!("{}", line);
        });
        */

    files
}

#[derive(Debug, Default)]
struct Timer {
    rustc_time: AtomicUsize,
    clippy_time: AtomicUsize,
    rustdoc_time: AtomicUsize,
    rla_time: AtomicUsize,
    rustfmt_time: AtomicUsize,
    miri_time: AtomicUsize,
    craneliftlocal_time: AtomicUsize,
    clippyfix_time: AtomicUsize,
    rustfix_time: AtomicUsize,
    kani_time: AtomicUsize,
    rustc_codegen_gcc: AtomicUsize,
}

impl Timer {
    fn update_from_executable(&self, exe: &Executable, elapsed_duration: usize) {
        match exe {
            Executable::Rustc => {
                let _ = self
                    .rustc_time
                    .fetch_add(elapsed_duration, Ordering::SeqCst);
            }
            Executable::Clippy => {
                let _ = self
                    .clippy_time
                    .fetch_add(elapsed_duration, Ordering::SeqCst);
            }
            Executable::Rustdoc => {
                let _ = self
                    .rustdoc_time
                    .fetch_add(elapsed_duration, Ordering::SeqCst);
            }
            Executable::RustAnalyzer => {
                let _ = self.rla_time.fetch_add(elapsed_duration, Ordering::SeqCst);
            }
            Executable::Rustfmt => {
                let _ = self
                    .rustfmt_time
                    .fetch_add(elapsed_duration, Ordering::SeqCst);
            }
            Executable::Miri => {
                let _ = self.miri_time.fetch_add(elapsed_duration, Ordering::SeqCst);
            }

            Executable::Cranelift => {
                let _ = self
                    .craneliftlocal_time
                    .fetch_add(elapsed_duration, Ordering::SeqCst);
            }
            Executable::ClippyFix => {
                let _ = self
                    .clippyfix_time
                    .fetch_add(elapsed_duration, Ordering::SeqCst);
            }
            Executable::RustFix => {
                let _ = self
                    .rustfix_time
                    .fetch_add(elapsed_duration, Ordering::SeqCst);
            }
            Executable::Kani => {
                let _ = self.kani_time.fetch_add(elapsed_duration, Ordering::SeqCst);
            }
            Executable::RustcCodegenGCC => {
                let _ = self
                    .rustc_codegen_gcc
                    .fetch_add(elapsed_duration, Ordering::SeqCst);
            }
        }
    }

    fn new() -> Self {
        Timer::default()
    }

    fn into_seconds(self) -> Self {
        use std::time::Duration;
        Timer {
            rustc_time: AtomicUsize::new(
                Duration::from_millis(self.rustc_time.into_inner() as u64).as_secs() as usize,
            ),
            clippy_time: AtomicUsize::new(
                Duration::from_millis(self.clippy_time.into_inner() as u64).as_secs() as usize,
            ),
            rustdoc_time: AtomicUsize::new(
                Duration::from_millis(self.rustdoc_time.into_inner() as u64).as_secs() as usize,
            ),
            rla_time: AtomicUsize::new(
                Duration::from_millis(self.rla_time.into_inner() as u64).as_secs() as usize,
            ),
            rustfmt_time: AtomicUsize::new(
                Duration::from_millis(self.rustfmt_time.into_inner() as u64).as_secs() as usize,
            ),
            miri_time: AtomicUsize::new(
                Duration::from_millis(self.miri_time.into_inner() as u64).as_secs() as usize,
            ),

            craneliftlocal_time: AtomicUsize::new(
                Duration::from_millis(self.craneliftlocal_time.into_inner() as u64).as_secs()
                    as usize,
            ),
            clippyfix_time: AtomicUsize::new(
                Duration::from_millis(self.clippyfix_time.into_inner() as u64).as_secs() as usize,
            ),
            rustfix_time: AtomicUsize::new(
                Duration::from_millis(self.rustfix_time.into_inner() as u64).as_secs() as usize,
            ),
            kani_time: AtomicUsize::new(
                Duration::from_millis(self.kani_time.into_inner() as u64).as_secs() as usize,
            ),
            rustc_codegen_gcc: AtomicUsize::new(
                Duration::from_millis(self.rustc_codegen_gcc.into_inner() as u64).as_secs()
                    as usize,
            ),
        }
    }
}

fn main() {
    // how long did we take?
    let global_start_time = Instant::now();

    // do not dump backtraces to disk all the time
    // RUSTC_ICE=..
    // https://github.com/rust-lang/rust/pull/108714
    std::env::set_var("RUSTC_ICE", "0");

    let args = Args::parse();

    let global_tempdir = if let Some(ref custom_tempdir_path) = args.global_tempdir_path {
        let mut custom_tmpdir = std::path::PathBuf::from(&custom_tempdir_path);
        let dir_display = custom_tmpdir.display();

        assert!(
            custom_tmpdir.is_dir(),
            "global tempdir '{}' not found",
            dir_display
        );

        // otherwise tempdir would be  foo.af32ed2 and not foo/af32ed2
        custom_tmpdir.push("dir");
        let dir_display = custom_tmpdir.display();

        TempDir::new_in("icemaker_global_tempdir", &format!("{}", dir_display))
            .expect("failed to create global icemaker tempdir")
    } else {
        TempDir::new("icemaker_global_tempdir").expect("failed to create global icemaker tempdir")
    };

    let global_tempdir_path_closure: PathBuf = global_tempdir.path().to_owned();
    let global_tempdir_path: PathBuf = global_tempdir_path_closure.clone();

    //dbg!(&global_tempdir_path);

    println!(
        "using {} threads",
        if args.threads != 0 {
            args.threads
        } else if let Ok(threads) = std::thread::available_parallelism() {
            threads.get()
        } else {
            // failed to get threads, eh
            0
        }
    );

    // rayon thread pool so we can configure number of threads easily
    rayon::ThreadPoolBuilder::new()
        .num_threads(args.threads)
        .build_global()
        .unwrap();

    // set up ctrl c handler so we can print ICEs so far when ctrl+'ing
    ctrlc::set_handler(move || {
        println!("\n\nCtrl+C: TERMINATED\n");

        eprintln!("triyng to rm tempdir:");
        dbg!(&global_tempdir_path_closure.clone());
        if std::fs::remove_dir_all(global_tempdir_path_closure.clone()).is_err() {
            eprintln!(
                "WARNING: failed to remove global tempdir '{}'",
                global_tempdir_path_closure.display()
            );
        }

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

        //dbg!(&global_tempdir_path_closure);

        std::process::exit(42);
    })
    .expect("Error setting Ctrl-C handler");

    // a path with rustc files that we want to check
    type ProjectDir = PathBuf;

    let projs: Vec<ProjectDir> = args.projects.to_vec();
    let projects: Vec<ProjectDir> = if projs.is_empty() {
        // if we didn't get anything passed, use cwd
        vec![std::env::current_dir().expect("cwd not found or does not exist!")]
    } else {
        println!(
            "checking projects: {}",
            projs
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<String>>()
                .join(", ")
        );
        projs
    };
    // assert these are all valid directories
    if let Some(invalid_path) = projects.iter().find(|proj_path| !proj_path.is_dir()) {
        eprintln!(
            "ERROR: --projects '{}' is not a directory",
            invalid_path.display()
        );
        std::process::exit(1);
    }

    let timer: Timer = Timer::new();

    let root_path = std::env::current_dir().expect("could not get CWD!");
    let tmp_file = root_path.join("errors_tmp.json");

    let errors_json_tmp = Arc::new(Mutex::new(std::fs::File::create(tmp_file).unwrap()));

    // all checked files

    let files = projects
        .iter()
        .map(|dir| check_dir(dir, &args, &global_tempdir_path, &timer, &errors_json_tmp))
        .flat_map(|v| v.into_iter())
        .collect::<Vec<PathBuf>>();

    let seconds_elapsed = global_start_time.elapsed().as_secs();

    let number_of_checked_files = files.len();
    if seconds_elapsed == 0 {
        println!("Checked {number_of_checked_files} files in <1 second");
        return;
    }
    let files_per_second = number_of_checked_files as f64 / seconds_elapsed as f64;
    println!(
        "\nChecked {} files in {:.2} minutes, {:.2} files/second",
        number_of_checked_files,
        seconds_elapsed as f64 / 60_f64,
        files_per_second
    );

    eprintln!("Timings in seconds:\n{:?}", timer.into_seconds());

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
                // @FIXME ? this might no longer be needed
                // println!("{}", flags.join(" "))
            }
        });
}

impl ICE {
    /// find out if a file crashes rustc with the given flags
    #[allow(clippy::too_many_arguments)]
    fn discover<'f, F: IntoIterator<Item = &'f &'f str>>(
        file: &Path,
        exec_path: &str,
        executable: &Executable,
        compiler_flags: F,
        miri_flags: &[&str],
        incremental: bool,
        counter: &AtomicUsize,
        total_number_of_files: usize,
        silent: bool,
        global_tempdir_path: &PathBuf,
    ) -> Option<Self> {
        // convert IntoIterator<Item &&str> to &[&str]
        let compiler_flags = &compiler_flags.into_iter().cloned().collect::<Vec<&str>>()[..];

        let thread_start = Instant::now();
        const SECONDS_LIMIT: u64 = PROCESS_TIMEOUT_S as u64;
        const _SECONDS_LIMIT_MIRI: u64 = 20;

        let incremental = if compiler_flags == ["INCR_COMP"] {
            true
        } else {
            incremental
        };

        // run the command with some flags (actual_args)

        let index = counter.load(Ordering::SeqCst); // the current file number
        let file_name = file.display().to_string();

        // print Checking ... + progress percentage for each file we are checking
        if !silent {
            PRINTER.log(PrintMessage::Progress {
                index,
                total_number_of_files,
                file_name,
            });
        }

        // note: `actual_args` are the VERY ORIGINAL ARGS so this contains something like -otmpdir_foo.AFXIU/outfile which will no longer be
        // a valid path as soon as the tempdir goes out of scope
        let (cmd_output, _cmd, actual_args) = match executable {
            Executable::Clippy => run_clippy(exec_path, file, global_tempdir_path),
            Executable::ClippyFix => run_clippy_fix(exec_path, file, global_tempdir_path),
            Executable::RustFix => run_rustfix(exec_path, file, global_tempdir_path),
            // Executable::Rustc => run_rustc_lazy_type_alias(exec_path, file, global_tempdir_path),
            Executable::Rustc => run_rustc(
                exec_path,
                file,
                incremental,
                compiler_flags,
                global_tempdir_path,
            ),
            Executable::Rustdoc => run_rustdoc(exec_path, file, global_tempdir_path),
            Executable::RustAnalyzer => run_rust_analyzer(exec_path, file, global_tempdir_path),
            Executable::Rustfmt => run_rustfmt(exec_path, file, global_tempdir_path),
            Executable::Miri => run_miri(
                exec_path,
                file,
                miri_flags,
                compiler_flags,
                global_tempdir_path,
            ),

            Executable::Cranelift => {
                let mut compiler_flags = compiler_flags.to_vec();
                compiler_flags.push("-Zcodegen-backend=cranelift");
                run_rustc(
                    exec_path,
                    file,
                    incremental,
                    &compiler_flags,
                    global_tempdir_path,
                )
            }
            Executable::Kani => run_kani(
                exec_path,
                file,
                miri_flags, // hack
                compiler_flags,
                global_tempdir_path,
            ),
            Executable::RustcCodegenGCC => {
                rustc_codegen_gcc_local(exec_path, file, false, compiler_flags, global_tempdir_path)
            }
        }
        .unwrap();

        // dbg!(&actual_args);

        // remap actual args

        // this tempdir should live until the end of the function
        let tempdir =
            TempDir::new_in(global_tempdir_path, "rustc_testrunner_tmpdir_discover").unwrap();
        let tempdir_path = tempdir.path().display();
        let actual_args = actual_args
            .into_iter()
            .map(|arg| {
                if arg.to_str().unwrap().starts_with("-o") {
                    let mut os = OsString::from("-o");
                    os.push::<&OsString>(&OsString::from(tempdir_path.to_string()));
                    os.push::<&OsString>(&OsString::from("/outputfile"));
                    os
                } else if arg.to_str().unwrap().starts_with("-Zdump-mir-dir)") {
                    let mut os = OsString::from("-Zdump-mir-dir=");
                    os.push::<&OsString>(&OsString::from(tempdir_path.to_string()));

                    os
                } else {
                    arg
                }
            })
            .collect::<Vec<OsString>>();

        // dbg!("after remapping");
        //  dbg!(&actual_args);

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
        // clippy_lint -> -Wclippy::clippy_lint
        //let actual_args = actual_args.into_iter().map(|arg| {}).collect::<Vec<_>>();

        // find out the ice message
        // https://github.com/rust-lang/rust/pull/112849 broke panic messages
        let stderr = String::from_utf8_lossy(&cmd_output.stderr);
        let mut lines_iter = stderr.lines();

        let mut ice_msg = lines_iter
            .find(|line| {
                line.contains("panicked at") || line.contains("error: internal compiler error: ")
            })
            .unwrap_or_default()
            .to_string();
        if ice_msg.contains("panicked at") {
            // the panick message is actually on the next line
            let panic_msg = lines_iter
                .next()
                .unwrap_or("icemaker did not find panic message");
            // reconstruct the old one-line panic msg, somewhat
            ice_msg = format!(r#"{ice_msg} '{panic_msg}'"#);
        }

        ice_msg = ice_msg.replace("error: internal compiler error:", "ICE:");

        // rustc sets 101 if it crashed
        let exit_status = cmd_output.status.code().unwrap_or(0);

        let mut found_error: Option<(String, ICEKind)> =
            find_ICE_string(file, executable, cmd_output);

        // if rustdoc crashes on a file that does not compile, turn this into a ICEKind::RustdocFrailness
        match (&found_error, executable) {
            (Some((errstring, ICEKind::Ice(_))), Executable::Rustdoc) => {
                if !file_compiles(
                    &file.to_path_buf(),
                    &Executable::Rustc.path(),
                    global_tempdir_path,
                ) {
                    found_error = Some((errstring.clone(), ICEKind::Ice(Interestingness::Boring)));
                }
            }
            _ => {}
        }

        // unmut
        let found_error = found_error;

        // check if the file enables any compiler features
        let uses_feature: bool = uses_feature(file);

        // this is basically an unprocessed ICE, we know we have crashed, but we have not reduced the flags yet.
        // prefer return this over returning an possible hang while minimizing flags later
        let raw_ice = if let Some((ice_msg, icekind)) = found_error.clone() {
            let ice = ICE {
                regresses_on: Regression::Master,
                needs_feature: uses_feature,
                file: file.to_owned(),
                args: compiler_flags
                    .iter()
                    .cloned()
                    .map(|f| f.to_string())
                    .collect::<Vec<String>>(),
                error_reason: ice_msg.clone(),
                ice_msg,
                executable: Executable::Rustc,
                kind: icekind,
            };
            Some(ice)
        } else {
            None
        };

        let exit_code_looks_like_crash = exit_status == 101 ||  /* segmentation fault etc */ (132..=139).contains(&exit_status) ||  /* llvm crash / assertion failure etc */ exit_status == 254;

        let miri_finding_is_potentially_interesting: bool =
            if matches!(executable, Executable::Miri) && found_error.is_some() {
                let miri_input_file = std::fs::read_to_string(file).unwrap_or_default();
                // finding is interesting if input file contains none of those strings
                !["unsafe", "repr(simd)"]
                    .into_iter()
                    .any(|kw| miri_input_file.contains(kw))
            } else {
                false
            };

        // @TODO merge the two  found_error.is_some() branches and print ice reason while checking
        #[allow(clippy::format_in_format_args)]
        if exit_code_looks_like_crash && found_error.is_some()
    // in miri, "cargo miri run" will return 101 if the run program (not miri!) just panics so ignore that
        || (matches!(executable, Executable::Miri) && found_error.is_some()) || (matches!(executable, Executable::ClippyFix) && found_error.is_some())
        {
            let _ = 0;
            // let (found_error, ice_kind) = found_error.clone().unwrap();
            /*              println!(
                "{kind}: {executable:?} {file_name:<20.80} {msg:<30.200} {feat}     {flags:<.30}",
                kind = if matches!(ice_kind, ICEKind::Ub(..)) {
                    if miri_finding_is_potentially_interesting {
                        " UB".green()
                    } else {
                        "UB ".normal()
                    }
                } else {
                    "ICE".red()
                },
                msg = {
                    let s = found_error; /*

                                         // we might have None error found but still a suspicious exit status, account, dont panic on None == found_error then
                                         .unwrap_or(format!("No error found but exit code: {}", exit_status)); */
                    let s = s.replace("error: internal compiler error:", "ICE:");
                    let mut s = s.replace("unexpected panic:", "ICE:");
                    s.push_str(&ice_msg);
                    s
                },
                feat = if uses_feature { "        " } else { "no feat!" },
                flags = format!("{compiler_flags:?}")
            );
            print!("\r");
            let _stdout = std::io::stdout().flush();
            */
        } else if !silent {
            //@FIXME this only advances the checking once the files has already been checked!
            // print_checking_progress(index, total_number_of_files, &file_name);
        }

        if exit_code_looks_like_crash || found_error.is_some() {
            crate::ALL_ICES_WITH_FLAGS
                .lock()
                .unwrap()
                .push(actual_args.clone());
        }

        // incremental ices don't need to have their flags reduced
        if incremental && found_error.is_some() {
            let (mut found_error, kind) = found_error.unwrap();
            if found_error.len() > ice_msg.len() {
                ice_msg = found_error.clone();
            } else {
                found_error = ice_msg.clone();
            }

            let ice = ICE {
                regresses_on: Regression::Nightly,

                needs_feature: uses_feature,
                file: file.to_owned(),
                args: [
                    "-Zincremental-verify-ich=yes",
                    "-Cincremental=<dir>",
                    "-Cdebuginfo=2",
                ]
                .into_iter()
                .map(|s| s.into())
                .collect::<Vec<String>>(),
                // executable: rustc_path.to_string(),
                error_reason: found_error,
                ice_msg,
                executable: executable.clone(),
                kind,
            };
            //  dbg!(&ice);

            // we know this is an ICE
            PRINTER.log(PrintMessage::IceFound {
                ice: ice.to_printable(),
            });

            return Some(ice);
        }

        let mut ret = None;
        if let Some((mut error_reason, ice_kind)) = found_error {
            let ice_kind = if matches!(ice_kind, ICEKind::Ub(..)) {
                if miri_finding_is_potentially_interesting {
                    ICEKind::Ub(UbKind::Interesting)
                } else {
                    ICEKind::Ub(UbKind::Uninteresting)
                }
            } else {
                ice_kind
            };
            //            eprintln!("ICE\n\n\nICE\n\n");
            if !matches!(executable, Executable::Miri) {
                // PRECHECK
                // optimization: check if rustc crashes on the file without needing any flags, if yes, return that as an ICE
                // we might produce several of those if we check different flags but they should all be deduplicated laster on?
                let mut pure_rustc_cmd = Command::new(Executable::Rustc.path());
                pure_rustc_cmd.arg(file);
                pure_rustc_cmd.current_dir(global_tempdir_path);

                let pure_rustc_output = prlimit_run_command(&mut pure_rustc_cmd).unwrap();
                let found_error0 = find_ICE_string(file, &Executable::Rustc, pure_rustc_output);

                // shitty destructing

                let seconds_elapsed = thread_start.elapsed().as_secs();
                if seconds_elapsed > (SECONDS_LIMIT) {
                    print!("\r");
                    println!(
                        "{}: {:?} {} ran for more ({} seconds) than {} seconds, killed!   \"{}\"",
                        "HANG".blue(),
                        executable,
                        file.display(),
                        seconds_elapsed,
                        SECONDS_LIMIT,
                        actual_args
                            .iter()
                            .cloned()
                            .map(|s| s.into_string().unwrap())
                            .map(|s| {
                                let mut tmp = s;
                                tmp.push(' ');
                                tmp
                            })
                            .collect::<String>()
                    );
                }
                if let Some((mut err_reason, icekind)) = found_error0 {
                    if err_reason.len() > ice_msg.len() {
                        ice_msg = err_reason.clone();
                    } else {
                        err_reason = ice_msg.clone();
                    }

                    let ice = ICE {
                        regresses_on: Regression::Master,
                        needs_feature: uses_feature,
                        file: file.to_owned(),
                        args: Vec::new(),
                        error_reason: err_reason,
                        ice_msg,
                        executable: Executable::Rustc,
                        kind: icekind,
                    };
                    PRINTER.log(PrintMessage::IceFound {
                        ice: ice.to_printable(),
                    });

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

            // the last one should be the full combination of flags
            let last = args3.iter().collect::<Vec<&&str>>();

            let mut flag_combinations = get_flag_combination(args3);
            flag_combinations.push(last.clone());
            let flag_combinations = flag_combinations;

            //            dbg!(&flag_combinations);

            match executable {
                Executable::Rustc
                | Executable::ClippyFix
                | Executable::RustFix
                | Executable::Cranelift
                | Executable::RustcCodegenGCC => {
                    // if the full set of flags (last) does not reproduce the ICE, bail out immediately (or assert?)
                    let tempdir =
                        TempDir::new_in(global_tempdir_path, "rustc_testrunner_tmpdir").unwrap();

                    // WE ALREADY HAVE filename etc in the args, rustc erros if we pass 2 files etc

                    // output: get the command output again from file, executable and flags
                    // clippyfix for example needs special handling here
                    let output = if matches!(executable, Executable::ClippyFix) {
                        let (output, _somestr, _flags) = run_clippy_fix_with_args(
                            &executable.path(),
                            file,
                            &last.iter().map(|x| **x).collect::<Vec<_>>(),
                            global_tempdir_path,
                        )
                        .unwrap();
                        output
                    } else {
                        // let tempdir_path = tempdir.path();
                        // let output_file = format!("-o{}/file1", tempdir_path.display());
                        //let dump_mir_dir = format!("-Zdump-mir-dir={}", tempdir_path.display());
                        let mut cmd = Command::new(exec_path);
                        cmd.args(&last);
                        prlimit_run_command(&mut cmd).unwrap()
                    };

                    //  dbg!(&output);
                    let found_error2 = find_ICE_string(file, executable, output);

                    // remove the tempdir
                    tempdir.close().unwrap();

                    if found_error2.is_some() {
                        // walk through the flag combinations and save the first (and smallest) one that reproduces the ice
                        flag_combinations.iter().any(|flag_combination| {
                            //  dbg!(&flag_combination);

                            // check if we have to timeout
                            // use limit * 2 to be a bit more generous since bisecting can take time
                            if thread_start.elapsed().as_secs() > SECONDS_LIMIT * 2 {
                                // break from the any()
                                return true;
                            }

                            let output = if matches!(executable, Executable::ClippyFix) {
                                let (output, _somestr, _flags) = run_clippy_fix_with_args(
                                    &executable.path(),
                                    file,
                                    &flag_combination.iter().map(|x| **x).collect::<Vec<_>>(),
                                    global_tempdir_path,
                                )
                                .unwrap();
                                output
                            } else {
                                let tempdir5 =
                                    TempDir::new_in(global_tempdir_path, "rustc_testrunner_tmpdir")
                                        .unwrap();
                                let tempdir_path = tempdir5.path();
                                let output_file = format!("-o{}/file1", tempdir_path.display());
                                let dump_mir_dir =
                                    format!("-Zdump-mir-dir={}", tempdir_path.display());

                                let mut cmd = Command::new(exec_path);
                                cmd.arg(file)
                                    .args(flag_combination.iter())
                                    .arg(output_file)
                                    .arg(dump_mir_dir);
                                let output = prlimit_run_command(&mut cmd).unwrap();
                                tempdir5.close().unwrap();
                                output
                            };

                            let found_error3 = find_ICE_string(file, executable, output);
                            // remove the tempdir
                            //  tempdir.close().unwrap();
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
                        eprintln!(
                            "\nfull set of flags on '{}' did not reproduce the ICE??\nflags: {}",
                            file.display(),
                            args3.iter().map(|s| format!("{s} ")).collect::<String>(),
                        );
                    }
                }
                Executable::Clippy
                | Executable::Rustdoc
                | Executable::RustAnalyzer
                | Executable::Rustfmt
                | Executable::Miri
                | Executable::Kani => {}
            }

            let seconds_elapsed = thread_start.elapsed().as_secs();
            if seconds_elapsed > (SECONDS_LIMIT) {
                print!("\r");
                println!(
                    "{}: {:?} {} ran for more ({} seconds) than {} seconds, killed!   \"{}\"",
                    "HANG".blue(),
                    executable,
                    file.display(),
                    seconds_elapsed,
                    SECONDS_LIMIT,
                    actual_args
                        .iter()
                        .cloned()
                        .map(|s| s.into_string().unwrap())
                        .map(|s| {
                            let mut tmp = s;
                            tmp.push(' ');
                            tmp
                        })
                        .collect::<String>()
                );

                if raw_ice.is_some() {
                    return raw_ice;
                }
                // the process was killed by prlimit because it exceeded time limit
                let hang = ICE {
                    regresses_on: Regression::Master,
                    needs_feature: uses_feature,
                    file: file.to_owned(),
                    args: compiler_flags
                        .iter()
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>(),

                    error_reason: String::from("HANG"),
                    ice_msg: "HANG".into(),
                    executable: executable.clone(),
                    kind: ICEKind::Hang(seconds_elapsed),
                };
                PRINTER.log(PrintMessage::IceFound {
                    ice: hang.to_printable(),
                });

                return Some(hang);
            }

            let regressing_channel =
                find_out_crashing_channel(&bad_flags, file, global_tempdir_path);
            // add these for a more accurate representation of what we ran originally
            bad_flags.push(&"-ooutputfile");
            bad_flags.push(&"-Zdump-mir-dir=dir");

            if error_reason.len() > ice_msg.len() {
                ice_msg = error_reason.clone();
            } else {
                error_reason = ice_msg.clone();
            }

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
                ice_msg: ice_msg.clone(),
                executable: executable.clone(),
                kind: ice_kind,
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
        // @TODO this only reports if the file finishes running, if we are stuck, we wont
        let seconds_elapsed = thread_start.elapsed().as_secs();
        if seconds_elapsed > (SECONDS_LIMIT) {
            print!("\r");
            println!(
                "{}: {:?} {} ran for more ({} seconds) than {} seconds, killed!   \"{}\"",
                "HANG".blue(),
                executable,
                file.display(),
                seconds_elapsed,
                SECONDS_LIMIT,
                actual_args
                    .iter()
                    .cloned()
                    .map(|s| s.into_string().unwrap())
                    .map(|s| {
                        let mut tmp = s;
                        tmp.push(' ');
                        tmp
                    })
                    .collect::<String>()
            );

            if raw_ice.is_some() {
                return raw_ice;
            }

            // the process was killed by prlimit because it exceeded time limit
            let ret_hang = ICE {
                regresses_on: Regression::Master,
                needs_feature: uses_feature,
                file: file.to_owned(),
                args: compiler_flags
                    .iter()
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>(),

                error_reason: String::from("HANG"),
                ice_msg,
                executable: executable.clone(),
                kind: ICEKind::Hang(seconds_elapsed),
            };
            ret = Some(ret_hang);
        }

        if let Some(ice) = ret.clone() {
            PRINTER.log(PrintMessage::IceFound {
                ice: ice.to_printable(),
            });
        }
        drop(tempdir);
        ret
    }
}

/// find out if we crash on master, nightly, beta or stable
fn find_out_crashing_channel(
    bad_flags: &Vec<&&str>,
    file: &Path,
    global_tempdir_path: &PathBuf,
) -> Regression {
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

    let tempdir = TempDir::new_in(global_tempdir_path, "rustc_testrunner_tmpdir").unwrap();
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
        file,
        &Executable::Rustc,
        prlimit_run_command(
            Command::new(stable_path)
                .arg(file)
                .args(&bad_but_no_nightly_flags)
                .arg(&output_file), //.arg(&dump_mir_dir)
        )
        .unwrap(),
    )
    .is_some();

    let beta_ice: bool = find_ICE_string(
        file,
        &Executable::Rustc,
        prlimit_run_command(
            Command::new(beta_path)
                .arg(file)
                .args(&bad_but_no_nightly_flags)
                .arg(&output_file), //.arg(&dump_mir_dir)
        )
        .unwrap(),
    )
    .is_some();

    let nightly_ice: bool = find_ICE_string(
        file,
        &Executable::Rustc,
        prlimit_run_command(
            Command::new(nightly_path)
                .arg(file)
                .args(bad_flags)
                .arg(&output_file)
                .arg(dump_mir_dir),
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

/// take the executable we used and the executables/runs output and determine whether the should raise an ICE or not (by looking at the exit status / stderr for example)
#[allow(non_snake_case)]
fn find_ICE_string(
    input_file: &Path,
    executable: &Executable,
    output: Output,
) -> Option<(String, ICEKind)> {
    const IN_CODE_FP_KEYWORDS: &[&str] = &[
        "panicked at",
        "RUST_BACKTRACE=",
        "(core dumped)",
        "mir!",
        "#![no_core]",
        "#[rustc_symbol_name]",
        "break rust",
        "feature(lang_items)",
        "#[rustc_variance]",
        "qemu: uncaught target signal",
        "core_intrinsics",     // feature(..)
        "platform_intrinsics", // feature(..)
        "::SIGSEGV",
        "SIGSEGV::",
        "delay_span_bug_from_inside_query",
        "#[rustc_variance]",
        "rustc_layout_scalar_valid_range_end", // rustc attr
        "rustc_attrs",
    ];

    let interestingness = {
        let file_text: &str = &std::fs::read_to_string(input_file).unwrap_or_default();

        if IN_CODE_FP_KEYWORDS.iter().any(|kw| file_text.contains(kw)) {
            // if we have any of the keywords in the file there likely will be crashes
            Interestingness::Boring
        } else {
            Interestingness::Interesting
        }
    };

    let mut internal_feature = false;

    let keywords_miri_ub = [
        "error: Undefined Behavior",
        // "the evaluated program leaked memory", // memleaks are save apparently
        "this indicates a bug in the program",
        "the compiler unexpectedly panicked",
        "thread 'rustc' panicked at",
        "we would appreciate a bug report",
        "misaligned pointer dereference",
        "Miri caused an ICE during evaluation.",
    ]
    .into_iter()
    .map(|kw| Regex::new(kw).unwrap_or_else(|_| panic!("failed to construct regex: {kw}")))
    .collect::<Vec<_>>();

    let keywords_clippyfix_failure = [
        ".*likely indicates a bug in either rustc or cargo itself.*",
        ".*after fixes were automatically applied the compiler reported errors within these files.*",
        ".*fixing code with the `--broken-code` flag.*",
    ]
    .into_iter()
    .map(|kw| Regex::new(kw).unwrap_or_else(|_| panic!("failed to construct regex: {kw}")))
    .collect::<Vec<_>>();

    let keywords_generic_ice = [
        "^LLVM ERROR",
        "Miri caused an ICE during evaluation.",
        "^thread '.*' panicked at",
        "^query stack during panic",
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
        // llvm assertion failure
        "Assertion `.*' failed",
        // do not include anything like libc::SIGSEGV
        // note: rustc regex crate does not support this v  :( 
        //"(?!.*lib::)^.*(SIGABRT)",
        //"(?!.*libc::)^.*(SIGSEGV)",
        "process abort signal",
        "SIGKILL: kill",
        "SIGSEGV:",
        // rustc_codegen_gcc
        // "libgccjit.so: error:",
        // "thread '.*' panicked at",
        // rustfmt formatting failure:
        "left behind trailing whitespace",
        "cycle encountered after",
        "error: rustc interrupted by",

      //  "we would appreciate a bug report",

    ].into_iter()
    .map(|kw| Regex::new(kw).unwrap_or_else(|_| panic!("failed to construct regex: {kw}")))
    .collect::<Vec<_>>();

    let keywords_double_panic_ice = [
        "thread caused non-unwinding panic. aborting.",
        "panic in a function that cannot unwind",
        "thread panicked while panicking. aborting.",
        "-Z treat-err-as-bug=",
    ]
    .into_iter()
    .collect::<Vec<_>>();

    // let output = cmd.output().unwrap();
    // let _exit_status = output.status;

    //check for prlimit output first by looking at the prlimit output (so only available in none-ci build)
    //(this worked better with systemd-run.. :/ )
    if !cfg!(feature = "ci") {
        let termination_reason = &std::io::Cursor::new(&output.stdout)
            .lines()
            .chain(std::io::Cursor::new(&output.stderr).lines())
            .filter_map(|line| line.ok())
            .find(|l| l.contains("prlimit"));
        if let Some(term_res) = termination_reason {
            if term_res.contains("killed") {
                // runtime limit
                return Some((term_res.to_owned(), ICEKind::OOM));
            } else {
                /* assume timeout */
                return Some((term_res.to_owned(), ICEKind::Hang(123)));
            }
        }
    }

    let delay_span_bug_regex = Regex::new("^error: internal compiler error: no errors encountered even though `delay_span_bug` issued$").unwrap();

    [&output.stdout, &output.stderr]
        .into_iter()
        .find_map(|executable_output| {

            let lines = std::io::Cursor::new(executable_output)
                .lines()
                .map_while(Result::ok)
                // FPs
                .filter(|line| !line.contains("pub const SIGSEGV"))
                 // the checked code itself might contain something like RUST_BACKTRACE=... :
                 // in the output this will look somewhat like this:
                 // 23 |         panic!(it.next(), Some("note: Run with `RUST_BACKTRACE=1` 
                // ignore such lines
                //.filter(|line|!(line.chars().next().map(|c| c.is_ascii_digit())  == Some(true) && line.contains(" | ") && line.contains("RUST_BACKTRACE=")));
                .filter(|line| {
                    let split = &line.split_ascii_whitespace().take(3).collect::<Vec<_>>()[..];
                    match split {
                        // skip these  1234 | bla...
                        [a, b] if a.parse::<i32>().is_ok() && *b == "|" => false,
                        [a, b, ..] if a.parse::<i32>().is_ok() && *b == "|" => false,
                         _ => true,
                }});

            match executable {
                Executable::Miri => {
                    // find the line where any (the first) ub keywords is contained in it
                    let ub_line = std::io::Cursor::new(executable_output)
                    .lines()
                    .map_while(Result::ok)
                    // filter out FPs
                    .filter(|line| !line.contains("pub const SIGSEGV") )
                    .find(|line| {
                        keywords_miri_ub.iter().any(|regex| {
                            // if the regex is equal to "panicked at: ", make sure the line does NOT contain "the evaluated program panicked at..."
                            // because that would be caused by somethink like panic!() in the code miri executes and we don't care about that
                            if regex.to_string() == "panicked at:" {
                                regex.is_match(line) && !line.contains("the evaluated program")
                            } else {
                                regex.is_match(line)
                            }
                        })
                    });
                    //  dbg!(&ub_line);
                    if let Some(ub_line_inner) = ub_line {
                        // this is a return inside the iterator
                        Some((ub_line_inner, ICEKind::Ub(UbKind::Uninteresting)))
                    } else {
                        // we didn't find ub, but perhaps miri crashed?
                        // TRICKY: from just looking at the output, we don't know if it is the program or miri that crashes which is tricky
                        std::io::Cursor::new(executable_output)
                            .lines()
                            .map_while(Result::ok)
                            .filter(|line| !line.contains("pub const SIGSEGV") /* FPs */)
                            .find(|line| {
                                keywords_generic_ice
                                    .iter()
                                    .any(|regex| regex.is_match(line))
                            })
                            // try to exclude panic! todo! assert! etc in the actual program we are checking
                            //.filter(|line|  !line.contains("main.rs"))
                            .map(|line| {

                                if line.contains("is internal to the compiler or standard library") {
                                    // internal feature are can be easily misused and crash rustc
                                    internal_feature = true;
                                }
                                // we found the line with for example "assertion failed: `(left == right)`" , but it would be nice to get some more insight what left and right is


                                let line = if line.contains("left == right") || line.contains("left != right") {
                                    // try to find a line that starts with "assertion (...  failed)"
                                    let left = std::io::Cursor::new(executable_output)
                                        .lines()
                                        .map_while(Result::ok).skip_while(|line| line.starts_with("assertion")).find(|line| line.starts_with("  left:")).unwrap_or_default();

                                let right = std::io::Cursor::new(executable_output)
                                    .lines()
                                    .map_while(Result::ok).skip_while(|line| line.starts_with("assertion")).find(|line| line.starts_with(" right:")).unwrap_or_default();


                                 let line = format!("{line}   '{left}' '{right}'");
                                    #[allow(clippy::let_and_return)]
                                    line
                                } else {
                                    line
                                };
                                // check if the backtrace mentiones "main.rs", this probably means the panic happened in our program and not directly in std which is boring
                                if std::io::Cursor::new(executable_output)
                                    .lines()
                                    .map_while(Result::ok).any(|line| line.contains("main.rs")) {
                                    (line, ICEKind::Ice(Interestingness::Boring))
                                } else {
                                    (line, ICEKind::Ice(interestingness))
                                }
                            })
                        }
                } // miri

                Executable::ClippyFix | Executable::RustFix => {
                    // unfortunately, lines().filter.. isn't clone so we have to hack around :(

                    let normal_ice = std::io::Cursor::new(executable_output)
                        .lines()
                        .map_while(Result::ok)
                        .find(|line| {
                            keywords_generic_ice
                                .iter()
                                .any(|regex| regex.is_match(line)) || line.contains("left == right") || line.contains("left != right") 
                        })
                        .map(|line| {
                           // we found the line with for example "assertion failed: `(left == right)`" , but it would be nice to get some more insight what left and right is
                           if line.contains("is internal to the compiler or standard library") {
                            // internal feature are can be easily misused and crash rustc
                            internal_feature = true;
                        }

                            // we found the line with for example "assertion failed: `(left == right)`" , but it would be nice to get some more insight what left and right is
                            let line = if line.contains("left == right") || line.contains("left != right") {
                                let left = std::io::Cursor::new(executable_output)
                                    .lines()
                                    .map_while(Result::ok).skip_while(|line| line.starts_with("assertion")).find(|line| line.starts_with("  left:")).unwrap_or_default();

                                let right = std::io::Cursor::new(executable_output)
                                    .lines()
                                    .map_while(Result::ok).skip_while(|line| line.starts_with("assertion")).find(|line| line.starts_with(" right:")).unwrap_or_default();

                                let line = format!("{line}   '{left}' '{right}'");
                                line
                            } else {
                                line
                            };
                            (line, ICEKind::Ice(interestingness))});
                    // if we have encounter a "normal" ICE while running clippy --fix, this obv. takes precedece over failure to
                    // apply clippy suggestions
                    if normal_ice.is_some() {
                        return normal_ice;
                    }
                    // rustfix failed to do anything because different lints modified the same line, ignore this/don't report ICE
                    let mut lines = std::io::Cursor::new(executable_output)
                        .lines()
                        .map_while(Result::ok);
                    if lines.any(|line| line.contains("maybe parts of it were already replaced?")) {
                        return None;
                    }
                    let mut lines = std::io::Cursor::new(executable_output)
                        .lines()
                        .map_while(Result::ok);
                    // clippy fix failure

                    lines
                        .find(|line| {
                            keywords_clippyfix_failure
                                .iter()
                                .any(|regex| regex.is_match(line))
                        })
                        .map(|line| (line, ICEKind::RustFix))
                }
                Executable::Rustc
                | Executable::Clippy
                | Executable::Kani //@FIXME
                | Executable::RustAnalyzer
                | Executable::Cranelift
                | Executable::Rustdoc
                | Executable::Rustfmt
                | Executable::RustcCodegenGCC => {
                    let mut double_ice = false;
                    let ice = lines
                        // collect all lines which might be ICE messages
                        .filter(|line| {
                            let is_double_ice =  keywords_double_panic_ice.iter().any(|kw| line.contains(kw));
                            if is_double_ice { double_ice = true }

                            keywords_generic_ice
                                .iter()
                                .any(|regex|
                                     regex.is_match(line)) || is_double_ice
                                    // assertion failure
                                     || line.contains("left == right") || line.contains("left != right") 
                        })
                        // bonus: if the line contains something like 
                        //  let _ = writeln!(err, "note: run with `RUST_BACKTRACE=1` \' keywords_double_panic_ice.iter().any(|kw| regex.contains(kw)
                        // do not yield it (skip it))
                        .filter(|line|  !(matches!(executable, Executable::Rustfmt) && (Regex::new("write.*RUST_BACKTRACE=").unwrap().is_match(line) || line.starts_with('-') || line.starts_with('+')) || line.contains("`RUST_BACKTRACE=")))
                        .map(|line| {
                            // we found the line with for example "assertion failed: `(left == right)`" , but it would be nice to get some more insight what left and right is
                            if line.contains("is internal to the compiler or standard library") {
                                // internal feature are can be easily misused and crash rustc
                                internal_feature = true;
                            }
                            if line.contains("left == right") || line.contains("left != right") {
                                let left = std::io::Cursor::new(executable_output)
                                    .lines()
                                    .map_while(Result::ok).skip_while(|line| line.starts_with("assertion")).find(|line| line.starts_with("  left:")).unwrap_or_default();

                                let right = std::io::Cursor::new(executable_output)
                                    .lines()
                                    .map_while(Result::ok).skip_while(|line| line.starts_with("assertion")).find(|line| line.starts_with(" right:")).unwrap_or_default();

                                    let line = format!("{line}   '{left}' '{right}'");

                                #[allow(clippy::let_and_return)]
                                line
                            } else if matches!(executable, Executable::Rustfmt) && line.contains("left behind trailing whitespace") {
                                String::from("error[internal]: left behind trailing whitespace")
                            } else {
                                 line
                            }})
                        // get the lonest ICE line 
                        .max_by_key(|line| {
                            // EXCEPTION: "error: internal compiler error: no errors encountered even though `delay_span_bug` issued" is usually longer than the actual ice line, so artifically decrease weight for this case
                            if delay_span_bug_regex.is_match(line) {
                                "internal compiler error".len()
                            } else {
                             line.len()
                            }
                        }
                        )
                        .map(|line| if double_ice {  (line, ICEKind::DoubleIce) } else { (line, ICEKind::Ice(interestingness)) });
                    if ice.is_some() {
                        ice
                    } else {
                        None
                    }
                }
            }
        }).map(|tup| {
            // if we deal with internal features, remap icekind::Interestingness:: to Boring
            let (txt, icekind) = tup;

            if internal_feature {
            let new_icekind = match icekind {
                ICEKind::Ice(_) => ICEKind::Ice(Interestingness::Boring),
                other => other,
            };

            (txt, new_icekind)
        } else {
            (txt, icekind)
        }


        })
}

pub(crate) fn run_random_fuzz(executable: Executable, global_tempdir_path: &PathBuf) -> Vec<ICE> {
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

            let filename = format!("icemaker_{num}.rs");
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
                        *compiler_flags,
                        &[],
                        false,
                        &counter,
                        LIMIT * RUSTC_FLAGS.len(),
                        false,
                        global_tempdir_path,
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
                    global_tempdir_path,
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
                        .map(|s| format!("{s} "))
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
pub(crate) fn run_space_heater(
    executable: Executable,
    chain_order: usize,
    global_tempdir_path: &PathBuf,
) -> Vec<ICE> {
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

            let filename = format!("icemaker_{num}.rs");
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
                        *compiler_flags,
                        &[],
                        false,
                        &counter,
                        LIMIT * RUSTC_FLAGS.len(),
                        false,
                        global_tempdir_path,
                    )
                }),
                _ => ICE::discover(
                    &path,
                    &exec_path,
                    &executable,
                    [&""],
                    &[],
                    false,
                    &counter,
                    LIMIT * RUSTC_FLAGS.len(),
                    false,
                    global_tempdir_path,
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
                        .map(|s| format!("{s} "))
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

    println!("converting to text ({} entries)", stdout.len());

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

    objects
        .par_iter()
        .map(|obj| {
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
            let dir = format!("{first}{second}");
            let file_path = format!("{}/{}.rs", &dir, obj);
            (text, file_path, dir)
        })
        .filter(|(_text, file_path, _dir)|
        //skip files that already exist
         !PathBuf::from(file_path).exists())
        .for_each(|(text, file_path, dir)| {
            std::fs::create_dir_all(dir).expect("failed to create directories");
            std::fs::write(file_path, text).expect("failed to write file");
        })
}

// try to sort files into their original directories
fn codegen_git_original_dirs() {
    println!("querying blobs");
    let stdout = std::process::Command::new("git")
        .arg("rev-list")
        .arg("--objects")
        .arg("--all")
        .output()
        .expect("git rev-list failed")
        .stdout;

    println!("converting to text ({} entries)", stdout.len());

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
        // only interested in rust files
        .filter(|line| line.ends_with(".rs"))
        // if we have more than 2 words, skip (skip paths witl spaces them because my shitty parsing does not handle that :D
        .filter(|line| line.chars().filter(|c| c == &' ').count() == 1)
        // since we filtered for .rs$, we should always encounter <hash> <path>
        .map(|line| -> String {
            let mut split: std::str::SplitWhitespace<'_> = line.split_whitespace();
            let hash = split.next().unwrap();
            let path = split.next().unwrap();
            assert_eq!(split.next(), None); // no third token

            // remove the .rs
            let path_without_extension = &path[0..path.len() - ".rs".len()];
            // make it
            // eb6c6f8f12a6d6db38bcfa741036d9622fad6c89 path/to/file_eb6c6f8f12a6d6db38bcfa741036d9622fad6c89.rs
            format!("{hash} {path_without_extension}_{hash}.rs")
        })
        .collect::<Vec<String>>();

    /*
    eb6c6f8f12a6d6db38bcfa741036d9622fad6c89 path/to/file_<hash>.rs
    fd0fd35ca74b281eb4753bc44d2f36583fefbca0 / file_<hash>.rs
    */

    objects.par_iter().for_each(|line| {
        //  eb6c6f8f12a6d6db38bcfa741036d9622fad6c89 path/to/file_<hash>.rs
        let mut split: std::str::SplitWhitespace<'_> = line.split_whitespace();
        let hash = split.next().unwrap();
        let path = split.next().unwrap();

        let obj = hash;

        let stdout = std::process::Command::new("git")
            .arg("cat-file")
            .arg("-p")
            .arg(obj)
            .output()
            .expect("git cat-file -p <obj> failed")
            .stdout;
        // the file content
        let text = String::from_utf8(stdout).unwrap();
        let path_path = PathBuf::from(path);
        let file_path = path_path.file_name().unwrap();
        let dir = path_path.parent().unwrap();

        if !(path.starts_with("..") || path.starts_with('/')) {
            // try to create the original directory the file was in
            if !PathBuf::from(file_path).exists() {
                std::fs::create_dir_all(dir).expect("failed to create directories");
            }

            let final_path = format!("{}/{}", dir.display(), file_path.to_str().unwrap());

            std::fs::write(&final_path, text)
                .expect(&format!("failed to write to file: '{final_path}'"));
        } else {
            eprintln!(
                "not writing file {}",
                format!("{}/{}", dir.display(), file_path.to_str().unwrap())
            );
        }
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
        std::fs::create_dir_all(format!("{first}/{second}")).expect("failed to create directories");
        std::fs::write(format!("{first}/{second}/{obj}.rs"), text).expect("failed to write file");
    })
}

// https://github.com/langston-barrett/tree-splicer
fn codegen_tree_splicer() {
    // notes: it seems to be optimal to fuzz with data from a single file only (no extenral files)
    // avoid undeclared symbols etc

    /*   use tree_sitter::{Language, Parser, Tree};
    use tree_sitter_rust;
    use tree_splicer::splice::{splice, Config}; */

    let root_path = std::env::current_dir().expect("no cwd!");

    let files = WalkDir::new(root_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|f| f.path().extension() == Some(OsStr::new("rs")))
        .map(|f| f.path().to_owned())
        .filter(|pb| !ignore_file_for_splicing(pb))
        .collect::<Vec<PathBuf>>();

    // dir to put the files in
    std::fs::create_dir("icemaker").expect("could not create icemaker dir");

    /*    let mut parser = Parser::new();
       // rust!
       parser.set_language(tree_sitter_rust::language()).unwrap();

       let hmap = files
           .iter()
           .map(|p| (p, std::fs::read_to_string(&p).unwrap_or_default()))
           .map(|(p, file_content)| {
               parser
                   .parse(&file_content, None)
                   .map(|t| (p, file_content, t))
           })
           .flatten()
           .map(|(path, file_content, tree)| {
               (
                   path.display().to_string(),
                   (file_content.into_bytes(), tree),
               )
           })
           .collect::<HashMap<String, (Vec<u8>, Tree)>>();
    */
    files
        .par_iter()
        .map(|path| {
            //  eprintln!("{}", path.display());
            // fuzz_tree_splicer::splice_file(&hmap)
            fuzz_tree_splicer::splice_file(path)
        })
        .flatten()
        .for_each(|file_content| {
            let mut hasher = Sha256::new();
            hasher.update(&file_content);
            let h = hasher.finalize();
            let hash = format!("{:X}", h);

            let mut file = std::fs::File::create(format!("icemaker/{hash}.rs"))
                .expect("could not create file");
            file.write_all(file_content.as_bytes())
                .expect("failed to write to file");
        });
}

// same but do not restrict fuzzing input set to a single file
fn codegen_tree_splicer_omni() {
    // run it on each dir
    // for i in `find .  -type d ` ; do ; echo $i ;  cd $i ; prlimit --noheadings --cpu=300  ~/vcs/github/icemaker/target/release/icemaker --codegen-splice-omni ; cd   ~/vcs/github/rust_codegen_grouped  ; done

    use std::collections::HashMap;
    use tree_sitter::{Parser, Tree};

    let root_path = std::env::current_dir().expect("no cwd!");

    println!("collecting files..");
    // files we use as dataset
    let files = WalkDir::new(root_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|f| f.path().extension() == Some(OsStr::new("rs")))
        .map(|f| f.path().to_owned())
        .filter(|pb| !ignore_file_for_splicing(pb))
        .collect::<Vec<PathBuf>>();

    // dir to put the files in
    std::fs::create_dir("icemaker_omni").expect("could not create icemaker_omni dir");

    let mut parser = Parser::new();
    // rust!
    parser.set_language(tree_sitter_rust::language()).unwrap();

    println!("parsing {} files..", files.len());

    // read all fhe files
    let hmap = files
        .iter()
        .map(|p| (p, std::fs::read_to_string(p).unwrap_or_default()))
        .filter_map(|(p, file_content)| {
            parser
                .parse(&file_content, None)
                .map(|t| (p, file_content, t))
        })
        .map(|(path, file_content, tree)| {
            (
                path.display().to_string(),
                (file_content.into_bytes(), tree),
            )
        })
        .collect::<HashMap<String, (Vec<u8>, Tree)>>();

    //

    println!("codegenning...");

    let total = hmap.len();
    let counter: AtomicUsize = 0.into();

    files
        .par_iter()
        .map(|_path| {
            //  eprintln!("{}", path.display());
            // fuzz_tree_splicer::splice_file(&hmap)
            fuzz_tree_splicer::splice_file_from_set(/* path , */ &hmap)
        })
        .flatten()
        .for_each(|file_content| {
            let mut hasher = Sha256::new();
            hasher.update(&file_content);
            let h = hasher.finalize();
            let hash = format!("{:X}", h);

            PRINTER.log(PrintMessage::Progress {
                index: counter.load(Ordering::SeqCst),
                total_number_of_files: total,
                file_name: String::new(),
            });

            counter.fetch_add(1, Ordering::SeqCst);

            let mut file = std::fs::File::create(format!("icemaker_omni/{hash}.rs"))
                .expect("could not create file");
            file.write_all(file_content.as_bytes())
                .expect("failed to write to file");
        });
}

fn tree_splice_incr_fuzz(global_tempdir_path: &Path) {
    // 1) read single source file
    // 1.5) make sure it compiles //
    // 2) mutate it //
    // 3) save mutation to disk // skipped
    // 4) make sure mutation compiles //
    // 5) run rustc on orig file and then on the mutation while sharing incr cache

    let root_path = std::env::current_dir().expect("no cwd!");

    // get a list of source files
    let files = WalkDir::new(root_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|f| f.path().extension() == Some(OsStr::new("rs")))
        .map(|f| f.path().to_owned())
        .filter(|p| {
            std::fs::read_to_string(p)
                .unwrap_or_default()
                .lines()
                .count()
                < 1000
        })
        .collect::<Vec<PathBuf>>();

    let ices = files
        .par_iter()
        .map(|orig_file| fuzz_icr_file(orig_file, global_tempdir_path))
        .flatten()
        .collect::<Vec<_>>();
    dbg!(ices);
    return;

    // dir to put the files in

    // take a single original file, mutate it and incr-check original + a mutation each, return vec of ICEs
    fn fuzz_icr_file(
        original_file_path: &PathBuf,
        global_tempdir_path: &Path,
    ) -> Vec<(String, ICEKind)> {
        // content of the original file
        let file_content =
            std::fs::read_to_string(original_file_path).expect("failed to read file");

        let tempdir = TempDir::new_in(global_tempdir_path, "rustc_testrunner_tmpdir").unwrap();
        let tempdir_path = tempdir.path();

        // if the original file does not compile, bail out
        if !file_compiles_from_string(
            &file_content,
            &Executable::Rustc.path(),
            &tempdir_path.to_path_buf(),
        ) {
            return Vec::new();
        }
        // get mutations of a single file
        let mutations = fuzz_tree_splicer::splice_file(original_file_path).into_par_iter();

        mutations
            .filter(|mutation| {
                // make sure the modified file compiles
                file_compiles_from_string(
                    mutation,
                    &Executable::Rustc.path(),
                    &tempdir_path.to_path_buf(),
                )
            })
            .map(|mutation| {
                let (output, _cmd_str, _args) = run_rustc_incremental_with_two_files(
                    &Executable::Rustc.path(),
                    original_file_path.as_path(),
                    &mutation,
                    &global_tempdir_path.to_path_buf(),
                )
                .unwrap();
                let maybeice = find_ICE_string(original_file_path, &Executable::Rustc, output);
                if maybeice.is_some() {
                    eprintln!("{}", original_file_path.display());
                    dbg!(&maybeice);
                    eprintln!(
                        "!!!\n\nINCR ICE\n\n orig:\n  {file_content} \n\n mutation:\n {mutation}"
                    );
                }
                maybeice
            })
            .flatten()
            .collect::<Vec<_>>()
    }
}

const REDUCTION_DIR: &str = "icemaker_reduced";

fn reduce_all(global_tempdir_path: &Path) {
    // todo handle all Executables

    // reduce code using $Executable,
    // make sure to prlimit that // check
    // try to fmt the mcve
    // ok => save fmttd
    // rustfmt needs --edition from ice.flags
    //         => try to reduce further?
    // not ok (ice gone after fmt) => save original

    // if we have mvce and flags, run cargo-bisect rustc IFF the Executable is shipped by rustc (clippy, rustc, rustdoc, miri?)

    // for some flags we could have an "all out" reduction and an
    // "rustc file.rs" && rustc $flags file.rs
    // reduction which makes sure the file still compiles under some condition

    // put both versions in the Report

    let root_path = std::env::current_dir().expect("no cwd!");
    // parse the reported ICEs
    let errors_json = root_path.join("errors.json");
    let ices: Vec<ICE> = if errors_json.exists() {
        let read = match std::fs::read_to_string(&errors_json) {
            Ok(content) => content,
            Err(_) => panic!("failed to read '{}'", errors_json.display()),
        };
        match serde_json::from_str(&read) {
            Ok(previous_errors) => previous_errors,
            Err(e) => {
                // this can happen if we for example change the representation of Ice so that that the previous file is no longer compatible with the new format
                eprintln!("Failed to parse errors.json, is it a json file?");
                eprintln!("original error: '{e:?}'");
                Vec::new()
            }
        }
    } else {
        // we don't have a file, start blank
        Vec::new()
    };

    std::fs::create_dir_all(REDUCTION_DIR).expect("could not create './icemaker_reduced/' dir");

    /*
    let ices_cloned = ices.clone();


    let debug_assertions = ices_cloned
        .iter()
        .find(|ice| ice.error_reason.contains("Span"))
        .is_some(); // must not be empty..
    */

    ices.into_iter().for_each(|ice| {
        reduce_ice_code(ice, global_tempdir_path);
    })
}

// minimize ICE code
pub(crate) fn reduce_ice_code(ice: ICE, global_tempdir_path: &Path) {
    let reduction_start_time = Instant::now();

    let file = &ice.file;
    // if we run inside a tempdir, we need an absolute path, because the file is not copied into the tempdir
    let file = &file.canonicalize().expect("file canonicalizsation failed");
    let flags = &ice.args;
    let executable = &ice.executable;
    let bin = executable.path();
    let kind = ice.kind.clone();

    let tempdir = TempDir::new_in(global_tempdir_path, "icemaker_reducing_tempdir").unwrap();
    let tempdir_path = tempdir.path();

    if matches!(executable, Executable::Rustc)
        && matches!(kind, ICEKind::Ice(_))
        &&
    // skip OOMs which treereduce cant really handle
    ! ice.error_reason.contains("allocating stack failed")
    {
        eprintln!("{}", ice.to_printable());

        /*  eprintln!("------------------original");
        eprintln!(
            "{}",
            std::fs::read_to_string(file).unwrap_or("FAILURE TO READ ICE FILE".into())
        );
        */

        let mut trd = std::process::Command::new("prlimit");
        trd.arg(format!("--as={}", 3076_u32 * 1000_u32 * 1000_u32)) // 3 gb of ram
            .arg(format!("--cpu=120")) //  2 mins
            .arg("treereduce-rust");
        trd.args([
            "--quiet",
            "--passes=10",
            "--min-reduction=10",
            "--interesting-exit-code=101",
            "--on-parse-error",
            "ignore",
            "--output", // output to stdout
            "-",
        ]);

        trd.arg("--source");
        trd.arg(file);

        trd.arg("--");
        // we also need to run the rustc that treereduce-rust launches inside prlimit to not blow up the system

        trd.args([
            "prlimit",
            "--noheadings",
            &format!("--as={}", 3076_u32 * 1000_u32 * 1000_u32),
            "--cpu=60",
        ]);
        trd.arg(&bin);

        if !flags.is_empty() {
            trd.args(flags);
        }
        trd.arg("@@.rs");
        trd.current_dir(tempdir_path);
        let output = trd.output().unwrap();
        let reduced_file = String::from_utf8_lossy(&output.stdout).to_string();
        let reduced_file_clone = reduced_file.clone();
        /*
              eprintln!("---------------------------reduced");
                    eprintln!("{reduced_file}");

                    eprintln!("---------------------------");
        */
        // find possible edition flags inside the rustcflags which we will also need to pass to rustfmt?
        let mut fmt = std::process::Command::new("rustfmt")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .current_dir(tempdir_path)
            .arg("--edition=2021")
            .spawn()
            .expect("Failed to spawn rustfmt process");

        let mut stdin = fmt.stdin.take().expect("Failed to open stdin");
        std::thread::spawn(move || {
            stdin
                .write_all(reduced_file_clone.as_bytes())
                .expect("Failed to write to stdin");
        });

        let output = fmt.wait_with_output().expect("Failed to read stdout");

        // if rustfmt failed, save the original file
        let reduced_fmt_file = if output.status.success() {
            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            reduced_file
        };

        let analysis = Analysis {
            ice: ice.clone(),
            mvce: reduced_fmt_file,
        };
        let seconds_elapsed = reduction_start_time.elapsed().as_secs();

        eprintln!("reduction took {seconds_elapsed} seconds...........formatted:");
        eprintln!("{}", analysis.mvce);
        eprintln!("\n\n\n");
        // write reduced file to disk
        let dir = PathBuf::from(REDUCTION_DIR);
        // REDUCTION_DIR/filename.rs
        // @TODO do not overwrite already reduced files here if flags are different
        let path_reduced_file: PathBuf =
            dir.join(file.file_name().expect("could not get filename"));
        std::fs::write(path_reduced_file, analysis.mvce).expect("could not write file content");
    }
}

pub(crate) fn reduce_ice_code_to_string(ice: ICE, global_tempdir_path: &Path) -> String {
    let reduction_start_time: Instant = Instant::now();

    let file = &ice.file;
    // if we run inside a tempdir, we need an absolute path, because the file is not copied into the tempdir
    let file = &file.canonicalize().expect("file canonicalizsation failed");
    let flags = &ice.args;
    let executable = &ice.executable;
    let bin = executable.path();
    let kind = ice.kind.clone();

    let tempdir = TempDir::new_in(global_tempdir_path, "icemaker_reducing_tempdir").unwrap();
    let tempdir_path = tempdir.path();

    if matches!(executable, Executable::Rustc) && matches!(kind, ICEKind::Ice(_))
        || matches!(kind, ICEKind::DoubleIce)
        &&
    // skip OOMs which treereduce cant really handle
    ! ice.error_reason.contains("allocating stack failed")
    {
        eprintln!("{}", ice.to_printable());

        /*  eprintln!("------------------original");
        eprintln!(
            "{}",
            std::fs::read_to_string(file).unwrap_or("FAILURE TO READ ICE FILE".into())
        );
        */

        let mut trd = std::process::Command::new("prlimit");
        trd.arg(format!("--as={}", 3076_u32 * 1000_u32 * 1000_u32)) // 3 gb of ram
            .arg(format!("--cpu=120")) //  2 mins
            .arg("treereduce-rust");
        trd.args([
            "--quiet",
            "--passes=10",
            "--min-reduction=10",
            "--interesting-exit-code=101",
            "--on-parse-error",
            "ignore",
            "--output", // output to stdout
            "-",
        ]);

        trd.arg("--source");
        trd.arg(file);

        trd.arg("--");
        // we also need to run the rustc that treereduce-rust launches inside prlimit to not blow up the system

        trd.args([
            "prlimit",
            "--noheadings",
            &format!("--as={}", 3076_u32 * 1000_u32 * 1000_u32),
            "--cpu=60",
        ]);
        trd.arg(&bin);

        if !flags.is_empty() {
            trd.args(flags);
        }
        trd.arg("@@.rs");
        trd.current_dir(tempdir_path);
        let output = trd.output().unwrap();
        let reduced_file = String::from_utf8_lossy(&output.stdout).to_string();
        let reduced_file_clone = reduced_file.clone();
        /*
              eprintln!("---------------------------reduced");
                    eprintln!("{reduced_file}");

                    eprintln!("---------------------------");
        */
        // find possible edition flags inside the rustcflags which we will also need to pass to rustfmt?
        let mut fmt = std::process::Command::new("rustfmt")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .current_dir(tempdir_path)
            .arg("--edition=2021")
            .spawn()
            .expect("Failed to spawn rustfmt process");

        let mut stdin = fmt.stdin.take().expect("Failed to open stdin");
        std::thread::spawn(move || {
            stdin
                .write_all(reduced_file_clone.as_bytes())
                .expect("Failed to write to stdin");
        });

        let output = fmt.wait_with_output().expect("Failed to read stdout");

        // if rustfmt failed, save the original file
        let reduced_fmt_file = if output.status.success() {
            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            reduced_file
        };

        return reduced_fmt_file;
    }
    String::from("ERROR in icemaker while reducing file with treereduce")
}

#[derive(Debug, Clone)]
struct Analysis {
    #[allow(unused)]
    ice: ICE,
    mvce: String,
}

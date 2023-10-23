use std::{io::Write, path::PathBuf};

use clap::Parser;
use colored::Colorize;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tempdir::TempDir;

use crate::{library::Args, reduce_ice_code_to_string, run_commands::prlimit_run_command};

// represents a crash that we found by running an `Executable` with a set of flags on a .rs file
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct ICE {
    // what release channel did we crash on?
    pub regresses_on: Regression,
    // do we need any special features for that ICE?
    pub needs_feature: bool,
    // file that reproduces the ice
    pub file: PathBuf,
    // path to the rustc binary
    //    executable: String,
    // args that are needed to crash rustc
    pub args: Vec<String>,
    // part of the error message
    pub error_reason: String,
    // ice message
    pub ice_msg: String,
    // the full command that we used to reproduce the crash
    //cmd: String,
    pub executable: Executable,
    // what kind of ice is this?
    pub kind: ICEKind,
}

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub enum ICEKind {
    // something crashed
    Ice(Interestingness),
    // miri found undefined behaviour
    Ub(UbKind),
    // program didn't terminate in time
    Hang(u64), // time in seconds
    OOM,
    // clippy / rustc lint siggestions failed to apply
    RustFix,
    // [type error] in output
    TypeError,
    // double, ice while ice via -Ztreat-err-as-bug
    DoubleIce,
    // rustfmt failed to format the code
    RustfmtFailure,
}

impl Default for ICEKind {
    fn default() -> Self {
        Self::Ice(Interestingness::Interesting)
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy, Eq, PartialEq)]
pub enum Interestingness {
    #[default]
    Interesting,
    Boring,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub enum UbKind {
    #[default]
    Interesting,
    Uninteresting,
}

// is this actually used?
impl std::fmt::Display for ICE {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        write!(
            f,
            "'{:?} {} {}' ICEs on {}, {} with: {} / '{}'",
            self.executable,
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

pub(crate) type ICEDisplay = String;

impl ICE {
    // print a ICE to stdout or something
    pub(crate) fn to_printable(&self, global_tempdir: &PathBuf) -> ICEDisplay {
        let kind = match self.kind {
            ICEKind::Ice(Interestingness::Interesting) => "ICE".red(),
            ICEKind::Ice(Interestingness::Boring) => "ice".normal(),
            ICEKind::Ub(UbKind::Interesting) => "UB".green(),
            ICEKind::Ub(UbKind::Uninteresting) => "ub".normal(),
            ICEKind::Hang(_) => "HANG".blue(),
            ICEKind::OOM => "OOM".red(),
            ICEKind::RustFix => "RustFix".yellow(),
            ICEKind::TypeError => "TypeError".yellow(),
            ICEKind::DoubleIce => "DoubleICE".red(),
            ICEKind::RustfmtFailure => "Fmt".yellow(),
        };

        let flags = self.args.join(" ");

        // HACK
        // also log the ICE to disk here since its probably most convenient at this place in time/code

        // @FIXME this is disabled because its not tempdir'd properly leading to disk trashing
        // let report: Report = self.clone().into_report(global_tempdir);
        //  report.to_disk();

        format!(
            "{kind}: {:?} {} '{flags}' '{}', '{}'",
            self.executable,
            self.file.display(),
            self.ice_msg.normal(),
            self.error_reason.normal()
        )
    }
}

/*
fn _run_treereduce(ice: &ICE) {
    let file = ice.file;
    let original_code = std::fs::read_to_strinaggregateg(&original_path).unwrap_or("<error>".into());
    let flags = self.args.clone().join(" ");
    let executable_bin = &self.executable.path();
    let prl_output = prlimit_run_command(&mut cmd).expect("prlimit process failed");
} */

#[allow(unused)]
#[derive(Debug, Clone)]
pub(crate) struct Report {
    ice: ICE,
    data: String,
}

impl ICE {
    #[allow(unused)]
    pub(crate) fn into_report(self, global_tempdir_path: &PathBuf) -> Report {
        let ice = &self;

        let mvce_string: String = reduce_ice_code_to_string(ice.clone(), global_tempdir_path);
        //dbg!(&mvce_string);

        //unreachable!("DO USE TMPDIR HERE!");
        let tempdir =
            TempDir::new_in(global_tempdir_path, "rustc_testrunner_tmpdir_reporting").unwrap();
        let tempdir_path = tempdir.path().display();

        let original_path = ice.file.clone();
        let original_path = original_path.canonicalize().unwrap();
        let original_path_display = original_path.display();
        let original_code = std::fs::read_to_string(&original_path).unwrap_or("<error>".into());

        // when fetching the Executable output, run Executable against the mvce inseatead of the original file
        // need to save the mvce to disk for this; put it into the tempdir
        let mvce_file_path = tempdir.path().join("mvce.rs");
        let mvce_display = mvce_file_path.display();
        let mut mvce_file = std::fs::File::create(&mvce_file_path)
            .expect(&format!("failed to create mvce file '{mvce_display}'"));
        write!(mvce_file, "{}", mvce_string)
            .expect(&format!("failed to write mvce '{mvce_display}'"));

        let flags = ice
            .args
            .clone()
            .into_iter()
            .filter(|flag| {
                !["-ooutputfile".to_string(), "-Zdump-mir-dir=dir".to_string()].contains(&flag)
            })
            .collect::<Vec<_>>();
        let flags = flags.join(" ");

        //let executable = &self.executable.clone();
        let executable_bin = &ice.executable.path();
        let mut cmd = std::process::Command::new(executable_bin);
        cmd.args(&ice.args);
        cmd.arg(&mvce_file_path);
        cmd.current_dir(tempdir_path.to_string());

        let prl_output = prlimit_run_command(&mut cmd).expect("prlimit process failed");
        //  let output_stderr = String::from_utf8(prl_output.stdout).unwrap();
        let output_stdout = String::from_utf8(prl_output.stderr).unwrap();

        let version_output: String = if let Ok(output) = std::process::Command::new(executable_bin)
            .arg("--version")
            .arg("--verbose")
            .output()
        {
            String::from_utf8(output.stdout).unwrap()
        } else if let Ok(output_verbose) = std::process::Command::new(executable_bin)
            .arg("--version")
            .output()
        {
            String::from_utf8(output_verbose.stdout).unwrap()
        } else {
            "<failed to get version>".to_string()
        };

        // if we failed to reduce the originl code, don't print original and snippet
        let snippet = if mvce_string == original_code {
            format!(
                "snippet:
````rust
{original_code}
````"
            )
        // if we have a very long original snippet. collapse it
        } else if original_code.len() > 999 {
            format!(
                "auto-reduced (treereduce-rust):
````rust
{mvce_string}
````

<details><summary><strong>original code</strong></summary>
<p>

original:
````rust
{original_code}
````
</p>
</details>"
            )
        } else {
            format!(
                "auto-reduced (treereduce-rust):
````rust
{mvce_string}
````

original:
````rust
{original_code}
````"
            )
        };

        let data = format!(
            "
File: {original_path_display}

{snippet}

Version information
````
{version_output}
````

Command:
`{executable_bin} {flags}`

<!--
Include a backtrace in the code block by setting `RUST_BACKTRACE=1` in your
environment. E.g. `RUST_BACKTRACE=1 cargo build`.
-->
<details><summary><strong>Program output</strong></summary>
<p>

```
{output_stdout}
```

</p>
</details>


"
        );

        Report {
            ice: ice.clone(),
            data,
        }
    }
}

pub(crate) static REPORTS_DIR: Lazy<PathBuf> = Lazy::new(|| {
    let system_temp_dir = std::env::temp_dir();
    let reports_dir = system_temp_dir.join("icemaker_reports");
    // ':' in paths may not work under windows, yolo!
    let date = chrono::offset::Local::now()
        .format("%Y-%m-%d_%H:%M:%S")
        .to_string();
    reports_dir.join(date)
});

#[allow(unused)]
impl Report {
    pub(crate) fn _print(&self) {
        println!("{}", self.data);
    }

    // save a report into /tmp/ for inspection while icemaker is still running
    pub(crate) fn to_disk(&self) {
        // only write ices and ub to disk for now
        if let ICEKind::Ice(..) | ICEKind::Ub(..) | ICEKind::DoubleIce = self.ice.kind {
            // we want these
            /*
            eprintln!("reported!");
            eprint!("{}", self.data);
             */
        } else {
            return;
        }

        // should just print Rustc, Miri, Clippy etc...
        // we need to append this so that if the miri and rustdoc crash on the file, we don't overwrite previous results :/
        let executable = format!("{:?}", self.ice.executable);

        let reports_dir = REPORTS_DIR.to_owned();
        if !PathBuf::from(&reports_dir).exists() {
            std::fs::create_dir_all(&reports_dir).expect("failed to create icemaker reports dir!");
        }

        let display = self
            .ice
            .file
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .to_string();
        let mut file_on_disk = display.to_string().replace(['/', '\\'], "_");
        file_on_disk.push('_');
        file_on_disk.push_str(&executable);
        let mut file_on_disk = file_on_disk.replace(".rs", "");
        file_on_disk.push_str(".md");

        let report_file_path = reports_dir.join(file_on_disk);

        dbg!(&report_file_path);
        eprintln!();

        //  FIXME file might already exist
        let mut file = std::fs::File::create(report_file_path)
            .expect("report.to_disk() failed to create file");
        file.write_all(self.data.as_bytes())
            .expect("failed to write report");
    }
}

// in what channel a regression is first noticed?
#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub enum Regression {
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

        write!(f, "{s}")
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
pub enum Executable {
    Rustc,
    Clippy,
    Rustdoc,
    RustAnalyzer,
    Rustfmt,
    Miri,
    // extra:
    // icemaker --local-debug-assertions --cranelift-local  --expensive-flags
    Cranelift,
    ClippyFix,
    RustFix,
    Kani,
    // https://github.com/rust-lang/rustc_codegen_gcc
    RustcCodegenGCC,
}

static LOCAL_DEBUG_ASSERTIONS: Lazy<bool> = Lazy::new(|| Args::parse().local_debug_assertions);

impl Executable {
    pub fn path(&self) -> String {
        match self {
            Executable::Rustc => {
                if *LOCAL_DEBUG_ASSERTIONS {
                    String::from(
                        "/home/matthias/.rustup/toolchains/local-debug-assertions/bin/rustc",
                    )
                } else {
                    let mut p = home::rustup_home().unwrap();
                    p.push("toolchains");
                    p.push("master");
                    p.push("bin");
                    p.push("rustc");
                    p.display().to_string()
                }
            }
            Executable::Clippy => {
                if *LOCAL_DEBUG_ASSERTIONS {
                    String::from(
                        "/home/matthias/.rustup/toolchains/local-debug-assertions/bin/clippy-driver",
                    )
                } else {
                    let mut p = home::rustup_home().unwrap();
                    p.push("toolchains");
                    p.push("master");
                    p.push("bin");
                    p.push("clippy-driver");
                    p.display().to_string()
                }
            }
            Executable::ClippyFix => {
                if *LOCAL_DEBUG_ASSERTIONS {
                    String::from(
                        "/home/matthias/.rustup/toolchains/local-debug-assertions/bin/clippy-driver",
                    )
                } else {
                    let mut p = home::rustup_home().unwrap();
                    p.push("toolchains");
                    p.push("master");
                    p.push("bin");
                    p.push("clippy-driver");
                    p.display().to_string()
                }
            }

            Executable::RustFix => {
                if *LOCAL_DEBUG_ASSERTIONS {
                    String::from(
                        "/home/matthias/.rustup/toolchains/local-debug-assertions/bin/rustc",
                    )
                } else {
                    let mut p = home::rustup_home().unwrap();
                    p.push("toolchains");
                    p.push("master");
                    p.push("bin");
                    p.push("rustc");
                    p.display().to_string()
                }
            }
            Executable::Rustdoc => {
                if *LOCAL_DEBUG_ASSERTIONS {
                    String::from(
                        "/home/matthias/.rustup/toolchains/local-debug-assertions/bin/rustdoc",
                    )
                } else {
                    let mut p = home::rustup_home().unwrap();
                    p.push("toolchains");
                    p.push("master");
                    p.push("bin");
                    p.push("rustdoc");
                    p.display().to_string()
                }
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
                if *LOCAL_DEBUG_ASSERTIONS {
                    String::from(
                        "/home/matthias/.rustup/toolchains/local-debug-assertions/bin/rustfmt",
                    )
                } else {
                    let mut p = home::rustup_home().unwrap();
                    p.push("toolchains");
                    p.push("master");
                    p.push("bin");
                    p.push("rustfmt");
                    p.display().to_string()
                }
            }
            Executable::Miri => {
                // note: this is actually not what we run in the end, we need to run "cargo miri test"
                let mut p = home::rustup_home().unwrap();
                p.push("toolchains");
                p.push("x86_64-unknown-linux-gnu");
                p.push("bin");
                p.push("miri");
                p.display().to_string()
            }

            Executable::Cranelift => {
                String::from("/home/matthias/.rustup/toolchains/local-debug-assertions/bin/rustc")
            }
            Executable::Kani => "kani".into(),
            // env vars + -Zcodegen-backend= to the rest of the stuff, similar to cranelift
            Executable::RustcCodegenGCC => "rustc".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ice::Executable;

    #[test]
    fn exec_rustc() {
        let ex = &Executable::Rustc.path();
        assert!(ex.contains("rustc"));
        assert!(ex.contains("master"));
    }

    #[test]
    fn exec_clippy() {
        let ex = &Executable::Clippy.path();
        assert!(ex.contains("master"));
        assert!(ex.contains("clippy-driver"));
    }

    #[test]
    fn exec_clippyfix() {
        assert_eq!(Executable::Clippy.path(), Executable::ClippyFix.path())
    }

    #[test]
    fn exec_rustdoc() {
        let ex = &Executable::Rustdoc.path();
        assert!(ex.contains("master"));
        assert!(ex.contains("rustdoc"));
    }

    #[test]
    fn exec_analyzer() {
        let ex = &Executable::RustAnalyzer.path();
        assert!(ex.contains("master"));
        assert!(ex.contains("rust-analyzer"));
    }

    #[test]
    fn exec_rustfmt() {
        let ex = &Executable::Rustfmt.path();
        assert!(ex.contains("master"));
        assert!(ex.contains("rustfmt"));
    }

    #[test]
    fn exec_miri() {
        let ex = &Executable::Miri.path();
        // not master toolchain, but nightly
        assert!(ex.contains("miri"));
    }
}

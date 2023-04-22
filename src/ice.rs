use std::path::PathBuf;

use clap::Parser;
use colored::Colorize;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::{library::Args, run_commands::prlimit_run_command};

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
    pub(crate) fn to_printable(&self) -> ICEDisplay {
        let kind = match self.kind {
            ICEKind::Ice(Interestingness::Interesting) => "ICE".red(),
            ICEKind::Ice(Interestingness::Boring) => "ice".normal(),
            ICEKind::Ub(UbKind::Interesting) => "UB".green(),
            ICEKind::Ub(UbKind::Uninteresting) => "ub".normal(),
            ICEKind::Hang(_) => "HANG".blue(),
            ICEKind::OOM => "OOM".red(),
            ICEKind::RustFix => "RustFix".yellow(),
            ICEKind::TypeError => "TypeError".yellow(),
        };

        let flags = self.args.join(" ");

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
    let original_code = std::fs::read_to_string(&original_path).unwrap_or("<error>".into());
    let flags = self.args.clone().join(" ");
    let executable_bin = &self.executable.path();
    let prl_output = prlimit_run_command(&mut cmd).expect("prlimit process failed");
} */

#[derive(Debug, Clone)]
pub(crate) struct Report {
    ice: ICE,
    data: String,
}

impl From<&ICE> for Report {
    fn from(ice: &ICE) -> Self {
        unreachable!("DO USE TMPDIR HERE!");
        
        let original_path = ice.file.clone();
        let original_path_display = original_path.display();
        let original_code = std::fs::read_to_string(&original_path).unwrap_or("<error>".into());
        let flags = ice.args.clone().join(" ");

        //let executable = &self.executable.clone();
        let executable_bin = &ice.executable.path();
        let mut cmd = std::process::Command::new(executable_bin);
        cmd.args(&ice.args);
        cmd.arg(&ice.file);

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

        let data = format!(
            "
File: {original_path_display}
````rust
{original_code}
````
Version information
````
{version_output}
````

Command:
`{executable_bin} {flags}`

Program output:
````
{output_stdout}
````
"
        );

        Report {
            ice: ice.clone(),
            data,
        }
    }
}

impl Report {
    pub(crate) fn print(&self) {
        println!("{}", self.data);
    }
}

impl ICE {
    pub(crate) fn to_disk(&self) {
        let original_path = self.file.clone();
        let original_path_display = original_path.display();
        let original_code = std::fs::read_to_string(&original_path).unwrap_or("<error>".into());
        let flags = self.args.clone().join(" ");

        //let executable = &self.executable.clone();
        let executable_bin = &self.executable.path();
        let mut cmd = std::process::Command::new(&executable_bin);
        cmd.args(&self.args);
        cmd.arg(&self.file);

        let prl_output = prlimit_run_command(&mut cmd).expect("prlimit process failed");
        //  let output_stderr = String::from_utf8(prl_output.stdout).unwrap();
        let output_stdout = String::from_utf8(prl_output.stderr).unwrap();

        let version_output: String = if let Ok(output) = std::process::Command::new(&executable_bin)
            .arg("--version")
            .arg("--verbose")
            .output()
        {
            String::from_utf8(output.stdout).unwrap()
        } else if let Ok(output_verbose) = std::process::Command::new(&executable_bin)
            .arg("--version")
            .output()
        {
            String::from_utf8(output_verbose.stdout).unwrap()
        } else {
            "<failed to get version>".to_string()
        };

        let text = format!(
            "
File: {original_path_display}
````rust
{original_code}
````
Version information
````
{version_output}
````

Command:
`{executable_bin} {flags}`

Program output:
````
{output_stdout}
````
"
        );

        let file_on_disk = original_path_display
            .to_string()
            .replace('/', "_")
            .replace("\\", "_");

        eprintln!("{text}");
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
    RustcCGClif,
    // extra:
    // icemaker --local-debug-assertions --cranelift-local  --expensive-flags
    CraneliftLocal,
    ClippyFix,
    RustFix,
    Kani,
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
            Executable::RustcCGClif => {
                String::from("/home/matthias/vcs/github/rustc_codegen_cranelift/dist/rustc-clif")
            }
            Executable::CraneliftLocal => {
                String::from("/home/matthias/.rustup/toolchains/local-debug-assertions/bin/rustc")
            }
            Executable::Kani => "kani".into(),
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

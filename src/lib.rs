use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub struct Args {
    pub clippy: bool,
    pub rustdoc: bool,
    pub analyzer: bool, // rla
    pub rustfmt: bool,
    pub silent: bool,
    pub threads: usize,
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

        write!(f, "{}", s)
    }
}

#[derive(PartialEq, Eq)]
pub enum Executable {
    Rustc,
    Clippy,
    Rustdoc,
    RustAnalyzer,
    Rustfmt,
}

impl Executable {
    pub fn path(&self) -> String {
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

// represents a crash
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

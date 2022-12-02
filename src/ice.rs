use std::path::PathBuf;

use clap::Parser;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::library::Args;

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
    // something crahed
    Ice,
    // miri found undefined behaviour
    Ub(UbKind),
    // program didn't terminate in time
    Hang,
    OOM,
    // clippy failed to apply fixes
    ClippyFix,
}

impl Default for ICEKind {
    fn default() -> Self {
        Self::Ice
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub enum UbKind {
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

#[derive(Debug, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub enum Executable {
    Rustc,
    Clippy,
    Rustdoc,
    RustAnalyzer,
    Rustfmt,
    Miri,
    RustcCGClif,
    ClippyFix,
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

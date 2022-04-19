use std::path::PathBuf;

use itertools::Itertools;
use serde::{Deserialize, Serialize};

pub struct Args {
    pub clippy: bool,
    pub rustdoc: bool,
    pub analyzer: bool, // rla
    pub rustfmt: bool,
    pub silent: bool,
    pub threads: usize,
    pub heat: bool, //spaceheater mode, try to catch ICEs from random code
    pub miri: bool,
    pub codegen: bool,
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
    Miri,
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
                //String::from("/home/matthias/vcs/github/rust_debug_assertions/build/x86_64-unknown-linux-gnu/stage1/bin/rustc")
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
            Executable::Miri => {
                // note: this is actually not what we run in the end, we need to run "cargo miri test"
                let mut p = home::rustup_home().unwrap();
                p.push("toolchains");
                p.push("x86_64-unknown-linux-gnu");
                p.push("bin");
                p.push("miri");
                p.display().to_string()
            }
        }
    }
}

// represents a crash
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

/// check whether a file uses features or not
pub fn uses_feature(file: &std::path::Path) -> bool {
    match std::fs::read_to_string(&file) {
        Ok(file) => file.contains("feature("),
        _ => {
            eprintln!("Failed to read '{}'", file.display());
            false
        }
    }
}

pub fn get_flag_combination(flags: &[&str]) -> Vec<Vec<String>> {
    // get the power set : [a, b, c] => [a], [b], [c], [a,b], [a,c], [b,c], [a,b,c]

    let mut combs = Vec::new();
    for numb_comb in 0..=flags.len() {
        let combinations = flags.iter().map(|s| s.to_string()).combinations(numb_comb);
        combs.push(combinations);
    }

    let combs: Vec<Vec<String>> = combs.into_iter().flatten().collect();

    // UPDATE: special cased in par_iter loop
    // add an empty "" flag so start with, in case an ice does not require any flags
    //   let mut tmp = vec![vec![String::new()]];
    //  tmp.extend(combs);
    let mut tmp = combs;
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
    //finally, sort by length
    tmp2.sort_by_key(|x| x.len());

    // we cant assert here when we remove redundant opt levels :/
    //    debug_assert_eq!(tmp2.iter().last().unwrap(), flags);

    tmp2
}

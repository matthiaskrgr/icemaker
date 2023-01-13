use clap::Parser;
use itertools::Itertools;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub(crate) struct Args {
    #[clap(short, short = 'c', long)]
    pub(crate) clippy: bool,

    #[clap(long = "cf", long = "clippy-fix")]
    pub(crate) clippy_fix: bool,

    #[clap(long = "rf", long = "rust-fix")]
    pub(crate) rust_fix: bool,

    #[clap(short = 'd', long)]
    pub(crate) rustdoc: bool,

    #[clap(short, long)]
    pub(crate) analyzer: bool, // rla

    #[clap(long)]
    pub(crate) rustfmt: bool,

    #[clap(short, long)]
    pub(crate) silent: bool,

    #[clap(short = 'H', long)]
    pub(crate) heat: bool, // spaceheater mode, try to catch ICEs from random codeu

    #[clap(short, long)]
    pub(crate) miri: bool,

    #[clap(long)]
    pub(crate) codegen: bool,

    #[clap(short, long)]
    pub(crate) incremental_test: bool,

    #[clap(long)]
    pub(crate) fuzz: bool,

    #[clap(long)]
    pub(crate) fuzz2: bool,

    #[clap(short, long)]
    pub(crate) rustc: bool,

    #[clap(long)]
    pub(crate) cranelift: bool,

    #[clap(long)]
    pub(crate) cranelift_local: bool,

    #[clap(long)]
    pub(crate) expensive_flags: bool,

    // use path to local rustc build with debug assertions
    #[clap(long)]
    pub(crate) local_debug_assertions: bool,

    #[clap(short = 'j', long = "jobs", long = "threads", default_value_t = 0)]
    pub(crate) threads: usize,

    #[clap(long = "order", long = "chain-order", default_value_t = 4)]
    pub(crate) chain_order: usize,

    #[clap(long = "projects", num_args = 1..)]
    pub(crate) projects: Vec<std::path::PathBuf>,

    #[clap(long)]
    pub(crate) smolfuzz: bool,
}

/// check whether a file uses features or not
pub fn uses_feature(file: &std::path::Path) -> bool {
    match std::fs::read_to_string(file) {
        Ok(file) => file.contains("feature("),
        _ => {
            eprintln!("Failed to read '{}'", file.display());
            false
        }
    }
}

pub fn get_flag_combination<'a, 'b>(flags: &'a [&'b str]) -> Vec<Vec<&'a &'b str>> {
    // get the power set : [a, b, c] => [a], [b], [c], [a,b], [a,c], [b,c], [a,b,c]

    // optimization: only check the first 5000 combinations to avoid OOM, usually that is good enough...
    let combs: Vec<Vec<&&str>> = (0..=flags.len())
        .flat_map(|numb_comb| flags.iter().combinations(numb_comb).take(10000))
        // when bisecting flag combinations, only keep sets smaller than 11
        // it is unlikely that we need the entire set of a set of flags to reproduce an ice
        .filter(|v| v.len() <= 10)
        .collect();

    // UPDATE: special cased in par_iter loop
    // add an empty "" flag so start with, in case an ice does not require any flags
    //   let mut tmp = vec![vec![String::new()]];
    //  tmp.extend(combs);
    let mut tmp = combs;
    //dbg!(&x);

    // we may have a lot of    Zmiroptlvl1 .. 2 .. 3 ,   1, 3 ..   1, 4 .. combinations, dedupe these to only keep the last one

    let tmp2 = tmp.iter_mut().map(|vec| {
        // reverse
        let vec_reversed: Vec<&&str> = {
            let mut v = vec.clone();
            v.reverse();
            v
        };

        // have we seen a mir-opt-level already?
        let mut seen: bool = false;

        // check the reversed vec for the first -Zmir and skip all other -Zmirs afterwards
        let vtemp: Vec<&&str> = vec_reversed
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
        let mut vfinal: Vec<&&str> = vtemp;
        vfinal.reverse();
        vfinal
    });

    let mut tmp2 = tmp2.collect::<Vec<Vec<&&str>>>();
    tmp2.sort();
    // remove duplicates that occurred due to removed mir opt levels
    tmp2.dedup();
    //finally, sort by length
    tmp2.sort_by_key(|x| x.len());

    // we cant assert here when we remove redundant opt levels :/
    //    debug_assert_eq!(tmp2.iter().last().unwrap(), flags);

    tmp2
}

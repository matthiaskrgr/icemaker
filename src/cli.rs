use clap::Parser;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(short, long)]
    clippy: bool,

    #[clap(short, long)]
    clippy_fix: bool,

    #[clap(short, long)]
    rustdoc: bool,

    #[clap(short, long)]
    analyzer: bool, // rla

    #[clap(short, long)]
    rustfmt: bool,

    #[clap(short, long)]
    silent: bool,

    #[clap(short, long)]
    heat: bool, // spaceheater mode, try to catch ICEs from random codeu

    #[clap(short, long)]
    miri: bool,

    #[clap(short, long)]
    codegen: bool,

    #[clap(short, long)]
    incremental_test: bool,

    #[clap(short, long)]
    fuzz: bool,

    #[clap(short, long)]
    rustc: bool,

    #[clap(short, long, default_value_t = 0)]
    threads: u8,
}

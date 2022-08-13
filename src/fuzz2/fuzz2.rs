use std::fs::File;
use std::io::prelude::*;

use crate::fuzz2::function::*;

pub(crate) fn fuzz2main() {
    let mut fngen = FunctionGenerator::new();

    let mut output = String::from("pub fn main() {");

    const MAX_FNS: u32 = 1000;

    for _ in 0..MAX_FNS {
        let fun = fngen.gen_fn();
        let fun_call = fun.gen_call();
        eprintln!("{fun}\n{fun_call}");

        output.push_str(&fun.to_string());
        output.push('\n');
        output.push_str(&fun_call.to_string());
        output.push('\n');
    }
    output.push('}'); // fn main

    let mut file = File::create("out.rs").unwrap_or(File::open("out.rs").unwrap());
    file.write_all(output.as_bytes())
        .expect("failed to write to file");
}

// lets us
trait Fuzzable {
    fn insert_pre(&self) -> String;
    fn insert_post(&self) -> String;
}

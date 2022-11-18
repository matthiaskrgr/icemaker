use std::fs::File;
use std::io::prelude::*;

use crate::fuzz2::function::*;
use crate::fuzz2::r#struct::StructGenerator;

pub(crate) fn fuzz2main() {
    let mut fngen = FunctionGenerator::new();

    let mut output = String::from("pub fn main() {");

    const MAX_FNS: u32 = 10;
    // generate an arbitrary number of functions
    for _ in 0..MAX_FNS {
        let fun = fngen.gen_fn();
        let fun_call = fun.gen_call();
        eprintln!("{fun}\n{fun_call}");

        output.push_str(&fun.to_string());
        output.push('\n');
        output.push_str(&fun_call.to_string());
        output.push('\n');
    }

    let mut structgen = StructGenerator::new();
    const MAX_STRUCTS: u32 = 10;
    for _ in 0..MAX_STRUCTS {
        let strct = structgen.gen_struct();
        output.push_str(&strct.to_string());
        output.push('\n');
    }

    output.push('}'); // fn main

    let mut file = File::create("out.rs").unwrap_or(File::open("out.rs").unwrap());
    file.write_all(output.as_bytes())
        .expect("failed to write to file");
}

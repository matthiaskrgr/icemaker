use itertools::Itertools;
use std::io::prelude::*;

pub(crate) fn gen_smol_code_char_set() -> impl Iterator<Item = String> {
    const CHARS: &[char] = &['0', '1', '[', ']', ',', '#', 'a', 'b', 'c', '=', ':', ';'];

    const SNIPPET_LENGTH: usize = 5;
    const ITEM_LIMIT: usize = 10_000;

    (0..=CHARS.len() + 3)
        .map(|combinations| CHARS.iter().combinations(combinations  ).collect::<Vec<_>>())
        .flatten()
        .map(|x| x.into_iter().collect::<String>())
}

pub(crate) fn codegen_smolfuzz() {
    let x = gen_smol_code_char_set().collect::<Vec<_>>();
    dbg!(x);
    panic!();

    gen_smol_code_char_set().enumerate().for_each(|(id, code)| {
        let mut file =
            std::fs::File::create(std::path::PathBuf::from(format!("{}.rs", id))).unwrap();
        file.write_all(code.as_bytes()).unwrap();
    })
}

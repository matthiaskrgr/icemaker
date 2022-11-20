use itertools::Itertools;
use std::io::prelude::*;

/// https://users.rust-lang.org/t/how-can-i-create-a-function-with-all-permutations-of-all-digits-up-to-the-number-of-permutations-asked/75675/2
fn variations_up_to_length(items: &[char]) -> impl Iterator<Item = String> + '_ {
    (1..=items.len())
        .flat_map(|n| {
            std::iter::repeat(items.iter())
                .take(n)
                .multi_cartesian_product()
        })
        .map(|v| v.into_iter().collect::<String>())
        .filter(|x| x.len() == items.len())
}

pub(crate) fn gen_smol_code_char_set() -> impl Iterator<Item = String> {
    const CHARS: &[char] = &['0', '1', '[', ']', ',', '#', 'a', 'b', 'c', '=', ':', ';'];

    const SNIPPET_LENGTH: usize = 5;
    const ITEM_LIMIT: usize = 10_000;

    variations_up_to_length(CHARS).take(ITEM_LIMIT)
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

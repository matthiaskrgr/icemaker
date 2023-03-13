use std::collections::HashMap;
use std::path::PathBuf;

use tree_sitter::{Language, Parser, Tree};
use tree_sitter_rust;
use tree_splicer::splice::{splice, Config};

const SPLICER_CFG: Config = Config {
    inter_splices: 20,
    seed: 0,
    tests: 9,
};

// read a file from a path and splice-fuzz it returning a set of String that we built from it
pub(crate) fn splice_file(path: &PathBuf) -> Vec<String> {
    let file_content = std::fs::read_to_string(path)
        .expect(&format!("splicer failed to read file {}", path.display()));
    // skip if its too long to avoid stack overflows somewhere
    if file_content.lines().count() > 1000 {
        return Vec::new();
    }

    let mut parser = Parser::new();
    // rust!
    parser.set_language(tree_sitter_rust::language()).unwrap();

    let tree = parser.parse(&file_content, None);

    let mut hm = HashMap::new();
    hm.insert(
        path.display().to_string(),
        (file_content.into_bytes(), tree.unwrap()),
    );

    // TODO just return Iterator here
    splice(SPLICER_CFG, &hm)
        .map(|f| String::from_utf8(f).unwrap_or_default())
        .collect::<Vec<String>>()
}

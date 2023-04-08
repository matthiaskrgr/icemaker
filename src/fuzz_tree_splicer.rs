use std::collections::HashMap;
use std::path::PathBuf;

use tree_sitter::Parser;
use tree_splicer::splice::{splice, Config};

// read a file from a path and splice-fuzz it returning a set of String that we built from it
// pub(crate) fn splice_file(hm: &HashMap<String, (Vec<u8>, Tree)>) -> Vec<String> {
pub(crate) fn splice_file(path: &PathBuf) -> Vec<String> {
    let splicer_cfg: Config = Config {
        inter_splices: 10, // 30
        seed: 10,
        tests: 30, // 10
        //
        chaos: 2,
        deletions: 0,
        node_types: tree_splicer::node_types::NodeTypes::new(tree_sitter_rust::NODE_TYPES).unwrap(),
        language: tree_sitter_rust::language(),
    };

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
    splice(splicer_cfg, &hm)
        .map(|f| String::from_utf8(f).unwrap_or_default())
        .collect::<Vec<String>>()
}

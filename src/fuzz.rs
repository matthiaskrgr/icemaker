use rand::prelude::IteratorRandom;
use rand::Rng;

static RANDOM_ITEMS: &[&str] = &[
    " main ",
    "{ }",
    "}",
    " fn ",
    " impl ",
    " use ",
    "::",
    "#",
    "#r",
    "=",
    "\"",
    "<",
    ">",
    ",",
    "{",
    "}",
    " ",
    "\n",
    "(",
    ")",
    "[",
    "]",
    "?",
    " pub ",
    " let ",
    ";",
    " const ",
    " static ",
    "&",
    "#[test]",
    " unsafe ",
    // "fn add() -> () {} ",
    " -> ",
    "-> ()", //"fn bar()",
    " type ",
    " struct ",
    " return ",
    " macro_rules! ",
    " match ",
    " if let ",
    " while let ",
    " for ",
    " None ",
    " Some(_) ",
    " _ ",
    " && ",
    " || ",
    " == ",
    ".",
    "#[cfg]",
    " extern ",
    " async ",
    " .. ",
    "..= ",
    "=>",
    "();",
    "{};",
    "x = 3",
    " String::new() ",
    "1",
    "3",
    "0",
];

pub(crate) fn get_random_string() -> String {
    const MAX_ITEMS_PER_FILE: usize = 10;
    let mut rng = rand::thread_rng();

    let identifier_limit = rng.gen_range(0..MAX_ITEMS_PER_FILE);

    let mut output = String::new();

    // push $identifier_limit random thingys into the string;
    (1..identifier_limit)
        .for_each(|_| output.push_str(RANDOM_ITEMS.iter().choose(&mut rng).unwrap()));

    //println!("{}", output);
    output
}

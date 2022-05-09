use rand::prelude::IteratorRandom;

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

/*
pub(crate) fn get_random_string() -> String {
    use rand::Rng;

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

*/

pub(crate) fn get_random_string() -> String {
    dbg!(gen_random_main())
}

/// get one random identifier
fn r() -> &'static str {
    let mut rng = rand::thread_rng();
    RANDOM_ITEMS.iter().choose(&mut rng).unwrap()
}

fn _gen_random_fn() -> String {
    format!(
        "pub fn fooo<{} , {}>(s: &String) -> () {{   todo!() }}\n",
        r(),
        r(),
    )
}

/// generate a main with a binding to r()
fn gen_random_main() -> String {
    format!(
        "pub fn main() {{
  let x =   {};
  let z = {}
    
}}",
        r(),
        tc()
    )
}

static TYPES_SIMPLE: &[&str] = &[
    "String::new()",
    "Vec::<u32>::new()",
    "std::path::PathBuf",
    "u32",
    "i8",
    "f32",
    "&str",
    "usize",
    "None",
    "Option<i32>",
];

static TYPES_COMPLEXE: &[&str] = &[
    "Vec::<TY>::new()",
    //"Option<T>",
    //"Result<T,T>",
    "Some(TY)",
    "Ok(TY)",
    "Err(TY)",
    "[TY]",
];

// get a simple type
fn ts() -> &'static str {
    let mut rng = rand::thread_rng();
    TYPES_SIMPLE.iter().choose(&mut rng).unwrap()
}

fn tc() -> String {
    let mut rng = rand::thread_rng();
    let type_raw = TYPES_COMPLEXE.iter().choose(&mut rng).unwrap().to_string();
    type_raw.replace("TY", ts())
}

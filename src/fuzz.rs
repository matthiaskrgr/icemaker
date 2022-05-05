use rand::prelude::IteratorRandom;
use rand::Rng;

static random_items: &[&str] = &[
    "main", "{", "}", "fn", "impl", "use", "::", "#", "#r", "=", "\"", "<", ">", ",", "{", "}",
    "(", ")", "[", "]", "?",
];

pub(crate) fn get_random_string() -> String {
    const MAX_LEN: usize = 100;
    let mut rng = rand::thread_rng();

    let identifier_limit = rng.gen_range(0..MAX_LEN);

    let mut output = String::new();

    // push $identifier_limit random thingys into the string;
    (1..identifier_limit)
        .for_each(|_| output.push_str(random_items.iter().choose(&mut rng).unwrap()));

    println!("{}", output);
    output
}

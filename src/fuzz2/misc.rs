use rand::prelude::IteratorRandom;

/// the item can be represented as code
pub(crate) trait Code {
    fn to_code(&self) -> String;
}

pub(crate) enum Vis {
    Pub,
    PubCrate,
    Private,
}

impl Vis {
    fn to_String(&self) -> String {
        match &self {
            Pub => "pub ",
            PubCrate => "pub(crate) ",
            Private => "",
        }
        .to_string()
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Lifetime(String);

impl Code for Lifetime {
    fn to_code(&self) -> String {
        format!("'{}", self.0)
    }
}

// TODO make this more generic
impl From<String> for Lifetime {
    fn from(lifetime: String) -> Self {
        Self(lifetime)
    }
}

// FIXME
impl std::fmt::Display for dyn Code {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "a")
    }
}

impl Lifetime {
    /// returns a random lifetime
    pub(crate) fn get_random() -> Self {
        static RANDOM_VALID: &[&str] = &["a", "b", "c", "d", "_", "&",
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "12a"];

        RANDOM_VALID
            .iter()
            .map(|x| Lifetime::from(x.to_string()))
            .choose(&mut rand::thread_rng())
            .unwrap()
    }
}

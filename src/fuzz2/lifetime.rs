use crate::fuzz2::misc::*;
use rand::prelude::IteratorRandom;

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

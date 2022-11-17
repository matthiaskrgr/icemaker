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

pub(crate) type Lifetime = String;

/// the item can be represented as code
pub(crate) trait Code {
    fn to_code(&self) -> String;
}

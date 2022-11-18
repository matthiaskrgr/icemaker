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
    pub(crate) fn to_string(&self) -> String {
        match &self {
            Vis::Pub => "pub ",
            Vis::PubCrate => "pub(crate) ",
            Vis::Private => "",
        }
        .to_string()
    }
}

// FIXME
impl std::fmt::Display for dyn Code {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.to_code())
    }
}

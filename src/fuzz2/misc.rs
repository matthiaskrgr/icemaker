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

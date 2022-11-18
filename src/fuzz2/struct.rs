use crate::fuzz2::lifetime::*;
use crate::fuzz2::misc::*;
use crate::fuzz2::ty::*;

pub(crate) struct StructGenerator {
    id: usize,
    // keep a list of generated functions so we can reference them in other functions..?
    structs: Vec<Struct>,
}

pub(crate) struct Struct {
    name: String,
    lifetimes: Vec<Lifetime>,
    fields: Vec<StructField>,
    vis: Vis,
    tuplestruct: bool,
}

impl Struct {
    /// adds a lifetime to the struct, but not a specific Field
    fn push_lifetime(&mut self, lifetime: Lifetime) {
        self.lifetimes.push(lifetime);
    }

    fn new(name: &str) -> Self {
        let name = name.to_string();
        let lifetime_1 = Lifetime::get_random();
        let lifetime_2 = Lifetime::get_random();

        let tygen = TyGen::new();

        let field_1 = StructField::new(
            String::from("field1"),
            lifetime_1.clone(),
            tygen.random_ty(),
            Vis::Pub,
        );

        let field_2 = StructField::new(
            String::from("field2"),
            lifetime_2.clone(),
            tygen.random_ty(),
            Vis::Pub,
        );

        let structvis = Vis::Pub;

        Self {
            name,
            lifetimes: vec![lifetime_1, lifetime_2],
            fields: vec![field_1, field_2],
            vis: structvis,
            tuplestruct: false,
        }
    }
}

pub(crate) struct StructField {
    name: String,
    lifetime: Lifetime,
    ty: Ty,
    vis: Vis,
}

impl StructField {
    fn new(name: String, lifetime: Lifetime, ty: Ty, vis: Vis) -> Self {
        StructField {
            name,
            lifetime,
            ty,
            vis,
        }
    }
}

impl std::fmt::Display for StructField {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{} {}: {},", self.name, self.ty, self.vis.to_string())
    }
}

impl std::fmt::Display for Struct {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        // @TODO handle tuplestruct !

        write!(
            f,
            "{} struct {} {{
            {}
            }}
            ",
            self.vis.to_string(),
            self.name,
            self.fields
                .iter()
                .map(|f| f.to_string())
                .collect::<Vec<String>>()
                .join("\n")
        )
    }
}

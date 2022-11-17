use std::fmt;

use rand::prelude::IteratorRandom;

use crate::fuzz2::lifetime::*;
use crate::fuzz2::misc::*;
use crate::fuzz2::ty::*;

pub(crate) const LIFETIMES: &[&str] = &["a", "b", "c", "d", "_", "&",
 "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "12a"];

pub(crate) struct StructGenerator {
    id: usize,
    // keep a list of generated functions so we can reference them in other functions..?
    structs: Vec<Struct>,
}

pub(crate) struct Struct {
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
}

pub(crate) struct StructField {
    name: String,
    lifetimes: String,
    ty: Ty,
    vis: Vis,
}

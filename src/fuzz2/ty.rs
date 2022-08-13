use rand::prelude::IteratorRandom;
use std::fmt;

pub(crate) const TYPES: &[Ty] = &[
    Ty::u8,
    Ty::u16,
    Ty::u32,
    Ty::u64,
    Ty::i8,
    Ty::i16,
    Ty::i32,
    Ty::i64,
    Ty::usize,
    Ty::isize,
    Ty::String,
];

#[allow(non_camel_case_types)]
#[derive(Debug, Clone)]
pub(crate) enum Ty {
    u8,
    u16,
    u32,
    u64,
    i8,
    i16,
    i32,
    i64,
    usize,
    isize,
    String,
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let x = match self {
            Self::u8 => "u8",
            Self::u16 => "u16",
            Self::u32 => "u32",
            Self::u64 => "u64",
            Self::i8 => "i8",
            Self::i16 => "i16",
            Self::i32 => "i32",
            Self::i64 => "i64",
            Self::usize => "usize",
            Self::isize => "isize",
            Self::String => "String",
        };
        write!(f, "{}", x)
    }
}

// get a random type
pub(crate) struct TyGen {}
impl TyGen {
    pub(crate) fn new() -> Self {
        TyGen {}
    }

    pub(crate) fn random_ty(&self) -> Ty {
        TYPES
            .iter()
            .choose(&mut rand::thread_rng())
            .unwrap()
            .clone()
    }
}

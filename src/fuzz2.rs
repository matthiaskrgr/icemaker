use rand::prelude::IteratorRandom;
use std::fmt;

const TYPES: &[Ty] = &[
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

const LIFETIMES: &[&str] = &["a", "b", "c", "d", "_", "&",
 "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "12a"];

struct FunctionGenerator {
    id: usize,
    // keep a list of generated functions so we can reference them in other functions..?
    functions: Vec<Function>,
}

impl FunctionGenerator {
    fn new() -> Self {
        Self {
            id: 0,
            functions: Vec::new(),
        }
    }

    fn gen_fn(&mut self) -> Function {
        let tygen = TyGen::new();
        //let mut rng = rand::thread_rng();
        let ty = tygen.random_ty();
        let function_id = format!("{:X?}", self.id);
        self.id += 1;

        let args_number = (0..20).into_iter().choose(&mut rand::thread_rng()).unwrap();
        let args = (0..args_number)
            .into_iter()
            .map(|argnr| format!("a_{argnr}: {}", tygen.random_ty()));

        let fun = Function {
            keyword: Vec::new(),
            lifetimes: LIFETIMES.iter().map(|x| x.to_string()).choose_multiple(
                &mut rand::thread_rng(),
                (0..10).into_iter().choose(&mut rand::thread_rng()).unwrap(),
            ),

            name: function_id,
            return_ty: ty,
            args: args.collect::<Vec<String>>(),
            body: "todo!()".into(),
        };
        self.functions.push(fun.clone());
        fun
    }
}

#[derive(Debug, Clone)]
struct Function {
    /// such as const, async etc
    keyword: Vec<String>,
    lifetimes: Vec<String>,
    name: String,
    return_ty: Ty,
    args: Vec<String>,
    body: String,
}

impl std::fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let args_fmtd = self
            .args
            .iter()
            .enumerate()
            .map(|(i, arg_ty)| format!("arg_{i}: {arg_ty}, "))
            .collect::<String>();
        let body = &self.body;
        write!(
            f,
            "fn fn_{}({}) -> {} {{ {body} }}",
            &self.name, args_fmtd, self.return_ty
        )
    }
}

pub(crate) fn fuzz2main() {
    let mut fngen = FunctionGenerator::new();

    for _ in 0..1000 {
        let fun = fngen.gen_fn();

        eprintln!("{fun}");
    }
}

// lets us
trait Fuzzable {
    fn insert_pre(&self) -> String;
    fn insert_post(&self) -> String;
}

#[allow(non_camel_case_types)]
#[derive(Debug, Clone)]
enum Ty {
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
struct TyGen {}
impl TyGen {
    fn new() -> Self {
        TyGen {}
    }

    fn random_ty(&self) -> Ty {
        TYPES
            .iter()
            .choose(&mut rand::thread_rng())
            .unwrap()
            .clone()
    }
}

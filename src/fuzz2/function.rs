use std::fmt;

use rand::prelude::IteratorRandom;

use crate::fuzz2::lifetime::*;
use crate::fuzz2::misc::*;
use crate::fuzz2::ty::*;

//  https://doc.rust-lang.org/reference/items/functions.html

pub(crate) struct FunctionGenerator {
    id: usize,
    // keep a list of generated functions so we can reference them in other functions..?
    functions: Vec<Function>,
}

impl FunctionGenerator {
    pub(crate) fn new() -> Self {
        Self {
            id: 0,
            functions: Vec::new(),
        }
    }

    pub(crate) fn gen_fn(&mut self) -> Function {
        let possible_fn_keywords: &[FnQualifier] = &[
            FnQualifier::FnConst,
            FnQualifier::FnAsync,
            FnQualifier::FnExtern,
            FnQualifier::FnUnsafe,
            FnQualifier::Other(String::from("foo")),
        ];

        let tygen = TyGen::new();
        //let mut rng = rand::thread_rng();
        let ty = tygen.random_ty();
        let function_id = format!("{:X?}", self.id);
        self.id += 1;

        const MAX_FN_ARGS: u32 = 100;

        let args_number = (0..MAX_FN_ARGS)
            .into_iter()
            .choose(&mut rand::thread_rng())
            .unwrap();
        let args = (0..args_number)
            .into_iter()
            .map(|_argnr| format!("{}", tygen.random_ty()));

        let num_keywords = (0..=std::mem::variant_count::<FnQualifier>())
            .into_iter()
            .choose(&mut rand::thread_rng())
            .unwrap_or_default();

        let keywords = (0..num_keywords)
            .into_iter()
            .filter_map(|_| possible_fn_keywords.iter().choose(&mut rand::thread_rng()))
            .cloned()
            .collect::<Vec<FnQualifier>>();

        let fun = Function {
            keywords,
            lifetimes: vec![Lifetime::get_random(); /*number of lifetimes: */ 3],

            name: format!("fn_{function_id}"),
            return_ty: ty,
            args: args.collect::<Vec<String>>(),
            // @FIXME
            body: "todo!()".into(),
        };
        self.functions.push(fun.clone());
        fun
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Function {
    /// such as const, async etc
    keywords: Vec<FnQualifier>,
    lifetimes: Vec<Lifetime>,
    name: String,
    return_ty: Ty,
    args: Vec<String>,
    body: String,
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub(crate) struct FunctionArg {
    name: String,
    ty: Ty,
    lifetime: Lifetime,
}

impl Function {
    pub(crate) fn gen_call(&self) -> String {
        let name = &self.name;
        let args = self
            .args
            .iter()
            .map(|_x| "unimplemented!(), ")
            .collect::<String>();
        format!("{name}({args});")
    }
}

#[derive(Debug, Clone)]
enum FnQualifier {
    FnConst,       // const
    FnAsync,       // async
    FnExtern,      // extern
    FnUnsafe,      // unsafe
    Other(String), // we can do custom stuff here
}

impl std::fmt::Display for FnQualifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                FnQualifier::FnConst => "const",
                FnQualifier::FnAsync => "async",
                FnQualifier::FnExtern => "extern",
                FnQualifier::FnUnsafe => "unsafe",
                FnQualifier::Other(kw) => kw,
            }
        )
    }
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
        let keywords = self
            .keywords
            .iter()
            .map(|kw| format!(" {kw} "))
            .collect::<String>();

        if self.lifetimes.is_empty() {
            write!(
                f,
                "{keywords} fn {}({}) -> {} {{ {body} }}",
                &self.name, args_fmtd, self.return_ty
            )
        } else {
            let lifetimes = self
                .lifetimes
                .iter()
                .map(|l| l.to_code())
                .collect::<Vec<String>>()
                .join(", ");
            write!(
                f,
                "{keywords} fn {}<{lifetimes}>({}) -> {} {{ {body} }}",
                &self.name, args_fmtd, self.return_ty
            )
        }
    }
}

impl Code for Function {
    fn to_code(&self) -> String {
        self.to_string()
    }
}

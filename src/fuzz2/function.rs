use std::fmt;

use rand::prelude::IteratorRandom;

use crate::fuzz2::ty::*;

pub(crate) const LIFETIMES: &[&str] = &["a", "b", "c", "d", "_", "&",
 "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "12a"];

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

        let fun = Function {
            keyword: Vec::new(),
            lifetimes: LIFETIMES.iter().map(|x| x.to_string()).choose_multiple(
                &mut rand::thread_rng(),
                (0..10).into_iter().choose(&mut rand::thread_rng()).unwrap(),
            ),

            name: format!("fn_{}", function_id),
            return_ty: ty,
            args: args.collect::<Vec<String>>(),
            body: "todo!()".into(),
        };
        self.functions.push(fun.clone());
        fun
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Function {
    /// such as const, async etc
    keyword: Vec<String>,
    lifetimes: Vec<String>,
    name: String,
    return_ty: Ty,
    args: Vec<String>,
    body: String,
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
            "fn {}({}) -> {} {{ {body} }}",
            &self.name, args_fmtd, self.return_ty
        )
    }
}

use std::fmt;

struct FunctionGenerator<'a> {
    id: usize,
    // keep a list of generated functions so we can reference them in other functions..?
    functions: Vec<&'a Function>,
}

impl<'a> FunctionGenerator<'a> {
    fn new() -> Self {
        Self {
            id: 0,
            functions: Vec::new(),
        }
    }

    fn gen_function(&mut self) -> Function {
        let function_id = format!("{:X?}", self.id);
        self.id += 1;

        Function {
            name: function_id,
            return_ty: Ty::usize,
            args: Vec::new(),
            body: "todo!()".into(),
        }
    }
}

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

struct Function {
    name: String,
    return_ty: Ty,
    args: Vec<Ty>,
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
        write!(f, "fn {}({}) -> {{ {body} }}", &self.name, args_fmtd)
    }
}

impl Function {
    fn to_string(&self) -> String {
        let args_fmtd = self
            .args
            .iter()
            .enumerate()
            .map(|(i, arg_ty)| format!("arg_{i}: {arg_ty}, "))
            .collect::<String>();
        let body = &self.body;
        format!("fn {}({}) -> {{ {body} }}", &self.name, args_fmtd)
    }
}
pub(crate) fn fuzz2main() {}

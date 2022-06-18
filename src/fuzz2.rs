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

struct Function {
    name: String,
    return_ty: Ty,
    args: Vec<Ty>,
    body: String,
}

pub(crate) fn fuzz2main() {}

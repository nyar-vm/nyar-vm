impl_attr!(I32Attr, "i32", "wasm");
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct I32Attr {
    value: i32,
}

impl_attr!(I64Attr, "i64", "wasm");
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct I64Attr {
    value: i64,
}

impl_attr!(F32Attr, "f32", "wasm");
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct F32Attr {
    value: [u8; 4],
}

impl_attr!(F64Attr, "f64", "wasm");
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct F64Attr {
    value: [u8; 8],
}

impl_attr!(V128Attr, "v128", "wasm");
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct V128Attr {
    value: [u8; 16],
}

impl_attr!(FuncRefAttr, "funcref", "wasm");
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FuncRefAttr {
    value: i32,
}

impl_attr!(ExternRefAttr, "externref", "wasm");
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ExternRefAttr {
    value: i32,
}

impl_attr!(ValueAttr, "valtype", "wasm");
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ValueAttr {
    I32(I32Attr),
    I64(I64Attr),
    F32(F32Attr),
    F64(F64Attr),
    V128(V128Attr),
}

impl_attr!(RefAttr, "reftype", "wasm");
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RefAttr {
    Funcref(FuncRefAttr),
    Externref(ExternRefAttr),
}


impl_attr!(FuncAttr, "functype", "wasm");
#[derive( Clone, Debug, PartialEq)]
pub struct FuncAttr {
    input: Vec<ValueAttr>,
    output: Vec<ValueAttr>,
}
impl_attr!(LimitsAttr, "limits", "wasm");
#[derive( Clone, Debug, PartialEq)]
pub struct LimitsAttr {
    min: u32,
    max: Option<u32>,
}
impl_attr!(MemAttr, "memtype", "wasm");
#[derive( Clone, Debug, PartialEq)]
pub struct MemAttr {
    limits: LimitsAttr,
}

impl_attr!(TableAttr, "tabletype", "wasm");
#[derive( Clone, Debug, PartialEq)]
pub struct TableAttr {
    ptr: RefAttr,
    limits: LimitsAttr,
}

impl_attr!(GlobalAttr, "globaltype", "wasm");
#[derive( Clone, Debug, PartialEq)]
pub struct GlobalAttr {
    value: ValueAttr,
    mutable: bool,
}



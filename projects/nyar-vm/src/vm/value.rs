use std::ptr::null_mut;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueTag { Int = 0, Float = 1, Bool = 2, Null = 3, Object = 4, Array = 5, String = 6, Function = 7, Closure = 8, TraitObject = 9, Code = 10, Continuation = 11, Effect = 12, WitnessTable = 13 }

#[repr(C)]
#[derive(Clone, Copy)]
pub union ValueData { pub int: i64, pub float: f64, pub bool_: u8, pub ptr: *mut () }

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Value { pub tag: ValueTag, pub data: ValueData }

impl Value {
    pub fn int(v: i64) -> Self { Self { tag: ValueTag::Int, data: ValueData { int: v } } }
    pub fn float(v: f64) -> Self { Self { tag: ValueTag::Float, data: ValueData { float: v } } }
    pub fn bool(v: bool) -> Self { Self { tag: ValueTag::Bool, data: ValueData { bool_: if v { 1 } else { 0 } } } }
    pub fn null() -> Self { Self { tag: ValueTag::Null, data: ValueData { ptr: null_mut() } } }
    pub unsafe fn as_int(&self) -> i64 { self.data.int }
    pub unsafe fn as_float(&self) -> f64 { self.data.float }
    pub unsafe fn as_bool(&self) -> bool { self.data.bool_ != 0 }
}

#[derive(Clone)]
pub struct TraitObject { pub data: *mut (), pub witness: *const () }

#[derive(Clone)]
pub struct Upvalue(pub Value);

#[derive(Clone)]
pub struct Closure { pub func: usize, pub upvalues: Vec<Upvalue> }

#[derive(Clone)]
pub struct Continuation { pub ip: usize, pub stack_slice: Vec<Value> }

use std::collections::HashMap;
use std::ptr::null_mut;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueTag {
    Int = 0,
    Float = 1,
    Bool = 2,
    Null = 3,
    Object = 4,
    Array = 5,
    String = 6,
    Function = 7,
    Closure = 8,
    TraitObject = 9,
    Code = 10,
    Continuation = 11,
    Effect = 12,
    WitnessTable = 13,
    BigInt = 14,
    DynObject = 15,
    List = 16,
    Tuple = 17,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union ValueData {
    pub int: i64,
    pub float: f64,
    pub bool_: u8,
    pub ptr: *mut (),
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Value {
    pub tag: ValueTag,
    pub data: ValueData,
}

impl Value {
    pub fn int(v: i64) -> Self {
        Self {
            tag: ValueTag::Int,
            data: ValueData { int: v },
        }
    }
    pub fn float(v: f64) -> Self {
        Self {
            tag: ValueTag::Float,
            data: ValueData { float: v },
        }
    }
    pub fn bool(v: bool) -> Self {
        Self {
            tag: ValueTag::Bool,
            data: ValueData {
                bool_: if v { 1 } else { 0 },
            },
        }
    }
    pub fn null() -> Self {
        Self {
            tag: ValueTag::Null,
            data: ValueData { ptr: null_mut() },
        }
    }
    pub fn string(s: String) -> Self {
        let b = Box::new(s);
        Self {
            tag: ValueTag::String,
            data: ValueData {
                ptr: Box::into_raw(b) as *mut (),
            },
        }
    }
    pub fn array(items: Vec<Value>) -> Self {
        let a = Box::new(Array { items });
        Self {
            tag: ValueTag::Array,
            data: ValueData {
                ptr: Box::into_raw(a) as *mut (),
            },
        }
    }
    pub fn bigint(sign: u8, bytes: Vec<u8>) -> Self {
        let b = Box::new(BigInt { sign, bytes });
        Self {
            tag: ValueTag::BigInt,
            data: ValueData {
                ptr: Box::into_raw(b) as *mut (),
            },
        }
    }
    pub fn dyn_object() -> Self {
        let o = Box::new(DynObject {
            entries: HashMap::new(),
        });
        Self {
            tag: ValueTag::DynObject,
            data: ValueData {
                ptr: Box::into_raw(o) as *mut (),
            },
        }
    }
    pub fn list(items: Vec<Value>) -> Self {
        let l = Box::new(List { items });
        Self {
            tag: ValueTag::List,
            data: ValueData {
                ptr: Box::into_raw(l) as *mut (),
            },
        }
    }
    pub fn tuple(items: Vec<Value>) -> Self {
        let t = Box::new(Tuple { items });
        Self {
            tag: ValueTag::Tuple,
            data: ValueData {
                ptr: Box::into_raw(t) as *mut (),
            },
        }
    }
    pub fn effect(type_idx: u16, args: Vec<Value>) -> Self {
        let e = Box::new(Effect { type_idx, args });
        Self {
            tag: ValueTag::Effect,
            data: ValueData {
                ptr: Box::into_raw(e) as *mut (),
            },
        }
    }
    pub fn bigint_from_i64(v: i64) -> Self {
        let mut bytes = Vec::new();
        let mut u = if v < 0 { (-v) as u64 } else { v as u64 };
        while u > 0 {
            bytes.push((u & 0xFF) as u8);
            u >>= 8;
        }
        let b = Box::new(BigInt {
            sign: if v < 0 { 1 } else { 0 },
            bytes,
        });
        Self {
            tag: ValueTag::BigInt,
            data: ValueData {
                ptr: Box::into_raw(b) as *mut (),
            },
        }
    }
    pub fn closure(func_idx: u16, upvalues: Vec<Upvalue>) -> Self {
        let c = Box::new(Closure {
            func: func_idx as usize,
            upvalues,
        });
        Self {
            tag: ValueTag::Closure,
            data: ValueData {
                ptr: Box::into_raw(c) as *mut (),
            },
        }
    }
    pub fn object(class_idx: u16, fields: Vec<Value>) -> Self {
        let o = Box::new(Object { class_idx, fields });
        Self {
            tag: ValueTag::Object,
            data: ValueData {
                ptr: Box::into_raw(o) as *mut (),
            },
        }
    }
    pub fn continuation(ip: usize, stack_slice: Vec<Value>) -> Self {
        let c = Box::new(Continuation { ip, stack_slice });
        Self {
            tag: ValueTag::Continuation,
            data: ValueData {
                ptr: Box::into_raw(c) as *mut (),
            },
        }
    }
    pub unsafe fn as_int(&self) -> i64 {
        self.data.int
    }
    pub unsafe fn as_float(&self) -> f64 {
        self.data.float
    }
    pub unsafe fn as_bool(&self) -> bool {
        self.data.bool_ != 0
    }
    pub unsafe fn as_string<'a>(&self) -> &'a String {
        &*(self.data.ptr as *const String)
    }
    pub unsafe fn as_array<'a>(&self) -> &'a Array {
        &*(self.data.ptr as *const Array)
    }
    pub unsafe fn as_bigint<'a>(&self) -> &'a BigInt {
        &*(self.data.ptr as *const BigInt)
    }
    pub unsafe fn as_dyn_object<'a>(&self) -> &'a DynObject {
        &*(self.data.ptr as *const DynObject)
    }
    pub unsafe fn as_list<'a>(&self) -> &'a List {
        &*(self.data.ptr as *const List)
    }
    pub unsafe fn as_tuple<'a>(&self) -> &'a Tuple {
        &*(self.data.ptr as *const Tuple)
    }
    pub unsafe fn as_effect<'a>(&self) -> &'a Effect {
        &*(self.data.ptr as *const Effect)
    }
    pub unsafe fn as_cont<'a>(&self) -> &'a Continuation {
        &*(self.data.ptr as *const Continuation)
    }
}

#[derive(Clone)]
pub struct TraitObject {
    pub data: *mut (),
    pub witness: *const (),
}

#[derive(Clone)]
pub struct Upvalue(pub Value);

#[derive(Clone)]
pub struct Closure {
    pub func: usize,
    pub upvalues: Vec<Upvalue>,
}

#[derive(Clone)]
pub struct Object {
    pub class_idx: u16,
    pub fields: Vec<Value>,
}

#[derive(Clone)]
pub struct Continuation {
    pub ip: usize,
    pub stack_slice: Vec<Value>,
}

#[derive(Clone)]
pub struct BigInt {
    pub sign: u8,
    pub bytes: Vec<u8>,
}

impl BigInt {
    pub fn to_i64(&self) -> i64 {
        let mut v: u64 = 0;
        let mut shift = 0u32;
        for &b in &self.bytes {
            let part = (b as u64) << shift;
            v = v.wrapping_add(part);
            shift += 8;
            if shift >= 64 {
                break;
            }
        }
        if self.sign != 0 {
            -(v as i64)
        } else {
            v as i64
        }
    }
    pub fn from_i64(v: i64) -> Self {
        let mut bytes = Vec::new();
        let mut u = if v < 0 { (-v) as u64 } else { v as u64 };
        while u > 0 {
            bytes.push((u & 0xFF) as u8);
            u >>= 8;
        }
        BigInt {
            sign: if v < 0 { 1 } else { 0 },
            bytes,
        }
    }
}
#[derive(Clone)]
pub struct DynObject {
    pub entries: HashMap<String, Value>,
}
#[derive(Clone)]
pub struct Array {
    pub items: Vec<Value>,
}

#[derive(Clone)]
pub struct List {
    pub items: Vec<Value>,
}

#[derive(Clone)]
pub struct Tuple {
    pub items: Vec<Value>,
}

#[derive(Clone)]
pub struct Effect {
    pub type_idx: u16,
    pub args: Vec<Value>,
}

 

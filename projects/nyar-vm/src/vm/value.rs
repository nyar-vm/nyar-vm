use nyar_gc::Trace;
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::ptr::null_mut;
use std::mem::transmute;

const NAN_BASE: u64 = 0x7FF8_0000_0000_0000;
const TAG_SHIFT: u32 = 47;
const PAYLOAD_MASK: u64 = (1 << 47) - 1;

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.is_float() {
            return write!(f, "{}", self.as_float());
        }
        match self.tag() {
            ValueTag::Int => write!(f, "{}", self.as_int()),
            ValueTag::Bool => write!(f, "{}", self.as_bool()),
            ValueTag::Null => write!(f, "null"),
            ValueTag::String => write!(f, "{}", unsafe { self.as_string() }),
            ValueTag::BigInt => write!(f, "{}", unsafe { self.as_bigint().to_i64() }),
            ValueTag::Array => write!(f, "[...]"),
            ValueTag::Object => write!(f, "{{...}}"),
            ValueTag::Closure => write!(f, "<closure>"),
            ValueTag::DynObject => write!(f, "<dyn_object>"),
            _ => write!(f, "<value>"),
        }
    }
}

impl Trace for Value {
    fn trace(&self) {
        if self.is_float() { return; }
        match self.tag() {
            ValueTag::Int
            | ValueTag::Bool
            | ValueTag::Null
            | ValueTag::BigInt
            | ValueTag::Code
            | ValueTag::WitnessTable
            | ValueTag::String => {}
            ValueTag::Array => unsafe {
                let array = self.as_array();
                for item in &array.items {
                    item.trace();
                }
            },
            ValueTag::Object => unsafe {
                let obj = self.as_object();
                for field in &obj.fields {
                    field.trace();
                }
            },
            ValueTag::Closure => unsafe {
                let closure = self.as_closure();
                for upvalue in &closure.upvalues {
                    upvalue.0.trace();
                }
            },
            ValueTag::DynObject => unsafe {
                let obj = self.as_dyn_object();
                for value in obj.entries.values() {
                    value.trace();
                }
            },
            ValueTag::List => unsafe {
                let list = self.as_list();
                for item in &list.items {
                    item.trace();
                }
            },
            ValueTag::Tuple => unsafe {
                let tuple = self.as_tuple();
                for item in &tuple.items {
                    item.trace();
                }
            },
            ValueTag::Continuation => unsafe {
                let cont = self.as_continuation();
                for val in &cont.stack_slice {
                    val.trace();
                }
            },
            ValueTag::Effect => unsafe {
                let effect = self.as_effect();
                for arg in &effect.args {
                    arg.trace();
                }
            },
            _ => {}
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueTag {
    Int = 0,
    Bool = 1,
    Null = 2,
    Object = 3,
    Array = 4,
    String = 5,
    Function = 6,
    Closure = 7,
    TraitObject = 8,
    Code = 9,
    Continuation = 10,
    Effect = 11,
    WitnessTable = 12,
    BigInt = 13,
    DynObject = 14,
    List = 15,
    Tuple = 16,
    Float = 17, // Not used in NaN-boxing but kept for compatibility
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Value(u64);

impl Value {
    #[inline(always)]
    pub fn is_float(&self) -> bool {
        (self.0 & NAN_BASE) != NAN_BASE
    }

    #[inline(always)]
    pub fn tag(&self) -> ValueTag {
        if self.is_float() {
            return ValueTag::Float;
        }
        let sign = (self.0 >> 63) as u8;
        let t = ((self.0 >> TAG_SHIFT) & 0xF) as u8;
        let tag_val = (sign << 4) | t;
        unsafe { transmute(tag_val) }
    }

    #[inline(always)]
    fn encode(tag: ValueTag, payload: u64) -> Self {
        let tag_val = tag as u8;
        let sign = (tag_val >> 4) as u64;
        let t = (tag_val & 0xF) as u64;
        Value((sign << 63) | NAN_BASE | (t << TAG_SHIFT) | (payload & PAYLOAD_MASK))
    }

    #[inline(always)]
    fn payload(&self) -> u64 {
        self.0 & PAYLOAD_MASK
    }

    pub fn to_string(&self) -> String {
        format!("{}", self)
    }
    pub unsafe fn as_closure<'a>(&self) -> &'a Closure {
        &*(self.payload() as *const Closure)
    }
    pub unsafe fn as_object<'a>(&self) -> &'a Object {
        &*(self.payload() as *const Object)
    }
    pub unsafe fn as_dyn_object<'a>(&self) -> &'a DynObject {
        &*(self.payload() as *const DynObject)
    }
    pub unsafe fn as_list<'a>(&self) -> &'a List {
        &*(self.payload() as *const List)
    }
    pub unsafe fn as_tuple<'a>(&self) -> &'a Tuple {
        &*(self.payload() as *const Tuple)
    }
    pub unsafe fn as_effect<'a>(&self) -> &'a Effect {
        &*(self.payload() as *const Effect)
    }
    pub unsafe fn as_continuation<'a>(&self) -> &'a Continuation {
        &*(self.payload() as *const Continuation)
    }
    pub fn int(v: i64) -> Self {
        Self::encode(ValueTag::Int, v as u64)
    }
    pub fn float(v: f64) -> Self {
        let u: u64 = unsafe { transmute(v) };
        // Ensure it's not a quiet NaN that would be mistaken for a tagged value
        if (u & NAN_BASE) == NAN_BASE {
            Value(u & !0x0008_0000_0000_0000) // Clear bit 51
        } else {
            Value(u)
        }
    }
    pub fn bool(v: bool) -> Self {
        Self::encode(ValueTag::Bool, if v { 1 } else { 0 })
    }
    pub fn null() -> Self {
        Self::encode(ValueTag::Null, 0)
    }
    pub fn string(s: String) -> Self {
        let b = Box::new(s);
        Self::encode(ValueTag::String, Box::into_raw(b) as u64)
    }
    pub fn array(items: Vec<Value>) -> Self {
        let a = Box::new(Array { items });
        Self::encode(ValueTag::Array, Box::into_raw(a) as u64)
    }
    pub fn bigint(sign: u8, bytes: Vec<u8>) -> Self {
        let b = Box::new(BigInt { sign, bytes });
        Self::encode(ValueTag::BigInt, Box::into_raw(b) as u64)
    }
    pub fn dyn_object() -> Self {
        let o = Box::new(DynObject {
            entries: HashMap::new(),
        });
        Self::encode(ValueTag::DynObject, Box::into_raw(o) as u64)
    }
    pub fn list(items: Vec<Value>) -> Self {
        let l = Box::new(List { items });
        Self::encode(ValueTag::List, Box::into_raw(l) as u64)
    }
    pub fn tuple(items: Vec<Value>) -> Self {
        let t = Box::new(Tuple { items });
        Self::encode(ValueTag::Tuple, Box::into_raw(t) as u64)
    }
    pub fn effect(type_idx: u16, args: Vec<Value>) -> Self {
        let e = Box::new(Effect { type_idx, args });
        Self::encode(ValueTag::Effect, Box::into_raw(e) as u64)
    }
    pub fn bigint_from_i64(v: i64) -> Self {
        let b = Box::new(BigInt::from_i64(v));
        Self::encode(ValueTag::BigInt, Box::into_raw(b) as u64)
    }
    pub fn closure(module_idx: usize, func_idx: u16, upvalues: Vec<Upvalue>) -> Self {
        let c = Box::new(Closure {
            module_idx,
            func: func_idx as usize,
            upvalues,
        });
        Self::encode(ValueTag::Closure, Box::into_raw(c) as u64)
    }
    pub fn object(class_idx: u16, fields: Vec<Value>) -> Self {
        let o = Box::new(Object { class_idx, fields });
        Self::encode(ValueTag::Object, Box::into_raw(o) as u64)
    }
    pub fn continuation(ip: usize, stack_slice: Vec<Value>) -> Self {
        let c = Box::new(Continuation { ip, stack_slice });
        Self::encode(ValueTag::Continuation, Box::into_raw(c) as u64)
    }
    pub fn as_int(&self) -> i64 {
        // Sign-extend from 47 bits if necessary, but here we just cast
        self.payload() as i64
    }
    pub fn as_float(&self) -> f64 {
        unsafe { transmute(self.0) }
    }
    pub fn as_bool(&self) -> bool {
        self.payload() != 0
    }
    pub unsafe fn as_string<'a>(&self) -> &'a String {
        transmute(self.payload())
    }
    pub unsafe fn as_array<'a>(&self) -> &'a Array {
        transmute(self.payload())
    }
    pub unsafe fn as_bigint<'a>(&self) -> &'a BigInt {
        transmute(self.payload())
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
    pub module_idx: usize,
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

use nyar_gc::{Trace, NyarGc, GcHeader, GcBox};
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::ptr::{null_mut, NonNull};
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
        if self.is_float() {
            return;
        }
        match self.tag() {
            ValueTag::Int | ValueTag::Bool | ValueTag::Null | ValueTag::Code | ValueTag::WitnessTable => {}

            ValueTag::String => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark_and_trace(header_ptr);
                }
            },
            ValueTag::BigInt => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark_and_trace(header_ptr);
                }
            },
            ValueTag::Array => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark_and_trace(header_ptr);
                }
            },
            ValueTag::Object => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark_and_trace(header_ptr);
                }
            },
            ValueTag::Closure => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark_and_trace(header_ptr);
                }
            },
            ValueTag::DynObject => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark_and_trace(header_ptr);
                }
            },
            ValueTag::List => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark_and_trace(header_ptr);
                }
            },
            ValueTag::Tuple => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark_and_trace(header_ptr);
                }
            },
            ValueTag::Continuation => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark_and_trace(header_ptr);
                }
            },
            ValueTag::Effect => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark_and_trace(header_ptr);
                }
            },
            _ => {}
        }
    }
}

impl Trace for Closure {
    fn trace(&self) {
        for upvalue in &self.upvalues {
            upvalue.0.trace();
        }
    }
}

impl Trace for Object {
    fn trace(&self) {
        for field in &self.fields {
            field.trace();
        }
    }
}

impl Trace for Continuation {
    fn trace(&self) {
        for val in &self.stack_slice {
            val.trace();
        }
    }
}

impl Trace for BigInt {
    fn trace(&self) {}
}

impl Trace for DynObject {
    fn trace(&self) {
        for value in self.entries.values() {
            value.trace();
        }
    }
}

impl Trace for Array {
    fn trace(&self) {
        for item in &self.items {
            item.trace();
        }
    }
}

impl Trace for List {
    fn trace(&self) {
        for item in &self.items {
            item.trace();
        }
    }
}

impl Trace for Tuple {
    fn trace(&self) {
        for item in &self.items {
            item.trace();
        }
    }
}

impl Trace for Effect {
    fn trace(&self) {
        for arg in &self.args {
            arg.trace();
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ValueTag {
    Int = 0,
    Bool = 1,
    Null = 2,
    String = 3,
    Array = 4,
    BigInt = 5,
    Object = 6,
    Closure = 7,
    DynObject = 8,
    List = 9,
    Tuple = 10,
    Continuation = 11,
    Effect = 12,
    Code = 13,
    WitnessTable = 14,
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Value(u64);

impl Value {
    pub fn tag(&self) -> ValueTag {
        if self.is_float() {
            panic!("Cannot get tag of float");
        }
        unsafe { transmute(((self.0 & !NAN_BASE) >> TAG_SHIFT) as u8) }
    }

    pub fn payload(&self) -> u64 {
        self.0 & PAYLOAD_MASK
    }

    fn encode(tag: ValueTag, payload: u64) -> Self {
        Value(NAN_BASE | ((tag as u64) << TAG_SHIFT) | (payload & PAYLOAD_MASK))
    }

    pub fn is_float(&self) -> bool {
        (self.0 & NAN_BASE) != NAN_BASE
    }

    pub unsafe fn as_dyn_object<'a>(&self) -> &'a DynObject {
        let ptr = self.payload() as *const GcBox<DynObject>;
        &(*ptr).data
    }
    pub unsafe fn as_dyn_object_mut<'a>(&self) -> &'a mut DynObject {
        let ptr = self.payload() as *mut GcBox<DynObject>;
        &mut (*ptr).data
    }
    pub unsafe fn as_object<'a>(&self) -> &'a Object {
        let ptr = self.payload() as *const GcBox<Object>;
        &(*ptr).data
    }
    pub unsafe fn as_object_mut<'a>(&self) -> &'a mut Object {
        let ptr = self.payload() as *mut GcBox<Object>;
        &mut (*ptr).data
    }
    pub unsafe fn as_closure<'a>(&self) -> &'a Closure {
        let ptr = self.payload() as *const GcBox<Closure>;
        &(*ptr).data
    }
    pub unsafe fn as_closure_mut<'a>(&self) -> &'a mut Closure {
        let ptr = self.payload() as *mut GcBox<Closure>;
        &mut (*ptr).data
    }
    pub unsafe fn as_list<'a>(&self) -> &'a List {
        let ptr = self.payload() as *const GcBox<List>;
        &(*ptr).data
    }
    pub unsafe fn as_list_mut<'a>(&self) -> &'a mut List {
        let ptr = self.payload() as *mut GcBox<List>;
        &mut (*ptr).data
    }
    pub unsafe fn as_tuple<'a>(&self) -> &'a Tuple {
        let ptr = self.payload() as *const GcBox<Tuple>;
        &(*ptr).data
    }
    pub unsafe fn as_tuple_mut<'a>(&self) -> &'a mut Tuple {
        let ptr = self.payload() as *mut GcBox<Tuple>;
        &mut (*ptr).data
    }
    pub unsafe fn as_array_ptr<'a>(&self) -> &'a Array {
        let ptr = self.payload() as *const GcBox<Array>;
        &(*ptr).data
    }
    pub unsafe fn as_array_mut<'a>(&self) -> &'a mut Array {
        let ptr = self.payload() as *mut GcBox<Array>;
        &mut (*ptr).data
    }
    pub unsafe fn as_effect<'a>(&self) -> &'a Effect {
        let ptr = self.payload() as *const GcBox<Effect>;
        &(*ptr).data
    }
    pub unsafe fn as_effect_mut<'a>(&self) -> &'a mut Effect {
        let ptr = self.payload() as *mut GcBox<Effect>;
        &mut (*ptr).data
    }
    pub unsafe fn as_continuation<'a>(&self) -> &'a Continuation {
        let ptr = self.payload() as *const GcBox<Continuation>;
        &(*ptr).data
    }
    pub unsafe fn as_continuation_mut<'a>(&self) -> &'a mut Continuation {
        let ptr = self.payload() as *mut GcBox<Continuation>;
        &mut (*ptr).data
    }
    pub fn int(v: i64) -> Self {
        Self::encode(ValueTag::Int, v as u64)
    }
    pub fn float(v: f64) -> Self {
        let u: u64 = unsafe { transmute(v) };
        if (u & NAN_BASE) == NAN_BASE {
            Value(u & !0x0008_0000_0000_0000)
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
    pub fn string(s: String, gc: &NyarGc) -> Self {
        let g = gc.alloc(s);
        Self::encode(ValueTag::String, g.ptr.as_ptr() as u64)
    }
    pub fn array(items: Vec<Value>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Array { items });
        Self::encode(ValueTag::Array, g.ptr.as_ptr() as u64)
    }
    pub fn bigint(sign: u8, bytes: Vec<u8>, gc: &NyarGc) -> Self {
        let g = gc.alloc(BigInt { sign, bytes });
        Self::encode(ValueTag::BigInt, g.ptr.as_ptr() as u64)
    }
    pub fn dyn_object(gc: &NyarGc) -> Self {
        let g = gc.alloc(DynObject {
            entries: HashMap::new(),
        });
        Self::encode(ValueTag::DynObject, g.ptr.as_ptr() as u64)
    }
    pub fn list(items: Vec<Value>, gc: &NyarGc) -> Self {
        let g = gc.alloc(List { items });
        Self::encode(ValueTag::List, g.ptr.as_ptr() as u64)
    }
    pub fn tuple(items: Vec<Value>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Tuple { items });
        Self::encode(ValueTag::Tuple, g.ptr.as_ptr() as u64)
    }
    pub fn effect(type_idx: u16, args: Vec<Value>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Effect { type_idx, args });
        Self::encode(ValueTag::Effect, g.ptr.as_ptr() as u64)
    }
    pub fn bigint_from_i64(v: i64, gc: &NyarGc) -> Self {
        let g = gc.alloc(BigInt::from_i64(v));
        Self::encode(ValueTag::BigInt, g.ptr.as_ptr() as u64)
    }
    pub fn closure(module_idx: usize, func_idx: u16, upvalues: Vec<Upvalue>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Closure {
            module_idx,
            func: func_idx as usize,
            upvalues,
        });
        Self::encode(ValueTag::Closure, g.ptr.as_ptr() as u64)
    }
    pub fn object(class_idx: u16, fields: Vec<Value>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Object { class_idx, fields });
        Self::encode(ValueTag::Object, g.ptr.as_ptr() as u64)
    }
    pub fn continuation(ip: usize, stack_slice: Vec<Value>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Continuation { ip, stack_slice });
        Self::encode(ValueTag::Continuation, g.ptr.as_ptr() as u64)
    }
    pub fn as_int(&self) -> i64 {
        self.payload() as i64
    }
    pub fn as_float(&self) -> f64 {
        unsafe { transmute(self.0) }
    }
    pub fn as_bool(&self) -> bool {
        self.payload() != 0
    }
    pub unsafe fn as_string<'a>(&self) -> &'a String {
        let ptr = self.payload() as *const GcBox<String>;
        &(*ptr).data
    }
    pub unsafe fn as_array<'a>(&self) -> &'a Array {
        let ptr = self.payload() as *const GcBox<Array>;
        &(*ptr).data
    }
    pub unsafe fn as_bigint<'a>(&self) -> &'a BigInt {
        let ptr = self.payload() as *const GcBox<BigInt>;
        &(*ptr).data
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

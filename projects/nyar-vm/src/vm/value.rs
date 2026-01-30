use nyar_gc::{Trace, NyarGc, GcHeader, GcBox, MarkContext};
use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};
use std::ptr::NonNull;

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
            ValueTag::WitnessTable => write!(f, "<witness_table>"),
            _ => write!(f, "<value>"),
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Value(")?;
        Display::fmt(self, f)?;
        write!(f, ")")
    }
}

impl Trace for Value {
    fn trace(&self, ctx: &mut MarkContext) {
        if self.is_float() {
            return;
        }
        match self.tag() {
            ValueTag::Int | ValueTag::Bool | ValueTag::Null | ValueTag::Code => {}

            ValueTag::WitnessTable => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark(header_ptr, ctx);
                }
            },

            ValueTag::String => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark(header_ptr, ctx);
                }
            },
            ValueTag::BigInt => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark(header_ptr, ctx);
                }
            },
            ValueTag::Array => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark(header_ptr, ctx);
                }
            },
            ValueTag::Object => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark(header_ptr, ctx);
                }
            },
            ValueTag::Closure => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark(header_ptr, ctx);
                }
            },
            ValueTag::DynObject => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark(header_ptr, ctx);
                }
            },
            ValueTag::List => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark(header_ptr, ctx);
                }
            },
            ValueTag::Tuple => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark(header_ptr, ctx);
                }
            },
            ValueTag::Continuation => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark(header_ptr, ctx);
                }
            },
            ValueTag::Effect => unsafe {
                let payload = self.payload();
                if payload != 0 {
                    let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                    GcHeader::mark(header_ptr, ctx);
                }
            },
            _ => {}
        }
    }
}

impl Trace for Closure {
    fn trace(&self, ctx: &mut MarkContext) {
        for upvalue in &self.upvalues {
            upvalue.0.trace(ctx);
        }
    }
}

impl Trace for Object {
    fn trace(&self, ctx: &mut MarkContext) {
        for field in &self.fields {
            field.trace(ctx);
        }
    }
}

impl Trace for Continuation {
    fn trace(&self, ctx: &mut MarkContext) {
        for val in &self.stack_slice {
            val.trace(ctx);
        }
    }
}

impl Trace for BigInt {
    fn trace(&self, _ctx: &mut MarkContext) {}
}

impl Trace for DynObject {
    fn trace(&self, ctx: &mut MarkContext) {
        for value in self.entries.values() {
            value.trace(ctx);
        }
    }
}

impl Trace for Array {
    fn trace(&self, ctx: &mut MarkContext) {
        for item in &self.items {
            item.trace(ctx);
        }
    }
}

impl Trace for List {
    fn trace(&self, ctx: &mut MarkContext) {
        for item in &self.items {
            item.trace(ctx);
        }
    }
}

impl Trace for Tuple {
    fn trace(&self, ctx: &mut MarkContext) {
        for item in &self.items {
            item.trace(ctx);
        }
    }
}

impl Trace for Effect {
    fn trace(&self, ctx: &mut MarkContext) {
        for arg in &self.args {
            arg.trace(ctx);
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
    Float = 15,
    Function = 16,
    TraitObject = 17,
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Value(u64);

impl Value {
    pub fn tag(&self) -> ValueTag {
        if self.is_float() {
            return ValueTag::Float;
        }
        let tag_val = ((self.0 & !NAN_BASE) >> TAG_SHIFT) as u8;
        match tag_val {
            0 => ValueTag::Int,
            1 => ValueTag::Bool,
            2 => ValueTag::Null,
            3 => ValueTag::String,
            4 => ValueTag::Array,
            5 => ValueTag::BigInt,
            6 => ValueTag::Object,
            7 => ValueTag::Closure,
            8 => ValueTag::DynObject,
            9 => ValueTag::List,
            10 => ValueTag::Tuple,
            11 => ValueTag::Continuation,
            12 => ValueTag::Effect,
            13 => ValueTag::Code,
            14 => ValueTag::WitnessTable,
            15 => ValueTag::Float,
            16 => ValueTag::Function,
            17 => ValueTag::TraitObject,
            _ => panic!("Invalid tag value: {}", tag_val),
        }
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
    pub unsafe fn as_witness_table<'a>(&self) -> &'a WitnessTable {
        let ptr = self.payload() as *const GcBox<WitnessTable>;
        &(*ptr).data
    }
    pub unsafe fn as_witness_table_mut<'a>(&self) -> &'a mut WitnessTable {
        let ptr = self.payload() as *mut GcBox<WitnessTable>;
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
        let u: u64 = v.to_bits();
        if (u & NAN_BASE) == NAN_BASE {
            // Canonicalize NaN to avoid collision with tags.
            // We clear bit 51 (the bit that makes it look like a tag)
            // and set bit 0 to ensure it remains a NaN.
            Value((u & !0x0008_0000_0000_0000) | 1)
        } else {
            Value(u)
        }
    }

    pub fn is_int(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::Int
    }

    pub fn is_bool(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::Bool
    }

    pub fn is_null(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::Null
    }

    pub fn is_string(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::String
    }

    pub fn is_object(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::Object
    }

    pub fn is_array(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::Array
    }

    pub fn is_list(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::List
    }

    pub fn is_tuple(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::Tuple
    }

    pub fn is_dyn_object(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::DynObject
    }

    pub fn is_closure(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::Closure
    }

    pub fn is_effect(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::Effect
    }

    pub fn is_continuation(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::Continuation
    }

    pub fn is_function(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::Function
    }

    pub fn is_trait_object(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::TraitObject
    }

    pub fn is_witness_table(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::WitnessTable
    }

    pub fn is_bigint(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::BigInt
    }

    pub fn is_truthy(&self) -> bool {
        if self.is_float() {
            return !self.as_float().is_nan() && self.as_float() != 0.0;
        }
        match self.tag() {
            ValueTag::Bool => self.as_bool(),
            ValueTag::Null => false,
            ValueTag::Int => self.as_int() != 0,
            _ => true,
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
        Self::encode(ValueTag::String, g.as_ptr() as u64)
    }
    pub fn array(items: Vec<Value>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Array { items });
        Self::encode(ValueTag::Array, g.as_ptr() as u64)
    }
    pub fn bigint(sign: u8, bytes: Vec<u8>, gc: &NyarGc) -> Self {
        let g = gc.alloc(BigInt { sign, bytes });
        Self::encode(ValueTag::BigInt, g.as_ptr() as u64)
    }
    pub fn dyn_object(gc: &NyarGc) -> Self {
        let g = gc.alloc(DynObject {
            entries: HashMap::new(),
        });
        Self::encode(ValueTag::DynObject, g.as_ptr() as u64)
    }
    pub fn list(items: Vec<Value>, gc: &NyarGc) -> Self {
        let g = gc.alloc(List { items });
        Self::encode(ValueTag::List, g.as_ptr() as u64)
    }
    pub fn tuple(items: Vec<Value>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Tuple { items });
        Self::encode(ValueTag::Tuple, g.as_ptr() as u64)
    }
    pub fn effect(type_idx: u16, args: Vec<Value>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Effect { type_idx, args });
        Self::encode(ValueTag::Effect, g.as_ptr() as u64)
    }
    pub fn witness_table(module_idx: usize, methods: Vec<u16>, gc: &NyarGc) -> Self {
        let g = gc.alloc(WitnessTable { module_idx, methods });
        Self::encode(ValueTag::WitnessTable, g.as_ptr() as u64)
    }
    pub fn bigint_from_i64(v: i64, gc: &NyarGc) -> Self {
        let g = gc.alloc(BigInt::from_i64(v));
        Self::encode(ValueTag::BigInt, g.as_ptr() as u64)
    }
    pub fn closure(module_idx: usize, func_idx: u16, upvalues: Vec<Upvalue>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Closure {
            module_idx,
            func: func_idx as usize,
            upvalues,
        });
        Self::encode(ValueTag::Closure, g.as_ptr() as u64)
    }
    pub fn object(class_idx: u16, fields: Vec<Value>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Object { class_idx, fields });
        Self::encode(ValueTag::Object, g.as_ptr() as u64)
    }
    pub fn continuation(ip: usize, stack_slice: Vec<Value>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Continuation { ip, stack_slice });
        Self::encode(ValueTag::Continuation, g.as_ptr() as u64)
    }
    pub fn as_int(&self) -> i64 {
        self.payload() as i64
    }
    pub fn as_float(&self) -> f64 {
        f64::from_bits(self.0)
    }
    pub fn as_bool(&self) -> bool {
        self.payload() != 0
    }
    pub unsafe fn as_string<'a>(&self) -> &'a String {
        let ptr = self.payload() as *const GcBox<String>;
        &(*ptr).data
    }
    pub fn try_as_str(&self) -> Option<&str> {
        if self.is_string() {
            Some(unsafe { self.as_string().as_str() })
        } else {
            None
        }
    }
    pub unsafe fn as_array<'a>(&self) -> &'a Array {
        let ptr = self.payload() as *const GcBox<Array>;
        &(*ptr).data
    }
    pub fn try_as_array(&self) -> Option<&Array> {
        if self.is_array() {
            Some(unsafe { self.as_array() })
        } else {
            None
        }
    }
    pub unsafe fn as_bigint<'a>(&self) -> &'a BigInt {
        let ptr = self.payload() as *const GcBox<BigInt>;
        &(*ptr).data
    }
    pub fn try_as_bigint(&self) -> Option<&BigInt> {
        if self.is_bigint() {
            Some(unsafe { self.as_bigint() })
        } else {
            None
        }
    }
    pub fn try_as_int(&self) -> Option<i64> {
        if self.is_int() {
            Some(self.as_int())
        } else {
            None
        }
    }
    pub fn try_as_float(&self) -> Option<f64> {
        if self.is_float() {
            Some(self.as_float())
        } else {
            None
        }
    }
    pub fn try_as_bool(&self) -> Option<bool> {
        if self.is_bool() {
            Some(self.as_bool())
        } else {
            None
        }
    }
    pub fn try_as_object(&self) -> Option<&Object> {
        if self.is_object() {
            Some(unsafe { self.as_object() })
        } else {
            None
        }
    }
    pub fn try_as_closure(&self) -> Option<&Closure> {
        if self.is_closure() {
            Some(unsafe { self.as_closure() })
        } else {
            None
        }
    }
    pub fn try_as_dyn_object(&self) -> Option<&DynObject> {
        if self.is_dyn_object() {
            Some(unsafe { self.as_dyn_object() })
        } else {
            None
        }
    }
    pub fn try_as_list(&self) -> Option<&List> {
        if self.is_list() {
            Some(unsafe { self.as_list() })
        } else {
            None
        }
    }
    pub fn try_as_tuple(&self) -> Option<&Tuple> {
        if self.is_tuple() {
            Some(unsafe { self.as_tuple() })
        } else {
            None
        }
    }
    pub fn try_as_continuation(&self) -> Option<&Continuation> {
        if self.is_continuation() {
            Some(unsafe { self.as_continuation() })
        } else {
            None
        }
    }
    pub fn try_as_witness_table(&self) -> Option<&WitnessTable> {
        if self.is_witness_table() {
            Some(unsafe { self.as_witness_table() })
        } else {
            None
        }
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

#[derive(Clone)]
pub struct WitnessTable {
    pub module_idx: usize,
    pub methods: Vec<u16>,
}

impl Trace for WitnessTable {
    fn trace(&self, _ctx: &mut MarkContext) {}
}

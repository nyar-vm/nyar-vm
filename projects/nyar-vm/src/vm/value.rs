use crate::bytecode::instruction::Instruction;
use num_bigint::BigInt as NativeBigInt;
use num_traits::{FromPrimitive, ToPrimitive};
use nyar_gc::{GcBox, GcHeader, MarkContext, NyarGc, Trace};
use nyar_types::QualifiedName;
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
            ValueTag::String => {
                if let Some(s) = self.try_as_str() {
                    write!(f, "{}", s)
                } else {
                    write!(f, "<invalid_string>")
                }
            }
            ValueTag::BigInt => {
                if let Some(bi) = self.try_as_bigint() {
                    write!(f, "{}", bi.0)
                } else {
                    write!(f, "<invalid_bigint>")
                }
            }
            ValueTag::Array => write!(f, "[...]"),
            ValueTag::Object => write!(f, "{{...}}"),
            ValueTag::Closure => write!(f, "<closure>"),
            ValueTag::QualifiedName => {
                if let Some(qn) = self.try_as_qualified_name() {
                    write!(f, "{}", qn)
                } else {
                    write!(f, "<invalid_qualified_name>")
                }
            }
            ValueTag::DynObject => write!(f, "<dyn_object>"),
            ValueTag::WitnessTable => write!(f, "<witness_table>"),
            ValueTag::Bytes => write!(f, "<bytes>"),
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
        let payload = self.payload();
        if payload == 0 {
            return;
        }
        match self.tag() {
            ValueTag::Int | ValueTag::Bool | ValueTag::Null | ValueTag::Code => {}
            ValueTag::WitnessTable
            | ValueTag::String
            | ValueTag::BigInt
            | ValueTag::Array
            | ValueTag::Object
            | ValueTag::Closure
            | ValueTag::DynObject
            | ValueTag::List
            | ValueTag::Tuple
            | ValueTag::Continuation
            | ValueTag::Function
            | ValueTag::TraitObject
            | ValueTag::QualifiedName
            | ValueTag::Future
            | ValueTag::Effect
            | ValueTag::Bytes => unsafe {
                let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                GcHeader::mark(header_ptr, ctx);
            },
            _ => {}
        }
    }
}

impl Trace for TraitObject {
    fn trace(&self, ctx: &mut MarkContext) {
        self.data.trace(ctx);
        self.witness.trace(ctx);
    }
}

impl Trace for Closure {
    fn trace(&self, ctx: &mut MarkContext) {
        for upvalue in &self.upvalues {
            upvalue.get().trace(ctx);
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
        for frame in &self.frames {
            frame.trace(ctx);
        }
    }
}

impl Trace for Frame {
    fn trace(&self, ctx: &mut MarkContext) {
        self.closure.trace(ctx);
        for local in &self.locals {
            local.trace(ctx);
        }
    }
}

impl Trace for BigInt {
    #[inline(always)]
    fn trace(&self, _ctx: &mut MarkContext) {
        // BigInt optimization:
        // 1. Trace is a no-op because it contains no GC-managed pointers.
        // 2. Marked as #[inline(always)] to minimize call overhead during GC marking.
        // 
        // Memory Layout & GC Integration:
        // [ GcBox<BigInt> (GC Heap) ]
        // |-> [ GcHeader (8 bytes) ]
        // |-> [ BigInt (24 bytes) ] 
        //     |-> [ NativeBigInt (num_bigint::BigInt) ]
        //         |-> sign: Sign (1 byte + padding)
        //         |-> data: Vec<u32> (24 bytes)
        //             |-> [ ptr ] -> [ u32, u32, ... ] (Standard Heap)
        //             |-> [ cap ]
        //             |-> [ len ]
        // 
        // Note: The Vec<u32> is allocated via the standard allocator.
        // For future optimization, a custom allocator for num_bigint that
        // uses GC-managed memory could be implemented to avoid hybrid heap usage.
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FutureStatus {
    Pending,
    Ready,
    Failed,
}

pub struct Future {
    pub status: FutureStatus,
    pub result: Value,
    pub waker: Option<std::task::Waker>,
}

impl Trace for Future {
    fn trace(&self, ctx: &mut MarkContext) {
        self.result.trace(ctx);
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
    F64 = 15,
    Function = 16,
    TraitObject = 17,
    QualifiedName = 18,
    Future = 19,
    F32 = 20,
    Bytes = 21,
}

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Value(u64);

impl Value {
    pub fn write_barrier(&self, gc: &NyarGc) {
        if self.is_float() {
            return;
        }
        let payload = self.payload();
        if payload == 0 {
            return;
        }
        match self.tag() {
            ValueTag::WitnessTable
            | ValueTag::String
            | ValueTag::BigInt
            | ValueTag::Array
            | ValueTag::Object
            | ValueTag::Closure
            | ValueTag::DynObject
            | ValueTag::List
            | ValueTag::Tuple
            | ValueTag::Continuation
            | ValueTag::Function
            | ValueTag::TraitObject
            | ValueTag::QualifiedName
            | ValueTag::Effect
            | ValueTag::Bytes => unsafe {
                let header_ptr = NonNull::new_unchecked(payload as *mut GcHeader);
                gc.write_barrier_ptr(header_ptr);
            },
            ValueTag::Code => {} // Code objects are usually immutable/static but can be traced
            _ => {}
        }
    }

    pub fn tag(&self) -> ValueTag {
        if self.is_float() {
            return ValueTag::F64;
        }
        let mut tag_val = ((self.0 & 0x0007_8000_0000_0000) >> TAG_SHIFT) as u8;
        if (self.0 & 0x8000_0000_0000_0000) != 0 {
            tag_val |= 0x10;
        }
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
            15 => ValueTag::F64,
            16 => ValueTag::Function,
            17 => ValueTag::TraitObject,
            18 => ValueTag::QualifiedName,
            19 => ValueTag::Future,
            20 => ValueTag::F32,
            21 => ValueTag::Bytes,
            _ => panic!("Invalid tag value: {} (raw={:016x})", tag_val, self.0),
        }
    }

    pub fn payload(&self) -> u64 {
        self.0 & PAYLOAD_MASK
    }

    pub fn pointer_payload(&self) -> *mut u8 {
        self.payload() as *mut u8
    }

    pub fn as_raw_ptr(&self) -> *mut u8 {
        if self.is_float() {
            return std::ptr::null_mut();
        }
        match self.tag() {
            ValueTag::Int => self.as_int() as *mut u8,
            ValueTag::String => unsafe {
                let ptr = self.payload() as *const GcBox<String>;
                (*ptr).data.as_ptr() as *mut u8
            }
            ValueTag::Bytes => unsafe {
                let ptr = self.payload() as *const GcBox<Bytes>;
                (*ptr).data.data.as_ptr() as *mut u8
            }
            _ => std::ptr::null_mut(),
        }
    }

    fn encode(tag: ValueTag, payload: u64) -> Self {
        let tag_val = tag as u64;
        let mut u = NAN_BASE | ((tag_val & 0xF) << TAG_SHIFT) | (payload & PAYLOAD_MASK);
        if (tag_val & 0x10) != 0 {
            u |= 0x8000_0000_0000_0000;
        }
        Value(u)
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
    pub unsafe fn as_bytes<'a>(&self) -> &'a Bytes {
        let ptr = self.payload() as *const GcBox<Bytes>;
        &(*ptr).data
    }
    pub unsafe fn as_bytes_mut<'a>(&self) -> &'a mut Bytes {
        let ptr = self.payload() as *mut GcBox<Bytes>;
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
    pub unsafe fn as_trait_object<'a>(&self) -> &'a TraitObject {
        let ptr = self.payload() as *const GcBox<TraitObject>;
        &(*ptr).data
    }
    pub unsafe fn as_trait_object_mut<'a>(&self) -> &'a mut TraitObject {
        let ptr = self.payload() as *mut GcBox<TraitObject>;
        &mut (*ptr).data
    }
    pub unsafe fn as_future<'a>(&self) -> &'a Future {
        let ptr = self.payload() as *const GcBox<Future>;
        &(*ptr).data
    }
    pub unsafe fn as_future_mut<'a>(&self) -> &'a mut Future {
        let ptr = self.payload() as *mut GcBox<Future>;
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

    pub fn f32(v: f32) -> Self {
        Self::encode(ValueTag::F32, v.to_bits() as u64)
    }

    pub fn is_int(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::Int
    }

    pub fn is_f32(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::F32
    }

    pub fn is_f64(&self) -> bool {
        self.is_float() || self.tag() == ValueTag::F64
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

    pub fn is_future(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::Future
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

    pub fn is_bytes(&self) -> bool {
        !self.is_float() && self.tag() == ValueTag::Bytes
    }

    pub fn is_truthy(&self) -> bool {
        if self.is_f64() {
            let f = self.as_f64();
            return !f.is_nan() && f != 0.0;
        }
        if self.is_f32() {
            let f = self.as_f32();
            return !f.is_nan() && f != 0.0;
        }
        match self.tag() {
            ValueTag::Bool => self.as_bool(),
            ValueTag::Null => false,
            ValueTag::Int => self.as_int() != 0,
            ValueTag::String => self.try_as_str().map(|s| !s.is_empty()).unwrap_or(true),
            ValueTag::Array => self
                .try_as_array()
                .map(|a| !a.items.is_empty())
                .unwrap_or(true),
            ValueTag::List => self
                .try_as_list()
                .map(|l| !l.items.is_empty())
                .unwrap_or(true),
            ValueTag::DynObject => self
                .try_as_dyn_object()
                .map(|d| !d.entries.is_empty())
                .unwrap_or(true),
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
    pub fn bigint(bi: BigInt, gc: &NyarGc) -> Self {
        let g = gc.alloc(bi);
        Self::encode(ValueTag::BigInt, g.as_ptr() as u64)
    }
    pub fn bytes(data: Vec<u8>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Bytes { data });
        Self::encode(ValueTag::Bytes, g.as_ptr() as u64)
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
    pub fn effect(info: nyar_types::EffectInfo, args: Vec<Value>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Effect { info, args });
        Self::encode(ValueTag::Effect, g.as_ptr() as u64)
    }
    pub fn qualified_name(name: QualifiedName, gc: &NyarGc) -> Self {
        let g = gc.alloc(name);
        Self::encode(ValueTag::QualifiedName, g.as_ptr() as u64)
    }
    pub fn witness_table(module_idx: usize, methods: Vec<u16>, gc: &NyarGc) -> Self {
        let g = gc.alloc(WitnessTable {
            module_idx,
            methods,
        });
        Self::encode(ValueTag::WitnessTable, g.as_ptr() as u64)
    }
    pub fn function(module_idx: usize, chunk_idx: usize, gc: &NyarGc) -> Self {
        let g = gc.alloc(Code {
            module_idx,
            chunk_idx,
        });
        Self::encode(ValueTag::Function, g.as_ptr() as u64)
    }
    pub fn code(module_idx: usize, chunk_idx: usize, gc: &NyarGc) -> Self {
        let g = gc.alloc(Code {
            module_idx,
            chunk_idx,
        });
        Self::encode(ValueTag::Code, g.as_ptr() as u64)
    }
    pub fn bigint_from_i64(v: i64, gc: &NyarGc) -> Self {
        Self::bigint(BigInt::from_i64(v), gc)
    }
    pub fn closure(module_idx: usize, func_idx: u16, upvalues: Vec<Upvalue>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Closure {
            module_idx,
            func: func_idx as usize,
            upvalues,
        });
        Self::encode(ValueTag::Closure, g.as_ptr() as u64)
    }
    pub fn object(module_idx: usize, class_idx: u16, fields: Vec<Value>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Object { module_idx, class_idx, fields });
        Self::encode(ValueTag::Object, g.as_ptr() as u64)
    }
    pub fn trait_object(data: Value, witness: Value, gc: &NyarGc) -> Self {
        let g = gc.alloc(TraitObject { data, witness });
        Self::encode(ValueTag::TraitObject, g.as_ptr() as u64)
    }
    pub fn continuation(ip: usize, stack_slice: Vec<Value>, frames: Vec<Frame>, gc: &NyarGc) -> Self {
        let g = gc.alloc(Continuation {
            ip,
            stack_slice,
            frames,
        });
        Self::encode(ValueTag::Continuation, g.as_ptr() as u64)
    }
    pub fn future(gc: &NyarGc) -> Self {
        let g = gc.alloc(Future {
            status: FutureStatus::Pending,
            result: Value::null(),
            waker: None,
        });
        Self::encode(ValueTag::Future, g.as_ptr() as u64)
    }
    pub fn as_int(&self) -> i64 {
        self.payload() as i64
    }
    pub fn as_float(&self) -> f64 {
        if self.is_f32() {
            self.as_f32() as f64
        } else {
            self.as_f64()
        }
    }
    pub fn to_f64(&self) -> f64 {
        if self.is_float() {
            self.as_f64()
        } else {
            match self.tag() {
                ValueTag::Int => self.as_int() as f64,
                ValueTag::F32 => self.as_f32() as f64,
                ValueTag::F64 => self.as_f64(),
                _ => f64::NAN,
            }
        }
    }
    pub fn as_f32(&self) -> f32 {
        f32::from_bits(self.payload() as u32)
    }
    pub fn as_f64(&self) -> f64 {
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
    pub fn try_as_qualified_name(&self) -> Option<&QualifiedName> {
        if self.tag() == ValueTag::QualifiedName {
            let ptr = self.payload() as *const GcBox<QualifiedName>;
            Some(unsafe { &(*ptr).data })
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
    pub fn try_as_array_mut(&self) -> Option<&mut Array> {
        if self.is_array() {
            Some(unsafe { self.as_array_mut() })
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
        if self.is_f64() {
            Some(self.as_f64())
        } else if self.is_f32() {
            Some(self.as_f32() as f64)
        } else {
            None
        }
    }
    pub fn try_as_f32(&self) -> Option<f32> {
        if self.is_f32() {
            Some(self.as_f32())
        } else {
            None
        }
    }
    pub fn try_as_f64(&self) -> Option<f64> {
        if self.is_f64() {
            Some(self.as_f64())
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
    pub fn try_as_trait_object(&self) -> Option<&TraitObject> {
        if self.tag() == ValueTag::TraitObject {
            Some(unsafe { self.as_trait_object() })
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
    pub fn try_as_closure_mut(&self) -> Option<&mut Closure> {
        if self.is_closure() {
            Some(unsafe { self.as_closure_mut() })
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
    pub fn try_as_list_mut(&self) -> Option<&mut List> {
        if self.is_list() {
            Some(unsafe { self.as_list_mut() })
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
    pub fn try_as_tuple_mut(&self) -> Option<&mut Tuple> {
        if self.is_tuple() {
            Some(unsafe { self.as_tuple_mut() })
        } else {
            None
        }
    }
    pub fn try_as_bytes(&self) -> Option<&Bytes> {
        if self.is_bytes() {
            Some(unsafe { self.as_bytes() })
        } else {
            None
        }
    }
    pub fn try_as_bytes_mut(&self) -> Option<&mut Bytes> {
        if self.is_bytes() {
            Some(unsafe { self.as_bytes_mut() })
        } else {
            None
        }
    }
    pub fn try_as_effect(&self) -> Option<&Effect> {
        if self.is_effect() {
            Some(unsafe { self.as_effect() })
        } else {
            None
        }
    }
    pub fn try_as_object_mut(&self) -> Option<&mut Object> {
        if self.is_object() {
            Some(unsafe { self.as_object_mut() })
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
    pub fn try_as_dyn_object_mut(&self) -> Option<&mut DynObject> {
        if self.is_dyn_object() {
            Some(unsafe { self.as_dyn_object_mut() })
        } else {
            None
        }
    }
    pub fn try_as_future(&self) -> Option<&Future> {
        if self.is_future() {
            Some(unsafe { self.as_future() })
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
    pub data: Value,
    pub witness: Value,
}

#[derive(Clone)]
pub struct Upvalue(pub std::sync::Arc<std::sync::atomic::AtomicU64>);

impl Upvalue {
    pub fn new(val: Value) -> Self {
        Self(std::sync::Arc::new(std::sync::atomic::AtomicU64::new(val.0)))
    }
    pub fn get(&self) -> Value {
        Value(self.0.load(std::sync::atomic::Ordering::Relaxed))
    }
    pub fn set(&self, val: Value) {
        self.0.store(val.0, std::sync::atomic::Ordering::Relaxed);
    }
}

#[derive(Clone)]
pub struct Closure {
    pub module_idx: usize,
    pub func: usize,
    pub upvalues: Vec<Upvalue>,
}

#[derive(Clone)]
pub struct Object {
    pub module_idx: usize,
    pub class_idx: u16,
    pub fields: Vec<Value>,
}

#[derive(Clone)]
pub struct Continuation {
    pub ip: usize,
    pub stack_slice: Vec<Value>,
    pub frames: Vec<Frame>,
}

#[derive(Clone)]
pub struct Frame {
    pub instrs: std::sync::Arc<Vec<(Instruction, u32)>>,
    pub ip: usize,
    pub locals: Vec<Value>,
    pub upvalues: Vec<Option<Upvalue>>,
    pub closure: Value,
    pub module_idx: usize,
    pub chunk_idx: Option<usize>,
    pub location: nyar_types::SourceLocation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BigInt(pub NativeBigInt);

impl BigInt {
    pub fn to_i64(&self) -> i64 {
        self.0.to_i64().unwrap_or(0)
    }
    pub fn from_i64(v: i64) -> Self {
        BigInt(NativeBigInt::from_i64(v).unwrap())
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
pub struct Bytes {
    pub data: Vec<u8>,
}

impl Trace for Bytes {
    fn trace(&self, _ctx: &mut MarkContext) {
        // Vec<u8> is now traceable (no-op)
    }
}

#[derive(Clone)]
pub struct Effect {
    pub info: nyar_types::EffectInfo,
    pub args: Vec<Value>,
}

impl Trace for Effect {
    fn trace(&self, ctx: &mut MarkContext) {
        self.info.name.trace(ctx);
        for arg in &self.args {
            arg.trace(ctx);
        }
    }
}

#[derive(Clone)]
pub struct WitnessTable {
    pub module_idx: usize,
    pub methods: Vec<u16>,
}

#[derive(Clone)]
pub struct Code {
    pub module_idx: usize,
    pub chunk_idx: usize,
}

impl Trace for Code {
    fn trace(&self, _ctx: &mut MarkContext) {}
}

impl Trace for WitnessTable {
    fn trace(&self, _ctx: &mut MarkContext) {}
}

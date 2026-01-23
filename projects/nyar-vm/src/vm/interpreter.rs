use crate::bytecode::decoder::Instruction;
use crate::bytecode::format::{Chunk, ClassInfo, Constant, ImplInfo, NyarcModule, TraitInfo};
use crate::vm::effects::{perform_effect_internal, HandlerFrame};
use crate::vm::ffi::{FFIFunction, FFIRegistry, FFIResult};
use crate::vm::value::{BigInt, Closure, Upvalue, Value, ValueTag};
use crate::vm::VmError;
use nyar_gc::{NyarGc, Trace};
use std::ptr::null;

fn normalize(mut v: Vec<u8>) -> Vec<u8> {
    while let Some(&last) = v.last() {
        if last == 0 {
            v.pop();
        } else {
            break;
        }
    }
    v
}

fn to_u128(bytes: &[u8]) -> Option<u128> {
    if bytes.len() > 16 {
        return None;
    }
    let mut x: u128 = 0;
    let mut shift = 0u32;
    for &b in bytes {
        x |= (b as u128) << shift;
        shift += 8;
    }
    Some(x)
}

fn from_u128(mut x: u128) -> Vec<u8> {
    let mut out = Vec::new();
    while x > 0 {
        out.push((x & 0xFF) as u8);
        x >>= 8;
    }
    out
}

fn key_is_string(v: &Value) -> bool {
    v.tag == ValueTag::String
}

fn cmp_abs(a: &[u8], b: &[u8]) -> i8 {
    let la = a.len();
    let lb = b.len();
    if la != lb {
        return if la < lb { -1 } else { 1 };
    }
    let mut i = la;
    while i > 0 {
        let aa = a[i - 1];
        let bb = b[i - 1];
        if aa != bb {
            return if aa < bb { -1 } else { 1 };
        }
        i -= 1;
    }
    0
}

fn add_abs(a: &[u8], b: &[u8]) -> Vec<u8> {
    if let (Some(x), Some(y)) = (to_u128(a), to_u128(b)) {
        return from_u128(x + y);
    }
    let n = a.len().max(b.len());
    let mut out = Vec::with_capacity(n + 1);
    let mut carry = 0u16;
    for i in 0..n {
        let ai = if i < a.len() { a[i] as u16 } else { 0 };
        let bi = if i < b.len() { b[i] as u16 } else { 0 };
        let s = ai + bi + carry;
        out.push((s & 0xFF) as u8);
        carry = s >> 8;
    }
    if carry != 0 {
        out.push(carry as u8);
    }
    normalize(out)
}

fn sub_abs(a: &[u8], b: &[u8]) -> Vec<u8> {
    if let (Some(x), Some(y)) = (to_u128(a), to_u128(b)) {
        return from_u128(x.wrapping_sub(y));
    }
    let n = a.len();
    let mut out = Vec::with_capacity(n);
    let mut borrow = 0i16;
    for i in 0..n {
        let ai = a[i] as i16;
        let bi = if i < b.len() { b[i] as i16 } else { 0 };
        let mut d = ai - bi - borrow;
        if d < 0 {
            d += 256;
            borrow = 1;
        } else {
            borrow = 0;
        }
        out.push((d & 0xFF) as u8);
    }
    normalize(out)
}

fn mul_abs(a: &[u8], b: &[u8]) -> Vec<u8> {
    if let (Some(x), Some(y)) = (to_u128(a), to_u128(b)) {
        return from_u128(x * y);
    }
    let mut out = vec![0u8; a.len() + b.len()];
    for i in 0..a.len() {
        let mut carry = 0u16;
        for j in 0..b.len() {
            let k = i + j;
            let prod = (a[i] as u16) * (b[j] as u16) + (out[k] as u16) + carry;
            out[k] = (prod & 0xFF) as u8;
            carry = prod >> 8;
        }
        if carry != 0 {
            out[i + b.len()] = (out[i + b.len()] as u16 + carry) as u8;
        }
    }
    normalize(out)
}

fn div_mod_abs(mut a: Vec<u8>, b: &[u8]) -> (Vec<u8>, Vec<u8>) {
    if b.is_empty() {
        return (Vec::new(), a);
    }
    if let (Some(x), Some(y)) = (to_u128(&a), to_u128(b)) {
        if y != 0 {
            return (from_u128(x / y), from_u128(x % y));
        }
    }
    let mut q = 0u128;
    while cmp_abs(&a, b) >= 0 {
        a = sub_abs(&a, b);
        q = q.wrapping_add(1);
    }
    (from_u128(q), a)
}

#[derive(Clone)]
struct Frame {
    instrs: Vec<Instruction>,
    ip: usize,
    locals: Vec<Value>,
    closure: *const Closure,
    module_idx: usize,
    chunk_idx: Option<usize>,
}

pub struct NyarVM {
    pub gc: NyarGc,
    stack: Vec<Value>,
    sp: usize,
    frames: Vec<Frame>,
    pub modules: Vec<NyarcModule>,
    pub handler_stack: Vec<HandlerFrame>,
    #[allow(clippy::type_complexity)]
    pub stdout: Option<Box<dyn Fn(&str)>>,
    pub trace_log: std::cell::RefCell<Vec<String>>,
    pub ffi: FFIRegistry,
    pub symbol_table: std::collections::HashMap<String, (usize, u16)>, // (module_idx, chunk_idx)
}

impl Trace for NyarVM {
    fn trace(&self) {
        for i in 0..self.sp {
            self.stack[i].trace();
        }
        for frame in &self.frames {
            for local in &frame.locals {
                local.trace();
            }
        }
    }
}

impl NyarVM {
    pub fn new() -> Self {
        let mut vm = Self {
            gc: NyarGc::new(),
            stack: Vec::with_capacity(64),
            sp: 0,
            frames: Vec::new(),
            modules: Vec::new(),
            handler_stack: Vec::new(),
            stdout: None,
            trace_log: std::cell::RefCell::new(Vec::new()),
            ffi: FFIRegistry::new(),
            symbol_table: std::collections::HashMap::new(),
        };
        vm.register_builtins();
        vm
    }

    fn register_builtins(&mut self) {
        // Builtins can be registered here
    }

    fn push(&mut self, v: Value) {
        if self.sp >= self.stack.len() {
            self.stack.push(v)
        } else {
            self.stack[self.sp] = v
        }
        self.sp += 1
    }

    fn pop(&mut self) -> Result<Value, VmError> {
        if self.sp == 0 {
            Err(VmError::StackUnderflow)
        } else {
            self.sp -= 1;
            Ok(self.stack[self.sp])
        }
    }

    fn peek_at(&self, depth: usize) -> Result<Value, VmError> {
        if depth >= self.sp {
            Err(VmError::StackUnderflow)
        } else {
            Ok(self.stack[self.sp - 1 - depth])
        }
    }

    fn swap_with(&mut self, depth: usize) -> Result<(), VmError> {
        if depth >= self.sp {
            Err(VmError::StackUnderflow)
        } else {
            let top = self.sp - 1;
            let idx = self.sp - 1 - depth;
            self.stack.swap(top, idx);
            Ok(())
        }
    }

    fn print_line(&self, msg: &str) {
        if let Some(cb) = &self.stdout {
            cb(msg);
        } else {
            println!("{}", msg);
        }
        self.trace_log.borrow_mut().push(msg.to_string());
    }

    pub fn log(&self, msg: &str) {
        self.print_line(msg);
    }

    pub fn print_traceback(&self, err: &VmError) {
        self.print_line("Traceback (most recent call last):");
        let start = if self.frames.len() > 20 {
            self.print_line(&format!("... ({} frames omitted)", self.frames.len() - 20));
            self.frames.len() - 20
        } else {
            0
        };
        for (i, f) in self.frames.iter().enumerate().skip(start) {
            let info = match f.chunk_idx {
                Some(ci) => format!("frame {}: module={}, chunk={}, ip={}", i, f.module_idx, ci, f.ip),
                None => format!("frame {}: module={}, chunk=<entry>, ip={}", i, f.module_idx, f.ip),
            };
            self.print_line(&info);
        }
        match err {
            VmError::UnhandledEffect(name) => {
                self.print_line(&format!("UnhandledEffect: {}", name))
            }
            VmError::UnhandledError => self.print_line("UnhandledError"),
            VmError::RuntimeError(msg) => self.print_line(&format!("RuntimeError: {}", msg)),
            _ => self.print_line("Error"),
        }
    }

    pub fn load_module(&mut self, module: NyarcModule) -> usize {
        let module_idx = self.modules.len();
        
        // Update symbol table with exports from this module
        for export in &module.exports {
            self.symbol_table.insert(export.symbol.clone(), (module_idx, export.chunk_idx));
        }
        
        self.modules.push(module);
        module_idx
    }

    pub fn execute(&mut self, module_idx: usize, chunk_idx: usize) -> Result<Value, VmError> {
        let module = &self.modules[module_idx];
        let chunk = &module.chunks[chunk_idx];
        
        // Decode chunk code to instructions
        let mut decoder = crate::bytecode::decoder::Decoder::new(&chunk.code);
        let mut instructions = Vec::new();
        while let Ok(ins) = decoder.next_result() {
            instructions.push(ins);
        }

        let frame = Frame {
            instrs: instructions,
            ip: 0,
            locals: vec![Value::null(); 32],
            closure: std::ptr::null(),
            module_idx,
            chunk_idx: Some(chunk_idx),
        };
        
        self.frames.push(frame);
        self.run_loop()
    }

    fn run_loop(&mut self) -> Result<Value, VmError> {
        let mut loop_count = 0u64;
        loop {
            loop_count += 1;
            if loop_count > 10_000_000 {
                let err = VmError::RuntimeError(
                    "Maximum instruction limit exceeded (potential infinite loop)".to_string(),
                );
                self.print_traceback(&err);
                return Err(err);
            }
            
            let (ins, cur_ip, module_idx) = {
                let f = self.frames.last().unwrap();
                if f.ip >= f.instrs.len() {
                    break;
                }
                (f.instrs[f.ip].clone(), f.ip, f.module_idx)
            };

            let mut next_ip = Some(cur_ip + 1);
            
            match ins {
                Instruction::Nop => {}
                Instruction::Push(idx) => {
                    let module = &self.modules[module_idx];
                    let constant = &module.constants[idx as usize];
                    match constant {
                        Constant::Int(v) => self.push(Value::int(*v)),
                        Constant::Float(v) => self.push(Value::float(*v)),
                        Constant::String(v) => self.push(Value::string(v.clone())),
                        _ => return Err(VmError::RuntimeError("Unsupported constant type".to_string())),
                    }
                }
                Instruction::Pop => { self.pop()?; }
                Instruction::Return => {
                    let val = self.pop().unwrap_or(Value::null());
                    self.frames.pop();
                    return Ok(val);
                }

                Instruction::BigIntConst { sign, bytes } => {
                    self.push(Value::bigint(sign, bytes));
                }
                Instruction::BigIntAdd => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let l = unsafe { lhs.as_bigint().clone() };
                    let r = unsafe { rhs.as_bigint().clone() };
                    let res = if l.sign == r.sign {
                        BigInt {
                            sign: l.sign,
                            bytes: add_abs(&l.bytes, &r.bytes),
                        }
                    } else {
                        match cmp_abs(&l.bytes, &r.bytes) {
                            0 => BigInt {
                                sign: 0,
                                bytes: Vec::new(),
                            },
                            1 => BigInt {
                                sign: l.sign,
                                bytes: sub_abs(&l.bytes, &r.bytes),
                            },
                            _ => BigInt {
                                sign: r.sign,
                                bytes: sub_abs(&r.bytes, &l.bytes),
                            },
                        }
                    };
                    self.push(Value::bigint(res.sign, res.bytes));
                }
                Instruction::BigIntSub => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let mut r = unsafe { rhs.as_bigint().clone() };
                    if !r.bytes.is_empty() {
                        r.sign ^= 1;
                    }
                    let l = unsafe { lhs.as_bigint().clone() };
                    let res = if l.sign == r.sign {
                        BigInt {
                            sign: l.sign,
                            bytes: add_abs(&l.bytes, &r.bytes),
                        }
                    } else {
                        match cmp_abs(&l.bytes, &r.bytes) {
                            0 => BigInt {
                                sign: 0,
                                bytes: Vec::new(),
                            },
                            1 => BigInt {
                                sign: l.sign,
                                bytes: sub_abs(&l.bytes, &r.bytes),
                            },
                            _ => BigInt {
                                sign: r.sign,
                                bytes: sub_abs(&r.bytes, &l.bytes),
                            },
                        }
                    };
                    self.push(Value::bigint(res.sign, res.bytes));
                }
                Instruction::BigIntMul => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let l = unsafe { lhs.as_bigint().clone() };
                    let r = unsafe { rhs.as_bigint().clone() };
                    let sign = if l.bytes.is_empty() || r.bytes.is_empty() {
                        0
                    } else {
                        l.sign ^ r.sign
                    };
                    let bytes = mul_abs(&l.bytes, &r.bytes);
                    self.push(Value::bigint(sign, bytes));
                }
                Instruction::BigIntDiv => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let l = unsafe { lhs.as_bigint().clone() };
                    let r = unsafe { rhs.as_bigint().clone() };
                    let (q, _) = div_mod_abs(l.bytes.clone(), &r.bytes);
                    let sign = if q.is_empty() { 0 } else { l.sign ^ r.sign };
                    self.push(Value::bigint(sign, q));
                }
                Instruction::BigIntMod => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let l = unsafe { lhs.as_bigint().clone() };
                    let r = unsafe { rhs.as_bigint().clone() };
                    let (_, rem) = div_mod_abs(l.bytes.clone(), &r.bytes);
                    let sign = if rem.is_empty() { 0 } else { l.sign };
                    self.push(Value::bigint(sign, rem));
                }
                Instruction::BigIntNeg => {
                    let v = self.pop()?;
                    let mut b = unsafe { v.as_bigint().clone() };
                    if !b.bytes.is_empty() {
                        b.sign ^= 1;
                    } else {
                        b.sign = 0;
                    }
                    self.push(Value::bigint(b.sign, b.bytes));
                }
                Instruction::BigIntEq => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let l = unsafe { lhs.as_bigint() };
                    let r = unsafe { rhs.as_bigint() };
                    let eq = l.sign == r.sign && cmp_abs(&l.bytes, &r.bytes) == 0;
                    self.push(Value::bool(eq));
                }
                Instruction::BigIntNe => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let l = unsafe { lhs.as_bigint() };
                    let r = unsafe { rhs.as_bigint() };
                    let ne = !(l.sign == r.sign && cmp_abs(&l.bytes, &r.bytes) == 0);
                    self.push(Value::bool(ne));
                }
                Instruction::BigIntLt => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let l = unsafe { lhs.as_bigint() };
                    let r = unsafe { rhs.as_bigint() };
                    let res = if l.sign != r.sign {
                        l.sign != 0 && r.sign == 0
                    } else {
                        let c = cmp_abs(&l.bytes, &r.bytes);
                        if l.sign == 0 {
                            c < 0
                        } else {
                            c > 0
                        }
                    };
                    self.push(Value::bool(res));
                }
                Instruction::BigIntLe => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let l = unsafe { lhs.as_bigint() };
                    let r = unsafe { rhs.as_bigint() };
                    let res = if l.sign != r.sign {
                        l.sign != 0 && r.sign == 0
                    } else {
                        let c = cmp_abs(&l.bytes, &r.bytes);
                        if l.sign == 0 {
                            c <= 0
                        } else {
                            c >= 0
                        }
                    };
                    self.push(Value::bool(res));
                }
                Instruction::BigIntGt => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let l = unsafe { lhs.as_bigint() };
                    let r = unsafe { rhs.as_bigint() };
                    let res = if l.sign != r.sign {
                        l.sign == 0 && r.sign != 0
                    } else {
                        let c = cmp_abs(&l.bytes, &r.bytes);
                        if l.sign == 0 {
                            c > 0
                        } else {
                            c < 0
                        }
                    };
                    self.push(Value::bool(res));
                }
                Instruction::BigIntGe => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let l = unsafe { lhs.as_bigint() };
                    let r = unsafe { rhs.as_bigint() };
                    let res = if l.sign != r.sign {
                        l.sign == 0 && r.sign != 0
                    } else {
                        let c = cmp_abs(&l.bytes, &r.bytes);
                        if l.sign == 0 {
                            c >= 0
                        } else {
                            c <= 0
                        }
                    };
                    self.push(Value::bool(res));
                }
                Instruction::BigIntToI64 => {
                    let v = self.pop()?;
                    let b = unsafe { v.as_bigint() };
                    let i = b.to_i64();
                    self.push(Value::int(i));
                }
                Instruction::BigIntFromI64 => {
                    let v = self.pop()?;
                    let i = unsafe { v.as_int() };
                    self.push(Value::bigint_from_i64(i));
                }
                Instruction::BigIntToString => {
                    let v = self.pop()?;
                    let b = unsafe { v.as_bigint() };
                    let s = b.to_i64().to_string();
                    self.push(Value::string(s));
                }
                Instruction::I32Const(v) => {
                    self.push(Value::int(v as i64));
                }
                Instruction::I32DivS => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let (r, _) =
                        unsafe { (lhs.as_int() as i32).overflowing_div(rhs.as_int() as i32) };
                    self.push(Value::int(r as i64));
                }
                Instruction::I32DivU => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let (r, _) =
                        unsafe { (lhs.as_int() as u32).overflowing_div(rhs.as_int() as u32) };
                    self.push(Value::int(r as i64));
                }
                Instruction::I32RemS => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let (r, _) =
                        unsafe { (lhs.as_int() as i32).overflowing_rem(rhs.as_int() as i32) };
                    self.push(Value::int(r as i64));
                }
                Instruction::I32RemU => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let (r, _) =
                        unsafe { (lhs.as_int() as u32).overflowing_rem(rhs.as_int() as u32) };
                    self.push(Value::int(r as i64));
                }
                Instruction::I32Add => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r =
                        unsafe { (lhs.as_int() as i32).wrapping_add(rhs.as_int() as i32) } as i64;
                    self.push(Value::int(r));
                }
                Instruction::I32Sub => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r =
                        unsafe { (lhs.as_int() as i32).wrapping_sub(rhs.as_int() as i32) } as i64;
                    self.push(Value::int(r));
                }
                Instruction::I32Mul => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r =
                        unsafe { (lhs.as_int() as i32).wrapping_mul(rhs.as_int() as i32) } as i64;
                    self.push(Value::int(r));
                }
                Instruction::I32Neg => {
                    let v = self.pop()?;
                    let r = unsafe { -(v.as_int() as i32) } as i64;
                    self.push(Value::int(r));
                }
                Instruction::I32Eq => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i32) == (rhs.as_int() as i32) };
                    self.push(Value::bool(r));
                }
                Instruction::I32Ne => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i32) != (rhs.as_int() as i32) };
                    self.push(Value::bool(r));
                }
                Instruction::I32LtS => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i32) < (rhs.as_int() as i32) };
                    self.push(Value::bool(r));
                }
                Instruction::I32LtU => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as u32) < (rhs.as_int() as u32) };
                    self.push(Value::bool(r));
                }
                Instruction::I32LeS => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i32) <= (rhs.as_int() as i32) };
                    self.push(Value::bool(r));
                }
                Instruction::I32LeU => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as u32) <= (rhs.as_int() as u32) };
                    self.push(Value::bool(r));
                }
                Instruction::I32GtS => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i32) > (rhs.as_int() as i32) };
                    self.push(Value::bool(r));
                }
                Instruction::I32GtU => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as u32) > (rhs.as_int() as u32) };
                    self.push(Value::bool(r));
                }
                Instruction::I32GeS => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i32) >= (rhs.as_int() as i32) };
                    self.push(Value::bool(r));
                }
                Instruction::I32GeU => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as u32) >= (rhs.as_int() as u32) };
                    self.push(Value::bool(r));
                }
                Instruction::I32ToF32S => {
                    let v = self.pop()?;
                    let r = unsafe { (v.as_int() as i32) as f32 } as f64;
                    self.push(Value::float(r));
                }
                Instruction::I32ToF32U => {
                    let v = self.pop()?;
                    let r = unsafe { (v.as_int() as u32) as f32 } as f64;
                    self.push(Value::float(r));
                }
                Instruction::I32ToF64S => {
                    let v = self.pop()?;
                    let r = unsafe { (v.as_int() as i32) as f64 };
                    self.push(Value::float(r));
                }
                Instruction::I32ToF64U => {
                    let v = self.pop()?;
                    let r = unsafe { (v.as_int() as u32) as f64 };
                    self.push(Value::float(r));
                }
                Instruction::I32Extend64S => {
                    let v = self.pop()?;
                    let r = unsafe { (v.as_int() as i32) as i64 };
                    self.push(Value::int(r));
                }
                Instruction::I32Extend64U => {
                    let v = self.pop()?;
                    let r = unsafe { (v.as_int() as u32) as u64 } as i64;
                    self.push(Value::int(r));
                }
                Instruction::I32Trunc64SLow => {
                    let v = self.pop()?;
                    let low = unsafe { (v.as_int() as u64) as u32 };
                    let r = low as i32;
                    self.push(Value::int(r as i64));
                }
                Instruction::I32Trunc64S => {
                    let v = self.pop()?;
                    let r = unsafe { (v.as_int() as i64) as i32 };
                    self.push(Value::int(r as i64));
                }
                Instruction::I32Trunc64U => {
                    let v = self.pop()?;
                    let r = unsafe { (v.as_int() as u64) as u32 };
                    self.push(Value::int(r as i64));
                }
                Instruction::I64Const(v) => {
                    self.push(Value::int(v));
                }
                Instruction::I64Add => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i64).wrapping_add(rhs.as_int() as i64) };
                    self.push(Value::int(r));
                }
                Instruction::I64Sub => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i64).wrapping_sub(rhs.as_int() as i64) };
                    self.push(Value::int(r));
                }
                Instruction::I64Mul => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i64).wrapping_mul(rhs.as_int() as i64) };
                    self.push(Value::int(r));
                }
                Instruction::I64DivS => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let (r, _) =
                        unsafe { (lhs.as_int() as i64).overflowing_div(rhs.as_int() as i64) };
                    self.push(Value::int(r));
                }
                Instruction::I64DivU => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let (r, _) =
                        unsafe { (lhs.as_int() as u64).overflowing_div(rhs.as_int() as u64) };
                    self.push(Value::int(r as i64));
                }
                Instruction::I64RemS => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let (r, _) =
                        unsafe { (lhs.as_int() as i64).overflowing_rem(rhs.as_int() as i64) };
                    self.push(Value::int(r));
                }
                Instruction::I64RemU => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let (r, _) =
                        unsafe { (lhs.as_int() as u64).overflowing_rem(rhs.as_int() as u64) };
                    self.push(Value::int(r as i64));
                }
                Instruction::I64Neg => {
                    let v = self.pop()?;
                    let r = unsafe { -(v.as_int() as i64) };
                    self.push(Value::int(r));
                }
                Instruction::I64Eq => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i64) == (rhs.as_int() as i64) };
                    self.push(Value::bool(r));
                }
                Instruction::I64Ne => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i64) != (rhs.as_int() as i64) };
                    self.push(Value::bool(r));
                }
                Instruction::I64LtS => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i64) < (rhs.as_int() as i64) };
                    self.push(Value::bool(r));
                }
                Instruction::I64LtU => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as u64) < (rhs.as_int() as u64) };
                    self.push(Value::bool(r));
                }
                Instruction::I64LeS => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i64) <= (rhs.as_int() as i64) };
                    self.push(Value::bool(r));
                }
                Instruction::I64LeU => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as u64) <= (rhs.as_int() as u64) };
                    self.push(Value::bool(r));
                }
                Instruction::I64GtS => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i64) > (rhs.as_int() as i64) };
                    self.push(Value::bool(r));
                }
                Instruction::I64GtU => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as u64) > (rhs.as_int() as u64) };
                    self.push(Value::bool(r));
                }
                Instruction::I64GeS => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as i64) >= (rhs.as_int() as i64) };
                    self.push(Value::bool(r));
                }
                Instruction::I64GeU => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_int() as u64) >= (rhs.as_int() as u64) };
                    self.push(Value::bool(r));
                }
                Instruction::I64ToF32S => {
                    let v = self.pop()?;
                    let r = unsafe { (v.as_int() as i64) as f32 } as f64;
                    self.push(Value::float(r));
                }
                Instruction::I64ToF32U => {
                    let v = self.pop()?;
                    let r = unsafe { (v.as_int() as u64) as f32 } as f64;
                    self.push(Value::float(r));
                }
                Instruction::I64ToF64S => {
                    let v = self.pop()?;
                    let r = unsafe { (v.as_int() as i64) as f64 };
                    self.push(Value::float(r));
                }
                Instruction::I64ToF64U => {
                    let v = self.pop()?;
                    let r = unsafe { (v.as_int() as u64) as f64 };
                    self.push(Value::float(r));
                }
                Instruction::F32Const(v) => {
                    self.push(Value::float(v as f64));
                }
                Instruction::F32Add => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_float() as f32) + (rhs.as_float() as f32) } as f64;
                    self.push(Value::float(r));
                }
                Instruction::F32Sub => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_float() as f32) - (rhs.as_float() as f32) } as f64;
                    self.push(Value::float(r));
                }
                Instruction::F32Mul => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_float() as f32) * (rhs.as_float() as f32) } as f64;
                    self.push(Value::float(r));
                }
                Instruction::F32Div => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_float() as f32) / (rhs.as_float() as f32) } as f64;
                    self.push(Value::float(r));
                }
                Instruction::F32Neg => {
                    let v = self.pop()?;
                    let r = unsafe { -v.as_float() as f32 } as f64;
                    self.push(Value::float(r));
                }
                Instruction::F32Eq => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_float() as f32) == (rhs.as_float() as f32) };
                    self.push(Value::bool(r));
                }
                Instruction::F32Ne => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_float() as f32) != (rhs.as_float() as f32) };
                    self.push(Value::bool(r));
                }
                Instruction::F32Lt => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_float() as f32) < (rhs.as_float() as f32) };
                    self.push(Value::bool(r));
                }
                Instruction::F32Le => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_float() as f32) <= (rhs.as_float() as f32) };
                    self.push(Value::bool(r));
                }
                Instruction::F32Gt => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_float() as f32) > (rhs.as_float() as f32) };
                    self.push(Value::bool(r));
                }
                Instruction::F32Ge => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { (lhs.as_float() as f32) >= (rhs.as_float() as f32) };
                    self.push(Value::bool(r));
                }
                Instruction::F32ToI32S => {
                    let v = self.pop()?;
                    let r = unsafe { v.as_float() as i32 };
                    self.push(Value::int(r as i64));
                }
                Instruction::F32ToI32U => {
                    let v = self.pop()?;
                    let r = unsafe { v.as_float() as u32 };
                    self.push(Value::int(r as i64));
                }
                Instruction::F32ToI64S => {
                    let v = self.pop()?;
                    let r = unsafe { v.as_float() as i64 };
                    self.push(Value::int(r));
                }
                Instruction::F32ToI64U => {
                    let v = self.pop()?;
                    let r = unsafe { v.as_float() as u64 };
                    self.push(Value::int(r as i64));
                }
                Instruction::F32ToF64 => {
                    let v = self.pop()?;
                    let r = unsafe { v.as_float() };
                    self.push(Value::float(r));
                }
                Instruction::F64Const(v) => {
                    self.push(Value::float(v));
                }
                Instruction::F64Add => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_float() + rhs.as_float() };
                    self.push(Value::float(r));
                }
                Instruction::F64Sub => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_float() - rhs.as_float() };
                    self.push(Value::float(r));
                }
                Instruction::F64Mul => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_float() * rhs.as_float() };
                    self.push(Value::float(r));
                }
                Instruction::F64Div => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_float() / rhs.as_float() };
                    self.push(Value::float(r));
                }
                Instruction::F64Neg => {
                    let v = self.pop()?;
                    let r = unsafe { -v.as_float() };
                    self.push(Value::float(r));
                }
                Instruction::F64Eq => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_float() == rhs.as_float() };
                    self.push(Value::bool(r));
                }
                Instruction::F64Ne => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_float() != rhs.as_float() };
                    self.push(Value::bool(r));
                }
                Instruction::F64Lt => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_float() < rhs.as_float() };
                    self.push(Value::bool(r));
                }
                Instruction::F64Le => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_float() <= rhs.as_float() };
                    self.push(Value::bool(r));
                }
                Instruction::F64Gt => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_float() > rhs.as_float() };
                    self.push(Value::bool(r));
                }
                Instruction::F64Ge => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_float() >= rhs.as_float() };
                    self.push(Value::bool(r));
                }
                Instruction::F64ToI32S => {
                    let v = self.pop()?;
                    let r = unsafe { v.as_float() as i32 };
                    self.push(Value::int(r as i64));
                }
                Instruction::F64ToI32U => {
                    let v = self.pop()?;
                    let r = unsafe { v.as_float() as u32 };
                    self.push(Value::int(r as i64));
                }
                Instruction::F64ToI64S => {
                    let v = self.pop()?;
                    let r = unsafe { v.as_float() as i64 };
                    self.push(Value::int(r));
                }
                Instruction::F64ToI64U => {
                    let v = self.pop()?;
                    let r = unsafe { v.as_float() as u64 };
                    self.push(Value::int(r as i64));
                }
                Instruction::F64ToF32 => {
                    let v = self.pop()?;
                    let r = unsafe { (v.as_float() as f32) as f64 };
                    self.push(Value::float(r));
                }
                Instruction::StringConst(s) => {
                    self.push(Value::string(s));
                }
                Instruction::StringConcat => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { format!("{}{}", lhs.as_string(), rhs.as_string()) };
                    self.push(Value::string(r));
                }
                Instruction::StringLenBytes => {
                    let v = self.pop()?;
                    let n = unsafe { v.as_string().len() } as i64;
                    self.push(Value::int(n));
                }
                Instruction::StringLenChars => {
                    let v = self.pop()?;
                    let n = unsafe { v.as_string().chars().count() } as i64;
                    self.push(Value::int(n));
                }
                Instruction::StringEq => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_string() == rhs.as_string() };
                    self.push(Value::bool(r));
                }
                Instruction::StringNe => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_string() != rhs.as_string() };
                    self.push(Value::bool(r));
                }
                Instruction::StringLt => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_string() < rhs.as_string() };
                    self.push(Value::bool(r));
                }
                Instruction::StringLe => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_string() <= rhs.as_string() };
                    self.push(Value::bool(r));
                }
                Instruction::StringGt => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_string() > rhs.as_string() };
                    self.push(Value::bool(r));
                }
                Instruction::StringGe => {
                    let rhs = self.pop()?;
                    let lhs = self.pop()?;
                    let r = unsafe { lhs.as_string() >= rhs.as_string() };
                    self.push(Value::bool(r));
                }
                Instruction::StringSubstr => {
                    let len_v = self.pop()?;
                    let start_v = self.pop()?;
                    let s_v = self.pop()?;
                    let s = unsafe { s_v.as_string().clone() };
                    let start = unsafe { start_v.as_int() } as usize;
                    let len = unsafe { len_v.as_int() } as usize;
                    let end = start.saturating_add(len);
                    let end = end.min(s.len());
                    let sub = if start <= end {
                        s[start..end].to_string()
                    } else {
                        String::new()
                    };
                    self.push(Value::string(sub));
                }
                Instruction::Push(idx) => {
                    let c = self
                        .constants
                        .get(idx as usize)
                        .ok_or(VmError::IndexOutOfBounds)?;
                    match c {
                        Constant::Int(i) => self.push(Value::int(*i)),
                        Constant::Float(x) => self.push(Value::float(*x)),
                        Constant::String(s) => self.push(Value::string(s.clone())),
                    }
                }
                Instruction::Pop => {
                    let _ = self.pop()?;
                }
                Instruction::Dup(d) => {
                    let v = self.peek_at(d as usize)?;
                    self.push(v);
                }
                Instruction::Swap(d) => {
                    let sp = self.sp;
                    if sp == 0 {
                        return Err(VmError::StackUnderflow);
                    }
                    let eff = if (d as usize) >= sp {
                        sp - 1
                    } else {
                        d as usize
                    };
                    self.swap_with(eff)?;
                }
                Instruction::LoadLocal(idx) => {
                    let f = self.frames.last().unwrap();
                    if (idx as usize) < f.locals.len() {
                        let v = f.locals[idx as usize];
                        self.push(v);
                    } else {
                        return Err(VmError::StackUnderflow);
                    }
                }
                Instruction::StoreLocal(idx) => {
                    let v = self.pop()?;
                    let f = self.frames.last_mut().unwrap();
                    if (idx as usize) >= f.locals.len() {
                        f.locals.resize((idx as usize) + 1, Value::null());
                    }
                    f.locals[idx as usize] = v;
                }
                Instruction::Jump(off) => {
                    let target = (cur_ip as isize + off as isize) as usize;
                    next_ip = Some(target);
                }
                Instruction::JumpIfFalse(off) => {
                    let v = self.pop()?;
                    let cond = unsafe {
                        match v.tag {
                            ValueTag::Bool => v.as_bool(),
                            ValueTag::Null => false,
                            _ => true,
                        }
                    };
                    if !cond {
                        next_ip = Some((cur_ip as isize + off as isize) as usize);
                    }
                }
                Instruction::Return => {
                    let v = self.pop()?;
                    self.frames.pop();
                    while let Some(hf) = self.handler_stack.last() {
                        if hf.frame_depth > self.frames.len() {
                            self.handler_stack.pop();
                        } else {
                            break;
                        }
                    }
                    if self.frames.is_empty() {
                        return Ok(v);
                    }
                    self.push(v);
                    next_ip = None;
                }
                Instruction::MakeClosure(idx, ref upvalues) => {
                    let mut captured = Vec::with_capacity(upvalues.len());
                    for up in upvalues {
                        let val = if up.is_local {
                            let f = self.frames.last().unwrap();
                            f.locals[up.index as usize]
                        } else {
                            let f = self.frames.last().unwrap();
                            if f.closure.is_null() {
                                return Err(VmError::InvalidOpcode);
                            }
                            let closure = unsafe { &*f.closure };
                            closure.upvalues[up.index as usize].0
                        };
                        captured.push(Upvalue(val));
                    }
                    let v = Value::closure(idx, captured);
                    self.push(v);
                }
                Instruction::LoadUpvalue(idx) => {
                    let f = self.frames.last().unwrap();
                    if f.closure.is_null() {
                        return Err(VmError::InvalidOpcode);
                    }
                    let closure = unsafe { &*f.closure };
                    if (idx as usize) < closure.upvalues.len() {
                        self.push(closure.upvalues[idx as usize].0);
                    } else {
                        return Err(VmError::IndexOutOfBounds);
                    }
                }
                Instruction::StoreUpvalue(idx) => {
                    let val = self.pop()?;
                    let f = self.frames.last().unwrap();
                    if f.closure.is_null() {
                        return Err(VmError::InvalidOpcode);
                    }
                    // Upvalues are effectively immutable copies for now unless we implement interior mutability
                    // But if we want to update the copy in the closure:
                    // We need mutable access to the closure.
                    // But `f.closure` is *const.
                    // Since we own the VM and everything is single threaded here, we can cast to *mut.
                    let closure = unsafe { &mut *(f.closure as *mut Closure) };
                    if (idx as usize) < closure.upvalues.len() {
                        closure.upvalues[idx as usize].0 = val;
                    } else {
                        return Err(VmError::IndexOutOfBounds);
                    }
                }
                Instruction::Call(chunk_idx, argc) => {
                    let module = &self.modules[module_idx];
                    let chunk = module
                        .chunks
                        .get(chunk_idx as usize)
                        .cloned()
                        .ok_or(VmError::IndexOutOfBounds)?;
                    use crate::bytecode::decoder::Decoder;
                    let decoder = Decoder::new(&chunk.code);
                    let instrs = decoder.decode_all().map_err(|_| VmError::InvalidOpcode)?;

                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    if args.len() < chunk.locals as usize {
                        args.resize(chunk.locals as usize, Value::null());
                    }

                    let new_frame = Frame {
                        instrs,
                        ip: 0,
                        locals: args,
                        closure: null(),
                        module_idx,
                        chunk_idx: Some(chunk_idx as usize),
                    };

                    if let Some(next) = next_ip {
                        self.frames.last_mut().unwrap().ip = next;
                    }
                    self.frames.push(new_frame);
                    next_ip = None;
                }
                Instruction::CallClosure(argc) => {
                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    let callee = self.pop()?;
                    if callee.tag != ValueTag::Closure {
                        return Err(VmError::InvalidOpcode); // Expected closure
                    }

                    let closure_ptr = unsafe { callee.data.ptr as *mut crate::vm::value::Closure };
                    let closure = unsafe { &*closure_ptr };
                    let chunk_idx = closure.func;

                    let chunk = self.modules[module_idx]
                        .chunks
                        .get(chunk_idx)
                        .cloned()
                        .ok_or(VmError::IndexOutOfBounds)?;
                    use crate::bytecode::decoder::Decoder;
                    let decoder = Decoder::new(&chunk.code);
                    let instrs = decoder.decode_all().map_err(|_| VmError::InvalidOpcode)?;

                    if args.len() < chunk.locals as usize {
                        args.resize(chunk.locals as usize, Value::null());
                    }

                    let new_frame = Frame {
                        instrs,
                        ip: 0,
                        locals: args,
                        closure: closure_ptr,
                        module_idx,
                        chunk_idx: Some(chunk_idx),
                    };

                    if let Some(next) = next_ip {
                        self.frames.last_mut().unwrap().ip = next;
                    }
                    self.frames.push(new_frame);
                    next_ip = None;
                }
                Instruction::CallSymbol(name_idx, argc) => {
                    let module = &self.modules[module_idx];
                    let name = match module.constants.get(name_idx as usize) {
                        Some(Constant::String(s)) => s.as_str(),
                        _ => return Err(VmError::IndexOutOfBounds),
                    };

                    if let Some(&(m_idx, chunk_idx)) = self.symbol_table.get(name) {
                        let mut args = Vec::with_capacity(argc as usize);
                        for _ in 0..argc {
                            args.push(self.pop()?);
                        }
                        args.reverse();

                        let chunk = self.modules[m_idx]
                            .chunks
                            .get(chunk_idx as usize)
                            .cloned()
                            .ok_or(VmError::IndexOutOfBounds)?;
                        use crate::bytecode::decoder::Decoder;
                        let decoder = Decoder::new(&chunk.code);
                        let instrs = decoder.decode_all().map_err(|_| VmError::InvalidOpcode)?;

                        if args.len() < chunk.locals as usize {
                            args.resize(chunk.locals as usize, Value::null());
                        }

                        let new_frame = Frame {
                            instrs,
                            ip: 0,
                            locals: args,
                            closure: null(),
                            module_idx: m_idx,
                            chunk_idx: Some(chunk_idx as usize),
                        };

                        if let Some(next) = next_ip {
                            self.frames.last_mut().unwrap().ip = next;
                        }
                        self.frames.push(new_frame);
                        next_ip = None;
                    } else {
                        return Err(VmError::RuntimeError(format!("Symbol not found: {}", name)));
                    }
                }
                Instruction::InvokeMethod(name_idx, argc) => {
                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    let receiver = self.pop()?;

                    let name = match self.modules[module_idx].constants.get(name_idx as usize) {
                        Some(Constant::String(s)) => s.as_str(),
                        _ => return Err(VmError::InvalidOpcode),
                    };
                    // self.log(&format!(
                    //    "InvokeMethod: name={}, argc={}, receiver_tag={:?}",
                    //    name, argc, receiver.tag
                    // ));

                    if receiver.tag != ValueTag::Object {
                        match name {
                            "add" => {
                                if args.len() == 1 {
                                    let rhs = args[0];
                                    let lhs = receiver;
                                    unsafe {
                                        match (lhs.tag, rhs.tag) {
                                            (ValueTag::Int, ValueTag::Int) => {
                                                self.push(Value::int(lhs.as_int() + rhs.as_int()))
                                            }
                                            (ValueTag::Float, ValueTag::Float) => self.push(
                                                Value::float(lhs.as_float() + rhs.as_float()),
                                            ),
                                            (ValueTag::String, ValueTag::String) => {
                                                let mut s = lhs.as_string().clone();
                                                s.push_str(rhs.as_string());
                                                self.push(Value::string(s));
                                            }
                                            _ => self.push(Value::null()),
                                        }
                                    }
                                } else {
                                    self.push(Value::null());
                                }
                            }
                            "sub" => {
                                if args.len() == 1 {
                                    let rhs = args[0];
                                    let lhs = receiver;
                                    unsafe {
                                        match (lhs.tag, rhs.tag) {
                                            (ValueTag::Int, ValueTag::Int) => {
                                                self.push(Value::int(lhs.as_int() - rhs.as_int()))
                                            }
                                            (ValueTag::Float, ValueTag::Float) => self.push(
                                                Value::float(lhs.as_float() - rhs.as_float()),
                                            ),
                                            _ => self.push(Value::null()),
                                        }
                                    }
                                } else {
                                    self.push(Value::null());
                                }
                            }
                            "mul" => {
                                if args.len() == 1 {
                                    let rhs = args[0];
                                    let lhs = receiver;
                                    unsafe {
                                        match (lhs.tag, rhs.tag) {
                                            (ValueTag::Int, ValueTag::Int) => {
                                                self.push(Value::int(lhs.as_int() * rhs.as_int()))
                                            }
                                            (ValueTag::Float, ValueTag::Float) => self.push(
                                                Value::float(lhs.as_float() * rhs.as_float()),
                                            ),
                                            _ => self.push(Value::null()),
                                        }
                                    }
                                } else {
                                    self.push(Value::null());
                                }
                            }
                            "div" => {
                                if args.len() == 1 {
                                    let rhs = args[0];
                                    let lhs = receiver;
                                    unsafe {
                                        match (lhs.tag, rhs.tag) {
                                            (ValueTag::Int, ValueTag::Int) => {
                                                self.push(Value::int(lhs.as_int() / rhs.as_int()))
                                            }
                                            (ValueTag::Float, ValueTag::Float) => self.push(
                                                Value::float(lhs.as_float() / rhs.as_float()),
                                            ),
                                            _ => self.push(Value::null()),
                                        }
                                    }
                                } else {
                                    self.push(Value::null());
                                }
                            }
                            "eq" => {
                                if args.len() == 1 {
                                    let rhs = args[0];
                                    let lhs = receiver;
                                    let eq = unsafe {
                                        if lhs.tag != rhs.tag {
                                            false
                                        } else {
                                            match lhs.tag {
                                                ValueTag::Int => lhs.as_int() == rhs.as_int(),
                                                ValueTag::Float => lhs.as_float() == rhs.as_float(),
                                                ValueTag::Bool => lhs.as_bool() == rhs.as_bool(),
                                                ValueTag::Null => true,
                                                ValueTag::String => {
                                                    lhs.as_string() == rhs.as_string()
                                                }
                                                ValueTag::Object => lhs.data.ptr == rhs.data.ptr,
                                                _ => false,
                                            }
                                        }
                                    };
                                    self.push(Value::bool(eq));
                                } else {
                                    self.push(Value::bool(false));
                                }
                            }
                            "ne" => {
                                if args.len() == 1 {
                                    let rhs = args[0];
                                    let lhs = receiver;
                                    let eq = unsafe {
                                        if lhs.tag != rhs.tag {
                                            false
                                        } else {
                                            match lhs.tag {
                                                ValueTag::Int => lhs.as_int() == rhs.as_int(),
                                                ValueTag::Float => lhs.as_float() == rhs.as_float(),
                                                ValueTag::Bool => lhs.as_bool() == rhs.as_bool(),
                                                ValueTag::Null => true,
                                                ValueTag::String => {
                                                    lhs.as_string() == rhs.as_string()
                                                }
                                                ValueTag::Object => lhs.data.ptr == rhs.data.ptr,
                                                _ => false,
                                            }
                                        }
                                    };
                                    self.push(Value::bool(!eq));
                                } else {
                                    self.push(Value::bool(true));
                                }
                            }
                            "lt" => {
                                if args.len() == 1 {
                                    let rhs = args[0];
                                    let lhs = receiver;
                                    if lhs.tag == ValueTag::Int && rhs.tag == ValueTag::Int {
                                        self.push(Value::bool(unsafe {
                                            lhs.as_int() < rhs.as_int()
                                        }));
                                    } else if lhs.tag == ValueTag::Float
                                        && rhs.tag == ValueTag::Float
                                    {
                                        self.push(Value::bool(unsafe {
                                            lhs.as_float() < rhs.as_float()
                                        }));
                                    } else {
                                        self.push(Value::bool(false));
                                    }
                                } else {
                                    self.push(Value::bool(false));
                                }
                            }
                            "le" => {
                                if args.len() == 1 {
                                    let rhs = args[0];
                                    let lhs = receiver;
                                    if lhs.tag == ValueTag::Int && rhs.tag == ValueTag::Int {
                                        self.push(Value::bool(unsafe {
                                            lhs.as_int() <= rhs.as_int()
                                        }));
                                    } else if lhs.tag == ValueTag::Float
                                        && rhs.tag == ValueTag::Float
                                    {
                                        self.push(Value::bool(unsafe {
                                            lhs.as_float() <= rhs.as_float()
                                        }));
                                    } else {
                                        self.push(Value::bool(false));
                                    }
                                } else {
                                    self.push(Value::bool(false));
                                }
                            }
                            "gt" => {
                                if args.len() == 1 {
                                    let rhs = args[0];
                                    let lhs = receiver;
                                    if lhs.tag == ValueTag::Int && rhs.tag == ValueTag::Int {
                                        self.push(Value::bool(unsafe {
                                            lhs.as_int() > rhs.as_int()
                                        }));
                                    } else if lhs.tag == ValueTag::Float
                                        && rhs.tag == ValueTag::Float
                                    {
                                        self.push(Value::bool(unsafe {
                                            lhs.as_float() > rhs.as_float()
                                        }));
                                    } else {
                                        self.push(Value::bool(false));
                                    }
                                } else {
                                    self.push(Value::bool(false));
                                }
                            }
                            "ge" => {
                                if args.len() == 1 {
                                    let rhs = args[0];
                                    let lhs = receiver;
                                    if lhs.tag == ValueTag::Int && rhs.tag == ValueTag::Int {
                                        self.push(Value::bool(unsafe {
                                            lhs.as_int() >= rhs.as_int()
                                        }));
                                    } else if lhs.tag == ValueTag::Float
                                        && rhs.tag == ValueTag::Float
                                    {
                                        self.push(Value::bool(unsafe {
                                            lhs.as_float() >= rhs.as_float()
                                        }));
                                    } else {
                                        self.push(Value::bool(false));
                                    }
                                } else {
                                    self.push(Value::bool(false));
                                }
                            }
                            "and" => {
                                if args.len() == 1 {
                                    let rhs = args[0];
                                    let lhs = receiver;
                                    let ba = unsafe {
                                        if lhs.tag == ValueTag::Bool {
                                            lhs.as_bool()
                                        } else {
                                            false
                                        }
                                    };
                                    let bb = unsafe {
                                        if rhs.tag == ValueTag::Bool {
                                            rhs.as_bool()
                                        } else {
                                            false
                                        }
                                    };
                                    self.push(Value::bool(ba && bb));
                                } else {
                                    self.push(Value::bool(false));
                                }
                            }
                            "or" => {
                                if args.len() == 1 {
                                    let rhs = args[0];
                                    let lhs = receiver;
                                    let ba = unsafe {
                                        if lhs.tag == ValueTag::Bool {
                                            lhs.as_bool()
                                        } else {
                                            false
                                        }
                                    };
                                    let bb = unsafe {
                                        if rhs.tag == ValueTag::Bool {
                                            rhs.as_bool()
                                        } else {
                                            false
                                        }
                                    };
                                    self.push(Value::bool(ba || bb));
                                } else {
                                    self.push(Value::bool(false));
                                }
                            }
                            "neg" => {
                                if args.is_empty() {
                                    let a = receiver;
                                    if a.tag == ValueTag::Int {
                                        self.push(Value::int(unsafe { -a.as_int() }));
                                    } else if a.tag == ValueTag::Float {
                                        self.push(Value::float(unsafe { -a.as_float() }));
                                    } else {
                                        self.push(Value::null());
                                    }
                                } else {
                                    self.push(Value::null());
                                }
                            }
                            "not" => {
                                if args.is_empty() {
                                    let a = receiver;
                                    if a.tag == ValueTag::Bool {
                                        self.push(Value::bool(unsafe { !a.as_bool() }));
                                    } else {
                                        self.push(Value::bool(false));
                                    }
                                } else {
                                    self.push(Value::bool(false));
                                }
                            }
                            "len" => {
                                let len = match receiver.tag {
                                    ValueTag::String => unsafe { receiver.as_string().len() },
                                    ValueTag::Array => unsafe { receiver.as_array().items.len() },
                                    ValueTag::List => unsafe { receiver.as_list().items.len() },
                                    ValueTag::Tuple => unsafe { receiver.as_tuple().items.len() },
                                    ValueTag::DynObject => unsafe { receiver.as_dyn_object().entries.len() },
                                    _ => 0,
                                };
                                self.push(Value::int(len as i64));
                            }
                            "get" => {
                                let idx_v = args.first().cloned().unwrap_or(Value::int(0));
                                if receiver.tag == ValueTag::List && idx_v.tag == ValueTag::Int {
                                    let list = unsafe { receiver.as_list() };
                                    let idx = unsafe { idx_v.as_int() } as usize;
                                    if idx < list.items.len() {
                                        self.push(list.items[idx]);
                                    } else {
                                        self.push(Value::null());
                                    }
                                } else if receiver.tag == ValueTag::String && idx_v.tag == ValueTag::Int {
                                    let s = unsafe { receiver.as_string() };
                                    let idx = unsafe { idx_v.as_int() } as usize;
                                    let c = s.chars().nth(idx).map(|c| c.to_string()).unwrap_or_default();
                                    self.push(Value::string(c));
                                } else {
                                    self.push(Value::null());
                                }
                            }
                            "push" => {
                                if let Some(val) = args.first() {
                                    if receiver.tag == ValueTag::List {
                                        let list_ptr = unsafe { receiver.data.ptr as *mut crate::vm::value::List };
                                        let list_mut = unsafe { &mut *list_ptr };
                                        list_mut.items.push(*val);
                                        self.push(Value::null());
                                    } else {
                                        self.push(Value::null());
                                    }
                                } else {
                                    self.push(Value::null());
                                }
                            }
                            "pop" => {
                                if receiver.tag == ValueTag::List {
                                    let list_ptr = unsafe { receiver.data.ptr as *mut crate::vm::value::List };
                                    let list_mut = unsafe { &mut *list_ptr };
                                    if let Some(val) = list_mut.items.pop() {
                                        self.push(val);
                                    } else {
                                        self.push(Value::null());
                                    }
                                } else {
                                    self.push(Value::null());
                                }
                            }
                            "unshift" => {
                                if let Some(val) = args.first() {
                                    if receiver.tag == ValueTag::List {
                                        let list_ptr = unsafe { receiver.data.ptr as *mut crate::vm::value::List };
                                        let list_mut = unsafe { &mut *list_ptr };
                                        list_mut.items.insert(0, *val);
                                        self.push(Value::null());
                                    } else {
                                        self.push(Value::null());
                                    }
                                } else {
                                    self.push(Value::null());
                                }
                            }
                            "shift" => {
                                if receiver.tag == ValueTag::List {
                                    let list_ptr = unsafe { receiver.data.ptr as *mut crate::vm::value::List };
                                    let list_mut = unsafe { &mut *list_ptr };
                                    if !list_mut.items.is_empty() {
                                        let val = list_mut.items.remove(0);
                                        self.push(val);
                                    } else {
                                        self.push(Value::null());
                                    }
                                } else {
                                    self.push(Value::null());
                                }
                            }
                            _ => {
                                return Err(VmError::RuntimeError(format!(
                                    "Receiver is not an object. tag={:?}, method={}",
                                    receiver.tag, name
                                )))
                            }
                        }
                    } else {
                        let obj_ptr = unsafe { receiver.data.ptr as *mut crate::vm::value::Object };
                        let obj_ref = unsafe { &*obj_ptr };
                        let class_idx = obj_ref.class_idx;
                        let class_name = self.modules[module_idx]
                            .classes
                            .get(class_idx as usize)
                            .map(|c| c.name.clone())
                            .unwrap_or_else(|| "<unknown>".to_string());

                        let mut chunk_idx = None;
                        for impl_info in &self.modules[module_idx].impls {
                            if impl_info.class_idx == class_idx {
                                if let Some(trait_info) =
                                    self.modules[module_idx].traits.get(impl_info.trait_idx as usize)
                                {
                                    if let Some(idx) =
                                        trait_info.methods.iter().position(|m| m == name)
                                    {
                                        if idx < impl_info.methods.len() {
                                            chunk_idx = Some(impl_info.methods[idx]);
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        let chunk_idx = chunk_idx.ok_or_else(|| {
                            VmError::RuntimeError(format!(
                                "Method {} not found for class {}",
                                name, class_idx
                            ))
                        })?;
                        // self.log(&format!(
                        //    "InvokeMethod: dispatch class={}({}), chunk_idx={}",
                        //    class_name, class_idx, chunk_idx
                        // ));
                        let chunk = self.modules[module_idx]
                            .chunks
                            .get(chunk_idx as usize)
                            .cloned()
                            .ok_or(VmError::IndexOutOfBounds)?;

                        let mut full_args = Vec::with_capacity(args.len() + 1);
                        full_args.push(receiver);
                        full_args.extend(args);

                        use crate::bytecode::decoder::Decoder;
                        let decoder = Decoder::new(&chunk.code);
                        let instrs = decoder.decode_all().map_err(|_| VmError::InvalidOpcode)?;

                        if full_args.len() < chunk.locals as usize {
                            full_args.resize(chunk.locals as usize, Value::null());
                        }

                        let new_frame = Frame {
                            instrs,
                            ip: 0,
                            locals: full_args,
                            closure: null(),
                            chunk_idx: Some(chunk_idx as usize),
                        };

                        if let Some(next) = next_ip {
                            self.frames.last_mut().unwrap().ip = next;
                        }
                        self.frames.push(new_frame);
                        next_ip = None;
                    }
                }
                Instruction::Call(idx, argc) => {
                    let chunk = self
                        .chunks
                        .get(idx as usize)
                        .cloned()
                        .ok_or(VmError::IndexOutOfBounds)?;
                    use crate::bytecode::decoder::Decoder;
                    let decoder = Decoder::new(&chunk.code);
                    let instrs = decoder.decode_all().map_err(|_| VmError::InvalidOpcode)?;

                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    if args.len() < chunk.locals as usize {
                        args.resize(chunk.locals as usize, Value::null());
                    }

                    let new_frame = Frame {
                        instrs,
                        ip: 0,
                        locals: args,
                        closure: null(),
                        chunk_idx: Some(idx as usize),
                    };

                    if let Some(next) = next_ip {
                        self.frames.last_mut().unwrap().ip = next;
                    }
                    self.frames.push(new_frame);
                    next_ip = None;
                }
                Instruction::Perform(idx, argc) => {
                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    let name = self.effects.get(idx as usize).cloned().unwrap_or_default();
                    if name == "await" {
                        if let Some(v) = args.get(0) {
                            if v.tag == ValueTag::Closure {
                                let closure_ptr =
                                    unsafe { v.data.ptr as *mut crate::vm::value::Closure };
                                let closure = unsafe { &*closure_ptr };
                                let chunk_idx = closure.func;
                                let chunk = self
                                    .chunks
                                    .get(chunk_idx)
                                    .cloned()
                                    .ok_or(VmError::IndexOutOfBounds)?;
                                use crate::bytecode::decoder::Decoder;
                                let decoder = Decoder::new(&chunk.code);
                                let instrs =
                                    decoder.decode_all().map_err(|_| VmError::InvalidOpcode)?;
                                let new_frame = Frame {
                                    instrs,
                                    ip: 0,
                                    locals: Vec::new(),
                                    closure: closure_ptr,
                                    chunk_idx: Some(chunk_idx),
                                };
                                if let Some(next) = next_ip {
                                    self.frames.last_mut().unwrap().ip = next;
                                }
                                self.frames.push(new_frame);
                                next_ip = None;
                            } else {
                                self.push(*v);
                            }
                        }
                    } else {
                        if let Some(hf) = {
                            let mut chosen = None;
                            for h in self.handler_stack.iter().rev() {
                                let chunk = self
                                    .chunks
                                    .get(h.catch_chunk)
                                    .cloned()
                                    .ok_or(VmError::IndexOutOfBounds)?;
                                use crate::bytecode::decoder::Decoder;
                                let decoder = Decoder::new(&chunk.code);
                                let instrs =
                                    decoder.decode_all().map_err(|_| VmError::InvalidOpcode)?;
                                let mut matches = true;
                                if let Some(crate::bytecode::decoder::Instruction::MatchEffect(
                                    name_idx,
                                )) = instrs.get(0)
                                {
                                    let module = &self.modules[module_idx];
                                    let name0 = match module.constants.get(*name_idx as usize) {
                                        Some(Constant::String(s)) => s.as_str(),
                                        _ => "",
                                    };
                                    let eff_name = module
                                        .effects
                                        .get(idx as usize)
                                        .map(|s| s.as_str())
                                        .unwrap_or("");
                                    matches = name0 == eff_name;
                                }
                                if matches {
                                    chosen = Some(h.clone());
                                    break;
                                }
                            }
                            chosen
                        } {
                            let chunk = self.modules[module_idx]
                                .chunks
                                .get(hf.catch_chunk)
                                .cloned()
                                .ok_or(VmError::IndexOutOfBounds)?;
                            use crate::bytecode::decoder::Decoder;
                            let decoder = Decoder::new(&chunk.code);
                            let instrs =
                                decoder.decode_all().map_err(|_| VmError::InvalidOpcode)?;
                            let mut locals = Vec::new();
                            locals.push(Value::effect(idx as u16, args.clone()));
                            locals.push(Value::list(args.clone()));
                            let cont_ip = if let Some(next) = next_ip {
                                next
                            } else {
                                cur_ip + 1
                            };
                            let cont_slice = self.stack[..self.sp].to_vec();
                            let cont = Value::continuation(cont_ip, cont_slice);
                            locals.push(cont);
                            if locals.len() < chunk.locals as usize {
                                locals.resize(chunk.locals as usize, Value::null());
                            }
                            let new_frame = Frame {
                                instrs,
                                ip: 0,
                                locals,
                                closure: null(),
                                module_idx,
                                chunk_idx: Some(hf.catch_chunk),
                            };
                            if let Some(next) = next_ip {
                                self.frames.last_mut().unwrap().ip = next;
                            }
                            self.frames.push(new_frame);
                            next_ip = None;
                        } else {
                            if name == "throw" {
                                self.log("Traceback (most recent call last):");
                                self.log("UnhandledError");
                            }
                            match perform_effect_internal(self, name, args) {
                                Ok(Some(val)) => self.push(val),
                                Ok(None) => {}
                                Err(e) => {
                                    self.print_traceback(&e);
                                    return Err(e);
                                }
                            }
                        }
                    }
                }
                Instruction::WithHandler(handler_chunk_idx) => {
                    let hf = HandlerFrame {
                        catch_chunk: handler_chunk_idx as usize,
                        frame_depth: self.frames.len(),
                    };
                    self.handler_stack.push(hf);
                }
                Instruction::TypeOf => {
                    let v = self.pop()?;
                    let tid = match v.tag {
                        ValueTag::Int => 0i64,
                        ValueTag::Float => 1,
                        ValueTag::Bool => 2,
                        ValueTag::Null => 3,
                        _ => 4,
                    };
                    self.push(Value::int(tid));
                }
                Instruction::FFICall(desc, argc) => {
                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();
                    let name = match self.constants.get(desc as usize) {
                        Some(Constant::String(s)) => s.as_str(),
                        _ => "",
                    };

                    if let Some(func) = self.ffi.get(name) {
                        let res = func.call(args)?;
                        self.push(res);
                    } else {
                        match name {
                            "read_file" => {
                            let path_v = args.pop().unwrap_or(Value::string("".to_string()));
                            let mut res = Value::string("".to_string());
                            unsafe {
                                use std::fs;
                                if path_v.tag == ValueTag::String {
                                    let path = path_v.as_string().clone();
                                    if let Ok(content) = fs::read_to_string(&path) {
                                        res = Value::string(content);
                                    }
                                }
                            }
                            self.push(res);
                        }
                        "write_file" => {
                            let data_v = args.pop().unwrap_or(Value::string("".to_string()));
                            let path_v = args.pop().unwrap_or(Value::string("".to_string()));
                            let mut ok = false;
                            unsafe {
                                use std::fs;
                                use std::path::Path;
                                if path_v.tag == ValueTag::String && data_v.tag == ValueTag::String
                                {
                                    let path = path_v.as_string().clone();
                                    let bytes = data_v.as_string().as_bytes().to_vec();
                                    if let Some(dir) = Path::new(&path).parent() {
                                        let _ = fs::create_dir_all(dir);
                                    }
                                    let res = fs::write(&path, &bytes);
                                    ok = res.is_ok();
                                    println!(
                                        "DEBUG: write_file path={} len={} ok={}",
                                        path,
                                        bytes.len(),
                                        ok
                                    );
                                } else {
                                    println!(
                                        "DEBUG: write_file invalid args path_tag={:?} data_tag={:?}",
                                        path_v.tag, data_v.tag
                                    );
                                }
                            }
                            self.push(Value::bool(ok));
                        }
                        "write_bytes" => {
                            let bytes_v = args.pop().unwrap_or(Value::list(vec![]));
                            let path_v = args.pop().unwrap_or(Value::string("".to_string()));
                            let mut ok = false;
                            unsafe {
                                use std::fs;
                                use std::path::Path;
                                if path_v.tag == ValueTag::String && bytes_v.tag == ValueTag::List {
                                    let path = path_v.as_string().clone();
                                    let list_ref = bytes_v.as_list();
                                    let mut data = Vec::with_capacity(list_ref.items.len());
                                    for item in &list_ref.items {
                                        if item.tag == ValueTag::Int {
                                            let v = item.as_int();
                                            let b = (v & 0xFF) as u8;
                                            data.push(b);
                                        } else {
                                            data.push(0u8);
                                        }
                                    }
                                    if let Some(dir) = Path::new(&path).parent() {
                                        let _ = fs::create_dir_all(dir);
                                    }
                                    ok = fs::write(&path, &data).is_ok();
                                }
                            }
                            self.push(Value::bool(ok));
                        }
                        "compile_jvm" => {
                            let mut ok = false;
                            self.push(Value::bool(ok));
                        }
                        "print" => {
                            if let Some(v) = args.last() {
                                let msg = match v.tag {
                                    ValueTag::Int => format!("{}", unsafe { v.as_int() }),
                                    ValueTag::Float => format!("{}", unsafe { v.as_float() }),
                                    ValueTag::Bool => format!("{}", unsafe { v.as_bool() }),
                                    ValueTag::Null => "null".to_string(),
                                    ValueTag::String => unsafe { v.as_string().clone() },
                                    ValueTag::Object => {
                                        let obj_ptr =
                                            unsafe { v.data.ptr as *mut crate::vm::value::Object };
                                        let obj_ref = unsafe { &*obj_ptr };
                                        let cls_name = self
                                            .classes
                                            .get(obj_ref.class_idx as usize)
                                            .map(|c| c.name.clone())
                                            .unwrap_or_else(|| "Object".to_string());
                                        let variant = obj_ref
                                            .fields
                                            .get(0)
                                            .and_then(|f| {
                                                if f.tag == ValueTag::String {
                                                    Some(unsafe { f.as_string().clone() })
                                                } else {
                                                    None
                                                }
                                            })
                                            .unwrap_or_else(|| "".to_string());
                                        if variant.is_empty() {
                                            format!("{}", cls_name)
                                        } else {
                                            format!("{}::{}", cls_name, variant)
                                        }
                                    }
                                    _ => "<unsupported>".to_string(),
                                };
                                if let Some(cb) = &self.stdout {
                                    cb(&msg);
                                } else {
                                    println!("{}", msg);
                                }
                            }
                            self.push(Value::null());
                        }
                        "true" => {
                            self.push(Value::bool(true));
                        }
                        "false" => {
                            self.push(Value::bool(false));
                        }
                        "yield" => {
                            if let Some(v) = args.last() {
                                let msg = match v.tag {
                                    ValueTag::Int => format!("{}", unsafe { v.as_int() }),
                                    ValueTag::Float => format!("{}", unsafe { v.as_float() }),
                                    ValueTag::Bool => format!("{}", unsafe { v.as_bool() }),
                                    ValueTag::Null => "null".to_string(),
                                    _ => "<unsupported>".to_string(),
                                };
                                if let Some(cb) = &self.stdout {
                                    cb(&msg);
                                } else {
                                    println!("{}", msg);
                                }
                            }
                            self.push(Value::null());
                        }
                        "true" => self.push(Value::bool(true)),
                        "false" => self.push(Value::bool(false)),
                        "not" => {
                            if let Some(v) = args.first() {
                                let b = unsafe {
                                    match v.tag {
                                        ValueTag::Bool => v.as_bool(),
                                        ValueTag::Null => false,
                                        _ => true,
                                    }
                                };
                                self.push(Value::bool(!b));
                            } else {
                                self.push(Value::bool(true));
                            }
                        }
                        "assert" => {
                            let msg = if let Some(v) = args.last() {
                                match v.tag {
                                    ValueTag::Int => {
                                        format!("assertion failed: {}", unsafe { v.as_int() })
                                    }
                                    ValueTag::Float => {
                                        format!("assertion failed: {}", unsafe { v.as_float() })
                                    }
                                    ValueTag::Bool => {
                                        format!("assertion failed: {}", unsafe { v.as_bool() })
                                    }
                                    ValueTag::Null => "assertion failed".to_string(),
                                    _ => "assertion failed".to_string(),
                                }
                            } else {
                                "assertion failed".to_string()
                            };
                            return Err(VmError::RuntimeError(msg));
                        }
                        "len" => {
                            if let Some(v) = args.last() {
                                let len = match v.tag {
                                    ValueTag::String => unsafe { v.as_string().len() },
                                    ValueTag::Array => unsafe { v.as_array().items.len() },
                                    ValueTag::List => unsafe { v.as_list().items.len() },
                                    ValueTag::Tuple => unsafe { v.as_tuple().items.len() },
                                    ValueTag::DynObject => unsafe {
                                        v.as_dyn_object().entries.len()
                                    },
                                    _ => 0,
                                };
                                self.push(Value::int(len as i64));
                            } else {
                                self.push(Value::int(0));
                            }
                        }
                        "ord" => {
                            if let Some(v) = args.last() {
                                let code = if v.tag == ValueTag::String {
                                    let s = unsafe { v.as_string() };
                                    if let Some(c) = s.chars().next() {
                                        c as i64
                                    } else {
                                        0
                                    }
                                } else {
                                    0
                                };
                                self.push(Value::int(code));
                            } else {
                                self.push(Value::int(0));
                            }
                        }
                        "chr" => {
                            if let Some(v) = args.last() {
                                let c = if v.tag == ValueTag::Int {
                                    let i = unsafe { v.as_int() };
                                    std::char::from_u32(i as u32).unwrap_or('\0').to_string()
                                } else {
                                    "\0".to_string()
                                };
                                self.push(Value::string(c));
                            } else {
                                self.push(Value::string("".to_string()));
                            }
                        }
                        "get" => {
                            let idx_v = args.pop().unwrap_or(Value::int(0));
                            // Since this is FFICall, there is no separate 'receiver'.
                            // Everything is in 'args'.
                            // If user called get(obj, idx), args is [obj, idx] (after reversal).
                            // We popped idx_v. args is now [obj].
                            // So container is next pop.
                            let container = args.pop().unwrap_or(Value::null());

                            if container.tag == ValueTag::String && idx_v.tag == ValueTag::Int {
                                let s = unsafe { container.as_string() };
                                let idx = unsafe { idx_v.as_int() } as usize;
                                let c = s
                                    .chars()
                                    .nth(idx)
                                    .map(|c| c.to_string())
                                    .unwrap_or_default();
                                self.push(Value::string(c));
                            } else if container.tag == ValueTag::List && idx_v.tag == ValueTag::Int
                            {
                                let list = unsafe { container.as_list() };
                                let idx = unsafe { idx_v.as_int() } as usize;
                                if idx < list.items.len() {
                                    self.push(list.items[idx]);
                                } else {
                                    self.push(Value::null());
                                }
                            } else if container.tag == ValueTag::Array && idx_v.tag == ValueTag::Int
                            {
                                let arr = unsafe { container.as_array() };
                                let idx = unsafe { idx_v.as_int() } as usize;
                                if idx < arr.items.len() {
                                    self.push(arr.items[idx]);
                                } else {
                                    self.push(Value::null());
                                }
                            } else {
                                self.push(Value::null());
                            }
                        }
                        "push" => {
                            if args.len() == 2 {
                                let val = args.pop().unwrap_or(Value::null()); // val is last arg
                                let container = args.pop().unwrap_or(Value::null()); // container is first arg
                                if container.tag == ValueTag::List {
                                    let list_ptr = unsafe {
                                        container.data.ptr as *mut crate::vm::value::List
                                    };
                                    let list_mut = unsafe { &mut *list_ptr };
                                    list_mut.items.push(val);
                                    self.push(Value::null());
                                } else {
                                    self.push(Value::null());
                                }
                            } else {
                                self.push(Value::null());
                            }
                        }
                        "set" => {
                            // set(container, idx, val) -> args: [container, idx, val]
                            // reversed args: [container, idx, val] (wait, reverse() on [val, idx, container] -> [container, idx, val])
                            // pop() -> val
                            // pop() -> idx
                            // pop() -> container
                            if args.len() == 3 {
                                let val_v = args.pop().unwrap_or(Value::null());
                                let idx_v = args.pop().unwrap_or(Value::int(0));
                                let container = args.pop().unwrap_or(Value::null());

                                if container.tag == ValueTag::List && idx_v.tag == ValueTag::Int {
                                    let idx = unsafe { idx_v.as_int() } as usize;
                                    let list_ptr = unsafe {
                                        container.data.ptr as *mut crate::vm::value::List
                                    };
                                    let list_mut = unsafe { &mut *list_ptr };
                                    if idx < list_mut.items.len() {
                                        list_mut.items[idx] = val_v;
                                        self.push(Value::bool(true));
                                    } else {
                                        self.push(Value::bool(false));
                                    }
                                } else if container.tag == ValueTag::Array
                                    && idx_v.tag == ValueTag::Int
                                {
                                    let idx = unsafe { idx_v.as_int() } as usize;
                                    let arr_ptr = unsafe {
                                        container.data.ptr as *mut crate::vm::value::Array
                                    };
                                    let arr_mut = unsafe { &mut *arr_ptr };
                                    if idx < arr_mut.items.len() {
                                        arr_mut.items[idx] = val_v;
                                        self.push(Value::bool(true));
                                    } else {
                                        self.push(Value::bool(false));
                                    }
                                } else {
                                    self.push(Value::bool(false));
                                }
                            } else {
                                self.push(Value::bool(false));
                            }
                        }
                        "chars" => {
                            if let Some(v) = args.last() {
                                if v.tag == ValueTag::String {
                                    let s = unsafe { v.as_string() };
                                    let items: Vec<Value> =
                                        s.chars().map(|c| Value::string(c.to_string())).collect();
                                    self.push(Value::list(items));
                                } else {
                                    self.push(Value::list(vec![]));
                                }
                            } else {
                                self.push(Value::list(vec![]));
                            }
                        }
                        "str" => {
                            if let Some(v) = args.last() {
                                let s = match v.tag {
                                    ValueTag::Int => format!("{}", unsafe { v.as_int() }),
                                    ValueTag::Float => format!("{}", unsafe { v.as_float() }),
                                    ValueTag::Bool => format!("{}", unsafe { v.as_bool() }),
                                    ValueTag::Null => "null".to_string(),
                                    ValueTag::String => unsafe { v.as_string().clone() },
                                    _ => format!("{:?}", v.tag),
                                };
                                self.push(Value::string(s));
                            } else {
                                self.push(Value::string("".to_string()));
                            }
                        }
                        "eq" => {
                            let b = args.pop().unwrap_or(Value::null());
                            let a = args.pop().unwrap_or(Value::null());
                            let r = match (a.tag, b.tag) {
                                (ValueTag::Int, ValueTag::Int) => unsafe {
                                    a.as_int() == b.as_int()
                                },
                                (ValueTag::Float, ValueTag::Float) => unsafe {
                                    a.as_float() == b.as_float()
                                },
                                (ValueTag::Bool, ValueTag::Bool) => unsafe {
                                    a.as_bool() == b.as_bool()
                                },
                                (ValueTag::String, ValueTag::String) => unsafe {
                                    a.as_string() == b.as_string()
                                },
                                (ValueTag::Null, ValueTag::Null) => true,
                                _ => false,
                            };
                            self.push(Value::bool(r));
                        }
                        "ne" => {
                            let b = args.pop().unwrap_or(Value::null());
                            let a = args.pop().unwrap_or(Value::null());
                            let r = match (a.tag, b.tag) {
                                (ValueTag::Int, ValueTag::Int) => unsafe {
                                    a.as_int() != b.as_int()
                                },
                                (ValueTag::Float, ValueTag::Float) => unsafe {
                                    a.as_float() != b.as_float()
                                },
                                (ValueTag::Bool, ValueTag::Bool) => unsafe {
                                    a.as_bool() != b.as_bool()
                                },
                                (ValueTag::String, ValueTag::String) => unsafe {
                                    a.as_string() != b.as_string()
                                },
                                (ValueTag::Null, ValueTag::Null) => false,
                                _ => true,
                            };
                            self.push(Value::bool(r));
                        }
                        "lt" => {
                            let b = args.pop().unwrap_or(Value::null());
                            let a = args.pop().unwrap_or(Value::null());
                            let r = match (a.tag, b.tag) {
                                (ValueTag::Int, ValueTag::Int) => unsafe {
                                    a.as_int() < b.as_int()
                                },
                                (ValueTag::Float, ValueTag::Float) => unsafe {
                                    a.as_float() < b.as_float()
                                },
                                (ValueTag::String, ValueTag::String) => unsafe {
                                    a.as_string() < b.as_string()
                                },
                                _ => false,
                            };
                            self.push(Value::bool(r));
                        }
                        "le" => {
                            let b = args.pop().unwrap_or(Value::null());
                            let a = args.pop().unwrap_or(Value::null());
                            let r = match (a.tag, b.tag) {
                                (ValueTag::Int, ValueTag::Int) => unsafe {
                                    a.as_int() <= b.as_int()
                                },
                                (ValueTag::Float, ValueTag::Float) => unsafe {
                                    a.as_float() <= b.as_float()
                                },
                                (ValueTag::String, ValueTag::String) => unsafe {
                                    a.as_string() <= b.as_string()
                                },
                                _ => false,
                            };
                            self.push(Value::bool(r));
                        }
                        "gt" => {
                            let b = args.pop().unwrap_or(Value::null());
                            let a = args.pop().unwrap_or(Value::null());
                            let r = match (a.tag, b.tag) {
                                (ValueTag::Int, ValueTag::Int) => unsafe {
                                    a.as_int() > b.as_int()
                                },
                                (ValueTag::Float, ValueTag::Float) => unsafe {
                                    a.as_float() > b.as_float()
                                },
                                (ValueTag::String, ValueTag::String) => unsafe {
                                    a.as_string() > b.as_string()
                                },
                                _ => false,
                            };
                            self.push(Value::bool(r));
                        }
                        "ge" => {
                            let b = args.pop().unwrap_or(Value::null());
                            let a = args.pop().unwrap_or(Value::null());
                            let r = match (a.tag, b.tag) {
                                (ValueTag::Int, ValueTag::Int) => unsafe {
                                    a.as_int() >= b.as_int()
                                },
                                (ValueTag::Float, ValueTag::Float) => unsafe {
                                    a.as_float() >= b.as_float()
                                },
                                (ValueTag::String, ValueTag::String) => unsafe {
                                    a.as_string() >= b.as_string()
                                },
                                _ => false,
                            };
                            self.push(Value::bool(r));
                        }
                        "add" => {
                            let b = args.pop().unwrap_or(Value::null());
                            let a = args.pop().unwrap_or(Value::null());
                            unsafe {
                                match (a.tag, b.tag) {
                                    (ValueTag::Int, ValueTag::Int) => {
                                        self.push(Value::int(a.as_int() + b.as_int()))
                                    }
                                    (ValueTag::Float, ValueTag::Float) => {
                                        self.push(Value::float(a.as_float() + b.as_float()))
                                    }
                                    (ValueTag::String, ValueTag::String) => {
                                        let mut s = a.as_string().clone();
                                        s.push_str(b.as_string());
                                        self.push(Value::string(s));
                                    }
                                    _ => self.push(Value::null()),
                                }
                            }
                        }
                        "sub" => {
                            let b = args.pop().unwrap_or(Value::null());
                            let a = args.pop().unwrap_or(Value::null());
                            unsafe {
                                match (a.tag, b.tag) {
                                    (ValueTag::Int, ValueTag::Int) => {
                                        self.push(Value::int(a.as_int() - b.as_int()))
                                    }
                                    (ValueTag::Float, ValueTag::Float) => {
                                        self.push(Value::float(a.as_float() - b.as_float()))
                                    }
                                    _ => self.push(Value::null()),
                                }
                            }
                        }
                        "mul" => {
                            let b = args.pop().unwrap_or(Value::null());
                            let a = args.pop().unwrap_or(Value::null());
                            unsafe {
                                match (a.tag, b.tag) {
                                    (ValueTag::Int, ValueTag::Int) => {
                                        self.push(Value::int(a.as_int() * b.as_int()))
                                    }
                                    (ValueTag::Float, ValueTag::Float) => {
                                        self.push(Value::float(a.as_float() * b.as_float()))
                                    }
                                    _ => self.push(Value::null()),
                                }
                            }
                        }
                        "div" => {
                            let b = args.pop().unwrap_or(Value::null());
                            let a = args.pop().unwrap_or(Value::null());
                            unsafe {
                                match (a.tag, b.tag) {
                                    (ValueTag::Int, ValueTag::Int) => {
                                        self.push(Value::int(a.as_int() / b.as_int()))
                                    }
                                    (ValueTag::Float, ValueTag::Float) => {
                                        self.push(Value::float(a.as_float() / b.as_float()))
                                    }
                                    _ => self.push(Value::null()),
                                }
                            }
                        }
                        _ => {
                            if name.contains("::") {
                                let parts: Vec<&str> = name.split("::").collect();
                                let variant_name = parts.last().unwrap();
                                let class_name = parts[parts.len() - 2];

                                // Find class by name suffix
                                let class_idx = self.classes.iter().position(|c| {
                                    c.name.ends_with(&format!("::{}", class_name))
                                        || c.name == class_name
                                });

                                if let Some(idx) = class_idx {
                                    let idx = idx as u16;
                                    // Check if we have enough args.
                                    // For now, assume single arg constructor if args.len() > 0?
                                    // Actually, EnumDef variants can have multiple fields.
                                    // But FFICall doesn't tell us how many fields the variant EXPECTS unless we look it up.
                                    // But we have `args` from FFICall.
                                    // FFICall pops args.
                                    // We need to pop ALL args.
                                    // FFICall logic already popped args into `args` vec.
                                    // So we just use `args`.
                                    // But `args` are popped in reverse order (LIFO).
                                    // Wait, FFICall pops:
                                    // for _ in 0..argc { args.push(pop()) }
                                    // If I call C(a, b). Stack: [a, b].
                                    // pop -> b. pop -> a.
                                    // args = [b, a].
                                    // NewObject expects fields in order.
                                    // fields = [__variant__, _0, _1...]
                                    // _0 should be a. _1 should be b.
                                    // So we need to REVERSE args to get [a, b].

                                    let mut fields = Vec::new();
                                    fields.push(Value::string(variant_name.to_string()));

                                    // args is [last_arg, ..., first_arg]
                                    // We want [first_arg, ..., last_arg]
                                    // So we iterate args in reverse.
                                    for arg in args.iter().rev() {
                                        fields.push(*arg);
                                    }

                                    // We might need to pad with nulls if the class has more fields?
                                    // Enum classes have fields _0, _1... up to max fields of any variant.
                                    // If this variant has fewer fields, the remaining should be null?
                                    // Or assume `args` matches the variant fields count?
                                    // The VM class definition has `fields` count.
                                    let cls = &self.classes[idx as usize];
                                    while fields.len() < cls.fields.len() {
                                        fields.push(Value::null());
                                    }

                                    let obj = Value::object(idx, fields);
                                    self.push(obj);
                                } else {
                                    return Err(VmError::UnhandledEffect(name.to_string()));
                                }
                            } else {
                                return Err(VmError::UnhandledEffect(name.to_string()));
                            }
                        }
                    }
                }
                }
                Instruction::NewObject(class_idx) => {
                    let cls = self
                        .classes
                        .get(class_idx as usize)
                        .ok_or(VmError::IndexOutOfBounds)?;
                    let fields = vec![Value::null(); cls.fields.len()];
                    let obj = Value::object(class_idx, fields);
                    self.push(obj);
                }
                Instruction::NewDynObject => {
                    self.push(Value::dyn_object());
                }
                Instruction::NewArray(len) => {
                    let items = vec![Value::null(); len as usize];
                    let arr = Value::array(items);
                    self.push(arr);
                }
                Instruction::NewList(len) => {
                    let mut items = Vec::with_capacity(len as usize);
                    for _ in 0..len {
                        items.push(self.pop()?);
                    }
                    items.reverse();
                    let list = Value::list(items);
                    self.push(list);
                }
                Instruction::PushElementRight => {
                    let val = self.pop()?;
                    let list_v = self.pop()?;
                    if list_v.tag == ValueTag::List {
                        let list_ptr = unsafe { list_v.data.ptr as *mut crate::vm::value::List };
                        let list_mut = unsafe { &mut *list_ptr };
                        list_mut.items.push(val);
                        self.push(Value::null());
                    } else {
                        return Err(VmError::RuntimeError(format!("PushElementRight on non-list: found {:?}", list_v.tag).into()));
                    }
                }
                Instruction::PopElementRight => {
                    let list_v = self.pop()?;
                    if list_v.tag == ValueTag::List {
                        let list_ptr = unsafe { list_v.data.ptr as *mut crate::vm::value::List };
                        let list_mut = unsafe { &mut *list_ptr };
                        if let Some(val) = list_mut.items.pop() {
                            self.push(val);
                        } else {
                            self.push(Value::null());
                        }
                    } else {
                        return Err(VmError::RuntimeError("PopElementRight on non-list".into()));
                    }
                }
                Instruction::PushElementLeft => {
                    let val = self.pop()?;
                    let list_v = self.pop()?;
                    if list_v.tag == ValueTag::List {
                        let list_ptr = unsafe { list_v.data.ptr as *mut crate::vm::value::List };
                        let list_mut = unsafe { &mut *list_ptr };
                        list_mut.items.insert(0, val);
                        self.push(Value::null());
                    } else {
                        return Err(VmError::RuntimeError("PushElementLeft on non-list".into()));
                    }
                }
                Instruction::PopElementLeft => {
                    let list_v = self.pop()?;
                    if list_v.tag == ValueTag::List {
                        let list_ptr = unsafe { list_v.data.ptr as *mut crate::vm::value::List };
                        let list_mut = unsafe { &mut *list_ptr };
                        if !list_mut.items.is_empty() {
                            let val = list_mut.items.remove(0);
                            self.push(val);
                        } else {
                            self.push(Value::null());
                        }
                    } else {
                        return Err(VmError::RuntimeError("PopElementLeft on non-list".into()));
                    }
                }
                Instruction::GetElement => {
                    let idx_v = self.pop()?;
                    let arr_v = self.pop()?;
                    if arr_v.tag == ValueTag::Array {
                        let idx = unsafe { idx_v.as_int() };
                        let arr_ref = unsafe { arr_v.as_array() };
                        if idx < 0 || (idx as usize) >= arr_ref.items.len() {
                            return Err(VmError::IndexOutOfBounds);
                        }
                        self.push(arr_ref.items[idx as usize]);
                    } else if arr_v.tag == ValueTag::Tuple {
                        let idx = unsafe { idx_v.as_int() };
                        let tup_ref = unsafe { arr_v.as_tuple() };
                        if idx < 0 || (idx as usize) >= tup_ref.items.len() {
                            return Err(VmError::IndexOutOfBounds);
                        }
                        self.push(tup_ref.items[idx as usize]);
                    } else if arr_v.tag == ValueTag::List {
                        let idx = unsafe { idx_v.as_int() };
                        let list_ref = unsafe { arr_v.as_list() };
                        if idx < 0 || (idx as usize) >= list_ref.items.len() {
                            return Err(VmError::IndexOutOfBounds);
                        }
                        self.push(list_ref.items[idx as usize]);
                    } else if arr_v.tag == ValueTag::DynObject {
                        if key_is_string(&idx_v) {
                            let k = unsafe { idx_v.as_string() };
                            let obj_ref = unsafe { arr_v.as_dyn_object() };
                            if let Some(val) = obj_ref.entries.get(k) {
                                self.push(*val);
                            } else {
                                return Err(VmError::RuntimeError("Key not found".into()));
                            }
                        } else {
                            return Err(VmError::InvalidOpcode);
                        }
                    } else {
                        return Err(VmError::InvalidOpcode);
                    }
                }
                Instruction::SetElement => {
                    let val = self.pop()?;
                    let idx_v = self.pop()?;
                    let arr_v = self.pop()?;
                    if arr_v.tag == ValueTag::Array {
                        let idx = unsafe { idx_v.as_int() };
                        let arr_ptr = unsafe { arr_v.data.ptr as *mut crate::vm::value::Array };
                        let arr_mut = unsafe { &mut *arr_ptr };
                        if idx < 0 || (idx as usize) >= arr_mut.items.len() {
                            return Err(VmError::IndexOutOfBounds);
                        }
                        arr_mut.items[idx as usize] = val;
                        self.push(arr_v);
                    } else if arr_v.tag == ValueTag::Tuple {
                        let idx = unsafe { idx_v.as_int() };
                        let tup_ptr = unsafe { arr_v.data.ptr as *mut crate::vm::value::Tuple };
                        let tup_mut = unsafe { &mut *tup_ptr };
                        if idx < 0 || (idx as usize) >= tup_mut.items.len() {
                            return Err(VmError::IndexOutOfBounds);
                        }
                        let slot_tag = tup_mut.items[idx as usize].tag;
                        if slot_tag != val.tag {
                            return Err(VmError::RuntimeError("Tuple type mismatch".into()));
                        }
                        tup_mut.items[idx as usize] = val;
                        self.push(arr_v);
                    } else if arr_v.tag == ValueTag::List {
                        let idx = unsafe { idx_v.as_int() };
                        let list_ptr = unsafe { arr_v.data.ptr as *mut crate::vm::value::List };
                        let list_mut = unsafe { &mut *list_ptr };
                        if idx < 0 || (idx as usize) >= list_mut.items.len() {
                            return Err(VmError::IndexOutOfBounds);
                        }
                        list_mut.items[idx as usize] = val;
                        self.push(arr_v);
                    } else if arr_v.tag == ValueTag::DynObject {
                        if key_is_string(&idx_v) {
                            let k = unsafe { idx_v.as_string().clone() };
                            let obj_ptr =
                                unsafe { arr_v.data.ptr as *mut crate::vm::value::DynObject };
                            let obj_mut = unsafe { &mut *obj_ptr };
                            obj_mut.entries.insert(k, val);
                            self.push(arr_v);
                        } else {
                            return Err(VmError::InvalidOpcode);
                        }
                    } else {
                        return Err(VmError::InvalidOpcode);
                    }
                }
                Instruction::RemoveKey => {
                    let key_v = self.pop()?;
                    let container = self.pop()?;
                    if container.tag == ValueTag::DynObject {
                        if key_is_string(&key_v) {
                            let k = unsafe { key_v.as_string().clone() };
                            let obj_ptr =
                                unsafe { container.data.ptr as *mut crate::vm::value::DynObject };
                            let obj_mut = unsafe { &mut *obj_ptr };
                            let existed = obj_mut.entries.remove(&k).is_some();
                            self.push(Value::bool(existed));
                        } else {
                            return Err(VmError::InvalidOpcode);
                        }
                    } else if container.tag == ValueTag::List {
                        let idx = unsafe { key_v.as_int() };
                        let list_ptr = unsafe { container.data.ptr as *mut crate::vm::value::List };
                        let list_mut = unsafe { &mut *list_ptr };
                        if idx < 0 || (idx as usize) >= list_mut.items.len() {
                            self.push(Value::bool(false));
                        } else {
                            list_mut.items.remove(idx as usize);
                            self.push(Value::bool(true));
                        }
                    } else if container.tag == ValueTag::Array {
                        return Err(VmError::RuntimeError(
                            "Cannot remove from static array".into(),
                        ));
                    } else if container.tag == ValueTag::Tuple {
                        return Err(VmError::RuntimeError("Cannot remove from tuple".into()));
                    } else {
                        return Err(VmError::InvalidOpcode);
                    }
                }
                Instruction::MakeTuple(count) => {
                    let mut items = Vec::with_capacity(count as usize);
                    for _ in 0..count {
                        items.push(self.pop()?);
                    }
                    items.reverse();
                    self.push(Value::tuple(items));
                }
                Instruction::HasKey => {
                    let mut key = self.pop()?;
                    let mut container = self.pop()?;
                    if container.tag != ValueTag::Object && key.tag == ValueTag::Object {
                        let tmp = key;
                        key = container;
                        container = tmp;
                    }
                    let mut exists = false;
                    if container.tag == ValueTag::Object {
                        let obj_ptr =
                            unsafe { container.data.ptr as *mut crate::vm::value::Object };
                        let obj_ref = unsafe { &*obj_ptr };
                        if let ValueTag::String = key.tag {
                            let name = unsafe { key.as_string() };
                            let cls = self
                                .classes
                                .get(obj_ref.class_idx as usize)
                                .ok_or(VmError::IndexOutOfBounds)?;
                            exists = cls.fields.iter().any(|f| f == name);
                            if !exists && !cls.fields.is_empty() {
                                exists = true;
                            }
                        }
                    } else if container.tag == ValueTag::DynObject {
                        if let ValueTag::String = key.tag {
                            let k = unsafe { key.as_string() };
                            let obj_ref = unsafe { container.as_dyn_object() };
                            exists = obj_ref.entries.contains_key(k);
                        }
                    } else if container.tag == ValueTag::Array {
                        if let ValueTag::Int = key.tag {
                            let idx = unsafe { key.as_int() };
                            let arr_ref = unsafe { container.as_array() };
                            exists = idx >= 0 && (idx as usize) < arr_ref.items.len();
                        }
                    } else if container.tag == ValueTag::Tuple {
                        if let ValueTag::Int = key.tag {
                            let idx = unsafe { key.as_int() };
                            let tup_ref = unsafe { container.as_tuple() };
                            exists = idx >= 0 && (idx as usize) < tup_ref.items.len();
                        }
                    } else if container.tag == ValueTag::List {
                        if let ValueTag::Int = key.tag {
                            let idx = unsafe { key.as_int() };
                            let list_ref = unsafe { container.as_list() };
                            exists = idx >= 0 && (idx as usize) < list_ref.items.len();
                        }
                    }
                    self.push(Value::bool(exists));
                }
                Instruction::MatchVariant(class_idx) => {
                    let val = self.pop()?;
                    let is_match = if val.tag == ValueTag::Object {
                        let obj_ptr = unsafe { val.data.ptr as *mut crate::vm::value::Object };
                        let obj_ref = unsafe { &*obj_ptr };
                        obj_ref.class_idx == class_idx
                    } else {
                        false
                    };
                    self.push(Value::bool(is_match));
                }
                Instruction::MatchEffect(name_idx) => {
                    let module = &self.modules[module_idx];
                    let name = match module.constants.get(name_idx as usize) {
                        Some(Constant::String(s)) => s.as_str(),
                        _ => "",
                    };
                    let eff_idx = module.effects.iter().position(|e| e == name);
                    let f = self.frames.last().unwrap();
                    let mut ok = false;
                    if !f.locals.is_empty() {
                        let v = f.locals[0];
                        if v.tag == ValueTag::String {
                            unsafe {
                                ok = v.as_string() == name;
                            }
                        } else if v.tag == ValueTag::Effect {
                            if let Some(i) = eff_idx {
                                let e = unsafe { v.as_effect() };
                                ok = e.type_idx as usize == i;
                            }
                        }
                    }
                    self.push(Value::bool(ok));
                }
                Instruction::SizeOf => {
                    use std::mem::size_of;
                    let v = self.pop()?;
                    let ptr_sz = size_of::<*mut ()>() as i64;
                    let n = match v.tag {
                        ValueTag::Int => size_of::<i64>() as i64,
                        ValueTag::Float => size_of::<f64>() as i64,
                        ValueTag::Bool => size_of::<u8>() as i64,
                        ValueTag::Null => 0,
                        ValueTag::String => ptr_sz,
                        ValueTag::BigInt => ptr_sz,
                        ValueTag::Array => ptr_sz,
                        ValueTag::Tuple => ptr_sz,
                        ValueTag::Object => ptr_sz,
                        ValueTag::DynObject => ptr_sz,
                        ValueTag::List => ptr_sz,
                        ValueTag::Function => ptr_sz,
                        ValueTag::Closure => ptr_sz,
                        ValueTag::TraitObject => ptr_sz,
                        ValueTag::Code => ptr_sz,
                        ValueTag::Continuation => ptr_sz,
                        ValueTag::Effect => ptr_sz,
                        ValueTag::WitnessTable => ptr_sz,
                    };
                    self.push(Value::int(n));
                }
                Instruction::GetField(name_idx) => {
                    let obj = self.pop()?;
                    let module = &self.modules[module_idx];
                    // self.log(&format!("GetField: obj_tag={:?}, name_idx={}", obj.tag, name_idx));
                    match module.constants.get(name_idx as usize) {
                        Some(Constant::String(s)) => {
                            // self.log(&format!("GetField: name_const=String({})", s));
                        }
                        Some(c) => {
                            // self.log(&format!("GetField: name_const_non_string={:?}", c));
                        }
                        None => {
                            // self.log("GetField: name_const_missing");
                        }
                    }
                    if obj.tag != ValueTag::Object {
                        return Err(VmError::RuntimeError(format!(
                            "GetField on non-object: found {:?}",
                            obj.tag
                        )));
                    }
                    let obj_ptr = unsafe { obj.data.ptr as *mut crate::vm::value::Object };
                    let obj_ref = unsafe { &*obj_ptr };

                    let name = match module.constants.get(name_idx as usize) {
                        Some(Constant::String(s)) => s,
                        _ => {
                            return Err(VmError::RuntimeError(format!(
                                "GetField with non-string field name at constant {}",
                                name_idx
                            )))
                        }
                    };
                    let cls = module
                        .classes
                        .get(obj_ref.class_idx as usize)
                        .ok_or(VmError::IndexOutOfBounds)?;
                    if let Some(idx) = cls.fields.iter().position(|f| f == name) {
                        self.push(obj_ref.fields[idx]);
                    } else {
                        return Err(VmError::RuntimeError(format!("Field not found: {}", name)));
                    }
                }
                Instruction::SetField(name_idx) => {
                    let val = self.pop()?;
                    let obj = self.pop()?;
                    let module = &self.modules[module_idx];
                    // self.log(&format!("SetField: obj_tag={:?}, name_idx={}", obj.tag, name_idx));
                    match module.constants.get(name_idx as usize) {
                        Some(Constant::String(s)) => {
                            // self.log(&format!("SetField: name_const=String({})", s));
                        }
                        Some(c) => {
                            // self.log(&format!("SetField: name_const_non_string={:?}", c));
                        }
                        None => {
                            // self.log("SetField: name_const_missing");
                        }
                    }
                    // self.log(&format!("SetField: value_tag={:?}", val.tag));
                    if obj.tag != ValueTag::Object {
                        return Err(VmError::RuntimeError(format!(
                            "SetField on non-object: found {:?}",
                            obj.tag
                        )));
                    }
                    let obj_ptr = unsafe { obj.data.ptr as *mut crate::vm::value::Object };
                    let obj_mut = unsafe { &mut *obj_ptr };

                    let name = match module.constants.get(name_idx as usize) {
                        Some(Constant::String(s)) => s,
                        _ => {
                            return Err(VmError::RuntimeError(format!(
                                "SetField with non-string field name at constant {}",
                                name_idx
                            )))
                        }
                    };
                    let cls = module
                        .classes
                        .get(obj_mut.class_idx as usize)
                        .ok_or(VmError::IndexOutOfBounds)?;
                    if let Some(idx) = cls.fields.iter().position(|f| f == name) {
                        obj_mut.fields[idx] = val;
                        self.push(val);
                    } else {
                        return Err(VmError::RuntimeError(format!("Field not found: {}", name)));
                    }
                }
                Instruction::InstanceOf(class_idx) => {
                    let obj = self.pop()?;
                    let is_instance = if obj.tag == ValueTag::Object {
                        let obj_ptr = unsafe { obj.data.ptr as *mut crate::vm::value::Object };
                        let obj_ref = unsafe { &*obj_ptr };
                        obj_ref.class_idx == class_idx
                    } else {
                        false
                    };
                    self.push(Value::bool(is_instance));
                }
                Instruction::CheckCast(class_idx) => {
                    let obj = self.pop()?;
                    if obj.tag == ValueTag::Object {
                        let obj_ptr = unsafe { obj.data.ptr as *mut crate::vm::value::Object };
                        let obj_ref = unsafe { &*obj_ptr };
                        if obj_ref.class_idx == class_idx {
                            self.push(obj);
                        } else {
                            self.push(Value::null());
                        }
                    } else {
                        self.push(Value::null());
                    }
                }
                Instruction::Cast(class_idx) => {
                    let obj = self.peek_at(0)?;
                    if obj.tag == ValueTag::Object {
                        let obj_ptr = unsafe { obj.data.ptr as *mut crate::vm::value::Object };
                        let obj_ref = unsafe { &*obj_ptr };
                        if obj_ref.class_idx != class_idx {
                            return Err(VmError::RuntimeError("Cast failed".into()));
                        }
                    } else {
                        return Err(VmError::RuntimeError(format!(
                            "Cast failed: not an object, found {:?}",
                            obj.tag
                        )));
                    }
                }
                Instruction::CaptureCont => {
                    let ip = if let Some(next) = next_ip {
                        next
                    } else {
                        cur_ip + 1
                    };
                    let slice = self.stack[..self.sp].to_vec();
                    let cont = Value::continuation(ip, slice);
                    self.push(cont);
                }
                Instruction::ResumeWith => {
                    let result = self.pop()?;
                    let cont_v = self.pop()?;
                    if cont_v.tag != ValueTag::Continuation {
                        return Err(VmError::InvalidOpcode);
                    }
                    let cont = unsafe { cont_v.as_cont().clone() };
                    if self.frames.is_empty() {
                        return Err(VmError::StackUnderflow);
                    }
                    self.frames.pop();
                    self.stack.clear();
                    self.stack.extend_from_slice(&cont.stack_slice);
                    self.sp = cont.stack_slice.len();
                    self.push(result);
                    next_ip = Some(cont.ip);
                }
                Instruction::Await => {
                    let v = self.pop()?;
                    if v.tag == ValueTag::Closure {
                        let closure_ptr = unsafe { v.data.ptr as *mut crate::vm::value::Closure };
                        let closure = unsafe { &*closure_ptr };
                        let chunk_idx = closure.func;
                        let chunk = self
                            .chunks
                            .get(chunk_idx)
                            .cloned()
                            .ok_or(VmError::IndexOutOfBounds)?;
                        use crate::bytecode::decoder::Decoder;
                        let decoder = Decoder::new(&chunk.code);
                        let instrs = decoder.decode_all().map_err(|_| VmError::InvalidOpcode)?;
                        let args: Vec<Value> = Vec::new();
                        let new_frame = Frame {
                            instrs,
                            ip: 0,
                            locals: args,
                            closure: closure_ptr,
                            chunk_idx: Some(chunk_idx),
                        };
                        if let Some(next) = next_ip {
                            self.frames.last_mut().unwrap().ip = next;
                        }
                        self.frames.push(new_frame);
                        next_ip = None;
                    } else {
                        self.push(v);
                    }
                }
                Instruction::BlockOn => {
                    let v = self.pop()?;
                    if v.tag == ValueTag::Closure {
                        let closure_ptr = unsafe { v.data.ptr as *mut crate::vm::value::Closure };
                        let closure = unsafe { &*closure_ptr };
                        let chunk_idx = closure.func;
                        let chunk = self
                            .chunks
                            .get(chunk_idx)
                            .cloned()
                            .ok_or(VmError::IndexOutOfBounds)?;
                        use crate::bytecode::decoder::Decoder;
                        let decoder = Decoder::new(&chunk.code);
                        let instrs = decoder.decode_all().map_err(|_| VmError::InvalidOpcode)?;
                        let args: Vec<Value> = Vec::new();
                        let new_frame = Frame {
                            instrs,
                            ip: 0,
                            locals: args,
                            closure: closure_ptr,
                            chunk_idx: Some(chunk_idx),
                        };
                        if let Some(next) = next_ip {
                            self.frames.last_mut().unwrap().ip = next;
                        }
                        self.frames.push(new_frame);
                        next_ip = None;
                    } else {
                        self.push(v);
                    }
                }
                Instruction::Halt => break,
                _ => {}
            }
            if let Some(next) = next_ip {
                self.frames.last_mut().unwrap().ip = next;
            }
        }
        Ok(Value::null())
    }
}

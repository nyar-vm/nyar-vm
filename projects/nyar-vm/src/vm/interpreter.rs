use crate::bytecode::decoder::Instruction;
use crate::bytecode::format::{Chunk, Constant, ClassInfo, TraitInfo, ImplInfo};
use crate::vm::effects::{perform_effect_internal, HandlerFrame};
use crate::vm::value::{Value, ValueTag, Closure, Upvalue};
use crate::vm::VmError;
use std::ptr::null;

#[derive(Clone)]
struct Frame {
    instrs: Vec<Instruction>,
    ip: usize,
    locals: Vec<Value>,
    closure: *const Closure,
}

pub struct NyarVM {
    stack: Vec<Value>,
    sp: usize,
    frames: Vec<Frame>,
    pub constants: Vec<Constant>,
    pub chunks: Vec<Chunk>,
    pub classes: Vec<ClassInfo>,
    pub traits: Vec<TraitInfo>,
    pub impls: Vec<ImplInfo>,
    pub effects: Vec<String>,
    pub handler_stack: Vec<HandlerFrame>,
}

impl NyarVM {
    pub fn new(constants: Vec<Constant>, chunks: Vec<Chunk>, classes: Vec<ClassInfo>, traits: Vec<TraitInfo>, impls: Vec<ImplInfo>, effects: Vec<String>) -> Self {
        Self {
            stack: Vec::with_capacity(64),
            sp: 0,
            frames: Vec::new(),
            constants,
            chunks,
            classes,
            traits,
            impls,
            effects,
            handler_stack: Vec::new(),
        }
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
    pub fn execute(&mut self, program: &[Instruction]) -> Result<Value, VmError> {
        let frame = Frame {
            instrs: program.to_vec(),
            ip: 0,
            locals: vec![Value::null(); 32],
            closure: null(),
        };
        self.frames.push(frame.clone());
        loop {
            let (ins, cur_ip) = {
                let f = self.frames.last().unwrap();
                if f.ip >= f.instrs.len() {
                    break;
                }
                (f.instrs[f.ip].clone(), f.ip)
            };
            let mut next_ip = Some(cur_ip + 1);
            match ins {
                Instruction::Nop => {}
                Instruction::Push(idx) => {
                    let c = self
                        .constants
                        .get(idx as usize)
                        .ok_or(VmError::IndexOutOfBounds)?;
                    match c {
                        Constant::Int(i) => self.push(Value::int(*i)),
                        Constant::Float(x) => self.push(Value::float(*x)),
                        Constant::String(_) => self.push(Value::null()),
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
                    self.swap_with(d as usize)?;
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
                            _ => false,
                        }
                    };
                    if !cond {
                        next_ip = Some((cur_ip as isize + off as isize) as usize);
                    }
                }
                Instruction::Return => {
                    let v = self.pop()?;
                    self.frames.pop();
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
                    if f.closure.is_null() { return Err(VmError::InvalidOpcode); }
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
                    if f.closure.is_null() { return Err(VmError::InvalidOpcode); }
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
                    
                    let chunk = self.chunks.get(chunk_idx).cloned().ok_or(VmError::IndexOutOfBounds)?;
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
                    };

                    if let Some(next) = next_ip {
                        self.frames.last_mut().unwrap().ip = next;
                    }
                    self.frames.push(new_frame);
                    next_ip = None;
                }
                Instruction::InvokeMethod(name_idx, argc) => {
                    let mut args = Vec::with_capacity(argc as usize);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    let receiver = self.pop()?;
                    if receiver.tag != ValueTag::Object {
                        return Err(VmError::RuntimeError("Receiver is not an object".into()));
                    }
                    
                    let obj_ptr = unsafe { receiver.data.ptr as *mut crate::vm::value::Object };
                    let obj_ref = unsafe { &*obj_ptr };
                    let class_idx = obj_ref.class_idx;
                    
                    let name = match self.constants.get(name_idx as usize) {
                        Some(Constant::String(s)) => s,
                        _ => return Err(VmError::InvalidOpcode),
                    };

                    let mut chunk_idx = None;
                    for impl_info in &self.impls {
                        if impl_info.class_idx == class_idx {
                            if let Some(trait_info) = self.traits.get(impl_info.trait_idx as usize) {
                                if let Some(idx) = trait_info.methods.iter().position(|m| m == name) {
                                    if idx < impl_info.methods.len() {
                                        chunk_idx = Some(impl_info.methods[idx]);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    
                    let chunk_idx = chunk_idx.ok_or_else(|| VmError::RuntimeError(format!("Method {} not found for class {}", name, class_idx)))?;
                    
                    let chunk = self.chunks.get(chunk_idx as usize).cloned().ok_or(VmError::IndexOutOfBounds)?;
                    
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
                    };

                    if let Some(next) = next_ip {
                        self.frames.last_mut().unwrap().ip = next;
                    }
                    self.frames.push(new_frame);
                    next_ip = None;
                }
                Instruction::Call(idx, argc) => {
                    let chunk = self.chunks.get(idx as usize).cloned().ok_or(VmError::IndexOutOfBounds)?;
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
                    let name = self.effects.get(idx as usize).cloned().unwrap_or_default();
                    let r = perform_effect_internal(self, name, args)?;
                    if let Some(val) = r {
                        self.push(val)
                    }
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
                    for _ in 0..argc { args.push(self.pop()?); }
                    let name = match self.constants.get(desc as usize) {
                        Some(Constant::String(s)) => s.as_str(),
                        _ => "",
                    };
                    match name {
                        "print" => {
                            if let Some(v) = args.last() {
                                match v.tag {
                                    ValueTag::Int => println!("{}", unsafe { v.as_int() }),
                                    ValueTag::Float => println!("{}", unsafe { v.as_float() }),
                                    ValueTag::Bool => println!("{}", unsafe { v.as_bool() }),
                                    ValueTag::Null => println!("null"),
                                    _ => println!("<unsupported>"),
                                }
                            }
                            self.push(Value::null());
                        }
                        "add" => {
                            let mut acc = 0i64;
                            for v in args.iter().rev() {
                                match v.tag { ValueTag::Int => { acc += unsafe { v.as_int() } }, _ => { acc += 0; } }
                            }
                            self.push(Value::int(acc));
                        }
                        _ => return Err(VmError::UnhandledEffect(name.to_string())),
                    }
                }
                Instruction::NewObject(class_idx) => {
                    let cls = self.classes.get(class_idx as usize).ok_or(VmError::IndexOutOfBounds)?;
                    let fields = vec![Value::null(); cls.fields.len()];
                    let obj = Value::object(class_idx, fields);
                    self.push(obj);
                }
                Instruction::GetField(name_idx) => {
                    let obj = self.pop()?;
                    if obj.tag != ValueTag::Object { return Err(VmError::InvalidOpcode); }
                    let obj_ptr = unsafe { obj.data.ptr as *mut crate::vm::value::Object };
                    let obj_ref = unsafe { &*obj_ptr };
                    
                    let name = match self.constants.get(name_idx as usize) {
                        Some(Constant::String(s)) => s,
                        _ => return Err(VmError::InvalidOpcode),
                    };
                    let cls = self.classes.get(obj_ref.class_idx as usize).ok_or(VmError::IndexOutOfBounds)?;
                    if let Some(idx) = cls.fields.iter().position(|f| f == name) {
                        self.push(obj_ref.fields[idx]);
                    } else {
                        return Err(VmError::RuntimeError(format!("Field not found: {}", name)));
                    }
                }
                Instruction::SetField(name_idx) => {
                    let val = self.pop()?;
                    let obj = self.pop()?;
                    if obj.tag != ValueTag::Object { return Err(VmError::InvalidOpcode); }
                    let obj_ptr = unsafe { obj.data.ptr as *mut crate::vm::value::Object };
                    let obj_mut = unsafe { &mut *obj_ptr };
                    
                    let name = match self.constants.get(name_idx as usize) {
                        Some(Constant::String(s)) => s,
                        _ => return Err(VmError::InvalidOpcode),
                    };
                    let cls = self.classes.get(obj_mut.class_idx as usize).ok_or(VmError::IndexOutOfBounds)?;
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
                Instruction::Cast(class_idx) => {
                     let obj = self.peek_at(0)?;
                     if obj.tag == ValueTag::Object {
                        let obj_ptr = unsafe { obj.data.ptr as *mut crate::vm::value::Object };
                        let obj_ref = unsafe { &*obj_ptr };
                        if obj_ref.class_idx != class_idx {
                             return Err(VmError::RuntimeError("Cast failed".into()));
                        }
                     } else {
                         return Err(VmError::RuntimeError(format!("Cast failed: not an object, found {:?}", obj.tag)));
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

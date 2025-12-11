use nyar_error::WasmAotError;
use nyar_vm::aot::AotBackend;
use nyar_vm::bytecode::decoder::{Decoder, Instruction};
use nyar_vm::bytecode::format::{Constant, NyarcModule, Chunk};
use wasm_encoder::{CodeSection, ConstExpr, DataSection, ExportKind, ExportSection, Function, FunctionSection, ImportSection, Instruction as WasmInst, MemorySection, MemoryType, Module, TypeSection, ValType, TableSection, TableType, ElementSection, RefType, Elements, GlobalSection, GlobalType, MemArg};

pub struct WasmBackend;

impl AotBackend for WasmBackend {
    type Error = WasmAotError;
    fn compile(&self, module: &NyarcModule) -> Result<Vec<u8>, WasmAotError> {
        compile_module_to_wasm(module)
    }
}

pub fn compile_module_to_wasm(module: &NyarcModule) -> Result<Vec<u8>, WasmAotError> {
    if module.chunks.is_empty() {
        return Err(WasmAotError::EmptyModule);
    }

    let mut m = Module::new();

    let mut types = TypeSection::new();
    let max_locals = module.chunks.iter().map(|c| c.locals).max().unwrap_or(0);
    let mut params = Vec::new();
    for _ in 0..max_locals { params.push(ValType::I64); }
    params.push(ValType::I32); // env_ptr as last param
    let func_ty_idx = 0u32;
    types.function(params, vec![]);

    use std::collections::{HashMap, HashSet};
    let mut ffi_arities: HashMap<String, HashSet<u8>> = HashMap::new();
    for ch in &module.chunks {
        let instrs = Decoder::new(&ch.code)
            .decode_all()
            .map_err(|e| WasmAotError::Decode(format!("{:?}", e)))?;
        for ins in instrs {
            if let Instruction::FFICall(idx, argc) = ins {
                if let Some(Constant::String(name)) = module.constants.get(idx as usize) {
                    ffi_arities.entry(name.clone()).or_default().insert(argc);
                }
            }
        }
    }

    let mut arity_type_index: HashMap<u8, u32> = HashMap::new();
    let mut next_type_index = 1u32;
    for &arity in {
        let mut set = HashSet::new();
        for s in ffi_arities.values() { for &a in s { set.insert(a); } }
        set
    }.iter() {
        let mut params = Vec::new();
        for _ in 0..arity { params.push(ValType::I64); }
        types.function(params, vec![ValType::I64]);
        arity_type_index.insert(arity, next_type_index);
        next_type_index += 1;
    }
    m.section(&types);

    let mut mem = MemorySection::new();
    mem.memory(MemoryType { minimum: 1, maximum: None, memory64: false, shared: false });
    m.section(&mem);

    let mut globals = GlobalSection::new();
    let gtype = GlobalType { val_type: ValType::I32, mutable: true };
    let ginit = ConstExpr::i32_const(0);
    globals.global(gtype, &ginit);
    m.section(&globals);

    let mut str_offsets = Vec::new();
    let mut data_bytes: Vec<u8> = Vec::new();
    for c in &module.constants {
        if let Constant::String(s) = c {
            str_offsets.push(data_bytes.len() as u32);
            data_bytes.extend_from_slice(s.as_bytes());
            data_bytes.push(0);
        } else {
            str_offsets.push(u32::MAX);
        }
    }
    if !data_bytes.is_empty() {
        let mut data = DataSection::new();
        let offset = ConstExpr::i32_const(0);
        data.active(0, &offset, data_bytes.clone());
        m.section(&data);
    }

    let mut imports = ImportSection::new();
    let mut ffi_import_indices: HashMap<(String, u8), u32> = HashMap::new();
    let mut import_count = 0u32;
    for (name, arities) in &ffi_arities {
        for &arity in arities {
            let ty = arity_type_index.get(&arity).copied().unwrap_or(func_ty_idx);
            imports.import("ffi", name, wasm_encoder::EntityType::Function(ty));
            ffi_import_indices.insert((name.clone(), arity), import_count);
            import_count += 1;
        }
    }
    if import_count > 0 { m.section(&imports); }

    let mut table = TableSection::new();
    table.table(TableType { element_type: RefType::FUNCREF, minimum: module.chunks.len() as u32, maximum: None });
    m.section(&table);

    let mut elements = ElementSection::new();
    let mut func_indices: Vec<u32> = Vec::new();
    for i in 0..module.chunks.len() { func_indices.push(import_count + (i as u32)); }
    let offset0 = ConstExpr::i32_const(0);
    elements.active(Some(0), &offset0, Elements::Functions(&func_indices));
    m.section(&elements);

    let mut functions = FunctionSection::new();
    for _ in &module.chunks { functions.function(func_ty_idx); }
    m.section(&functions);

    let mut exports = ExportSection::new();
    exports.export("main", ExportKind::Func, 0);
    for (i, _) in module.chunks.iter().enumerate() {
        exports.export(&format!("chunk_{}", i), ExportKind::Func, i as u32);
    }
    m.section(&exports);

    let mut code = CodeSection::new();
    for ch in &module.chunks {
        let body = compile_chunk(module, ch, import_count, func_ty_idx, max_locals as u32, &ffi_import_indices)?;
        code.function(&body);
    }
    m.section(&code);

    Ok(m.finish())
}

fn compile_chunk(module: &NyarcModule, ch: &Chunk, import_count: u32, func_ty_idx: u32, max_locals: u32, ffi_import_indices: &std::collections::HashMap<(String, u8), u32>) -> Result<Function, WasmAotError> {
    let instrs = Decoder::new(&ch.code)
        .decode_all()
        .map_err(|e| WasmAotError::Decode(format!("{:?}", e)))?;
    let mut locals = Vec::new();
    // add one i32 local as temporary env pointer storage
    locals.push((1u32, ValType::I32));
    let env_param_index = max_locals; // last param index
    let env_tmp_local_index = 0u32; // first (and only) local we added
    let mut func = Function::new(locals);

    use std::collections::BTreeMap;
    let mut forward_end_map: BTreeMap<usize, usize> = BTreeMap::new();
    for (i_idx, ins) in instrs.iter().enumerate() {
        match *ins {
            Instruction::Jump(off) if off > 0 => {
                let end = (i_idx as isize + off as isize) as usize;
                forward_end_map.insert(i_idx, end);
            }
            Instruction::JumpIfFalse(off) if off > 0 => {
                let end = (i_idx as isize + off as isize) as usize;
                forward_end_map.insert(i_idx, end);
            }
            Instruction::JumpIfNull(off) if off > 0 => {
                let end = (i_idx as isize + off as isize) as usize;
                forward_end_map.insert(i_idx, end);
            }
            _ => {}
        }
    }

    let mut pending_ends: Vec<usize> = Vec::new();
    for (i_idx, ins) in instrs.into_iter().enumerate() {
        while let Some(&last_end) = pending_ends.last() {
            if last_end == i_idx { pending_ends.pop(); func.instruction(&WasmInst::End); } else { break; }
        }
        match ins {
            Instruction::Nop => {}
            Instruction::Push(idx) => {
                let c = module
                    .constants
                    .get(idx as usize)
                    .ok_or(WasmAotError::ConstantOutOfBounds(idx))?;
                match c {
                    Constant::Int(v) => {
                        func.instruction(&WasmInst::I64Const(*v));
                    }
                    Constant::Float(_) => return Err(WasmAotError::UnsupportedConstantType),
                    Constant::String(_) => return Err(WasmAotError::UnsupportedConstantType),
                }
            }
            Instruction::Pop => {
                func.instruction(&WasmInst::Drop);
            }
            Instruction::LoadLocal(i) => {
                func.instruction(&WasmInst::LocalGet(i as u32));
            }
            Instruction::StoreLocal(i) => {
                func.instruction(&WasmInst::LocalSet(i as u32));
            }
            Instruction::MakeClosure(func_idx, ref upvalues) => {
                let callee_index = import_count + (func_idx as u32);
                // ptr = heap_top
                func.instruction(&WasmInst::GlobalGet(0));
                func.instruction(&WasmInst::LocalSet(env_tmp_local_index));
                // store callee_index at [ptr + 0]
                func.instruction(&WasmInst::LocalGet(env_tmp_local_index));
                func.instruction(&WasmInst::I32Const(callee_index as i32));
                func.instruction(&WasmInst::I32Store(MemArg { offset: 0, align: 2, memory_index: 0 }));
                // store env_count at [ptr + 4]
                func.instruction(&WasmInst::LocalGet(env_tmp_local_index));
                func.instruction(&WasmInst::I32Const(upvalues.len() as i32));
                func.instruction(&WasmInst::I32Store(MemArg { offset: 4, align: 2, memory_index: 0 }));
                // store upvalues (currently zeros as placeholder)
                let mut off: u64 = 8;
                for _ in upvalues.iter() {
                    func.instruction(&WasmInst::LocalGet(env_tmp_local_index));
                    func.instruction(&WasmInst::I64Const(0));
                    func.instruction(&WasmInst::I64Store(MemArg { offset: off, align: 3, memory_index: 0 }));
                    off += 8;
                }
                // heap_top += (8 + upvalues*8)
                func.instruction(&WasmInst::GlobalGet(0));
                func.instruction(&WasmInst::I32Const((8 + upvalues.len() as u32 * 8) as i32));
                func.instruction(&WasmInst::I32Add);
                func.instruction(&WasmInst::GlobalSet(0));
                // push closure pointer as i64
                func.instruction(&WasmInst::LocalGet(env_tmp_local_index));
                func.instruction(&WasmInst::I64ExtendI32U);
            }
            Instruction::CallClosure(argc) => {
                // store closure ptr into tmp local
                func.instruction(&WasmInst::I32WrapI64);
                func.instruction(&WasmInst::LocalSet(env_tmp_local_index));
                // pad missing args with zeros
                let need_pad = (max_locals as i32) - (argc as i32);
                for _ in 0..need_pad.max(0) { func.instruction(&WasmInst::I64Const(0)); }
                // push env ptr as last param
                func.instruction(&WasmInst::LocalGet(env_tmp_local_index));
                func.instruction(&WasmInst::CallIndirect { ty: func_ty_idx, table: 0 });
            }
            Instruction::LoadUpvalue(i) => {
                let offset: u64 = (8 + (i as u32) * 8) as u64;
                func.instruction(&WasmInst::LocalGet(env_param_index));
                func.instruction(&WasmInst::I64Load(MemArg { offset, align: 3, memory_index: 0 }));
            }
            Instruction::StoreUpvalue(i) => {
                let offset: u64 = (8 + (i as u32) * 8) as u64;
                func.instruction(&WasmInst::LocalGet(env_param_index));
                func.instruction(&WasmInst::I64Store(MemArg { offset, align: 3, memory_index: 0 }));
            }
            Instruction::Jump(off) if off > 0 => {
                if let Some(&end) = forward_end_map.get(&i_idx) {
                    func.instruction(&WasmInst::Block(wasm_encoder::BlockType::Empty));
                    func.instruction(&WasmInst::Br(0));
                    pending_ends.push(end);
                } else {
                    return Err(WasmAotError::UnsupportedOpcode("Jump-negative-or-invalid".to_string()));
                }
            }
            Instruction::Jump(off) if off < 0 => {
                // basic backward jump support only to function start
                let target = (i_idx as isize + off as isize) as isize;
                if target == 0 {
                    // ensure outer loop exists
                    // open loop at function start by emitting it on first use
                    // we piggyback by emitting a block+loop at first backward jump
                    // For simplicity, emit a loop once at the very beginning
                    // Since we cannot inject retroactively, require i_idx==0 for now
                    func.instruction(&WasmInst::Br(0));
                } else {
                    return Err(WasmAotError::UnsupportedOpcode("Backward jump to non-zero target".to_string()));
                }
            }
            Instruction::JumpIfFalse(off) if off > 0 => {
                if let Some(&end) = forward_end_map.get(&i_idx) {
                    func.instruction(&WasmInst::Block(wasm_encoder::BlockType::Empty));
                    func.instruction(&WasmInst::I64Eqz);
                    func.instruction(&WasmInst::BrIf(0));
                    pending_ends.push(end);
                } else {
                    return Err(WasmAotError::UnsupportedOpcode("JumpIfFalse-negative-or-invalid".to_string()));
                }
            }
            Instruction::JumpIfFalse(off) if off < 0 => {
                let target = (i_idx as isize + off as isize) as isize;
                if target == 0 {
                    func.instruction(&WasmInst::I64Eqz);
                    func.instruction(&WasmInst::BrIf(0));
                } else {
                    return Err(WasmAotError::UnsupportedOpcode("Backward JumpIfFalse non-zero".to_string()));
                }
            }
            Instruction::JumpIfNull(off) if off > 0 => {
                if let Some(&end) = forward_end_map.get(&i_idx) {
                    func.instruction(&WasmInst::Block(wasm_encoder::BlockType::Empty));
                    func.instruction(&WasmInst::I64Eqz);
                    func.instruction(&WasmInst::BrIf(0));
                    pending_ends.push(end);
                } else {
                    return Err(WasmAotError::UnsupportedOpcode("JumpIfNull-negative-or-invalid".to_string()));
                }
            }
            Instruction::JumpIfNull(off) if off < 0 => {
                let target = (i_idx as isize + off as isize) as isize;
                if target == 0 {
                    func.instruction(&WasmInst::I64Eqz);
                    func.instruction(&WasmInst::BrIf(0));
                } else {
                    return Err(WasmAotError::UnsupportedOpcode("Backward JumpIfNull non-zero".to_string()));
                }
            }
            Instruction::Call(chunk_idx, argc) => {
                let target = import_count + (chunk_idx as u32);
                // pad missing args with zeros
                let need_pad = (max_locals as i32) - (argc as i32);
                for _ in 0..need_pad.max(0) { func.instruction(&WasmInst::I64Const(0)); }
                // env_ptr = 0
                func.instruction(&WasmInst::I32Const(0));
                func.instruction(&WasmInst::Call(target));
            }
            Instruction::FFICall(idx, argc) => {
                if let Some(Constant::String(name)) = module.constants.get(idx as usize) {
                    if let Some(&import_idx) = ffi_import_indices.get(&(name.clone(), argc)) {
                        func.instruction(&WasmInst::Call(import_idx));
                    } else {
                        return Err(WasmAotError::UnsupportedOpcode(format!("FFICall({}, {})", name, argc)));
                    }
                } else {
                    return Err(WasmAotError::UnsupportedConstantType);
                }
            }
            Instruction::Return | Instruction::Halt => {
                func.instruction(&WasmInst::Return);
                break;
            }
            other => {
                return Err(WasmAotError::UnsupportedOpcode(format!("{:?}", other)));
            }
        }
    }

    func.instruction(&WasmInst::End);
    Ok(func)
}

use nyar_error::WasmAotError;
use nyar_vm::aot::AotBackend;
use nyar_vm::bytecode::decoder::{Decoder, Instruction};
use nyar_vm::bytecode::format::{Constant, NyarcModule, Chunk};
use wasm_encoder::{CodeSection, ConstExpr, DataSection, ExportKind, ExportSection, Function, FunctionSection, ImportSection, Instruction as WasmInst, MemorySection, MemoryType, Module, TypeSection, ValType, TableSection, TableType, ElementSection, RefType, Elements, GlobalSection, GlobalType, MemArg};

pub struct WasmBackend;

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub enum ClosureTypeSelectionMode {
    ByCallsite,
}

#[derive(Clone, Copy, Debug, serde::Deserialize, serde::Serialize)]
pub struct WasmCompileOptions {
    pub strict_closure_arity_check: bool,
    pub closure_type_selection_by: ClosureTypeSelectionMode,
}

impl Default for WasmCompileOptions {
    fn default() -> Self {
        Self { strict_closure_arity_check: false, closure_type_selection_by: ClosureTypeSelectionMode::ByCallsite }
    }
}

impl AotBackend for WasmBackend {
    type Error = WasmAotError;
    type Config = WasmCompileOptions;
    fn compile(&self, module: &NyarcModule) -> Result<Vec<u8>, WasmAotError> {
        compile_module_to_wasm(module)
    }
    fn compile_with_config(&self, module: &NyarcModule, config: &Self::Config) -> Result<Vec<u8>, WasmAotError> {
        compile_module_to_wasm_with(module, *config)
    }
}

pub fn compile_module_to_wasm(module: &NyarcModule) -> Result<Vec<u8>, WasmAotError> {
    compile_module_to_wasm_with(module, WasmCompileOptions::default())
}

pub fn compile_module_to_wasm_with(module: &NyarcModule, options: WasmCompileOptions) -> Result<Vec<u8>, WasmAotError> {
    if module.chunks.is_empty() {
        return Err(WasmAotError::EmptyModule);
    }

    let mut m = Module::new();

    let mut types = TypeSection::new();
    use std::collections::{HashMap, HashSet};
    // 收集 FFI 的 arity
    let mut ffi_arities: HashMap<String, HashSet<u8>> = HashMap::new();
    // 收集闭包 Target 的 arity 以及模块内出现的闭包调用 arity
    let mut closure_target_arity_by_func: HashMap<u32, u32> = HashMap::new();
    let mut closure_arities: HashSet<u8> = HashSet::new();
    let mut closure_call_arities: HashSet<u8> = HashSet::new();
    for (func_idx, ch) in module.chunks.iter().enumerate() {
        closure_target_arity_by_func.insert(func_idx as u32, ch.locals as u32);
        let instrs = Decoder::new(&ch.code)
            .decode_all()
            .map_err(|e| WasmAotError::Decode(format!("{:?}", e)))?;
        for ins in instrs {
            match ins {
                Instruction::FFICall(idx, argc) => {
                    if let Some(Constant::String(name)) = module.constants.get(idx as usize) {
                        ffi_arities.entry(name.clone()).or_default().insert(argc);
                    }
                }
                Instruction::MakeClosure(target_idx, _uvs) => {
                    if let Some(arity) = closure_target_arity_by_func.get(&(target_idx as u32)) {
                        closure_arities.insert((*arity) as u8);
                    }
                }
                Instruction::CallClosure(argc) => {
                    closure_call_arities.insert(argc);
                }
                _ => {}
            }
        }
    }

    if options.strict_closure_arity_check {
        for call_a in &closure_call_arities {
            if !closure_arities.contains(call_a) {
                return Err(WasmAotError::Decode(format!("closure call arity {} not in target arities {:?}", call_a, closure_arities)));
            }
        }
    }

    // 细分函数签名：按 chunk.locals 以及闭包调用所需 arity 注册类型 (I64*arity, I32 env) -> ()
    let mut func_arity_type_index: HashMap<u32, u32> = HashMap::new();
    let mut next_type_index = 0u32;
    {
        let mut func_arities: HashSet<u32> = HashSet::new();
        for ch in &module.chunks { func_arities.insert(ch.locals as u32); }
        for &a in closure_arities.iter() { func_arities.insert(a as u32); }
        let mut arities: Vec<u32> = func_arities.into_iter().collect();
        arities.sort_unstable();
        for arity in arities {
            let mut params = Vec::new();
            for _ in 0..arity { params.push(ValType::I64); }
            params.push(ValType::I32); // env_ptr
            types.function(params, vec![]);
            func_arity_type_index.insert(arity, next_type_index);
            next_type_index += 1;
        }
    }

    // FFI 签名：按 arity 组织为 (I64*arity) -> I64
    let mut ffi_arity_type_index: HashMap<u8, u32> = HashMap::new();
    {
        let mut set = HashSet::new();
        for s in ffi_arities.values() { for &a in s { set.insert(a); } }
        let mut arities: Vec<u8> = set.into_iter().collect();
        arities.sort_unstable();
        for arity in arities {
            let mut params = Vec::new();
            for _ in 0..arity { params.push(ValType::I64); }
            types.function(params, vec![ValType::I64]);
            ffi_arity_type_index.insert(arity, next_type_index);
            next_type_index += 1;
        }
    }
    m.section(&types);

    let mut mem = MemorySection::new();
    mem.memory(MemoryType { minimum: 1, maximum: None, memory64: false, shared: false });
    m.section(&mem);

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

    let mut globals = GlobalSection::new();
    let gtype = GlobalType { val_type: ValType::I32, mutable: true };
    let initial_heap = data_bytes.len() as i32;
    let ginit = ConstExpr::i32_const(initial_heap);
    globals.global(gtype, &ginit);
    m.section(&globals);

    let mut imports = ImportSection::new();
    let mut ffi_import_indices: HashMap<(String, u8), u32> = HashMap::new();
    let mut import_count = 0u32;
    for (name, arities) in &ffi_arities {
        for &arity in arities {
            let ty = ffi_arity_type_index.get(&arity).copied().ok_or_else(|| WasmAotError::Decode("ffi arity type missing".to_string()))?;
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
    for ch in &module.chunks {
        let ty = func_arity_type_index.get(&(ch.locals as u32)).copied().ok_or_else(|| WasmAotError::Decode("chunk arity type missing".to_string()))?;
        functions.function(ty);
    }
    m.section(&functions);

    let mut exports = ExportSection::new();
    exports.export("main", ExportKind::Func, 0);
    for (i, _) in module.chunks.iter().enumerate() {
        exports.export(&format!("chunk_{}", i), ExportKind::Func, i as u32);
    }
    m.section(&exports);

    let mut code = CodeSection::new();
    for ch in &module.chunks {
        let body = compile_chunk(module, ch, import_count, &func_arity_type_index, &ffi_import_indices)?;
        code.function(&body);
    }
    m.section(&code);

    Ok(m.finish())
}

fn compile_chunk(module: &NyarcModule, ch: &Chunk, import_count: u32, func_arity_type_index: &std::collections::HashMap<u32, u32>, ffi_import_indices: &std::collections::HashMap<(String, u8), u32>) -> Result<Function, WasmAotError> {
    let instrs = Decoder::new(&ch.code)
        .decode_all()
        .map_err(|e| WasmAotError::Decode(format!("{:?}", e)))?;
    let mut locals = Vec::new();
    // add one i32 local as temporary env pointer storage
    locals.push((1u32, ValType::I32));
    let env_param_index = ch.locals as u32; // last param index = arity
    let env_tmp_local_index = 0u32; // first (and only) local we added
    let mut func = Function::new(locals);
    func.instruction(&WasmInst::Block(wasm_encoder::BlockType::Empty));
    func.instruction(&WasmInst::Loop(wasm_encoder::BlockType::Empty));

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
                let callee_index = func_idx as u32;
                let alloc_size = (8 + (upvalues.len() as u32) * 8) as i32;
                func.instruction(&WasmInst::GlobalGet(0));
                func.instruction(&WasmInst::I32Const(alloc_size));
                func.instruction(&WasmInst::I32Add);
                func.instruction(&WasmInst::MemorySize(0));
                func.instruction(&WasmInst::I32Const(65536));
                func.instruction(&WasmInst::I32Mul);
                func.instruction(&WasmInst::I32GtU);
                func.instruction(&WasmInst::If(wasm_encoder::BlockType::Empty));
                func.instruction(&WasmInst::GlobalGet(0));
                func.instruction(&WasmInst::I32Const(alloc_size));
                func.instruction(&WasmInst::I32Add);
                func.instruction(&WasmInst::MemorySize(0));
                func.instruction(&WasmInst::I32Const(65536));
                func.instruction(&WasmInst::I32Mul);
                func.instruction(&WasmInst::I32Sub);
                func.instruction(&WasmInst::I32Const(65535));
                func.instruction(&WasmInst::I32Add);
                func.instruction(&WasmInst::I32Const(65536));
                func.instruction(&WasmInst::I32DivU);
                func.instruction(&WasmInst::MemoryGrow(0));
                func.instruction(&WasmInst::Drop);
                func.instruction(&WasmInst::End);
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
                let mut off: u64 = 8;
                for uv in upvalues.iter() {
                    func.instruction(&WasmInst::LocalGet(env_tmp_local_index));
                    if uv.is_local {
                        func.instruction(&WasmInst::LocalGet(uv.index as u32));
                    } else {
                        let src_off: u64 = (8 + (uv.index as u32) * 8) as u64;
                        func.instruction(&WasmInst::LocalGet(env_param_index));
                        func.instruction(&WasmInst::I64Load(MemArg { offset: src_off, align: 3, memory_index: 0 }));
                    }
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
                func.instruction(&WasmInst::I32WrapI64);
                func.instruction(&WasmInst::LocalSet(env_tmp_local_index));
                // 不再填充到统一签名，只在栈上保留调用方提供的 argc 个参数
                func.instruction(&WasmInst::LocalGet(env_tmp_local_index));
                func.instruction(&WasmInst::LocalGet(env_tmp_local_index));
                func.instruction(&WasmInst::I32Load(MemArg { offset: 0, align: 2, memory_index: 0 }));
                let ty_idx = *func_arity_type_index.get(&(argc as u32)).ok_or_else(|| WasmAotError::Decode("closure arity type missing".to_string()))?;
                func.instruction(&WasmInst::CallIndirect { ty: ty_idx, table: 0 });
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
                let target = (i_idx as isize + off as isize) as isize;
                if target == 0 { func.instruction(&WasmInst::Br(0)); }
                else { return Err(WasmAotError::UnsupportedOpcode("Backward jump to non-zero target".to_string())); }
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
                if target == 0 { func.instruction(&WasmInst::I64Eqz); func.instruction(&WasmInst::BrIf(0)); }
                else { return Err(WasmAotError::UnsupportedOpcode("Backward JumpIfFalse non-zero".to_string())); }
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
                if target == 0 { func.instruction(&WasmInst::I64Eqz); func.instruction(&WasmInst::BrIf(0)); }
                else { return Err(WasmAotError::UnsupportedOpcode("Backward JumpIfNull non-zero".to_string())); }
            }
            Instruction::Call(chunk_idx, argc) => {
                let target = import_count + (chunk_idx as u32);
                let callee = module.chunks.get(chunk_idx as usize).ok_or_else(|| WasmAotError::Decode("callee out of range".to_string()))?;
                if (argc as u32) > (callee.locals as u32) {
                    return Err(WasmAotError::UnsupportedOpcode(format!("Call argc {} exceeds callee arity {}", argc, callee.locals)));
                }
                // pad missing args with zeros to callee arity
                let need_pad = (callee.locals as i32) - (argc as i32);
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
    func.instruction(&WasmInst::End);
    Ok(func)
}

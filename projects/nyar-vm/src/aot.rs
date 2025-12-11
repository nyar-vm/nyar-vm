use crate::bytecode::decoder::{Decoder, Instruction};
use crate::bytecode::format::{Constant, NyarcModule, Chunk};
use nyar_error::WasmAotError;
use wasm_encoder::{CodeSection, DataSection, ExportKind, ExportSection, Function, FunctionSection, Instruction as WasmInst, MemorySection, MemoryType, Module, TypeSection, ValType};

pub fn compile_module_to_wasm(module: &NyarcModule) -> Result<Vec<u8>, WasmAotError> {
    if module.chunks.is_empty() {
        return Err(WasmAotError::EmptyModule);
    }

    let mut m = Module::new();

    let mut types = TypeSection::new();
    let func_ty_idx = 0u32;
    types.function(vec![], vec![]);
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
        data.active(0, &WasmInst::I32Const(0), data_bytes.clone());
        m.section(&data);
    }

    let mut functions = FunctionSection::new();
    for _ in &module.chunks {
        functions.function(func_ty_idx);
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
        let body = compile_chunk(module, ch)?;
        code.function(&body);
    }
    m.section(&code);

    Ok(m.finish())
}

fn compile_chunk(module: &NyarcModule, ch: &Chunk) -> Result<Function, WasmAotError> {
    let instrs = Decoder::new(&ch.code)
        .decode_all()
        .map_err(|e| WasmAotError::Decode(format!("{:?}", e)))?;
    let mut locals = Vec::new();
    if ch.locals > 0 {
        locals.push((ch.locals as u32, ValType::I64));
    }
    let mut func = Function::new(locals);

    for ins in instrs {
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

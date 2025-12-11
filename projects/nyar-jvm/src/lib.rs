pub use nyar_error::JvmAotError;
pub use nyar_vm::bytecode::format::NyarcModule;
use nyar_vm::aot::AotBackend;
use nyar_vm::bytecode::decoder::{Decoder, Instruction};
use nyar_vm::bytecode::format::{Constant, Chunk};

pub struct JvmBackend;

impl AotBackend for JvmBackend {
    type Error = JvmAotError;
    type Config = ();
    fn compile(&self, module: &NyarcModule) -> Result<Vec<u8>, JvmAotError> {
        compile_module_to_jvm(module)
    }
    fn compile_with_config(&self, module: &NyarcModule, _config: &Self::Config) -> Result<Vec<u8>, JvmAotError> { compile_module_to_jvm(module) }
}

pub fn compile_module_to_jvm(module: &NyarcModule) -> Result<Vec<u8>, JvmAotError> {
    if module.chunks.is_empty() { return Err(JvmAotError::EmptyModule); }
    let mut class = Vec::new();
    class.extend_from_slice(&0xCAFEBABE_u32.to_be_bytes());
    class.extend_from_slice(&0u16.to_be_bytes());
    class.extend_from_slice(&52u16.to_be_bytes());

    let mut cp = Vec::new();
    let mut cp_count: u16 = 1;

    fn cp_utf8(cp: &mut Vec<u8>, s: &str, cp_count: &mut u16) -> u16 { cp.push(1); cp.extend_from_slice(&(s.len() as u16).to_be_bytes()); cp.extend_from_slice(s.as_bytes()); *cp_count += 1; *cp_count - 1 }
    fn cp_class(cp: &mut Vec<u8>, name_idx: u16, cp_count: &mut u16) -> u16 { cp.push(7); cp.extend_from_slice(&name_idx.to_be_bytes()); *cp_count += 1; *cp_count - 1 }
    fn cp_long(cp: &mut Vec<u8>, v: i64, cp_count: &mut u16) -> u16 { cp.push(5); cp.extend_from_slice(&(v as i64).to_be_bytes()); let idx = *cp_count; *cp_count += 2; idx }
    fn cp_name_and_type(cp: &mut Vec<u8>, name_idx: u16, desc_idx: u16, cp_count: &mut u16) -> u16 { cp.push(12); cp.extend_from_slice(&name_idx.to_be_bytes()); cp.extend_from_slice(&desc_idx.to_be_bytes()); *cp_count += 1; *cp_count - 1 }
    fn cp_methodref(cp: &mut Vec<u8>, class_idx: u16, nat_idx: u16, cp_count: &mut u16) -> u16 { cp.push(10); cp.extend_from_slice(&class_idx.to_be_bytes()); cp.extend_from_slice(&nat_idx.to_be_bytes()); *cp_count += 1; *cp_count - 1 }

    let idx_main_utf8 = cp_utf8(&mut cp, "Main", &mut cp_count);
    let idx_class_main = cp_class(&mut cp, idx_main_utf8, &mut cp_count);
    let idx_obj_utf8 = cp_utf8(&mut cp, "java/lang/Object", &mut cp_count);
    let idx_class_obj = cp_class(&mut cp, idx_obj_utf8, &mut cp_count);
    let idx_code_utf8 = cp_utf8(&mut cp, "Code", &mut cp_count);

    use std::collections::HashMap;
    let mut long_indices: HashMap<i64, u16> = HashMap::new();
    for ch in &module.chunks {
        let instrs = Decoder::new(&ch.code).decode_all().map_err(|e| JvmAotError::Decode(format!("{:?}", e)))?;
        for ins in &instrs {
            if let Instruction::Push(idx) = ins {
                if let Some(Constant::Int(v)) = module.constants.get(*idx as usize) {
                    if *v != 0 && *v != 1 { long_indices.entry(*v).or_insert_with(|| cp_long(&mut cp, *v, &mut cp_count)); }
                }
            }
        }
    }

    // Prepare per-chunk name/descriptor and methodrefs
    let mut chunk_name_idx: Vec<u16> = Vec::new();
    let mut chunk_desc_idx: Vec<u16> = Vec::new();
    let mut chunk_nat_idx: Vec<u16> = Vec::new();
    let mut chunk_mref_idx: Vec<u16> = Vec::new();
    for (i, ch) in module.chunks.iter().enumerate() {
        let name = format!("chunk_{}", i);
        let mut desc = String::new();
        desc.push('(');
        for _ in 0..ch.locals { desc.push('J'); }
        desc.push_str("Ljava/lang/Object;");
        desc.push(')');
        desc.push('J');
        let nidx = cp_utf8(&mut cp, &name, &mut cp_count);
        let didx = cp_utf8(&mut cp, &desc, &mut cp_count);
        let nat = cp_name_and_type(&mut cp, nidx, didx, &mut cp_count);
        let mref = cp_methodref(&mut cp, idx_class_main, nat, &mut cp_count);
        chunk_name_idx.push(nidx);
        chunk_desc_idx.push(didx);
        chunk_nat_idx.push(nat);
        chunk_mref_idx.push(mref);
    }

    class.extend_from_slice(&cp_count.to_be_bytes());
    class.extend_from_slice(&cp);

    let access = 0x0021u16;
    class.extend_from_slice(&access.to_be_bytes());
    class.extend_from_slice(&idx_class_main.to_be_bytes());
    class.extend_from_slice(&idx_class_obj.to_be_bytes());
    class.extend_from_slice(&0u16.to_be_bytes());
    class.extend_from_slice(&0u16.to_be_bytes());
    class.extend_from_slice(&(module.chunks.len() as u16).to_be_bytes());

    // Emit methods per chunk
    for (i, ch) in module.chunks.iter().enumerate() {
        class.extend_from_slice(&0x0009u16.to_be_bytes()); // public static
        class.extend_from_slice(&chunk_name_idx[i].to_be_bytes());
        class.extend_from_slice(&chunk_desc_idx[i].to_be_bytes());
        class.extend_from_slice(&1u16.to_be_bytes()); // attributes_count

        // Code attribute
        class.extend_from_slice(&idx_code_utf8.to_be_bytes());
        let instrs = Decoder::new(&ch.code).decode_all().map_err(|e| JvmAotError::Decode(format!("{:?}", e)))?;
        let mut code: Vec<u8> = Vec::new();
        let mut ins_offsets: Vec<u32> = Vec::with_capacity(instrs.len());
        let mut branches: Vec<(usize, usize)> = Vec::new();
        for (i_idx, ins) in instrs.into_iter().enumerate() {
            ins_offsets.push(code.len() as u32);
            match ins {
                Instruction::Push(idx) => {
                    match module.constants.get(idx as usize) {
                        Some(Constant::Int(v)) => {
                            if *v == 0 { code.push(0x09); } else if *v == 1 { code.push(0x0A); } else { let cp_idx = *long_indices.get(v).unwrap(); code.push(0x14); code.extend_from_slice(&cp_idx.to_be_bytes()); }
                        }
                        _ => return Err(JvmAotError::UnsupportedOpcode("Push-non-int".to_string())),
                    }
                }
                Instruction::Pop => { code.push(0x58); }
                Instruction::LoadLocal(i) => {
                    match i { 0 => code.push(0x1E), 1 => code.push(0x1F), 2 => code.push(0x20), 3 => code.push(0x21), _ => { code.push(0x16); code.push(i); } }
                }
                Instruction::StoreLocal(i) => {
                    match i { 0 => code.push(0x3F), 1 => code.push(0x40), 2 => code.push(0x41), 3 => code.push(0x42), _ => { code.push(0x37); code.push(i); } }
                }
                Instruction::Jump(off) => {
                    let target = (i_idx as isize + off as isize) as isize;
                    if target < 0 { return Err(JvmAotError::UnsupportedOpcode("Jump-negative-target".to_string())); }
                    code.push(0xA7);
                    let pos = code.len(); code.extend_from_slice(&0i16.to_be_bytes()); branches.push((pos, target as usize));
                }
                Instruction::JumpIfFalse(off) | Instruction::JumpIfNull(off) => {
                    let target = (i_idx as isize + off as isize) as isize;
                    if target < 0 { return Err(JvmAotError::UnsupportedOpcode("JumpIf-negative-target".to_string())); }
                    code.push(0x09); code.push(0x94); code.push(0x99);
                    let pos = code.len(); code.extend_from_slice(&0i16.to_be_bytes()); branches.push((pos, target as usize));
                }
                Instruction::Call(target_idx, argc) => {
                    let callee = module.chunks.get(target_idx as usize).ok_or_else(|| JvmAotError::Decode("callee out of range".to_string()))?;
                    let need_pad = if (argc as u16) >= callee.locals { 0 } else { (callee.locals - argc as u16) as usize };
                    for _ in 0..need_pad { code.push(0x09); }
                    code.push(0x01);
                    code.push(0xB8); // invokestatic
                    let mr = chunk_mref_idx[target_idx as usize];
                    code.extend_from_slice(&mr.to_be_bytes());
                }
                Instruction::Return | Instruction::Halt => { code.push(0xAD); }
                other => { return Err(JvmAotError::UnsupportedOpcode(format!("{:?}", other))); }
            }
        }
        for (pos, target_idx) in branches.iter().copied() {
            if target_idx >= ins_offsets.len() { return Err(JvmAotError::Decode("branch target out of range".to_string())); }
            let target_off = ins_offsets[target_idx] as i32; let next_off = (pos as i32) + 2; let rel = target_off - next_off; let rel16 = rel as i16; let bytes = rel16.to_be_bytes(); code[pos] = bytes[0]; code[pos + 1] = bytes[1];
        }
        let code_len = code.len() as u32;
        let max_stack = ch.max_stack; let max_locals = ch.locals * 2 + 1;
        let attr_len = 12 + code_len;
        class.extend_from_slice(&attr_len.to_be_bytes());
        class.extend_from_slice(&max_stack.to_be_bytes());
        class.extend_from_slice(&max_locals.to_be_bytes());
        class.extend_from_slice(&code_len.to_be_bytes());
        class.extend_from_slice(&code);
        class.extend_from_slice(&0u16.to_be_bytes()); // exception_table_length
        class.extend_from_slice(&0u16.to_be_bytes()); // attributes_count within Code
    }

    class.extend_from_slice(&0u16.to_be_bytes()); // class attributes_count
    Ok(class)
}

pub fn write_jar(path: &str, class_bytes: &[u8]) -> std::io::Result<()> {
    use std::fs;
    use std::io::Write;
    use zip::write::FileOptions;
    let file = fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = FileOptions::default();
    zip.start_file("META-INF/MANIFEST.MF", opts)?;
    zip.write_all(b"Manifest-Version: 1.0\n")?;
    zip.start_file("Main.class", opts)?;
    zip.write_all(class_bytes)?;
    zip.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyar_vm::bytecode::format::{minimal_module_with_chunk, Constant};
    use nyar_vm::bytecode::opcode::Opcode;

    #[test]
    fn compile_stub_class() {
        let mut code = Vec::new();
        code.push(Opcode::Halt as u8);
        let module = minimal_module_with_chunk(code, vec![Constant::Int(1)]);
        let class = compile_module_to_jvm(&module).expect("ok");
        assert!(class.len() > 8);
        assert_eq!(&class[0..4], &[0xCA, 0xFE, 0xBA, 0xBE]);
    }
}

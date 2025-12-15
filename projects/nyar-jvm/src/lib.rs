pub use nyar_error::JvmAotError;
use nyar_vm::aot::AotCompiler;
use nyar_vm::bytecode::decoder::{Decoder, Instruction};
use nyar_vm::bytecode::format::Constant;
pub use nyar_vm::bytecode::format::NyarcModule;

pub struct JvmBackend;

impl AotCompiler for JvmBackend {
    type Error = JvmAotError;
    type Config = ();
    fn compile(&self, module: &NyarcModule) -> Result<Vec<u8>, JvmAotError> {
        compile_module_to_jvm(module)
    }
    fn compile_with_config(
        &self,
        module: &NyarcModule,
        _config: &Self::Config,
    ) -> Result<Vec<u8>, JvmAotError> {
        compile_module_to_jvm(module)
    }
}

pub fn compile_module_to_jvm(module: &NyarcModule) -> Result<Vec<u8>, JvmAotError> {
    if module.chunks.is_empty() {
        return Err(JvmAotError::EmptyModule);
    }
    let mut class = Vec::new();
    class.extend_from_slice(&0xCAFEBABE_u32.to_be_bytes());
    class.extend_from_slice(&0u16.to_be_bytes());
    class.extend_from_slice(&49u16.to_be_bytes());

    let mut cp = Vec::new();
    let mut cp_count: u16 = 1;

    fn cp_utf8(cp: &mut Vec<u8>, s: &str, cp_count: &mut u16) -> u16 {
        let mut bytes = Vec::new();
        for c in s.chars() {
            let u = c as u32;
            if u == 0 {
                bytes.push(0xC0);
                bytes.push(0x80);
            } else if u <= 0x7F {
                bytes.push(u as u8);
            } else if u <= 0x7FF {
                bytes.push(0xC0 | ((u >> 6) as u8));
                bytes.push(0x80 | ((u & 0x3F) as u8));
            } else if u <= 0xFFFF {
                bytes.push(0xE0 | ((u >> 12) as u8));
                bytes.push(0x80 | (((u >> 6) & 0x3F) as u8));
                bytes.push(0x80 | ((u & 0x3F) as u8));
            } else {
                let u = u - 0x10000;
                let high = 0xD800 | (u >> 10);
                let low = 0xDC00 | (u & 0x3FF);
                bytes.push(0xE0 | ((high >> 12) as u8));
                bytes.push(0x80 | (((high >> 6) & 0x3F) as u8));
                bytes.push(0x80 | ((high & 0x3F) as u8));
                bytes.push(0xE0 | ((low >> 12) as u8));
                bytes.push(0x80 | (((low >> 6) & 0x3F) as u8));
                bytes.push(0x80 | ((low & 0x3F) as u8));
            }
        }
        cp.push(1);
        cp.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
        cp.extend_from_slice(&bytes);
        *cp_count += 1;
        *cp_count - 1
    }
    fn cp_class(cp: &mut Vec<u8>, name_idx: u16, cp_count: &mut u16) -> u16 {
        cp.push(7);
        cp.extend_from_slice(&name_idx.to_be_bytes());
        *cp_count += 1;
        *cp_count - 1
    }
    fn cp_long(cp: &mut Vec<u8>, v: i64, cp_count: &mut u16) -> u16 {
        cp.push(5);
        cp.extend_from_slice(&(v as i64).to_be_bytes());
        let idx = *cp_count;
        *cp_count += 2;
        idx
    }
    fn cp_name_and_type(cp: &mut Vec<u8>, name_idx: u16, desc_idx: u16, cp_count: &mut u16) -> u16 {
        cp.push(12);
        cp.extend_from_slice(&name_idx.to_be_bytes());
        cp.extend_from_slice(&desc_idx.to_be_bytes());
        *cp_count += 1;
        *cp_count - 1
    }
    fn cp_methodref(cp: &mut Vec<u8>, class_idx: u16, nat_idx: u16, cp_count: &mut u16) -> u16 {
        cp.push(10);
        cp.extend_from_slice(&class_idx.to_be_bytes());
        cp.extend_from_slice(&nat_idx.to_be_bytes());
        *cp_count += 1;
        *cp_count - 1
    }
    fn cp_string(cp: &mut Vec<u8>, utf8_idx: u16, cp_count: &mut u16) -> u16 {
        cp.push(8);
        cp.extend_from_slice(&utf8_idx.to_be_bytes());
        *cp_count += 1;
        *cp_count - 1
    }
    fn cp_fieldref(cp: &mut Vec<u8>, class_idx: u16, nat_idx: u16, cp_count: &mut u16) -> u16 {
        cp.push(9);
        cp.extend_from_slice(&class_idx.to_be_bytes());
        cp.extend_from_slice(&nat_idx.to_be_bytes());
        *cp_count += 1;
        *cp_count - 1
    }

    // Common CP entries
    let idx_main_utf8 = cp_utf8(&mut cp, "Main", &mut cp_count);
    let idx_class_main = cp_class(&mut cp, idx_main_utf8, &mut cp_count);
    let idx_obj_utf8 = cp_utf8(&mut cp, "java/lang/Object", &mut cp_count);
    let idx_class_obj = cp_class(&mut cp, idx_obj_utf8, &mut cp_count);
    let idx_obj_arr_utf8 = cp_utf8(&mut cp, "[Ljava/lang/Object;", &mut cp_count);
    let idx_class_obj_arr = cp_class(&mut cp, idx_obj_arr_utf8, &mut cp_count);
    let idx_code_utf8 = cp_utf8(&mut cp, "Code", &mut cp_count);

    // Boxed Long support
    let idx_long_utf8 = cp_utf8(&mut cp, "java/lang/Long", &mut cp_count);
    let idx_cls_long = cp_class(&mut cp, idx_long_utf8, &mut cp_count);
    let idx_valueof_utf8 = cp_utf8(&mut cp, "valueOf", &mut cp_count);
    let idx_valueof_desc_utf8 = cp_utf8(&mut cp, "(J)Ljava/lang/Long;", &mut cp_count);
    let idx_valueof_nat = cp_name_and_type(
        &mut cp,
        idx_valueof_utf8,
        idx_valueof_desc_utf8,
        &mut cp_count,
    );
    let idx_valueof_mref = cp_methodref(&mut cp, idx_cls_long, idx_valueof_nat, &mut cp_count);

    let idx_longvalue_utf8 = cp_utf8(&mut cp, "longValue", &mut cp_count);
    let idx_longvalue_desc_utf8 = cp_utf8(&mut cp, "()J", &mut cp_count);
    let idx_longvalue_nat = cp_name_and_type(
        &mut cp,
        idx_longvalue_utf8,
        idx_longvalue_desc_utf8,
        &mut cp_count,
    );
    let idx_longvalue_mref = cp_methodref(&mut cp, idx_cls_long, idx_longvalue_nat, &mut cp_count);

    // Boxed Boolean support
    let idx_bool_utf8 = cp_utf8(&mut cp, "java/lang/Boolean", &mut cp_count);
    let idx_cls_bool = cp_class(&mut cp, idx_bool_utf8, &mut cp_count);
    let idx_bool_valueof_utf8 = cp_utf8(&mut cp, "valueOf", &mut cp_count);
    let idx_bool_valueof_desc_utf8 = cp_utf8(&mut cp, "(Z)Ljava/lang/Boolean;", &mut cp_count);
    let idx_bool_valueof_nat = cp_name_and_type(
        &mut cp,
        idx_bool_valueof_utf8,
        idx_bool_valueof_desc_utf8,
        &mut cp_count,
    );
    let idx_bool_valueof_mref =
        cp_methodref(&mut cp, idx_cls_bool, idx_bool_valueof_nat, &mut cp_count);
    let idx_bool_boolvalue_utf8 = cp_utf8(&mut cp, "booleanValue", &mut cp_count);
    let idx_bool_boolvalue_desc_utf8 = cp_utf8(&mut cp, "()Z", &mut cp_count);
    let idx_bool_boolvalue_nat = cp_name_and_type(
        &mut cp,
        idx_bool_boolvalue_utf8,
        idx_bool_boolvalue_desc_utf8,
        &mut cp_count,
    );
    let idx_bool_boolvalue_mref =
        cp_methodref(&mut cp, idx_cls_bool, idx_bool_boolvalue_nat, &mut cp_count);

    let idx_str_utf8 = cp_utf8(&mut cp, "java/lang/String", &mut cp_count);
    let idx_cls_str = cp_class(&mut cp, idx_str_utf8, &mut cp_count);

    // I/O support
    let idx_ps_utf8 = cp_utf8(&mut cp, "java/io/PrintStream", &mut cp_count);
    let idx_cls_ps = cp_class(&mut cp, idx_ps_utf8, &mut cp_count);
    let idx_sys_utf8 = cp_utf8(&mut cp, "java/lang/System", &mut cp_count);
    let idx_cls_sys = cp_class(&mut cp, idx_sys_utf8, &mut cp_count);
    let idx_out_utf8 = cp_utf8(&mut cp, "out", &mut cp_count);
    let idx_out_desc_utf8 = cp_utf8(&mut cp, "Ljava/io/PrintStream;", &mut cp_count);
    let idx_out_nat = cp_name_and_type(&mut cp, idx_out_utf8, idx_out_desc_utf8, &mut cp_count);
    let idx_out_fref = cp_fieldref(&mut cp, idx_cls_sys, idx_out_nat, &mut cp_count);
    let idx_println_utf8 = cp_utf8(&mut cp, "println", &mut cp_count);
    let idx_println_desc_utf8 = cp_utf8(&mut cp, "(Ljava/lang/Object;)V", &mut cp_count);
    let idx_println_nat = cp_name_and_type(
        &mut cp,
        idx_println_utf8,
        idx_println_desc_utf8,
        &mut cp_count,
    );
    let idx_println_mref = cp_methodref(&mut cp, idx_cls_ps, idx_println_nat, &mut cp_count);

    let idx_exit_utf8 = cp_utf8(&mut cp, "exit", &mut cp_count);
    let idx_exit_desc_utf8 = cp_utf8(&mut cp, "(I)V", &mut cp_count);
    let idx_exit_nat = cp_name_and_type(&mut cp, idx_exit_utf8, idx_exit_desc_utf8, &mut cp_count);
    let idx_exit_mref = cp_methodref(&mut cp, idx_cls_sys, idx_exit_nat, &mut cp_count);

    // File I/O
    let idx_paths_utf8 = cp_utf8(&mut cp, "java/nio/file/Paths", &mut cp_count);
    let idx_cls_paths = cp_class(&mut cp, idx_paths_utf8, &mut cp_count);
    let idx_files_utf8 = cp_utf8(&mut cp, "java/nio/file/Files", &mut cp_count);
    let idx_cls_files = cp_class(&mut cp, idx_files_utf8, &mut cp_count);
    let idx_openopt_utf8 = cp_utf8(&mut cp, "java/nio/file/OpenOption", &mut cp_count);
    let idx_cls_openopt = cp_class(&mut cp, idx_openopt_utf8, &mut cp_count);

    // Other string methods
    let idx_concat_utf8 = cp_utf8(&mut cp, "concat", &mut cp_count);
    let idx_concat_desc_utf8 = cp_utf8(
        &mut cp,
        "(Ljava/lang/String;)Ljava/lang/String;",
        &mut cp_count,
    );
    let idx_concat_nat = cp_name_and_type(
        &mut cp,
        idx_concat_utf8,
        idx_concat_desc_utf8,
        &mut cp_count,
    );
    let idx_concat_mref = cp_methodref(&mut cp, idx_cls_str, idx_concat_nat, &mut cp_count);

    let idx_length_utf8 = cp_utf8(&mut cp, "length", &mut cp_count);
    let idx_length_desc_utf8 = cp_utf8(&mut cp, "()I", &mut cp_count);
    let idx_length_nat = cp_name_and_type(
        &mut cp,
        idx_length_utf8,
        idx_length_desc_utf8,
        &mut cp_count,
    );
    let idx_length_mref = cp_methodref(&mut cp, idx_cls_str, idx_length_nat, &mut cp_count);

    let idx_getbytes_utf8 = cp_utf8(&mut cp, "getBytes", &mut cp_count);
    let idx_getbytes_desc_utf8 = cp_utf8(&mut cp, "(Ljava/lang/String;)[B", &mut cp_count);
    let idx_getbytes_nat = cp_name_and_type(
        &mut cp,
        idx_getbytes_utf8,
        idx_getbytes_desc_utf8,
        &mut cp_count,
    );
    let idx_getbytes_mref = cp_methodref(&mut cp, idx_cls_str, idx_getbytes_nat, &mut cp_count);

    let idx_paths_get_utf8 = cp_utf8(&mut cp, "get", &mut cp_count);
    let idx_paths_get_desc_utf8 = cp_utf8(
        &mut cp,
        "(Ljava/lang/String;[Ljava/lang/String;)Ljava/nio/file/Path;",
        &mut cp_count,
    );
    let idx_paths_get_nat = cp_name_and_type(
        &mut cp,
        idx_paths_get_utf8,
        idx_paths_get_desc_utf8,
        &mut cp_count,
    );
    let idx_paths_get_mref = cp_methodref(&mut cp, idx_cls_paths, idx_paths_get_nat, &mut cp_count);

    let idx_files_read_utf8 = cp_utf8(&mut cp, "readAllBytes", &mut cp_count);
    let idx_files_read_desc_utf8 = cp_utf8(&mut cp, "(Ljava/nio/file/Path;)[B", &mut cp_count);
    let idx_files_read_nat = cp_name_and_type(
        &mut cp,
        idx_files_read_utf8,
        idx_files_read_desc_utf8,
        &mut cp_count,
    );
    let idx_files_read_mref =
        cp_methodref(&mut cp, idx_cls_files, idx_files_read_nat, &mut cp_count);

    let idx_files_write_utf8 = cp_utf8(&mut cp, "write", &mut cp_count);
    let idx_files_write_desc_utf8 = cp_utf8(
        &mut cp,
        "(Ljava/nio/file/Path;[B[Ljava/nio/file/OpenOption;)Ljava/nio/file/Path;",
        &mut cp_count,
    );
    let idx_files_write_nat = cp_name_and_type(
        &mut cp,
        idx_files_write_utf8,
        idx_files_write_desc_utf8,
        &mut cp_count,
    );
    let idx_files_write_mref =
        cp_methodref(&mut cp, idx_cls_files, idx_files_write_nat, &mut cp_count);

    let idx_init_utf8 = cp_utf8(&mut cp, "<init>", &mut cp_count);
    let idx_init_desc_utf8 = cp_utf8(&mut cp, "([BLjava/lang/String;)V", &mut cp_count);
    let idx_init_nat = cp_name_and_type(&mut cp, idx_init_utf8, idx_init_desc_utf8, &mut cp_count);
    let idx_init_mref = cp_methodref(&mut cp, idx_cls_str, idx_init_nat, &mut cp_count);

    let idx_substring_utf8 = cp_utf8(&mut cp, "substring", &mut cp_count);
    let idx_substring_desc_utf8 = cp_utf8(&mut cp, "(II)Ljava/lang/String;", &mut cp_count);
    let idx_substring_nat = cp_name_and_type(
        &mut cp,
        idx_substring_utf8,
        idx_substring_desc_utf8,
        &mut cp_count,
    );
    let idx_substring_mref = cp_methodref(&mut cp, idx_cls_str, idx_substring_nat, &mut cp_count);

    let idx_charat_utf8 = cp_utf8(&mut cp, "charAt", &mut cp_count);
    let idx_charat_desc_utf8 = cp_utf8(&mut cp, "(I)C", &mut cp_count);
    let idx_charat_nat = cp_name_and_type(
        &mut cp,
        idx_charat_utf8,
        idx_charat_desc_utf8,
        &mut cp_count,
    );
    let idx_charat_mref = cp_methodref(&mut cp, idx_cls_str, idx_charat_nat, &mut cp_count);

    let idx_equals_utf8 = cp_utf8(&mut cp, "equals", &mut cp_count);
    let idx_equals_desc_utf8 = cp_utf8(&mut cp, "(Ljava/lang/Object;)Z", &mut cp_count);
    let idx_equals_nat = cp_name_and_type(
        &mut cp,
        idx_equals_utf8,
        idx_equals_desc_utf8,
        &mut cp_count,
    );
    let idx_equals_mref = cp_methodref(&mut cp, idx_class_obj, idx_equals_nat, &mut cp_count);

    let idx_tostring_utf8 = cp_utf8(&mut cp, "toString", &mut cp_count);
    let idx_tostring_desc_utf8 = cp_utf8(&mut cp, "()Ljava/lang/String;", &mut cp_count);
    let idx_tostring_nat = cp_name_and_type(
        &mut cp,
        idx_tostring_utf8,
        idx_tostring_desc_utf8,
        &mut cp_count,
    );
    let idx_tostring_mref = cp_methodref(&mut cp, idx_class_obj, idx_tostring_nat, &mut cp_count);

    let idx_utf8_utf8 = cp_utf8(&mut cp, "UTF-8", &mut cp_count);
    let idx_utf8_str = cp_string(&mut cp, idx_utf8_utf8, &mut cp_count);

    // Collect Constants, Field Layouts and Needed Dispatches
    use std::collections::{HashMap, HashSet};
    let mut long_indices: HashMap<i64, u16> = HashMap::new();
    let mut string_indices: HashMap<String, u16> = HashMap::new();
    let mut needed_dispatches: HashSet<(String, u8)> = HashSet::new();
    let mut field_impls: HashMap<String, HashMap<u16, u8>> = HashMap::new();

    // Precompute field layouts: field name -> (class_idx -> field_slot)
    for (cls_idx, cls) in module.classes.iter().enumerate() {
        for (i, field) in cls.fields.iter().enumerate() {
            field_impls
                .entry(field.clone())
                .or_default()
                .insert(cls_idx as u16, i as u8);
        }
    }

    // Add builtins to needed_dispatches
    needed_dispatches.insert(("len".to_string(), 0));
    needed_dispatches.insert(("to_string".to_string(), 0));
    needed_dispatches.insert(("lt".to_string(), 1));
    needed_dispatches.insert(("gt".to_string(), 1));
    needed_dispatches.insert(("le".to_string(), 1));
    needed_dispatches.insert(("ge".to_string(), 1));
    needed_dispatches.insert(("eq".to_string(), 1));
    needed_dispatches.insert(("ne".to_string(), 1));
    needed_dispatches.insert(("get".to_string(), 2));
    needed_dispatches.insert(("get".to_string(), 1));
    needed_dispatches.insert(("set".to_string(), 2));
    needed_dispatches.insert(("push".to_string(), 1));
    needed_dispatches.insert(("add".to_string(), 1));
    needed_dispatches.insert(("sub".to_string(), 1));
    needed_dispatches.insert(("mul".to_string(), 1));
    needed_dispatches.insert(("div".to_string(), 1));
    needed_dispatches.insert(("rem".to_string(), 1));

    for ch in &module.chunks {
        let instrs = Decoder::new(&ch.code)
            .decode_all()
            .map_err(|e| JvmAotError::Decode(format!("{:?}", e)))?;
        for ins in &instrs {
            if let Instruction::Push(idx) = ins {
                if let Some(Constant::Int(v)) = module.constants.get(*idx as usize) {
                    long_indices
                        .entry(*v)
                        .or_insert_with(|| cp_long(&mut cp, *v, &mut cp_count));
                } else if let Some(Constant::String(s)) = module.constants.get(*idx as usize) {
                    let key = s.clone();
                    string_indices.entry(key.clone()).or_insert_with(|| {
                        let u = cp_utf8(&mut cp, &key, &mut cp_count);
                        cp_string(&mut cp, u, &mut cp_count)
                    });
                }
            } else if let Instruction::StringConst(s) = ins {
                let key = s.clone();
                string_indices.entry(key.clone()).or_insert_with(|| {
                    let u = cp_utf8(&mut cp, &key, &mut cp_count);
                    cp_string(&mut cp, u, &mut cp_count)
                });
            } else if let Instruction::InvokeMethod(idx, argc) = ins {
                if let Some(Constant::String(s)) = module.constants.get(*idx as usize) {
                    needed_dispatches.insert((s.clone(), *argc));
                }
            } else if let Instruction::NewObject(class_idx) = ins {
                long_indices
                    .entry(*class_idx as i64)
                    .or_insert_with(|| cp_long(&mut cp, *class_idx as i64, &mut cp_count));
            }
        }
    }

    // Method Impls: method_name -> (class_idx -> chunk_idx)
    let mut method_impls: HashMap<String, HashMap<u16, u16>> = HashMap::new();
    println!("DEBUG: impls len: {}", module.impls.len());
    for impl_info in &module.impls {
        if let Some(trait_info) = module.traits.get(impl_info.trait_idx as usize) {
            for (i, method_name) in trait_info.methods.iter().enumerate() {
                if i < impl_info.methods.len() {
                    let chunk_idx = impl_info.methods[i];
                    method_impls
                        .entry(method_name.clone())
                        .or_default()
                        .insert(impl_info.class_idx, chunk_idx);
                }
            }
        }
    }

    // Chunk methods
    let mut chunk_name_idx: Vec<u16> = Vec::new();
    let mut chunk_desc_idx: Vec<u16> = Vec::new();
    let mut chunk_nat_idx: Vec<u16> = Vec::new();
    let mut chunk_mref_idx: Vec<u16> = Vec::new();
    for (i, ch) in module.chunks.iter().enumerate() {
        let name = format!("chunk_{}", i);
        let mut desc = String::new();
        desc.push('(');
        for _ in 0..ch.locals {
            desc.push_str("Ljava/lang/Object;");
        }
        desc.push_str("Ljava/lang/Object;"); // closure context/extra
        desc.push(')');
        desc.push_str("Ljava/lang/Object;");
        let nidx = cp_utf8(&mut cp, &name, &mut cp_count);
        let didx = cp_utf8(&mut cp, &desc, &mut cp_count);
        let nat = cp_name_and_type(&mut cp, nidx, didx, &mut cp_count);
        let mref = cp_methodref(&mut cp, idx_class_main, nat, &mut cp_count);
        chunk_name_idx.push(nidx);
        chunk_desc_idx.push(didx);
        chunk_nat_idx.push(nat);
        chunk_mref_idx.push(mref);
    }

    // Dispatch methods
    let mut dispatch_mref_map: HashMap<(String, u8), u16> = HashMap::new();
    let mut dispatch_defs: HashMap<(String, u8), (u16, u16)> = HashMap::new();
    let mut sorted_dispatches: Vec<_> = needed_dispatches.iter().collect();
    sorted_dispatches.sort();

    for (name, argc) in &sorted_dispatches {
        let func_name = format!("dispatch_{}_{}", name, argc);
        let mut desc = String::new();
        desc.push('(');
        for _ in 0..(*argc + 1) {
            // receiver + args
            desc.push_str("Ljava/lang/Object;");
        }
        desc.push_str("Ljava/lang/Object;"); // closure context/extra
        desc.push(')');
        desc.push_str("Ljava/lang/Object;");
        let nidx = cp_utf8(&mut cp, &func_name, &mut cp_count);
        let didx = cp_utf8(&mut cp, &desc, &mut cp_count);
        let nat = cp_name_and_type(&mut cp, nidx, didx, &mut cp_count);
        let mref = cp_methodref(&mut cp, idx_class_main, nat, &mut cp_count);
        dispatch_mref_map.insert((name.clone(), *argc), mref);
        dispatch_defs.insert((name.clone(), *argc), (nidx, didx));
    }

    // Main method
    let idx_main_name = cp_utf8(&mut cp, "main", &mut cp_count);
    let idx_main_desc = cp_utf8(&mut cp, "([Ljava/lang/String;)V", &mut cp_count);
    class.extend_from_slice(&cp_count.to_be_bytes());
    class.extend_from_slice(&cp);

    let access = 0x0021u16;
    class.extend_from_slice(&access.to_be_bytes());
    class.extend_from_slice(&idx_class_main.to_be_bytes());
    class.extend_from_slice(&idx_class_obj.to_be_bytes());
    class.extend_from_slice(&0u16.to_be_bytes());
    class.extend_from_slice(&0u16.to_be_bytes());
    let methods_count = module.chunks.len() + needed_dispatches.len() + 1;
    class.extend_from_slice(&(methods_count as u16).to_be_bytes());

    // Emit chunk methods
    for (i, ch) in module.chunks.iter().enumerate() {
        class.extend_from_slice(&0x0009u16.to_be_bytes());
        class.extend_from_slice(&chunk_name_idx[i].to_be_bytes());
        class.extend_from_slice(&chunk_desc_idx[i].to_be_bytes());
        class.extend_from_slice(&1u16.to_be_bytes());

        class.extend_from_slice(&idx_code_utf8.to_be_bytes());
        let instrs = Decoder::new(&ch.code)
            .decode_all()
            .map_err(|e| JvmAotError::Decode(format!("{:?}", e)))?;
        let mut code: Vec<u8> = Vec::new();
        let mut ins_offsets: Vec<u32> = Vec::with_capacity(instrs.len());
        let mut branches: Vec<(usize, usize)> = Vec::new();

        for (i_idx, ins) in instrs.into_iter().enumerate() {
            ins_offsets.push(code.len() as u32);
            match ins {
                Instruction::Push(idx) => match module.constants.get(idx as usize) {
                    Some(Constant::Int(v)) => {
                        let cp_idx = *long_indices.get(v).unwrap();
                        code.push(0x14); // ldc2_w
                        code.extend_from_slice(&cp_idx.to_be_bytes());
                        code.push(0xB8); // invokestatic valueOf
                        code.extend_from_slice(&idx_valueof_mref.to_be_bytes());
                    }
                    Some(Constant::String(ref s)) => {
                        let cp_idx = *string_indices.get(s).unwrap();
                        code.push(0x13); // ldc
                        code.extend_from_slice(&cp_idx.to_be_bytes());
                    }
                    Some(_) => {
                        return Err(JvmAotError::UnsupportedOpcode(
                            "Push-unsupported".to_string(),
                        ))
                    }
                    None => return Err(JvmAotError::Decode("const out of range".to_string())),
                },
                Instruction::Pop => {
                    code.push(0x57);
                } // pop
                Instruction::Dup(n) => {
                    if n == 0 {
                        code.push(0x59); // dup
                    } else {
                        return Err(JvmAotError::UnsupportedOpcode(format!("Dup({})", n)));
                    }
                }
                Instruction::I64Add
                | Instruction::I64Sub
                | Instruction::I64Mul
                | Instruction::I64DivS
                | Instruction::I64RemS => {
                    code.push(0x5F); // swap -> B A
                    code.push(0xC0);
                    code.extend_from_slice(&idx_cls_long.to_be_bytes());
                    code.push(0xB6);
                    code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
                    // Stack: B long(A)
                    code.push(0x5E); // dup2_x1 -> long(A) B long(A)
                    code.push(0x58); // pop2 -> long(A) B
                    code.push(0xC0);
                    code.extend_from_slice(&idx_cls_long.to_be_bytes());
                    code.push(0xB6);
                    code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
                    // Stack: long(A) long(B)

                    let op = match ins {
                        Instruction::I64Add => 0x61,
                        Instruction::I64Sub => 0x65,
                        Instruction::I64Mul => 0x69,
                        Instruction::I64DivS => 0x6D,
                        Instruction::I64RemS => 0x71,
                        _ => unreachable!(),
                    };
                    code.push(op);
                    code.push(0xB8);
                    code.extend_from_slice(&idx_valueof_mref.to_be_bytes());
                }
                Instruction::StringConst(s) => {
                    let cp_idx = *string_indices.get(&s).unwrap();
                    code.push(0x13);
                    code.extend_from_slice(&cp_idx.to_be_bytes());
                }
                Instruction::StringConcat => {
                    code.push(0xB6);
                    code.extend_from_slice(&idx_concat_mref.to_be_bytes());
                }
                Instruction::LoadLocal(i) => {
                    if i <= 3 {
                        code.push(0x2A + i); // aload_0..3
                    } else {
                        code.push(0x19); // aload
                        code.push(i);
                    }
                }
                Instruction::StoreLocal(i) => {
                    if i <= 3 {
                        code.push(0x4B + i); // astore_0..3
                    } else {
                        code.push(0x3A); // astore
                        code.push(i);
                    }
                }
                Instruction::Jump(off) => {
                    let target = (i_idx as isize + off as isize) as isize;
                    code.push(0xA7);
                    let pos = code.len();
                    code.extend_from_slice(&0i16.to_be_bytes());
                    branches.push((pos, target as usize));
                }
                Instruction::JumpIfNull(off) => {
                    let target = (i_idx as isize + off as isize) as isize;
                    code.push(0x59); // dup
                    code.push(0xC7); // ifnonnull L_not_null
                    let pos_not_null = code.len();
                    code.extend_from_slice(&0i16.to_be_bytes());

                    code.push(0x57); // pop
                    code.push(0xA7); // goto target
                    let pos_jump = code.len();
                    code.extend_from_slice(&0i16.to_be_bytes());
                    branches.push((pos_jump, target as usize));

                    // L_not_null:
                    let pos_after = code.len();
                    let rel = (pos_after as i32) - ((pos_not_null as i32) - 1);
                    let b = (rel as i16).to_be_bytes();
                    code[pos_not_null] = b[0];
                    code[pos_not_null + 1] = b[1];

                    code.push(0x57); // pop
                }
                Instruction::JumpIfFalse(off) => {
                    let target = (i_idx as isize + off as isize) as isize;

                    // 1. dup
                    code.push(0x59);

                    // 2. ifnull L_pop_and_jump
                    code.push(0xC6);
                    let pos_ifnull = code.len();
                    code.extend_from_slice(&0i16.to_be_bytes());

                    // 3. dup
                    code.push(0x59);

                    // 4. instanceof Boolean
                    code.push(0xC1);
                    code.extend_from_slice(&idx_cls_bool.to_be_bytes());

                    // 5. ifeq L_pop_and_continue (Not boolean -> truthy)
                    code.push(0x99);
                    let pos_ifeq_not_bool = code.len();
                    code.extend_from_slice(&0i16.to_be_bytes());

                    // 6. checkcast Boolean (Is boolean)
                    code.push(0xC0);
                    code.extend_from_slice(&idx_cls_bool.to_be_bytes());

                    // 7. booleanValue
                    code.push(0xB6);
                    code.extend_from_slice(&idx_bool_boolvalue_mref.to_be_bytes());

                    // 8. ifeq target (False -> jump)
                    code.push(0x99);
                    let pos_ifeq_false = code.len();
                    code.extend_from_slice(&0i16.to_be_bytes());
                    branches.push((pos_ifeq_false, target as usize));

                    // 9. goto end (True -> continue)
                    code.push(0xA7);
                    let pos_goto_end = code.len();
                    code.extend_from_slice(&0i16.to_be_bytes());

                    // L_pop_and_jump:
                    let pos_pop_and_jump = code.len();
                    // Fixup ifnull
                    let rel = (pos_pop_and_jump as i32) - ((pos_ifnull as i32) - 1);
                    let b = (rel as i16).to_be_bytes();
                    code[pos_ifnull] = b[0];
                    code[pos_ifnull + 1] = b[1];

                    code.push(0x57); // pop
                    code.push(0xA7); // goto target
                    let pos_jump_null = code.len();
                    code.extend_from_slice(&0i16.to_be_bytes());
                    branches.push((pos_jump_null, target as usize));

                    // L_pop_and_continue:
                    let pos_pop_and_continue = code.len();
                    // Fixup ifeq_not_bool
                    let rel = (pos_pop_and_continue as i32) - ((pos_ifeq_not_bool as i32) - 1);
                    let b = (rel as i16).to_be_bytes();
                    code[pos_ifeq_not_bool] = b[0];
                    code[pos_ifeq_not_bool + 1] = b[1];

                    code.push(0x57); // pop

                    // L_end:
                    let pos_end = code.len();
                    // Fixup goto_end
                    let rel = (pos_end as i32) - ((pos_goto_end as i32) - 1);
                    let b = (rel as i16).to_be_bytes();
                    code[pos_goto_end] = b[0];
                    code[pos_goto_end + 1] = b[1];
                }
                Instruction::NewObject(class_idx) => {
                    let cls = module
                        .classes
                        .get(class_idx as usize)
                        .ok_or(JvmAotError::Decode("class out of range".to_string()))?;
                    let size = cls.fields.len() + 1;
                    code.push(0x10);
                    code.push(size as u8);
                    code.push(0xBD);
                    code.extend_from_slice(&idx_class_obj.to_be_bytes());
                    code.push(0x59);
                    code.push(0x03);
                    let cp_idx = *long_indices
                        .entry(class_idx as i64)
                        .or_insert_with(|| cp_long(&mut cp, class_idx as i64, &mut cp_count));
                    code.push(0x14);
                    code.extend_from_slice(&cp_idx.to_be_bytes());
                    code.push(0xB8);
                    code.extend_from_slice(&idx_valueof_mref.to_be_bytes());
                    code.push(0x53);
                }
                Instruction::GetField(name_idx) => {
                    let field_name = module
                        .constants
                        .get(name_idx as usize)
                        .and_then(|c| {
                            if let Constant::String(s) = c {
                                Some(s.as_str())
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| {
                            JvmAotError::Decode(format!(
                                "GetField with non-string field name at constant {}",
                                name_idx
                            ))
                        })?;
                    let map = field_impls.get(field_name).ok_or_else(|| {
                        JvmAotError::UnsupportedOpcode(format!(
                            "GetField: field {} has no layout",
                            field_name
                        ))
                    })?;
                    let tmp_arr_local: u8 = (ch.locals + 1) as u8;

                    // Store object into tmp_arr_local
                    code.push(0x3A);
                    code.push(tmp_arr_local);

                    // Cast to Object[] and store back
                    code.push(0x19);
                    code.push(tmp_arr_local);
                    code.push(0xC0);
                    code.extend_from_slice(&idx_class_obj_arr.to_be_bytes());
                    code.push(0x3A);
                    code.push(tmp_arr_local);

                    // Load class index from arr[0] and convert to int for switch
                    code.push(0x19);
                    code.push(tmp_arr_local);
                    code.push(0x03);
                    code.push(0x32);
                    code.push(0xC0);
                    code.extend_from_slice(&idx_cls_long.to_be_bytes());
                    code.push(0xB6);
                    code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
                    code.push(0x88); // l2i

                    code.push(0xAB);

                    let pad = (4 - (code.len() % 4)) % 4;
                    for _ in 0..pad {
                        code.push(0);
                    }
                    let switch_opcode_idx = code.len() - 1 - pad;

                    let mut sorted_keys: Vec<u16> = map.keys().cloned().collect();
                    sorted_keys.sort();

                    let default_off_pos = code.len();
                    code.extend_from_slice(&0u32.to_be_bytes());

                    let npairs = sorted_keys.len() as u32;
                    code.extend_from_slice(&npairs.to_be_bytes());

                    let mut case_offsets: Vec<(usize, u8)> = Vec::new();
                    for k in &sorted_keys {
                        code.extend_from_slice(&(*k as i32).to_be_bytes());
                        let off_pos = code.len();
                        code.extend_from_slice(&0u32.to_be_bytes());
                        let slot = 1 + *map.get(k).unwrap() as u8;
                        case_offsets.push((off_pos, slot));
                    }

                    let mut end_gotos: Vec<usize> = Vec::new();

                    // Default: return null
                    let default_target = code.len() as u32;
                    code.push(0x01);
                    code.push(0xA7);
                    let def_goto_pos = code.len();
                    code.extend_from_slice(&0i16.to_be_bytes());
                    end_gotos.push(def_goto_pos);

                    let def_rel = (default_target as i32) - (switch_opcode_idx as i32);
                    let def_bytes = def_rel.to_be_bytes();
                    code[default_off_pos] = def_bytes[0];
                    code[default_off_pos + 1] = def_bytes[1];
                    code[default_off_pos + 2] = def_bytes[2];
                    code[default_off_pos + 3] = def_bytes[3];

                    // Cases: load from per-class slot
                    for (off_pos, slot) in case_offsets {
                        let target = code.len() as u32;
                        let rel = (target as i32) - (switch_opcode_idx as i32);
                        let b = rel.to_be_bytes();
                        code[off_pos] = b[0];
                        code[off_pos + 1] = b[1];
                        code[off_pos + 2] = b[2];
                        code[off_pos + 3] = b[3];

                        code.push(0x19);
                        code.push(tmp_arr_local);
                        code.push(0x10);
                        code.push(slot);
                        code.push(0x32);

                        code.push(0xA7);
                        let gpos = code.len();
                        code.extend_from_slice(&0i16.to_be_bytes());
                        end_gotos.push(gpos);
                    }

                    let end_pos = code.len() as i32;
                    for gpos in end_gotos {
                        let opcode_addr = (gpos as i32) - 1;
                        let rel = (end_pos - opcode_addr) as i16;
                        let bytes = rel.to_be_bytes();
                        code[gpos] = bytes[0];
                        code[gpos + 1] = bytes[1];
                    }
                }
                Instruction::SetField(name_idx) => {
                    let field_name = module
                        .constants
                        .get(name_idx as usize)
                        .and_then(|c| {
                            if let Constant::String(s) = c {
                                Some(s.as_str())
                            } else {
                                None
                            }
                        })
                        .ok_or_else(|| {
                            JvmAotError::Decode(format!(
                                "SetField with non-string field name at constant {}",
                                name_idx
                            ))
                        })?;
                    let map = field_impls.get(field_name).ok_or_else(|| {
                        JvmAotError::UnsupportedOpcode(format!(
                            "SetField: field {} has no layout",
                            field_name
                        ))
                    })?;
                    let tmp_arr_local: u8 = (ch.locals + 1) as u8;
                    let tmp_val_local: u8 = (ch.locals + 2) as u8;

                    // Store value and object into locals
                    code.push(0x3A);
                    code.push(tmp_val_local);
                    code.push(0x3A);
                    code.push(tmp_arr_local);

                    // Cast object to Object[] and store back
                    code.push(0x19);
                    code.push(tmp_arr_local);
                    code.push(0xC0);
                    code.extend_from_slice(&idx_class_obj_arr.to_be_bytes());
                    code.push(0x3A);
                    code.push(tmp_arr_local);

                    // Load class index from arr[0] and convert to int
                    code.push(0x19);
                    code.push(tmp_arr_local);
                    code.push(0x03);
                    code.push(0x32);
                    code.push(0xC0);
                    code.extend_from_slice(&idx_cls_long.to_be_bytes());
                    code.push(0xB6);
                    code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
                    code.push(0x88); // l2i

                    code.push(0xAB);

                    let pad = (4 - (code.len() % 4)) % 4;
                    for _ in 0..pad {
                        code.push(0);
                    }
                    let switch_opcode_idx = code.len() - 1 - pad;

                    let mut sorted_keys: Vec<u16> = map.keys().cloned().collect();
                    sorted_keys.sort();

                    let default_off_pos = code.len();
                    code.extend_from_slice(&0u32.to_be_bytes());

                    let npairs = sorted_keys.len() as u32;
                    code.extend_from_slice(&npairs.to_be_bytes());

                    let mut case_offsets: Vec<(usize, u8)> = Vec::new();
                    for k in &sorted_keys {
                        code.extend_from_slice(&(*k as i32).to_be_bytes());
                        let off_pos = code.len();
                        code.extend_from_slice(&0u32.to_be_bytes());
                        let slot = 1 + *map.get(k).unwrap() as u8;
                        case_offsets.push((off_pos, slot));
                    }

                    let mut end_gotos: Vec<usize> = Vec::new();

                    // Default: leave value unchanged
                    let default_target = code.len() as u32;
                    code.push(0x19);
                    code.push(tmp_val_local);
                    code.push(0xA7);
                    let def_goto_pos = code.len();
                    code.extend_from_slice(&0i16.to_be_bytes());
                    end_gotos.push(def_goto_pos);

                    let def_rel = (default_target as i32) - (switch_opcode_idx as i32);
                    let def_bytes = def_rel.to_be_bytes();
                    code[default_off_pos] = def_bytes[0];
                    code[default_off_pos + 1] = def_bytes[1];
                    code[default_off_pos + 2] = def_bytes[2];
                    code[default_off_pos + 3] = def_bytes[3];

                    // Cases: store into per-class slot, then return value
                    for (off_pos, slot) in case_offsets {
                        let target = code.len() as u32;
                        let rel = (target as i32) - (switch_opcode_idx as i32);
                        let b = rel.to_be_bytes();
                        code[off_pos] = b[0];
                        code[off_pos + 1] = b[1];
                        code[off_pos + 2] = b[2];
                        code[off_pos + 3] = b[3];

                        code.push(0x19);
                        code.push(tmp_arr_local);
                        code.push(0x10);
                        code.push(slot);
                        code.push(0x19);
                        code.push(tmp_val_local);
                        code.push(0x53);

                        code.push(0x19);
                        code.push(tmp_val_local);
                        code.push(0xA7);
                        let gpos = code.len();
                        code.extend_from_slice(&0i16.to_be_bytes());
                        end_gotos.push(gpos);
                    }

                    let end_pos = code.len() as i32;
                    for gpos in end_gotos {
                        let opcode_addr = (gpos as i32) - 1;
                        let rel = (end_pos - opcode_addr) as i16;
                        let bytes = rel.to_be_bytes();
                        code[gpos] = bytes[0];
                        code[gpos + 1] = bytes[1];
                    }
                }
                Instruction::FFICall(desc, argc) => {
                    let name = module
                        .constants
                        .get(desc as usize)
                        .and_then(|c| {
                            if let Constant::String(s) = c {
                                Some(s.as_str())
                            } else {
                                None
                            }
                        })
                        .ok_or(JvmAotError::Decode("ffi name".to_string()))?;
                    match (name, argc) {
                        ("add", 2) => {
                            code.push(0xC0);
                            code.extend_from_slice(&idx_cls_long.to_be_bytes());
                            code.push(0xB6);
                            code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
                            code.push(0x5F);
                            code.push(0xC0);
                            code.extend_from_slice(&idx_cls_long.to_be_bytes());
                            code.push(0xB6);
                            code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
                            code.push(0x5F);
                            code.push(0x61); // ladd
                            code.push(0xB8);
                            code.extend_from_slice(&idx_valueof_mref.to_be_bytes());
                        }
                        ("print", 1) => {
                            code.push(0xB2);
                            code.extend_from_slice(&idx_out_fref.to_be_bytes());
                            code.push(0x5F);
                            code.push(0xB6);
                            code.extend_from_slice(&idx_println_mref.to_be_bytes());
                            code.push(0x01);
                        }
                        ("read_file", 1) => {
                            code.push(0xC0);
                            code.extend_from_slice(&idx_cls_str.to_be_bytes());
                            code.push(0x03);
                            code.push(0xBD);
                            code.extend_from_slice(&idx_cls_str.to_be_bytes());
                            code.push(0xB8);
                            code.extend_from_slice(&idx_paths_get_mref.to_be_bytes());
                            code.push(0xB8);
                            code.extend_from_slice(&idx_files_read_mref.to_be_bytes());

                            // Stack: [B
                            // new S -> [B, S
                            code.push(0xBB); // new String
                            code.extend_from_slice(&idx_cls_str.to_be_bytes());

                            // dup_x1 -> S, [B, S
                            code.push(0x5A); // dup_x1

                            // swap -> S, S, [B
                            code.push(0x5F); // swap

                            code.push(0x13); // ldc "UTF-8"
                            code.extend_from_slice(&idx_utf8_str.to_be_bytes());

                            code.push(0xB7); // invokespecial
                            code.extend_from_slice(&idx_init_mref.to_be_bytes());
                        }
                        ("write_file", 2) => {
                            code.push(0xC0);
                            code.extend_from_slice(&idx_cls_str.to_be_bytes());
                            code.push(0x13);
                            code.extend_from_slice(&idx_utf8_str.to_be_bytes());
                            code.push(0xB6);
                            code.extend_from_slice(&idx_getbytes_mref.to_be_bytes());
                            code.push(0x5F);
                            code.push(0xC0);
                            code.extend_from_slice(&idx_cls_str.to_be_bytes());
                            code.push(0x03);
                            code.push(0xBD);
                            code.extend_from_slice(&idx_cls_str.to_be_bytes());
                            code.push(0xB8);
                            code.extend_from_slice(&idx_paths_get_mref.to_be_bytes());
                            code.push(0x5F);
                            code.push(0x03);
                            code.push(0xBD);
                            code.extend_from_slice(&idx_cls_openopt.to_be_bytes());
                            code.push(0xB8);
                            code.extend_from_slice(&idx_files_write_mref.to_be_bytes());
                            code.push(0x57);
                            code.push(0x01);
                        }
                        ("len", 1) => {
                            let mref = dispatch_mref_map.get(&("len".to_string(), 0)).ok_or_else(
                                || {
                                    JvmAotError::UnsupportedOpcode(
                                        "len dispatch not found".to_string(),
                                    )
                                },
                            )?;
                            code.push(0x01); // null closure
                            code.push(0xB8);
                            code.extend_from_slice(&mref.to_be_bytes());
                        }
                        ("eq", 2) => {
                            // code.push(0x59); code.push(0xB2); code.extend_from_slice(&idx_out_fref.to_be_bytes()); code.push(0x5F); code.push(0xB6); code.extend_from_slice(&idx_println_mref.to_be_bytes());
                            code.push(0xB6);
                            code.extend_from_slice(&idx_equals_mref.to_be_bytes());
                            code.push(0xB8);
                            code.extend_from_slice(&idx_bool_valueof_mref.to_be_bytes());
                        }
                        ("chars", 1) => {
                            code.push(0xC0);
                            code.extend_from_slice(&idx_cls_str.to_be_bytes());
                        }
                        ("true", 0) => {
                            code.push(0x04);
                            code.push(0xB8);
                            code.extend_from_slice(&idx_bool_valueof_mref.to_be_bytes());
                        }
                        ("false", 0) => {
                            code.push(0x03);
                            code.push(0xB8);
                            code.extend_from_slice(&idx_bool_valueof_mref.to_be_bytes());
                        }
                        ("get", 2) => {
                            // code.push(0x5F); // swap - removed to handle pos, src order
                            code.push(0xC0);
                            code.extend_from_slice(&idx_cls_str.to_be_bytes());
                            code.push(0x5F); // swap
                            code.push(0xC0);
                            code.extend_from_slice(&idx_cls_long.to_be_bytes());
                            code.push(0xB6);
                            code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
                            code.push(0x88); // l2i
                            code.push(0x59); // dup
                            code.push(0x04); // iconst_1
                            code.push(0x60); // iadd
                            code.push(0xB6);
                            code.extend_from_slice(&idx_substring_mref.to_be_bytes());
                        }
                        ("ord", 1) => {
                            code.push(0xC0);
                            code.extend_from_slice(&idx_cls_str.to_be_bytes());
                            code.push(0x03); // iconst_0
                            code.push(0xB6);
                            code.extend_from_slice(&idx_charat_mref.to_be_bytes());
                            code.push(0x85); // i2l
                            code.push(0xB8);
                            code.extend_from_slice(&idx_valueof_mref.to_be_bytes());
                        }
                        _ => {
                            return Err(JvmAotError::UnsupportedOpcode(format!(
                                "FFICall({},{})",
                                name, argc
                            )))
                        }
                    }
                }
                Instruction::Call(target_idx, argc) => {
                    let callee = module
                        .chunks
                        .get(target_idx as usize)
                        .ok_or(JvmAotError::Decode("callee out of range".to_string()))?;
                    let need_pad = if (argc as u16) >= callee.locals {
                        0
                    } else {
                        (callee.locals - argc as u16) as usize
                    };
                    for _ in 0..need_pad {
                        code.push(0x01);
                    } // push null
                    code.push(0x01); // push null closure
                    code.push(0xB8);
                    let mr = chunk_mref_idx[target_idx as usize];
                    code.extend_from_slice(&mr.to_be_bytes());
                }
                Instruction::InvokeMethod(name_idx, argc) => {
                    let name = module
                        .constants
                        .get(name_idx as usize)
                        .and_then(|c| {
                            if let Constant::String(s) = c {
                                Some(s.as_str())
                            } else {
                                None
                            }
                        })
                        .ok_or(JvmAotError::Decode("method name".to_string()))?;
                    let mref = dispatch_mref_map
                        .get(&(name.to_string(), argc))
                        .ok_or_else(|| {
                            JvmAotError::UnsupportedOpcode(format!(
                                "InvokeMethod: {} not found",
                                name
                            ))
                        })?;

                    code.push(0x01); // push null closure (extra arg)
                    code.push(0xB8);
                    code.extend_from_slice(&mref.to_be_bytes());
                }
                Instruction::Halt => {
                    code.push(0x03); // iconst_0
                    code.push(0xB8); // invokestatic
                    code.extend_from_slice(&idx_exit_mref.to_be_bytes());
                    code.push(0x01); // aconst_null
                    code.push(0xB0); // areturn
                }
                Instruction::Return => {
                    code.push(0xB0);
                } // areturn
                _ => return Err(JvmAotError::UnsupportedOpcode(format!("{:?}", ins))),
            }
        }

        for (pos, target_idx) in branches.iter().copied() {
            if target_idx >= ins_offsets.len() {
                return Err(JvmAotError::Decode("branch out of range".to_string()));
            }
            let target_off = ins_offsets[target_idx] as i32;
            let opcode_addr = (pos as i32) - 1;
            let rel = (target_off - opcode_addr) as i16;
            let bytes = rel.to_be_bytes();
            code[pos] = bytes[0];
            code[pos + 1] = bytes[1];
        }

        let code_len = code.len() as u32;
        let max_stack = 128;
        let max_locals = ch.locals * 2 + 2;
        let attr_len = 12 + code_len;
        class.extend_from_slice(&attr_len.to_be_bytes());
        class.extend_from_slice(&(max_stack as u16).to_be_bytes());
        class.extend_from_slice(&(max_locals as u16).to_be_bytes());
        class.extend_from_slice(&code_len.to_be_bytes());
        class.extend_from_slice(&code);
        class.extend_from_slice(&0u16.to_be_bytes());
        class.extend_from_slice(&0u16.to_be_bytes());
    }

    // Emit dispatch methods
    for (name, argc) in &sorted_dispatches {
        let (nidx, didx) = *dispatch_defs.get(&(name.clone(), *argc)).unwrap();

        class.extend_from_slice(&0x0009u16.to_be_bytes());
        class.extend_from_slice(&nidx.to_be_bytes());
        class.extend_from_slice(&didx.to_be_bytes());
        class.extend_from_slice(&1u16.to_be_bytes());
        class.extend_from_slice(&idx_code_utf8.to_be_bytes());

        // Dispatch body
        let mut code: Vec<u8> = Vec::new();

        // Check if array (struct)
        code.push(0x2A); // aload_0
        code.push(0xC1); // instanceof [LObject;
        code.extend_from_slice(&idx_class_obj_arr.to_be_bytes());
        code.push(0x99); // ifeq HandlePrimitive
        let pos_primitive = code.len();
        code.extend_from_slice(&0u16.to_be_bytes());

        // Handle Array
        code.push(0x2A); // aload_0
        code.push(0xC0); // checkcast [LObject;
        code.extend_from_slice(&idx_class_obj_arr.to_be_bytes());
        code.push(0x03); // iconst_0
        code.push(0x32); // aaload
        code.push(0xC0);
        code.extend_from_slice(&idx_cls_long.to_be_bytes());
        code.push(0xB6);
        code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
        code.push(0x88); // l2i

        code.push(0xAB); // lookupswitch

        // Padding
        let pad = (4 - (code.len() % 4)) % 4;
        for _ in 0..pad {
            code.push(0);
        }
        let switch_opcode_idx = code.len() - 1 - pad;

        let empty_map = HashMap::new();
        let map = method_impls.get(name).unwrap_or(&empty_map);
        let mut sorted_keys: Vec<u16> = map.keys().cloned().collect();
        sorted_keys.sort();

        let default_off_pos = code.len();
        code.extend_from_slice(&0u32.to_be_bytes());

        let npairs = sorted_keys.len() as u32;
        code.extend_from_slice(&npairs.to_be_bytes());

        let mut case_offsets: Vec<(usize, u16)> = Vec::new();
        for k in &sorted_keys {
            code.extend_from_slice(&(*k as i32).to_be_bytes());
            let off_pos = code.len();
            code.extend_from_slice(&0u32.to_be_bytes());
            case_offsets.push((off_pos, *map.get(k).unwrap()));
        }

        // Default target (for array but no method found) -> return null
        let default_target = code.len() as u32;
        code.push(0x01); // aconst_null
        code.push(0xB0); // areturn

        // Fixup default offset
        let def_rel = (default_target as i32) - (switch_opcode_idx as i32);
        let def_bytes = def_rel.to_be_bytes();
        code[default_off_pos] = def_bytes[0];
        code[default_off_pos + 1] = def_bytes[1];
        code[default_off_pos + 2] = def_bytes[2];
        code[default_off_pos + 3] = def_bytes[3];

        // Generate cases
        for (off_pos, chunk_idx) in case_offsets {
            let target = code.len() as u32;
            let rel = (target as i32) - (switch_opcode_idx as i32);
            let b = rel.to_be_bytes();
            code[off_pos] = b[0];
            code[off_pos + 1] = b[1];
            code[off_pos + 2] = b[2];
            code[off_pos + 3] = b[3];

            let chunk = &module.chunks[chunk_idx as usize];
            let provided = 1 + *argc as u16;
            let need_pad = if provided >= chunk.locals {
                0
            } else {
                (chunk.locals - provided) as usize
            };

            code.push(0x2A); // receiver
            for i in 0..*argc {
                let local_idx = i + 1;
                if local_idx <= 3 {
                    code.push(0x2A + local_idx);
                } else {
                    code.push(0x19);
                    code.push(local_idx);
                }
            }
            for _ in 0..need_pad {
                code.push(0x01);
            } // padding

            code.push(0x01); // null closure
            code.push(0xB8);
            let mr = chunk_mref_idx[chunk_idx as usize];
            code.extend_from_slice(&mr.to_be_bytes());
            code.push(0xB0); // areturn
        }

        // Handle Primitive
        let pos_prim_start = code.len();
        let rel = (pos_prim_start as isize - (pos_primitive as isize - 1)) as i16;
        let bytes = rel.to_be_bytes();
        code[pos_primitive] = bytes[0];
        code[pos_primitive + 1] = bytes[1];

        if name == "len" && *argc == 0 {
            code.push(0x2A);
            code.push(0xC1);
            code.extend_from_slice(&idx_cls_str.to_be_bytes());
            code.push(0x99);
            let p = code.len();
            code.extend_from_slice(&0u16.to_be_bytes());
            code.push(0x2A);
            code.push(0xC0);
            code.extend_from_slice(&idx_cls_str.to_be_bytes());
            code.push(0xB6);
            code.extend_from_slice(&idx_length_mref.to_be_bytes());
            code.push(0x85);
            code.push(0xB8);
            code.extend_from_slice(&idx_valueof_mref.to_be_bytes());
            code.push(0xB0);
            let t = code.len();
            let r = (t as isize - (p as isize - 1)) as i16;
            code[p] = r.to_be_bytes()[0];
            code[p + 1] = r.to_be_bytes()[1];

            if let Some(map) = field_impls.get("len") {
                let tmp_arr_local = (*argc + 2) as u8;
                code.push(0x2A);
                code.push(0x3A);
                code.push(tmp_arr_local);
                code.push(0x19);
                code.push(tmp_arr_local);
                code.push(0xC0);
                code.extend_from_slice(&idx_class_obj_arr.to_be_bytes());
                code.push(0x3A);
                code.push(tmp_arr_local);
                code.push(0x19);
                code.push(tmp_arr_local);
                code.push(0x03);
                code.push(0x32);
                code.push(0xC0);
                code.extend_from_slice(&idx_cls_long.to_be_bytes());
                code.push(0xB6);
                code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
                code.push(0x88);
                code.push(0xAB);
                let pad = (4 - (code.len() % 4)) % 4;
                for _ in 0..pad {
                    code.push(0);
                }
                let switch_opcode_idx = code.len() - 1 - pad;
                let mut sorted_keys: Vec<u16> = map.keys().cloned().collect();
                sorted_keys.sort();
                let default_off_pos = code.len();
                code.extend_from_slice(&0u32.to_be_bytes());
                let npairs = sorted_keys.len() as u32;
                code.extend_from_slice(&npairs.to_be_bytes());
                let mut case_offsets: Vec<(usize, u8)> = Vec::new();
                for k in &sorted_keys {
                    code.extend_from_slice(&(*k as i32).to_be_bytes());
                    let off_pos = code.len();
                    code.extend_from_slice(&0u32.to_be_bytes());
                    let slot = 1 + *map.get(k).unwrap() as u8;
                    case_offsets.push((off_pos, slot));
                }
                let default_target = code.len() as u32;
                code.push(0x01);
                code.push(0xB0);
                let def_rel = (default_target as i32) - (switch_opcode_idx as i32);
                let def_bytes = def_rel.to_be_bytes();
                code[default_off_pos] = def_bytes[0];
                code[default_off_pos + 1] = def_bytes[1];
                code[default_off_pos + 2] = def_bytes[2];
                code[default_off_pos + 3] = def_bytes[3];
                for (off_pos, slot) in case_offsets {
                    let target = code.len() as u32;
                    let rel = (target as i32) - (switch_opcode_idx as i32);
                    let b = rel.to_be_bytes();
                    code[off_pos] = b[0];
                    code[off_pos + 1] = b[1];
                    code[off_pos + 2] = b[2];
                    code[off_pos + 3] = b[3];
                    code.push(0x19);
                    code.push(tmp_arr_local);
                    code.push(0x10);
                    code.push(slot);
                    code.push(0x32);
                    code.push(0xB0);
                }
            } else {
                code.push(0x01);
                code.push(0xB0);
            }
        } else if name == "eq" && *argc == 1 {
            code.push(0x2A);
            code.push(0x2B);
            code.push(0xB6);
            code.extend_from_slice(&idx_equals_mref.to_be_bytes());
            code.push(0xB8);
            code.extend_from_slice(&idx_bool_valueof_mref.to_be_bytes());
            code.push(0xB0);
        } else if (name == "lt" || name == "gt" || name == "le" || name == "ge") && *argc == 1 {
            code.push(0x2A);
            code.push(0xC0);
            code.extend_from_slice(&idx_cls_long.to_be_bytes());
            code.push(0xB6);
            code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
            code.push(0x2B);
            code.push(0xC0);
            code.extend_from_slice(&idx_cls_long.to_be_bytes());
            code.push(0xB6);
            code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
            code.push(0x94); // lcmp

            // ifop
            let op = match name.as_str() {
                "lt" => 0x9B, // iflt <
                "gt" => 0x9D, // ifgt >
                "le" => 0x9E, // ifle <=
                "ge" => 0x9C, // ifge >=
                _ => 0,
            };
            code.push(op);
            let pos_succ = code.len();
            code.extend_from_slice(&0u16.to_be_bytes());
            code.push(0x03); // iconst_0
            code.push(0xA7); // goto end
            let pos_end = code.len();
            code.extend_from_slice(&0u16.to_be_bytes());

            let pos_true = code.len();
            code.push(0x04); // iconst_1

            let pos_after = code.len();
            let rel = (pos_true as isize - (pos_succ as isize - 1)) as i16;
            code[pos_succ] = rel.to_be_bytes()[0];
            code[pos_succ + 1] = rel.to_be_bytes()[1];
            let rel = (pos_after as isize - (pos_end as isize - 1)) as i16;
            code[pos_end] = rel.to_be_bytes()[0];
            code[pos_end + 1] = rel.to_be_bytes()[1];

            code.push(0xB8);
            code.extend_from_slice(&idx_bool_valueof_mref.to_be_bytes());
            code.push(0xB0);
        } else if name == "to_string" && *argc == 0 {
            code.push(0x2A);
            code.push(0xB6); // invokevirtual Object.toString()
            code.extend_from_slice(&idx_tostring_mref.to_be_bytes());
            code.push(0xB0);
        } else if name == "get" && *argc == 2 {
            // get(i, name) -> name.substring(i, i+1)
            code.push(0x2C); // aload_2 (name)
            code.push(0xC0);
            code.extend_from_slice(&idx_cls_str.to_be_bytes());

            code.push(0x2B); // aload_1 (i)
            code.push(0xC0);
            code.extend_from_slice(&idx_cls_long.to_be_bytes());
            code.push(0xB6);
            code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
            code.push(0x88); // l2i
            code.push(0x59); // dup (i)

            code.push(0x04); // iconst_1
            code.push(0x60); // iadd (i+1)

            code.push(0xB6); // invokevirtual substring
            code.extend_from_slice(&idx_substring_mref.to_be_bytes());
            code.push(0xB0); // areturn
        } else if name == "get" && *argc == 1 {
            // get(i) on String -> substring(i, i+1)
            code.push(0x2A); // aload_0 (String)
            code.push(0xC0);
            code.extend_from_slice(&idx_cls_str.to_be_bytes());

            code.push(0x2B); // aload_1 (Index)
            code.push(0xC0);
            code.extend_from_slice(&idx_cls_long.to_be_bytes());
            code.push(0xB6);
            code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
            code.push(0x88); // l2i

            code.push(0x59); // dup
            code.push(0x04); // 1
            code.push(0x60); // add

            code.push(0xB6);
            code.extend_from_slice(&idx_substring_mref.to_be_bytes());
            code.push(0xB0); // areturn
        } else if name == "add" && *argc == 1 {
            // Check String
            code.push(0x2A);
            code.push(0xC1);
            code.extend_from_slice(&idx_cls_str.to_be_bytes());
            code.push(0x99);
            let p_not_str = code.len();
            code.extend_from_slice(&0u16.to_be_bytes());

            // String concat
            code.push(0x2A);
            code.push(0xC0);
            code.extend_from_slice(&idx_cls_str.to_be_bytes());
            code.push(0x2B);
            code.push(0xC0);
            code.extend_from_slice(&idx_cls_str.to_be_bytes());
            code.push(0xB6);
            code.extend_from_slice(&idx_concat_mref.to_be_bytes());
            code.push(0xB0);

            let t = code.len();
            let r = (t as isize - (p_not_str as isize - 1)) as i16;
            code[p_not_str] = r.to_be_bytes()[0];
            code[p_not_str + 1] = r.to_be_bytes()[1];

            // Check Long
            code.push(0x2A);
            code.push(0xC1);
            code.extend_from_slice(&idx_cls_long.to_be_bytes());
            code.push(0x99);
            let p_not_long = code.len();
            code.extend_from_slice(&0u16.to_be_bytes());

            // Long add
            code.push(0x2A);
            code.push(0xC0);
            code.extend_from_slice(&idx_cls_long.to_be_bytes());
            code.push(0xB6);
            code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
            code.push(0x2B);
            code.push(0xC0);
            code.extend_from_slice(&idx_cls_long.to_be_bytes());
            code.push(0xB6);
            code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
            code.push(0x61); // ladd
            code.push(0xB8);
            code.extend_from_slice(&idx_valueof_mref.to_be_bytes());
            code.push(0xB0);

            let t = code.len();
            let r = (t as isize - (p_not_long as isize - 1)) as i16;
            code[p_not_long] = r.to_be_bytes()[0];
            code[p_not_long + 1] = r.to_be_bytes()[1];
        } else if (name == "sub" || name == "mul" || name == "div" || name == "rem") && *argc == 1 {
            code.push(0x2A);
            code.push(0xC0);
            code.extend_from_slice(&idx_cls_long.to_be_bytes());
            code.push(0xB6);
            code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());
            code.push(0x2B);
            code.push(0xC0);
            code.extend_from_slice(&idx_cls_long.to_be_bytes());
            code.push(0xB6);
            code.extend_from_slice(&idx_longvalue_mref.to_be_bytes());

            let op = match name.as_str() {
                "sub" => 0x65, // lsub
                "mul" => 0x69, // lmul
                "div" => 0x6D, // ldiv
                "rem" => 0x71, // lrem
                _ => 0,
            };
            code.push(op);
            code.push(0xB8);
            code.extend_from_slice(&idx_valueof_mref.to_be_bytes());
            code.push(0xB0);
        }

        code.push(0x01); // aconst_null
        code.push(0xB0); // areturn

        let code_len = code.len() as u32;
        let attr_len = 12 + code_len;
        class.extend_from_slice(&attr_len.to_be_bytes());
        class.extend_from_slice(&(128u16).to_be_bytes()); // max_stack
        class.extend_from_slice(&((*argc as u16 + 1) * 2 + 2).to_be_bytes()); // max_locals
        class.extend_from_slice(&code_len.to_be_bytes());
        class.extend_from_slice(&code);
        class.extend_from_slice(&0u16.to_be_bytes());
        class.extend_from_slice(&0u16.to_be_bytes());
    }

    // Main stub
    {
        class.extend_from_slice(&0x0009u16.to_be_bytes());
        class.extend_from_slice(&idx_main_name.to_be_bytes());
        class.extend_from_slice(&idx_main_desc.to_be_bytes());
        class.extend_from_slice(&1u16.to_be_bytes());
        class.extend_from_slice(&idx_code_utf8.to_be_bytes());
        let mut code: Vec<u8> = Vec::new();
        let main_chunk = &module.chunks[0];
        for _ in 0..main_chunk.locals {
            code.push(0x01);
        } // aconst_null
        code.push(0x01); // aconst_null closure
        code.push(0xB8);
        let mr = chunk_mref_idx[0];
        code.extend_from_slice(&mr.to_be_bytes());
        code.push(0x57); // pop result
        code.push(0xB1); // return
        let code_len = code.len() as u32;
        let attr_len = 12 + code_len;
        class.extend_from_slice(&attr_len.to_be_bytes());
        class.extend_from_slice(&128u16.to_be_bytes());
        class.extend_from_slice(&1u16.to_be_bytes());
        class.extend_from_slice(&code_len.to_be_bytes());
        class.extend_from_slice(&code);
        class.extend_from_slice(&0u16.to_be_bytes());
        class.extend_from_slice(&0u16.to_be_bytes());
    }
    class.extend_from_slice(&0u16.to_be_bytes());
    Ok(class)
}

pub fn write_jar(path: &str, class_bytes: &[u8]) -> std::io::Result<()> {
    use std::fs;
    use std::io::Write;
    use zip::write::FileOptions;
    use zip::DateTime;
    let file = fs::File::create(path)?;
    let mut zip = zip::ZipWriter::new(file);
    let time = DateTime::from_date_and_time(1980, 1, 1, 0, 0, 0).unwrap();
    let opts = FileOptions::default().last_modified_time(time);
    zip.start_file("META-INF/MANIFEST.MF", opts)?;
    zip.write_all(b"Manifest-Version: 1.0\nMain-Class: Main\n")?;
    let opts = FileOptions::default().last_modified_time(time);
    zip.start_file("Main.class", opts)?;
    zip.write_all(class_bytes)?;
    zip.finish()?;
    Ok(())
}

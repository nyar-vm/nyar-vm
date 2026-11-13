use nyar_emitter::transitional::binary::{
    elf::{NativeElfImage, NativeElfWriter, SharedElfImage, SharedElfWriter, SharedObjectExport},
    pe::{NativePeImage, NativePeWriter},
};
use odin_bytecode::{OdinConstant, OdinFunction, OdinModuleData, Odin_VERSION};

use nyar_emitter::{
    nyar_backend_native::{NativeExecutableError, NativeExecutableKind, classify_native_executable},
    nyar_backend_vm::emit_nyar_module,
    nyar_backend_wasi::{WasmTraitFatPointer, plan_witness_table_layout, resolve_witness_call},
};

#[test]
fn emits_minimal_module_bytes() {
    let module = OdinModuleData {
        version: Odin_VERSION,
        name: "demo".to_string(),
        constants: vec![OdinConstant::Integer32(0)],
        functions: vec![OdinFunction { name: "main".to_string(), arity: 0, local_count: 0, code_offset: 0, code_length: 1 }],
        imports: Vec::new(),
        exports: Vec::new(),
        witness_entries: Vec::new(),
        code_bytes: vec![0x30],
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };
    let path = std::env::temp_dir().join(format!("nyar-backend-vm-{}.odin", std::process::id()));
    emit_nyar_module(&module, &path).expect("emit");
    assert!(path.is_file());
    let _ = std::fs::remove_file(path);
}

#[test]
fn classifies_writer_outputs() {
    let pe = NativePeWriter::write_executable(&NativePeImage {
        text: vec![0xC3],
        rdata: Vec::new(),
        idata: Vec::new(),
        imports: Vec::new(),
        entry_point: 0,
    })
    .expect("pe");
    assert_eq!(classify_native_executable(&pe).expect("pe"), NativeExecutableKind::Pe);

    let elf = NativeElfWriter::write_executable(&NativeElfImage { text: vec![0xC3], rodata: Vec::new(), entry_point: 0 }).expect("elf");
    assert_eq!(classify_native_executable(&elf).expect("elf"), NativeExecutableKind::Elf);

    let shared = SharedElfWriter::write_aarch64(
        &SharedElfImage { text: vec![0xC0, 0x03, 0x5F, 0xD6], rodata: Vec::new(), bss: Vec::new() },
        &[SharedObjectExport { name: "asgard_invoke_export".into(), text_offset: 0 }],
    )
    .expect("so");
    assert_eq!(classify_native_executable(&shared).expect("so"), NativeExecutableKind::ElfShared);
}

/// 反例：镜像可被解析但机器类型不受支持时必须返回结构化错误，不得 panic。
///
/// 修复前 `classify_native_executable` 对「解析成功、架构不支持」的镜像调用
/// `unwrap_err()`，把可诊断的拒绝变成进程 panic。
#[test]
fn parsed_but_unsupported_native_image_reports_structured_error() {
    let mut pe = NativePeWriter::write_executable(&NativePeImage {
        text: vec![0xC3],
        rdata: Vec::new(),
        idata: Vec::new(),
        imports: Vec::new(),
        entry_point: 0,
    })
    .expect("pe");
    let pe_offset = u32::from_le_bytes(pe[0x3C..0x40].try_into().expect("e_lfanew")) as usize;
    let machine_offset = pe_offset + 4;
    // 保留 PE32+ 头，只把 machine 改成 i386：parse_pe64 成功但不属于 AMD64 支持集。
    pe[machine_offset..machine_offset + 2].copy_from_slice(&0x014Cu16.to_le_bytes());
    let error = classify_native_executable(&pe).expect_err("不受支持的机器类型必须拒绝");
    assert!(
        matches!(error, NativeExecutableError::ParsedButUnsupported),
        "解析成功但架构不支持必须返回结构化错误，而不是 panic 或 Unrecognized: {error:?}"
    );
}

#[test]
fn witness_table_uses_non_intrusive_fat_pointer() {
    let layout = plan_witness_table_layout("Dog", "Animal", &["make_sound", "name"]);
    assert_eq!(layout.type_name, "Dog");
    assert_eq!(layout.trait_name, "Animal");
    assert_eq!(layout.methods.len(), 2);
    assert_eq!(resolve_witness_call(&layout, 1), Some(1));
}

#[test]
fn fat_pointer_carries_data_and_witness_separately() {
    let fat = WasmTraitFatPointer { data_ptr: 0x100, witness_ptr: 0x200 };
    assert_ne!(fat.data_ptr, fat.witness_ptr);
}

//! 原生文本程序的宿主导入 ABI：`.idata` 构造、IAT 槽与 `call [rip+disp32]` 回填。

use nyar_emitter::nyar_backend_native::{
    NativeTextInstruction, NativeTextProgramFunction, encode_native_text_program_with_imports,
};

#[test]
fn call_import_freezes_idata_and_rel32_fixups() {
    let functions = vec![NativeTextProgramFunction {
        name: "main".to_string(),
        slot_count: 0,
        instructions: vec![
            NativeTextInstruction::CallImport { module: "nyar_provider".to_string(), field: "nyar_probe".to_string() },
            NativeTextInstruction::Return,
        ],
    }];
    let imports = vec![("nyar_provider".to_string(), "nyar_probe".to_string())];
    let image = encode_native_text_program_with_imports(&functions, &imports).expect("encode program with import");

    assert_eq!(image.imports.len(), 1);
    assert_eq!(image.imports[0].dll, "nyar_provider");
    assert_eq!(image.imports[0].functions, vec!["nyar_probe".to_string()]);
    assert!(!image.idata.is_empty(), "import table must be materialized");
    assert_eq!(image.text_rel32_fixups.len(), 1, "one IAT call site");
    // fixup 指向 `call qword ptr [rip+disp32]` 的 disp32，前两字节必须是 `FF 15`。
    let field = image.text_rel32_fixups[0].0 as usize;
    assert_eq!(&image.text[field - 2..field], &[0xFF, 0x15], "import call must use the IAT slot");
}

#[test]
fn call_import_requires_a_declared_slot() {
    let functions = vec![NativeTextProgramFunction {
        name: "main".to_string(),
        slot_count: 0,
        instructions: vec![
            NativeTextInstruction::CallImport { module: "nyar_provider".to_string(), field: "missing".to_string() },
            NativeTextInstruction::Return,
        ],
    }];
    let error = encode_native_text_program_with_imports(&functions, &[]).expect_err("undeclared import must fail closed");
    assert!(error.contains("IAT"), "unexpected error: {error}");
}

#[cfg(windows)]
#[test]
fn exported_dll_resolves_a_host_import_at_load_time() {
    use acorn_pe::pe::{NativeExport, NativePeImage, NativePeWriter};

    unsafe extern "system" {
        fn LoadLibraryA(name: *const u8) -> *mut core::ffi::c_void;
        fn GetProcAddress(module: *mut core::ffi::c_void, name: *const u8) -> *mut core::ffi::c_void;
        fn FreeLibrary(module: *mut core::ffi::c_void) -> i32;
    }

    // provider 与 consumer 使用同一导出的探针：真实装载后经 IAT 调用。
    let provider = NativePeImage {
        text: vec![0x48, 0xB8, 11, 0, 0, 0, 0, 0, 0, 0, 0xC3],
        rdata: Vec::new(),
        idata: Vec::new(),
        imports: Vec::new(),
        exports: vec![NativeExport { name: "nyar_probe".to_string(), offset: 0 }],
        text_rel32_fixups: Vec::new(),
        is_dll: true,
        entry_point: 0,
    };
    let provider_bytes = NativePeWriter::write_executable(&provider).expect("write provider");

    let functions = vec![NativeTextProgramFunction {
        name: "main".to_string(),
        slot_count: 0,
        instructions: vec![
            NativeTextInstruction::CallImport { module: "nyar_provider".to_string(), field: "nyar_probe".to_string() },
            NativeTextInstruction::Return,
        ],
    }];
    let image = encode_native_text_program_with_imports(&functions, &[("nyar_provider".to_string(), "nyar_probe".to_string())])
        .expect("encode consumer");
    let consumer = NativePeImage {
        text: image.text,
        rdata: Vec::new(),
        idata: image.idata,
        imports: image.imports,
        exports: image.symbols.into_iter().map(|(name, offset)| NativeExport { name, offset }).collect(),
        text_rel32_fixups: image.text_rel32_fixups,
        is_dll: true,
        entry_point: 0,
    };
    let consumer_bytes = NativePeWriter::write_executable(&consumer).expect("write consumer");

    let dir = std::env::temp_dir().join(format!("nyar-native-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let provider_path = dir.join("nyar_provider.dll");
    let consumer_path = dir.join("nyar_consumer.dll");
    std::fs::write(&provider_path, &provider_bytes).expect("write provider");
    std::fs::write(&consumer_path, &consumer_bytes).expect("write consumer");
    let mut provider_name = provider_path.to_string_lossy().into_owned().into_bytes();
    provider_name.push(0);
    let mut consumer_name = consumer_path.to_string_lossy().into_owned().into_bytes();
    consumer_name.push(0);

    unsafe {
        let provider_module = LoadLibraryA(provider_name.as_ptr());
        assert!(!provider_module.is_null(), "provider load failed");
        let consumer_module = LoadLibraryA(consumer_name.as_ptr());
        assert!(!consumer_module.is_null(), "consumer load failed: import table must resolve");
        let symbol = GetProcAddress(consumer_module, b"main\0".as_ptr());
        assert!(!symbol.is_null(), "consumer export missing");
        let main: extern "system" fn() -> i64 = std::mem::transmute(symbol);
        assert_eq!(main(), 11, "call must reach the imported provider function");
        FreeLibrary(consumer_module);
        FreeLibrary(provider_module);
    }
    let _ = std::fs::remove_file(&consumer_path);
    let _ = std::fs::remove_file(&provider_path);
    let _ = std::fs::remove_dir(&dir);
}

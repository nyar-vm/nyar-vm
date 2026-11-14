//! 原生文本程序的宿主导入 ABI：`.idata` 构造、IAT 槽与 `call [rip+disp32]` 回填。

use nyar_emitter::nyar_backend_native::{
    NativeAbi, NativeTextInstruction, NativeTextProgramFunction, encode_native_text_program,
    encode_native_text_program_with_imports,
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
    let image = encode_native_text_program_with_imports(&functions, &imports, NativeAbi::Win64).expect("encode program with import");

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
    let error =
        encode_native_text_program_with_imports(&functions, &[], NativeAbi::Win64).expect_err("undeclared import must fail closed");
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
    let image =
        encode_native_text_program_with_imports(&functions, &[("nyar_provider".to_string(), "nyar_probe".to_string())], NativeAbi::Win64)
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

/// `add(a, b)` 形参与实参寄存器必须按 ABI 选择，而不是固定 Win64 顺序。
#[test]
fn parameter_registers_follow_the_declared_abi() {
    let functions = vec![NativeTextProgramFunction {
        name: "add".to_string(),
        slot_count: 3,
        instructions: vec![
            NativeTextInstruction::LoadParam { index: 0, slot: 0 },
            NativeTextInstruction::LoadParam { index: 1, slot: 1 },
            NativeTextInstruction::LoadSlot(0),
            NativeTextInstruction::LoadSlotToRcx(1),
            NativeTextInstruction::Add,
            NativeTextInstruction::Return,
        ],
    }];

    // SysV64：第 1/2 个形参来自 rdi/rsi。
    let sysv = encode_native_text_program(&functions, NativeAbi::SystemV64).expect("sysv program");
    assert!(
        sysv.text.windows(3).any(|window| window == [0x48, 0x89, 0xBD]),
        "SysV 第 1 形参必须存 rdi: {:02x?}",
        sysv.text
    );
    assert!(
        sysv.text.windows(3).any(|window| window == [0x48, 0x89, 0xB5]),
        "SysV 第 2 形参必须存 rsi: {:02x?}",
        sysv.text
    );

    // Win64：同一 LIR 必须落到 rcx/rdx。
    let win64 = encode_native_text_program(&functions, NativeAbi::Win64).expect("win64 program");
    assert!(
        win64.text.windows(3).any(|window| window == [0x48, 0x89, 0x8D]),
        "Win64 第 1 形参必须存 rcx: {:02x?}",
        win64.text
    );
    assert!(
        win64.text.windows(3).any(|window| window == [0x48, 0x89, 0x95]),
        "Win64 第 2 形参必须存 rdx: {:02x?}",
        win64.text
    );
}

/// SysV64 有 6 个寄存器实参；Win64 只有 4 个，第 5 个必须失败闭合。
#[test]
fn integer_argument_limit_depends_on_abi() {
    let fifth_argument = || NativeTextProgramFunction {
        name: "takes5".to_string(),
        slot_count: 5,
        instructions: vec![
            NativeTextInstruction::LoadSlotToArg { slot: 0, index: 4 },
            NativeTextInstruction::LoadI64(0),
            NativeTextInstruction::Return,
        ],
    };
    encode_native_text_program(&[fifth_argument()], NativeAbi::SystemV64).expect("SysV64 支持第 5 个寄存器实参");
    let error = encode_native_text_program(&[fifth_argument()], NativeAbi::Win64)
        .expect_err("Win64 第 5 个实参必须走栈，本 lane 尚未实现");
    assert!(error.contains("Win64"), "unexpected error: {error}");
}

/// `.idata`/IAT 是 Win64 `PE` 合同：ELF 目标上的导入必须显式失败。
#[test]
fn host_imports_are_rejected_on_the_system_v_lane() {
    let functions = vec![NativeTextProgramFunction {
        name: "main".to_string(),
        slot_count: 0,
        instructions: vec![
            NativeTextInstruction::CallImport { module: "libc".to_string(), field: "write".to_string() },
            NativeTextInstruction::Return,
        ],
    }];
    let error = encode_native_text_program_with_imports(
        &functions,
        &[("libc".to_string(), "write".to_string())],
        NativeAbi::SystemV64,
    )
    .expect_err("ELF 目标不得生成 PE 导入表");
    assert!(error.contains("宿主导入"), "unexpected error: {error}");
}

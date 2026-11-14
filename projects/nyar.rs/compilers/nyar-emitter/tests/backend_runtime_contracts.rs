use nyar_emitter::transitional::binary::{
    elf::{NativeElfImage, NativeElfWriter, SharedElfImage, SharedElfWriter, SharedObjectExport},
    pe::{NativePeImage, NativePeWriter},
};
use acorn_odin::{Odin_VERSION, OdinConstant, OdinFunction, OdinModuleData};

use nyar::{
    abstractions::{BinaryArch, BinaryFlavor, BinaryTarget, TargetFamily},
    backends::{CompilationOptions, TargetCodeGenBackend},
};
use nyar_emitter::{
    nyar_backend_native::{
        NativeBinaryBackend, NativeBinaryBackendInput, NativeExecutableError, NativeExecutableKind, classify_native_executable,
        classify_native_image,
    },
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
        exports: Vec::new(),
        text_rel32_fixups: Vec::new(),
            is_dll: false,
        entry_point: 0,
    })
    .expect("pe");
    assert_eq!(classify_native_executable(&pe).expect("pe"), NativeExecutableKind::Pe);

    let elf = NativeElfWriter::write_executable(&NativeElfImage { text: vec![0xC3], rodata: Vec::new(), entry_point: 0 }).expect("elf");
    assert_eq!(classify_native_executable(&elf).expect("elf"), NativeExecutableKind::Elf);

    let shared = SharedElfWriter::write_aarch64(
        &SharedElfImage { text: vec![0xC0, 0x03, 0x5F, 0xD6], rodata: Vec::new(), data: Vec::new(), bss: Vec::new() },
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
        exports: Vec::new(),
        text_rel32_fixups: Vec::new(),
            is_dll: false,
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

fn minimal_pe() -> Vec<u8> {
    NativePeWriter::write_executable(&NativePeImage {
        text: vec![0xC3],
        rdata: Vec::new(),
        idata: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
        text_rel32_fixups: Vec::new(),
            is_dll: false,
        entry_point: 0,
    })
    .expect("pe")
}

fn pe_offset(bytes: &[u8]) -> usize {
    u32::from_le_bytes(bytes[0x3C..0x40].try_into().expect("e_lfanew")) as usize
}

fn native_options(arch: BinaryArch) -> CompilationOptions {
    CompilationOptions {
        target: BinaryTarget::new(TargetFamily::Native, arch, BinaryFlavor::Native),
        artifact_name: "probe".to_string(),
        emit_debug_symbols: false,
        optimize: false,
    }
}

/// x86_64 `ET_DYN`（Linux `.node`/`.so`）必须可解析为共享库，而不是「解析成功但不支持」。
#[test]
fn x86_64_shared_elf_is_recognized_as_shared_library() {
    let shared = SharedElfWriter::write_aarch64(
        &SharedElfImage { text: vec![0xC0, 0x03, 0x5F, 0xD6], rodata: Vec::new(), data: Vec::new(), bss: Vec::new() },
        &[SharedObjectExport { name: "probe_export".into(), text_offset: 0 }],
    )
    .expect("so");
    let mut x86_64 = shared;
    // ELF64 e_machine 位于 e_ident 之后偏移 18；AArch64(183) → x86_64(62)。
    x86_64[18..20].copy_from_slice(&62u16.to_le_bytes());
    let facts = classify_native_image(&x86_64).expect("x86_64 ET_DYN 必须可解析");
    assert_eq!(facts.kind, NativeExecutableKind::ElfShared);
    assert_eq!(facts.arch, BinaryArch::X64);
    assert!(facts.shared, "ET_DYN 必须标记为共享库");
}

/// PE DLL 是 Node `.node` / Python `.pyd` 的 Windows 载体，必须按共享库包装。
#[test]
fn pe_dll_accepts_node_extension_as_dynamic_library() {
    let mut pe = minimal_pe();
    let characteristics = pe_offset(&pe) + 4 + 18;
    pe[characteristics..characteristics + 2].copy_from_slice(&0x2000u16.to_le_bytes());
    let facts = classify_native_image(&pe).expect("PE32+");
    assert!(facts.shared, "`IMAGE_FILE_DLL` 必须被识别为共享库");

    let output_dir = std::env::temp_dir().join(format!("nyar-native-node-{}", std::process::id()));
    let input = NativeBinaryBackendInput {
        executable: pe,
        output_dir: output_dir.clone(),
        entry_symbol: "napi_register_module".to_string(),
        artifact_extension: Some(".node".to_string()),
    };
    let artifacts = NativeBinaryBackend::new().compile(input, &native_options(BinaryArch::X64)).expect("`.node` 共享镜像必须可包装");
    assert_eq!(artifacts.artifacts.len(), 1, "共享库不伪造 COFF sidecar: {artifacts:?}");
    let artifact = &artifacts.artifacts[0];
    assert_eq!(artifact.path, "probe.node");
    assert_eq!(artifact.kind, nyar::ArtifactKind::DynamicLibrary);
    assert!(output_dir.join("probe.node").is_file());
    let _ = std::fs::remove_dir_all(output_dir);
}

/// 包装层不得声明自己无法识别的目标架构。
#[test]
fn native_descriptor_does_not_advertise_unrecognized_architecture() {
    let backend = NativeBinaryBackend::new();
    let descriptor = backend.descriptor();
    assert!(
        descriptor.supported_targets.iter().all(|target| target.arch != BinaryArch::X86),
        "X86 镜像无法被解析，不得出现在支持目标中: {:?}",
        descriptor.supported_targets
    );
}

/// 可执行镜像不得声明共享库扩展名：宿主会加载失败。
#[test]
fn node_extension_on_executable_image_is_rejected() {
    let output_dir = std::env::temp_dir().join(format!("nyar-native-bad-ext-{}", std::process::id()));
    let input = NativeBinaryBackendInput {
        executable: minimal_pe(),
        output_dir,
        entry_symbol: "main".to_string(),
        artifact_extension: Some(".node".to_string()),
    };
    let error = NativeBinaryBackend::new().compile(input, &native_options(BinaryArch::X64)).expect_err("可执行镜像 + `.node` 必须结构化拒绝");
    assert!(error.to_string().contains("需要共享库镜像"), "{error}");
}

/// 包装层不得把 AMD64 镜像写成其他架构的产物。
#[test]
fn native_backend_rejects_target_arch_mismatch() {
    let output_dir = std::env::temp_dir().join(format!("nyar-native-arch-{}", std::process::id()));
    let input = NativeBinaryBackendInput { executable: minimal_pe(), output_dir, entry_symbol: "main".to_string(), artifact_extension: None };
    let error = NativeBinaryBackend::new().compile(input, &native_options(BinaryArch::Arm64)).expect_err("镜像架构与请求目标不一致必须拒绝");
    assert!(error.to_string().contains("不一致"), "{error}");
}

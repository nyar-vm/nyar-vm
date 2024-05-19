use nyar_emitter::*;

#[test]
fn collapses_double_dashes_in_package_segment() {
    // Artifact stems like `legion__main_legion` must not yield WIT-illegal `--`.
    let wit = build_component_wit_document("legion__main_legion", &[]);
    assert!(wit.contains("package nyar:legion-main-legion@0.1.0;"), "wit={wit}");
    assert!(!wit.contains("--"), "wit={wit}");
}

#[test]
fn emits_local_interface_for_bare_env_module() {
    let wit = build_component_wit_document("demo_wasi", &[("env".to_string(), "cli_get_project".to_string())]);
    assert!(wit.contains("interface env {"), "wit={wit}");
    assert!(wit.contains("cli-get-project: func(arg0: s32) -> s32;"), "wit={wit}");
    assert!(wit.contains("import env;"), "wit={wit}");
}

#[test]
fn emits_command_world_without_imports() {
    let wit = build_component_wit_document("demo_wasi", &[]);
    assert!(wit.contains("package nyar:demo-wasi@0.1.0;"), "wit={wit}");
    assert!(wit.contains("world command"), "wit={wit}");
    assert!(wit.contains("export wasi:cli/run@0.2.12;"), "wit={wit}");
    assert!(!wit.contains("import "), "wit={wit}");
}

#[test]
fn emits_wasip3_command_world_with_0_3_package_train() {
    let wit = build_component_wit_document_for("demo_wasi", &[], WasiPreview::Preview3);
    assert!(wit.contains("export wasi:cli/run@0.3.0;"), "wit={wit}");
    assert!(!wit.contains("@0.2.12"), "wit={wit}");
}

#[test]
fn wasip3_remaps_console_to_cli_stdout_and_renames_resolution() {
    // p3-first: p2 wasi:io console → wasi:cli/stdout; clocks rename resolution.
    assert_eq!(
        wasi_adapt_import_for_preview("wasi:io/streams", "blocking-write-and-flush", WasiPreview::Preview3),
        Some(("wasi:cli/stdout".to_string(), "write-via-stream".to_string()))
    );
    assert_eq!(
        wasi_adapt_import_for_preview("wasi:clocks/monotonic-clock", "resolution", WasiPreview::Preview3),
        Some(("wasi:clocks/monotonic-clock".to_string(), "get-resolution".to_string()))
    );
    let adapted: Vec<_> = [
        ("wasi:clocks/monotonic-clock", "now"),
        ("wasi:clocks/monotonic-clock", "resolution"),
        ("wasi:io/streams", "blocking-write-and-flush"),
    ]
    .into_iter()
    .filter_map(|(module, field)| {
        let (module, field) = wasi_adapt_import_for_preview(module, field, WasiPreview::Preview3)?;
        Some((wasi_versioned_import_module_for(&module, WasiPreview::Preview3), field))
    })
    .collect();
    let wit = build_component_wit_document_for("demo_wasi", &adapted, WasiPreview::Preview3);
    assert!(wit.contains("import wasi:clocks/monotonic-clock@0.3.0;"), "wit={wit}");
    assert!(wit.contains("import wasi:cli/stdout@0.3.0;"), "wit={wit}");
    assert!(!wit.contains("wasi:io"), "wit={wit}");
    let deps = build_dependency_documents_for(&adapted, WasiPreview::Preview3);
    let clocks = deps.iter().find(|(name, _)| name.contains("clocks")).expect("clocks dep");
    assert!(clocks.1.contains("get-resolution: func() -> u64;"), "wit={}", clocks.1);
    // Avoid false positive: `get-resolution: func` contains the substring `resolution: func`.
    assert!(!clocks.1.lines().any(|line| line.trim_start().starts_with("resolution:")), "wit={}", clocks.1);
    let cli = deps.iter().find(|(name, _)| name.contains("cli")).expect("cli dep");
    assert!(cli.1.contains("write-via-stream: func(data: stream<u8>) -> future<result<_, error-code>>;"), "wit={}", cli.1);
    assert!(cli.1.contains("interface types"), "wit={}", cli.1);
    assert!(cli.1.contains("enum error-code"), "wit={}", cli.1);
    assert!(cli.1.contains("run: async func() -> result;"), "wit={}", cli.1);
    assert!(!cli.1.contains("async func(data: stream"), "wit={}", cli.1);
    assert!(!cli.1.contains("list<u8>"), "wit={}", cli.1);
}

#[test]
fn wasip2_cli_stream_methods_use_component_model_signature() {
    let imports = vec![("wasi:cli/stderr@0.2.12".to_string(), "write-via-stream".to_string())];
    let deps = build_dependency_documents_for(&imports, WasiPreview::Preview2);
    let cli = deps.iter().find(|(name, _)| name.contains("cli")).expect("cli dep");
    assert!(cli.1.contains("write-via-stream: func(data: stream<u8>) -> future<result>;"));
    assert!(!cli.1.contains("write-via-stream: func(arg0: s32) -> s32;"));
}

#[test]
fn derives_preview_from_host_flavor() {
    assert_eq!(WasiPreview::from_host_flavor("wasi-component-model"), WasiPreview::Preview2);
    assert_eq!(WasiPreview::from_host_flavor("wasi-component-model-p3"), WasiPreview::Preview3);
}

#[test]
fn emits_world_imports_for_wasi_packages() {
    let wit = build_component_wit_document(
        "demo_wasi",
        &[
            ("wasi:clocks/monotonic-clock".to_string(), "now".to_string()),
            ("wasi:io/streams".to_string(), "blocking-write-and-flush".to_string()),
        ],
    );
    assert!(wit.contains("import wasi:clocks/monotonic-clock@0.2.12;"), "wit={wit}");
    assert!(wit.contains("import wasi:io/streams@0.2.12;"), "wit={wit}");
}

#[test]
fn emits_honest_get_arguments_signature() {
    let dependencies = build_dependency_documents(&[("wasi:cli/environment".to_string(), "get-arguments".to_string())]);
    let cli = dependencies.iter().find(|(file_name, _)| file_name.contains("wasi-cli")).expect("cli dependency");
    assert!(cli.1.contains("package wasi:cli@0.2.12;"), "wit={}", cli.1);
    assert!(cli.1.contains("get-arguments: func() -> list<string>;"), "wit={}", cli.1);
    assert!(cli.1.contains("interface run"), "wit={}", cli.1);
    assert!(cli.1.contains("run: func() -> result;"), "wit={}", cli.1);
}

#[test]
fn builds_dependency_documents_for_import_packages() {
    let dependencies = build_dependency_documents(&[
        ("wasi:clocks/monotonic-clock".to_string(), "now".to_string()),
        ("wasi:io/streams".to_string(), "blocking-write-and-flush".to_string()),
    ]);
    let clocks = dependencies.iter().find(|(file_name, _)| file_name.contains("wasi-clocks")).expect("clocks dependency");
    let io = dependencies.iter().find(|(file_name, _)| file_name.contains("wasi-io")).expect("io dependency");
    assert!(clocks.1.contains("package wasi:clocks@0.2.12;"), "wit={}", clocks.1);
    assert!(clocks.1.contains("interface monotonic-clock"), "wit={}", clocks.1);
    assert!(clocks.1.contains("now: func() -> u64;"), "wit={}", clocks.1);
    assert!(io.1.contains("package wasi:io@0.2.12;"), "wit={}", io.1);
    assert!(io.1.contains("interface streams"), "wit={}", io.1);
    assert!(io.1.contains("blocking-write-and-flush: func(arg0: s32) -> s32;"), "wit={}", io.1);
}

#[test]
fn emits_honest_wall_clock_datetime_signature() {
    let dependencies = build_dependency_documents(&[("wasi:clocks/wall-clock".to_string(), "now".to_string())]);
    let clocks = dependencies.iter().find(|(file_name, _)| file_name.contains("wasi-clocks")).expect("clocks dependency");
    assert!(clocks.1.contains("package wasi:clocks@0.2.12;"), "wit={}", clocks.1);
    assert!(clocks.1.contains("interface wall-clock"), "wit={}", clocks.1);
    assert!(clocks.1.contains("record datetime"), "wit={}", clocks.1);
    assert!(clocks.1.contains("seconds: u64"), "wit={}", clocks.1);
    assert!(clocks.1.contains("nanoseconds: u32"), "wit={}", clocks.1);
    assert!(clocks.1.contains("now: func() -> datetime;"), "wit={}", clocks.1);
    assert!(!clocks.1.contains("now: func() -> u64;"), "wit={}", clocks.1);
}

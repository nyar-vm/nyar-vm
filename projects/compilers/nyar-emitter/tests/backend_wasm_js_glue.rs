use nyar_emitter::*;

/// 通过 `concat!` 拼接的被禁模式常量，避免在源码中出现完整字面量。
/// 这些模式用于验证生成的启动壳不包含上层专用协议污染。
const BANNED_BANNER: &str = concat!("Valkyrie", " ", "legion", " ", "CLI");
const BANNED_BOOTSTRAP_ENV: &str = concat!("LEGION", "_", "BOOTSTRAP", "_", "HOST");
const BANNED_HOST_ENV: &str = concat!("LEGION", "_", "HOST");
const BANNED_MANIFEST_PATH: &str = concat!("legion", ".", "von");
const BANNED_PROJECT_NAME: &str = concat!("legion", ".", "tools");
const BANNED_HOST_COMPILE_FROM_PLAN_RUST: &str = concat!("host", "_", "legion", "_", "compile", "_", "from", "_", "plan");
const BANNED_HOST_COMPILE_FROM_PLAN_JS: &str = concat!("host", "Legion", "Compile", "From", "Plan");

/// 验证 `collect_import_bindings` 仅消费源码声明的 imports，去重且不注入伪导入。
#[test]
fn collect_import_bindings_consumes_only_declared_imports() {
    let imports = vec![
        ("env".to_string(), "emit_byte".to_string()),
        ("env".to_string(), "emit_byte".to_string()),
        ("env".to_string(), "read_source_byte".to_string()),
    ];
    let bindings = collect_import_bindings(&imports);
    assert_eq!(bindings.len(), 2, "应去重重复的导入字段");
    assert!(bindings.contains(&("env".to_string(), "emit_byte".to_string())));
    assert!(bindings.contains(&("env".to_string(), "read_source_byte".to_string())));
    assert!(!bindings.iter().any(|(_, field)| field == BANNED_HOST_COMPILE_FROM_PLAN_RUST), "不应注入伪导入");
}

/// 验证 `collect_import_bindings` 对空导入列表返回空绑定，不塞默认 imports。
#[test]
fn collect_import_bindings_empty_input_yields_no_default_imports() {
    let bindings = collect_import_bindings(&[]);
    assert!(bindings.is_empty(), "空导入不应产生任何默认绑定");
}

/// 验证 `build_node_launcher` 生成的启动壳遵循 `logical_entry` 契约：
/// 优先 `exports.main`，回退 `exports._start`。
#[test]
fn build_node_launcher_honors_logical_entry_contract() {
    let imports: Vec<(String, String)> = vec![];
    let launcher = build_node_launcher("demo", &imports, &[], false, None);
    assert!(launcher.contains("exports.main ?? exports._start"), "启动壳必须按 logical_entry 契约选择入口");
}

#[test]
fn build_node_launcher_dispatches_build_with_requested_project_target_and_output() {
    let imports = vec![
        ("env".to_string(), "cli_get_project".to_string()),
        ("env".to_string(), "cli_get_target".to_string()),
        ("env".to_string(), "cli_get_output".to_string()),
        ("env".to_string(), "cli_get_verbose".to_string()),
    ];
    let launcher = build_node_launcher("legion", &imports, &[], false, None);

    assert!(launcher.contains("command === \"build\""));
    assert!(launcher.contains("cli_project = positional[0]"));
    assert!(launcher.contains("cli_target = args[++i]"));
    assert!(launcher.contains("cli_output = args[++i]"));
    assert!(launcher.contains("exports.build"));
    assert!(launcher.contains("exports.version"));
    assert!(launcher.contains("exports.help"));
    assert!(launcher.contains("command === \"\" && typeof exports.help === \"function\""), "空 argv 仅在存在 help 时走帮助");
    assert!(!launcher.contains(BANNED_HOST_COMPILE_FROM_PLAN_JS));
}

#[test]
fn build_node_launcher_library_mode_exposes_call_export_without_main_dispatch() {
    let launcher = build_node_launcher("demo", &[], &[], true, None);
    assert!(launcher.contains("export async function callExport"), "library glue must expose callExport");
    assert!(!launcher.contains("exports.main ?? exports._start"), "library glue must not auto-run main");
}

#[test]
fn build_node_launcher_library_mode_emits_invoke_marshaler_when_spec_present() {
    let spec = r#"{"exports":{"twoSum":{"params":["list_i64","i64"],"returns":"list_i64"}},"glue":{"list_i64":{"new":"__nyar_glue.ArrayList.new","push":"__nyar_glue.ArrayList.push","length":"__nyar_glue.ArrayList.length","at":"__nyar_glue_list_i64_at"}}}"#;
    let launcher = build_node_launcher("demo", &[], &[], true, Some(spec));
    assert!(launcher.contains("marshalLibraryArg"), "library glue must marshal args from invoke spec");
    assert!(launcher.contains("demarshalLibraryReturn"), "library glue must demarshal returns from invoke spec");
}

#[test]
fn build_node_launcher_without_cli_imports_skips_cli_dispatch() {
    let imports = vec![("env".to_string(), "emit_byte".to_string())];
    let launcher = build_node_launcher("demo", &imports, &[], false, None);
    assert!(!launcher.contains("command === \"build\""), "非 CLI 导入不应启用 build 分派");
    assert!(!launcher.contains("exports.help"), "非 CLI 导入不应要求 help 导出");
    assert!(launcher.contains("exports.main ?? exports._start"));
}

/// 验证单入口 stdout 契约：当 imports 包含 `emit_byte` 时，
/// 启动壳生成 `process.stdout.write` 输出路径。
#[test]
fn build_node_launcher_single_entry_stdout_contract() {
    let imports = vec![("env".to_string(), "emit_byte".to_string())];
    let launcher = build_node_launcher("demo", &imports, &[], false, None);
    assert!(launcher.contains("process.stdout.write"), "单入口 stdout 契约：应提供 stdout 输出路径");
    assert!(launcher.contains("output_bytes"), "单入口 stdout 契约：应使用 output_bytes 缓冲区");
}

/// 验证非 CLI 程序的 `stdout` 不被上层专用协议污染：
/// 启动壳不输出 banner、不读取被禁环境变量、不硬编码清单路径或 project_name。
#[test]
fn build_node_launcher_does_not_pollute_non_cli_stdout_with_banner() {
    let imports = vec![("env".to_string(), "emit_byte".to_string())];
    let launcher = build_node_launcher("demo", &imports, &[], false, None);
    assert!(!launcher.contains(BANNED_BANNER), "不应输出 banner");
    assert!(!launcher.contains(BANNED_BOOTSTRAP_ENV), "不应读取 bootstrap host 环境变量");
    assert!(!launcher.contains(BANNED_HOST_ENV), "不应读取 host 环境变量");
    assert!(!launcher.contains(BANNED_MANIFEST_PATH), "不应硬编码清单路径");
    assert!(!launcher.contains(BANNED_PROJECT_NAME), "不应硬编码 project_name");
    assert!(!launcher.contains(BANNED_HOST_COMPILE_FROM_PLAN_JS), "不应包含上层专用编译协议实现");
}

/// 验证无 imports 时启动壳仍可正常实例化 `WASM` 模块，且使用空 `importObject`。
#[test]
fn build_node_launcher_no_imports_uses_empty_import_object() {
    let imports: Vec<(String, String)> = vec![];
    let launcher = build_node_launcher("demo", &imports, &[], false, None);
    assert!(launcher.contains("WebAssembly.instantiate"), "应实例化 WASM 模块");
    assert!(launcher.contains("wasmResolveInstance(wasmBytes, {})"), "无 imports 时应使用空 importObject");
}

/// 验证 `render_import_object_literal` 仅渲染源码声明的 imports，不追加伪导入字段。
#[test]
fn render_import_object_literal_excludes_synthetic_imports() {
    let imports = vec![("env".to_string(), "emit_byte".to_string())];
    let literal = render_import_object_literal(&imports);
    assert!(literal.contains("emit_byte"), "应渲染声明的 emit_byte 字段");
    assert!(!literal.contains(BANNED_HOST_COMPILE_FROM_PLAN_RUST), "不应追加伪导入字段");
    assert!(!literal.contains(BANNED_HOST_COMPILE_FROM_PLAN_JS), "不应追加伪导入字段的 JS 实现");
}

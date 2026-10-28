use nyar_emitter::*;

/// 启动壳必须保持宿主中立：不输出 banner、不读取宿主环境变量、不启动子进程、
/// 不硬编码清单文件名，也不注入宿主桥接的伪导入。
const BANNED_HOST_ENV_READ: &str = concat!("process", ".", "env");
const BANNED_CHILD_PROCESS: &str = concat!("child", "_", "process");
const BANNED_CONSOLE_BANNER: &str = concat!("console", ".", "log");
const BANNED_MANIFEST_SUFFIX: &str = concat!(".", "von");
const BANNED_HOST_IMPORT_PREFIX: &str = concat!("host", "_");

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
    assert!(!bindings.iter().any(|(_, field)| field.starts_with(BANNED_HOST_IMPORT_PREFIX)), "不应注入宿主桥接伪导入");
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
    let launcher = build_node_launcher("demo_cli", &imports, &[], false, None);

    assert!(launcher.contains("command === \"build\""));
    assert!(launcher.contains("cli_project = positional[0]"));
    assert!(launcher.contains("cli_target = args[++i]"));
    assert!(launcher.contains("cli_output = args[++i]"));
    assert!(launcher.contains("exports.build"));
    assert!(launcher.contains("exports.version"));
    assert!(launcher.contains("exports.help"));
    assert!(launcher.contains("command === \"\" && typeof exports.help === \"function\""), "空 argv 仅在存在 help 时走帮助");
    assert!(!launcher.contains(BANNED_HOST_IMPORT_PREFIX), "不应注入宿主桥接伪导入");
}

#[test]
fn build_node_launcher_library_mode_exposes_call_export_without_main_dispatch() {
    let launcher = build_node_launcher("demo", &[], &[], true, None);
    assert!(launcher.contains("export async function callExport"), "library glue must expose callExport");
    assert!(!launcher.contains("exports.main ?? exports._start"), "library glue must not auto-run main");
}

#[test]
fn build_node_launcher_library_mode_emits_invoke_marshaler_when_spec_present() {
    // `nyar.library_invoke` v0 合同：version 0 + 带参数身份的 params + glue_entries + abi_digest。
    // abi_digest 在启动壳里只做携带，不参与校验，因此这里用固定占位值；真实值由
    // `library_glue::abi_contract_digest` 在编译期计算。
    let spec = r#"{
        "version": 0,
        "exports": {
            "twoSum": {
                "params": [
                    {"index":0,"name":"nums","type":{"kind":"NamedAggregate","name":"ArrayList","module_path":"std::collections","generic_args":[{"kind":"I64"}]},"carrier":"handle"},
                    {"index":1,"name":"target","type":{"kind":"I64"},"carrier":"i64"}
                ],
                "returns": {"kind":"NamedAggregate","name":"ArrayList","module_path":"std::collections","generic_args":[{"kind":"I64"}]}
            }
        },
        "glue_entries": [
            {
                "kind": "NamedAggregate",
                "name": "ArrayList",
                "module_path": "std::collections",
                "generic_args": [{"kind":"I64"}],
                "operations": [
                    {"operation":"construct","export_name":"$seq_std_collections_ArrayList_i64_new","param_carriers":["i32"],"return_carrier":"i32","export_source":"backend_synthetic"},
                    {"operation":"push","export_name":"$seq_std_collections_ArrayList_i64_push","param_carriers":["i32","i64"],"return_carrier":"void","export_source":"backend_synthetic"},
                    {"operation":"length","export_name":"$seq_std_collections_ArrayList_i64_len","param_carriers":["i32"],"return_carrier":"i32","export_source":"backend_synthetic"},
                    {"operation":"at","export_name":"$seq_std_collections_ArrayList_i64_at","param_carriers":["i32","i32"],"return_carrier":"i64","export_source":"backend_synthetic"}
                ],
                "source": "backend_synthetic",
                "status": "ready"
            }
        ],
        "abi_digest": "sha256-fixture"
    }"#;
    let launcher = build_node_launcher("demo", &[], &[], true, Some(spec));
    assert!(launcher.contains("marshalLibraryArg"), "library glue must marshal args from invoke spec");
    assert!(launcher.contains("demarshalLibraryReturn"), "library glue must demarshal returns from invoke spec");
    assert!(launcher.contains("glue_entries"), "launcher must carry the glue_entries contract verbatim");
    assert!(launcher.contains("\"version\": 0"), "launcher must carry the schema version verbatim");
    assert!(launcher.contains("abi_digest"), "launcher must carry the ABI digest verbatim");
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

/// 验证非 CLI 程序的 `stdout` 不被宿主层协议污染：
/// 启动壳不输出 banner、不读取宿主环境变量、不硬编码清单文件名或 project_name。
#[test]
fn build_node_launcher_does_not_pollute_non_cli_stdout_with_banner() {
    let imports = vec![("env".to_string(), "emit_byte".to_string())];
    let launcher = build_node_launcher("demo", &imports, &[], false, None);
    assert!(!launcher.contains(BANNED_CONSOLE_BANNER), "不应输出 banner");
    assert!(!launcher.contains(BANNED_HOST_ENV_READ), "不应读取宿主环境变量");
    assert!(!launcher.contains(BANNED_CHILD_PROCESS), "不应启动子进程");
    assert!(!launcher.contains(BANNED_MANIFEST_SUFFIX), "不应硬编码清单文件名");
    assert!(!launcher.contains(BANNED_HOST_IMPORT_PREFIX), "不应注入宿主桥接伪导入");
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
    let literal = nyar_emitter::backend::wasm_js_glue::render_import_object_literal(&imports);
    assert!(literal.contains("emit_byte"), "应渲染声明的 emit_byte 字段");
    assert!(!literal.contains(BANNED_HOST_IMPORT_PREFIX), "不应追加宿主桥接伪导入字段");
}

/// 验证 `cli_target` 硬编码默认值 `"node"` **只在 Node runner 层使用**，
/// 不参与编译事实决策——它不是编译目标（`TargetExecutionPlan.target`），
/// 也不影响 `cli_mode` 开关（CLI 分派是否生成）。
///
/// 这条防线确保未来即使有人把默认值改成 `"deno"` / `"bun"` 或从某个
/// 运行时配置读取，也不会意外污染编译目标的冻结语义。
#[test]
fn cli_target_default_is_runner_concern_not_compile_target() {
    let imports = vec![
        ("env".to_string(), "cli_get_project".to_string()),
        ("env".to_string(), "cli_get_target".to_string()),
        ("env".to_string(), "cli_get_output".to_string()),
    ];
    let launcher = build_node_launcher("demo_cli", &imports, &[], false, None);

    // 1. JS 硬编码默认值应保留在启动壳里（Node runner 层）
    assert!(
        launcher.contains("let cli_target = \"node\""),
        "启动壳应携带 runner CLI 默认值，供 --target 参数覆写"
    );

    // 2. 仅替换默认值为其他字符串不应影响 cli_mode（CLI 分派代码仍在）
    //    —— cli_mode 开关只基于 import 名存在性，和默认值无关
    let alt_launcher = launcher.replace("let cli_target = \"node\"", "let cli_target = \"deno\"");
    assert!(
        alt_launcher.contains("command === \"build\""),
        "cli_mode 不应被 cli_target 默认值影响：换默认值后 CLI 分派仍应存在"
    );
    assert!(
        alt_launcher.contains("cli_target = args[++i]"),
        "--target 参数覆写逻辑应保持"
    );

    // 3. 非 CLI imports 情况下，启动壳虽然仍声明 cli_target 变量（模板统一生成），
    //    但 **CLI 分派代码**（build 命令解析 + exports.build 调用）不应出现
    //    —— 这证明 cli_mode 开关只基于 import 名存在性，和模板里的默认值无关
    let non_cli_launcher = build_node_launcher("demo", &[("env".to_string(), "emit_byte".to_string())], &[], false, None);
    assert!(
        !non_cli_launcher.contains("command === \"build\""),
        "非 CLI 模式不应生成 build 命令分派——cli_target 默认值存在 ≠ 启用 CLI 模式"
    );
    assert!(
        !non_cli_launcher.contains("cli_target = args[++i]"),
        "非 CLI 模式不应生成 --target 参数解析逻辑"
    );
}

//! Node JS glue 合同层覆盖：`build_node_launcher`（library 模式）生成的 `.mjs`
//! 是交付产物本体，本文件按 marshal/demarshal 各形态锁定其真实合同——
//! 标量、Utf8 句柄、Option 空引用、handle 载体规则、无 ABI 桥复合类型的
//! 失败闭合与导入表渲染。
//!
//! 与 `backend_wasm_js_glue.rs`（launcher 骨架/CLI 隔离）互补：本文件只覆盖
//! library 模式的 ABI 桥合同。端到端行为（真实构建 + Node 调用）在
//! `rusty-valkyrie/tests/asgard/glue_contract.rs`，两处不重复造求值。

use nyar_emitter::{build_node_launcher, collect_import_bindings};

/// 构造 library_invoke v0 规格：导出面 + abi digest。
fn spec(exports: &str) -> String {
    format!(r#"{{"version":0,"exports":{exports},"abi_digest":"sha256-fixture"}}"#)
}

fn launcher_for(spec: &str) -> String {
    build_node_launcher("contract_demo", &[], &[], true, Some(spec))
}

// ── Utf8：host-intern 句柄与 JS 字符串双向桥 ───────────────────────────────

#[test]
fn glue_demarshals_utf8_return_through_host_intern_table() {
    // `Utf8` 返回在 JS glue 轨是宿主 host-intern 句柄；demarshal 必须经
    // `hostResolve` 解码回 JS 字符串，而不是把句柄当数字交给调用方。
    let glue = launcher_for(&spec(r#"{"title":{"params":[],"returns":{"kind":"Utf8"}}}"#));
    assert!(
        glue.contains(r#"if (kind === "Utf8" || kind === "Utf16") return raw == null ? "" : hostResolve(raw);"#),
        "Utf8 返回必须经 host-intern 表解码为字符串:\n{glue}"
    );
}

#[test]
fn glue_marshals_js_string_utf8_parameter_to_host_handle() {
    // Utf8/Utf16 参数位接受 JS 字符串并在宿主表登记句柄；数值句柄原样透传，
    // 不把字符串当裸 i32。
    let glue =
        launcher_for(&spec(r#"{"echo":{"params":[{"index":0,"name":"s","type":{"kind":"Utf8"},"carrier":"i32"}],"returns":{"kind":"Utf8"}}}"#));
    assert!(
        glue.contains(r#"return typeof value === "string" ? hostIntern(value) : Number(value ?? 0);"#),
        "JS 字符串入参必须登记为 host-intern 句柄:\n{glue}"
    );
}

// ── handle 载体规则：无桥复合类型一律失败闭合 ─────────────────────────────

#[test]
fn glue_fails_closed_for_unbridged_composite_kinds() {
    let glue = launcher_for(&spec(r#"{"noop":{"params":[],"returns":{"kind":"Unit"}}}"#));
    // 具名聚合 / 数组 / 元组当前没有 ABI 桥：两个方向都必须失败闭合而不是 passthrough。
    assert!(glue.contains("Cannot marshal schema kind"), "复合类型 marshal 必须失败闭合:\n{glue}");
    assert!(glue.contains("Cannot demarshal schema kind"), "复合类型 demarshal 必须失败闭合:\n{glue}");
    // Option 内层没有 handle 载体时（标量载荷需要 box/unbox 合同）拒绝裸标量。
    assert!(glue.contains("inner type has no Wasm handle carrier"), "Option 标量内层必须失败闭合:\n{glue}");
    // 未知 schema kind 不静默 passthrough。
    assert!(glue.contains("no ABI bridge implemented"), "未知 kind 必须失败闭合:\n{glue}");
}

#[test]
fn glue_handle_carrier_set_matches_backend_abi() {
    // JS 侧判定「自带 Wasm handle 载体」的集合必须与后端 abi_carrier_name 的
    // handle 分支一致：NamedAggregate / Array / FixedArray / Tuple / Option。
    let glue = launcher_for(&spec(r#"{"noop":{"params":[],"returns":{"kind":"Unit"}}}"#));
    for kind in ["NamedAggregate", "Array", "FixedArray", "Tuple", "Option"] {
        assert!(glue.contains(&format!(r#"kind === "{kind}""#)), "handle 载体集合必须包含 {kind}:\n{glue}");
    }
}

// ── 调用面：callExport 查找与规格驱动 marshal ─────────────────────────────

#[test]
fn glue_call_export_resolves_spec_params_by_position_and_identity() {
    // callExport 合同：按 LIBRARY_INVOKE.exports[name].params 逐位 marshal，
    // 返回按 returns schema demarshal；无规格命中时原样透传（宿主 helper 路径）。
    let glue =
        launcher_for(&spec(r#"{"solve":{"params":[{"index":0,"name":"n","type":{"kind":"I64"},"carrier":"i64"}],"returns":{"kind":"I64"}}}"#));
    assert!(glue.contains("export async function callExport"), "library glue 必须暴露 callExport:\n{glue}");
    assert!(glue.contains(r#"const spec = LIBRARY_INVOKE?.exports?.[name];"#), "调用规格必须按导出名查表:\n{glue}");
    assert!(
        glue.contains("spec.params.map((param, index) => marshalLibraryArg(paramType(param), args[index]))"),
        "参数必须按声明顺序逐位 marshal:\n{glue}"
    );
    assert!(glue.contains("demarshalLibraryReturn(spec.returns, raw)"), "返回值必须按返回 schema demarshal:\n{glue}");
    assert!(glue.contains("return BigInt(value ?? 0);"), "i64 参数必须 BigInt 化:\n{glue}");
}

#[test]
fn glue_i64_return_demarshals_as_bigint_without_number_narrowing() {
    // Wasm i64 返回值经 JS API 天然是 BigInt：demarshal 必须原样透出 BigInt，
    // 不得经 Number 收窄（>2^53 静默丢精度，u64 满值 / 负数高位全中招）。
    let glue =
        launcher_for(&spec(r#"{"solve":{"params":[{"index":0,"name":"n","type":{"kind":"I64"},"carrier":"i64"}],"returns":{"kind":"I64"}}}"#));
    assert!(glue.contains(r#"if (kind === "I64") return raw == null ? null : BigInt(raw);"#), "i64 返回必须透出 BigInt:\n{glue}");
    assert!(!glue.contains(r#"if (kind === "I64") return raw == null ? null : Number(raw);"#), "i64 返回不得经 Number 收窄:\n{glue}");
}

#[test]
fn glue_i64_parameter_rejects_unsafe_number_with_explicit_error() {
    // 反例合同：调用方用 Number 承载超安全整数范围的值时，marshal 必须显式
    // 报错并指引改用 BigInt，不得静默截断；安全整数范围内的 Number 仍接受。
    let glue =
        launcher_for(&spec(r#"{"echo":{"params":[{"index":0,"name":"v","type":{"kind":"I64"},"carrier":"i64"}],"returns":{"kind":"I64"}}}"#));
    assert!(glue.contains(r#"typeof value === "number" && !Number.isSafeInteger(value)"#), "i64 参数必须检查 Number 安全整数范围:\n{glue}");
    assert!(glue.contains("Cannot marshal I64 from Number"), "超范围 Number 必须有结构化报错:\n{glue}");
    assert!(glue.contains("Pass a BigInt instead"), "报错必须指引改用 BigInt:\n{glue}");
}

#[test]
fn glue_library_mode_never_auto_runs_or_exits() {
    // library 产物是被动调用面：不得自动执行 main/_start，不得以退出码结束宿主。
    let glue = launcher_for(&spec(r#"{"noop":{"params":[],"returns":{"kind":"Unit"}}}"#));
    assert!(!glue.contains("exports.main ?? exports._start"), "library glue 不得自动运行入口:\n{glue}");
    assert!(!glue.contains("process.exit"), "library glue 不得替宿主决定退出码:\n{glue}");
    assert!(glue.contains("let wasmInstance;"), "实例必须惰性构造（首次 callExport 才实例化）:\n{glue}");
}

// ── 导入表：只渲染声明的导入，按模块分组，非标识符字段加引号 ───────────────

#[test]
fn glue_import_object_groups_by_module_and_quotes_non_identifier_fields() {
    let imports = vec![
        ("env".to_string(), "add".to_string()),
        ("env".to_string(), "add".to_string()),
        ("std.text".to_string(), "utf8 concat".to_string()),
    ];
    let bindings = collect_import_bindings(&imports);
    assert_eq!(bindings.len(), 2, "重复导入必须去重");

    let glue = build_node_launcher("demo", &imports, &[], false, None);
    assert!(glue.contains("env: {"), "导入表必须按模块分组:\n{glue}");
    assert!(glue.contains(r#""std.text": {"#), "非标识符模块名必须加引号:\n{glue}");
    assert!(glue.contains(r#""utf8 concat":"#) || glue.contains(r#""utf8 concat": "#), "非标识符字段名必须加引号:\n{glue}");
    assert!(glue.contains("add: (a, b) => (a + b) | 0"), "已声明字段必须有宿主实现:\n{glue}");
}

#[test]
fn glue_unknown_import_field_falls_back_to_zero_impl() {
    // 现状锁定：未认识的导入字段生成 `() => 0` 占位实现（导入表按字段名查实现，
    // 未知字段不阻断产物生成）。这是当前真实合同；若未来改为编译期拒绝未映射
    // 导入，本用例转红并升级为拒绝断言。
    let imports = vec![("env".to_string(), "mystery_import".to_string())];
    let glue = build_node_launcher("demo", &imports, &[], false, None);
    assert!(glue.contains("mystery_import: () => 0"), "未知导入字段当前必须落 () => 0:\n{glue}");
}

//! js glue 合同的端到端覆盖：复用 `config_probe` 的正式 `legion build` 链路
//! （manifest → 前端 → canonical → emitter 产出 `.wasm` + `.mjs`），用 Node 经
//! glue `callExport` 断言可观察行为。禁止第二套求值或文本解析（设计 15 §5）。
//!
//! 覆盖层：
//! 1. 导出命名：`[export(name: ...)]` 的公开名就是 `callExport` 的查找键；
//! 2. 参数身份：`nyar.library_invoke` 合同携带 `{index, name, type, carrier}` 参数身份，
//!    标量参数经 marshal 往返正确；
//! 3. 错误路径：未知导出名失败闭合；glue 同目录 wasm 产物缺失时 Node 导入即失败，
//!    不静默降级（当前单体 wasm 形态下的「缺 chunk 文件」类比路径）。

use std::fs;

use super::config_probe::{ProbeFailure, build_probe_project, call_export_through_glue};

/// 标量导出（零参数）：导出名与 `[export(name: ...)]` 逐字符一致。
const SCALAR_EXPORT: &str = r#"[export(name: "configure")]
micro configure() -> u32 {
    return 7
}
"#;

/// 标量导出（带参数）：参数身份合同要求 canonical 参数名进入 ABI 规格。
/// 当前构建链对 u32 的 `infix +` 尚未接线（unresolved call 失败闭合），因此用
/// 两个导出各自透传一个参数位，覆盖逐位 marshal 而不依赖运算符 lowering。
const SCALAR_PARAMS_EXPORT: &str = r#"[export(name: "firstValue")]
micro firstValue(a: u32, b: u32) -> u32 {
    return a
}

[export(name: "secondValue")]
micro secondValue(a: u32, b: u32) -> u32 {
    return b
}
"#;

fn build_scalar(prefix: &str, source: &str) -> super::config_probe::ProbeArtifact {
    match build_probe_project(prefix, source) {
        Ok(artifact) => artifact,
        Err(ProbeFailure { message }) => panic!("glue 合同测试必须走通正式构建链，实际失败：{message}"),
    }
}

#[test]
fn glue_export_name_is_the_declared_public_name() {
    let artifact = build_scalar("glue-export-name", SCALAR_EXPORT);

    // 声明名逐字符可用。
    let value = call_export_through_glue(&artifact, "configure", "[]")
        .unwrap_or_else(|diagnostic| panic!("声明的导出名必须可经 callExport 调用：{diagnostic}"));
    assert_eq!(value, "7", "glue 必须读回导出返回值");

    // 大小写变体不是别名：导出命名合同是精确名。
    let wrong_case =
        call_export_through_glue(&artifact, "Configure", "[]").expect_err("大小写变体不得命中导出；若命中说明存在按名字猜测的第二层解析");
    assert!(wrong_case.contains("wasm export not found: Configure"), "诊断必须指明查找失败的导出名，实际：{wrong_case}");

    // 完全未知名同样失败闭合，不回落 main/_start 或默认导出。
    let unknown = call_export_through_glue(&artifact, "doesNotExist", "[]").expect_err("未知导出名必须失败闭合");
    assert!(unknown.contains("wasm export not found"), "实际：{unknown}");
}

#[test]
fn glue_scalar_parameters_roundtrip_with_parameter_identity() {
    let artifact = build_scalar("glue-scalar-params", SCALAR_PARAMS_EXPORT);

    // 标量参数经 marshal（Number）→ wasm → 返回 demarshal 全链路往返。
    // 两个导出各自读回一个参数位：若参数顺序错位，结果会互换。
    let first = call_export_through_glue(&artifact, "firstValue", "[40, 2]")
        .unwrap_or_else(|diagnostic| panic!("标量参数导出必须可调用：{diagnostic}"));
    assert_eq!(first, "40", "第一参数位必须按声明顺序 marshal 传递");
    let second = call_export_through_glue(&artifact, "secondValue", "[40, 2]")
        .unwrap_or_else(|diagnostic| panic!("标量参数导出必须可调用：{diagnostic}"));
    assert_eq!(second, "2", "第二参数位必须按声明顺序 marshal 传递");

    // `nyar.library_invoke` 合同把参数身份写进产物 glue：index 连续、canonical 名、
    // 冻结载体。参数名缺失时 backend 已在编译期失败，这里是产物侧核对。
    let glue = fs::read_to_string(&artifact.glue_path).expect("读取 Node glue 失败");
    assert!(glue.contains("\"name\": \"a\"") || glue.contains("\"name\":\"a\""), "glue 规格必须携带参数 a 的身份");
    assert!(glue.contains("\"name\": \"b\"") || glue.contains("\"name\":\"b\""), "glue 规格必须携带参数 b 的身份");
    assert!(glue.contains("\"index\": 0") || glue.contains("\"index\":0"), "参数身份必须带声明顺序下标");
    assert!(glue.contains("firstValue"), "glue 规格必须按公开导出名索引调用合同");
}

#[test]
fn glue_fails_closed_when_wasm_artifact_is_missing() {
    // 当前交付形态是单体 `.wasm` + `.mjs` 成对产物；glue 在模块顶层读取同目录
    // wasm。删除 wasm 后导入必须失败闭合（ENOENT），不得静默产出空实例。
    // 转红触发器：wasm chunk 拆分实现落地后，本用例升级为「缺任一被引用 chunk
    // 文件时 loader 结构化失败」的产物断言。
    let artifact = build_scalar("glue-missing-wasm", SCALAR_EXPORT);
    let wasm_path = artifact.glue_path.with_extension("wasm");
    assert!(wasm_path.exists(), "构建成功后 glue 同目录必须存在配对的 `.wasm` 产物");
    fs::remove_file(&wasm_path).expect("删除 wasm 产物失败");

    let diagnostic = call_export_through_glue(&artifact, "configure", "[]").expect_err("wasm 产物缺失时 glue 导入必须失败，不得静默成功");
    assert!(diagnostic.contains("ENOENT") || diagnostic.contains(".wasm"), "诊断必须指明缺失的 wasm 文件，实际：{diagnostic}");
}

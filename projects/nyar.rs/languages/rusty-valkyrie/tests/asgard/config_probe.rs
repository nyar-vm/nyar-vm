//! C0 配置求值探针：验证「V 源码 → 真实编译 → Node 执行 → 经 JS glue 读回导出返回值」候选路线。
//!
//! 账本（`内部决策/06-当前卡点.md`「Asgard 静态页面交付线」）指定的最早红点：
//! 仓库内不存在现成的「V 求值取回对象值」设施；C0 先验证 Node + Wasm GC 工具链能否
//! 承担 Asgard 配置模块的两阶段求值（设计 15 §5）。本探针走正式交付线：
//! `legion build`（manifest → 规划 → 前端 → canonical → emitter）产出 `artifact: "library"`
//! 的 Node `.mjs` glue + `.wasm`，再用 node 导入 glue 的 `callExport` 读回导出函数返回值。
//!
//! 禁止路线（探针未通过前）：在 Asgard 内写第二套求值器、任何文本解析
//! （字符扫描、VON 转换、按名字提取 `define_config` 大括号内容）。
//!
//! 分层：
//! 1. 标量：`u32` 返回值经 glue 读回整数；
//! 2. 文本：`utf8` 返回值，验证跨边界字符串 codec 或固化缺口；
//! 3. 聚合：结构体返回值，验证对象读回或固化缺口。

use rusty_valkyrie::legion::{
    CanonicalTarget,
    cmds::build::{BuildArgs, run},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};

/// 夹具项目源级 `std/primitive` 声明族，与 `tests/legion/support` 同一合同：
/// 数值原始类型名不是语言内建，必须绑定到源码声明身份。
const CORE_PRIMITIVE_DECLARATIONS: &str = concat!(
    "[primitive(\"std::primitive::i8\")] structure i8 { }\n",
    "[primitive(\"std::primitive::i16\")] structure i16 { }\n",
    "[primitive(\"std::primitive::i32\")] structure i32 { }\n",
    "[primitive(\"std::primitive::i64\")] structure i64 { }\n",
    "[primitive(\"std::primitive::i128\")] structure i128 { }\n",
    "[primitive(\"std::primitive::u8\")] structure u8 { }\n",
    "[primitive(\"std::primitive::u16\")] structure u16 { }\n",
    "[primitive(\"std::primitive::u32\")] structure u32 { }\n",
    "[primitive(\"std::primitive::u64\")] structure u64 { }\n",
    "[primitive(\"std::primitive::u128\")] structure u128 { }\n",
    "[primitive(\"std::primitive::isize\")] structure isize { }\n",
    "[primitive(\"std::primitive::usize\")] structure usize { }\n",
    "[primitive(\"std::primitive::f32\")] structure f32 { }\n",
    "[primitive(\"std::primitive::f64\")] structure f64 { }\n",
    "[primitive(\"std::primitive::f128\")] structure f128 { }\n",
    "[primitive(\"std::primitive::byte\")] structure byte { }\n",
    "[primitive(\"std::primitive::sbyte\")] structure sbyte { }\n",
);

/// 一次探针构建的可观察结果：产物目录与 Node 可导入的 glue 入口。
pub(crate) struct ProbeArtifact {
    _temp: tempfile::TempDir,
    pub(crate) output_dir: PathBuf,
    pub(crate) glue_path: PathBuf,
}

/// 构建失败的结构化诊断（保留编译输出原文，失败闭合，不伪造成功）。
pub(crate) struct ProbeFailure {
    pub(crate) message: String,
}

/// 用最小 `legion` 工程（`artifact: "library"` + `target: "node"`）走正式构建链。
///
/// 成功返回落盘产物目录与 `.mjs` glue 路径；失败返回聚合后的结构化诊断文本。
pub(crate) fn build_probe_project(prefix: &str, source: &str) -> Result<ProbeArtifact, ProbeFailure> {
    let temp = tempfile::Builder::new().prefix(prefix).tempdir().expect("探针临时目录");
    let root = temp.path();
    let project_dir = root.join("app");
    let source_dir = project_dir.join("source");
    fs::create_dir_all(&source_dir).expect("创建探针工程源码目录");
    fs::write(root.join("legions.von"), "{\n    name: \"asgard-config-probe\",\n    members: [\"app\"]\n}\n").expect("写入 workspace 清单");
    fs::write(project_dir.join("legion.von"), "{\n    name: \"app\",\n    artifact: \"library\",\n    build: [{ target: \"node\" }]\n}\n")
        .expect("写入工程清单");
    fs::write(source_dir.join("main.v"), source).expect("写入探针配置模块源码");
    fs::write(source_dir.join("core_primitive.v"), CORE_PRIMITIVE_DECLARATIONS).expect("写入 primitive 声明");

    let output_dir = project_dir.join("dist").join("probe");
    let status = run(&BuildArgs {
        project_dir: project_dir.clone(),
        target: CanonicalTarget::parse("node").expect("Node 目标"),
        output_dir: Some(output_dir.clone()),
        debug_artifacts: false,
    });
    let status = match status {
        Ok(code) => code,
        Err(error) => return Err(ProbeFailure { message: format!("legion build 返回错误: {error:?}") }),
    };
    if status != ExitCode::SUCCESS {
        return Err(ProbeFailure { message: format!("legion build 退出码非成功: {status:?}") });
    }

    let glue_path = find_glue(&output_dir)
        .ok_or_else(|| ProbeFailure { message: format!("构建成功但输出目录没有 `.mjs` Node glue: {}", list_dir(&output_dir)) })?;
    Ok(ProbeArtifact { _temp: temp, output_dir, glue_path })
}

fn find_glue(output_dir: &Path) -> Option<PathBuf> {
    fs::read_dir(output_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("mjs"))
}

fn list_dir(dir: &Path) -> String {
    fs::read_dir(dir)
        .map(|entries| {
            entries.filter_map(|entry| entry.ok()).map(|entry| entry.file_name().to_string_lossy().into_owned()).collect::<Vec<_>>().join(", ")
        })
        .unwrap_or_else(|_| "<目录不存在>".to_string())
}

/// Node 侧经 JS glue 的 `callExport` 读回导出函数返回值。
///
/// 返回 `Ok(stdout)`（glue 正常完成调用）或 `Err(诊断)`（glue/编译器抛出的结构化错误，
/// 含退出码与 stderr，供缺口固化断言使用）。
pub(crate) fn call_export_through_glue(artifact: &ProbeArtifact, export_name: &str, args_json: &str) -> Result<String, String> {
    let runner_path = artifact.output_dir.join("c0_probe_runner.mjs");
    fs::write(
        &runner_path,
        r#"import { pathToFileURL } from "node:url";
const glue = await import(pathToFileURL(process.argv[2]).href);
const value = await glue.callExport(process.argv[3], ...JSON.parse(process.argv[4]));
process.stdout.write(JSON.stringify(value ?? null));
"#,
    )
    .expect("写入 Node 探针 runner");
    let output = Command::new("node")
        .arg(&runner_path)
        .arg(&artifact.glue_path)
        .arg(export_name)
        .arg(args_json)
        .output()
        .expect("Node 必须可用（主动交付线宿主）");
    if !output.status.success() {
        return Err(format!("node callExport 失败 status={:?}\nstderr={}", output.status.code(), String::from_utf8_lossy(&output.stderr)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// 第一层（标量）：最小配置模块的整数返回值必须经真实编译 + Node + JS glue 读回。
#[test]
fn c0_scalar_config_value_roundtrips_through_node_glue() {
    let artifact = build_probe_project(
        "asgard-c0-scalar",
        r#"[export(name: "configure")]
micro configure() -> u32 {
    return 7
}
"#,
    )
    .unwrap_or_else(|failure| panic!("C0 标量层必须走通正式构建链，实际失败：{}", failure.message));

    let value = call_export_through_glue(&artifact, "configure", "[]")
        .unwrap_or_else(|diagnostic| panic!("C0 标量层必须经 glue 读回整数，实际失败：{diagnostic}"));
    assert_eq!(value, "7", "glue 读回的标量配置值必须是导出函数的返回值");
}

/// 第二层（文本）：`utf8` 返回值经正式构建 + Node + JS glue 读回字符串。
///
/// 本轮闭合的两处事实：文本常量的编码身份在 canonical→executable 边界由常量变体恢复，
/// 非入口（库导出）模块同样消费 plan 导入并合成 `const_utf8`；glue 侧 `Utf8` 句柄
/// 经 host-intern 表解码回 JS 字符串，入参字符串同样登记为句柄。
#[test]
fn c0_text_layer_roundtrips_through_node_glue() {
    let artifact = build_probe_project(
        "asgard-c0-text",
        r#"[export(name: "title")]
micro title() -> utf8 {
    return "asgard"
}
"#,
    )
    .unwrap_or_else(|failure| panic!("C0 文本层必须走通正式构建链，实际失败：{}", failure.message));

    let value = call_export_through_glue(&artifact, "title", "[]")
        .unwrap_or_else(|diagnostic| panic!("C0 文本层必须经 glue 读回字符串，实际失败：{diagnostic}"));
    assert_eq!(value, "\"asgard\"", "glue 读回的文本配置值必须是导出函数返回的字符串");
}

/// 第三层（聚合）：名义身份门禁已转通，聚合配置模块可完成正式构建。
///
/// 本轮事实：`NyarType::Nominal` 携带 canonical 冻结的实例级 `layout` 身份，
/// WASM 预发射 `validate_storage_identities` 只拒绝未注册 layout，因此返回用户
/// 声明结构体的配置模块不再停在名义身份门禁。
/// 第二道缺口仍在：JS glue 的 `NamedAggregate` demarshal 只支持序列族（length/at），
/// 没有按字段访问结构体的合同，`render_glue_entries` 对非序列聚合停在
/// `pending_export_name`。本用例固化该缺口的结构化诊断，按字段读回由后端 glue/ABI
/// 与配置 SDK 所有者补齐。
#[test]
fn c0_aggregate_layer_reports_the_missing_field_access_contract() {
    let artifact = build_probe_project(
        "asgard-c0-aggregate",
        r#"structure ConfigArtifact {
    target: i32
    entry: i32
}

[export(name: "configure")]
micro configure() -> ConfigArtifact {
    return ConfigArtifact { target: 1, entry: 2 }
}
"#,
    )
    .unwrap_or_else(|failure| panic!("聚合配置模块必须完成正式构建，实际失败：{}", failure.message));

    let diagnostic = call_export_through_glue(&artifact, "configure", "[]").expect_err("非序列聚合的 JS glue 读回合同尚未实现，必须结构化失败");
    assert!(diagnostic.contains("Cannot demarshal NamedAggregate"), "缺口诊断必须指明 NamedAggregate demarshal 合同，实际：{diagnostic}");
    assert!(diagnostic.contains("ConfigArtifact"), "缺口诊断必须指明具体聚合类型，实际：{diagnostic}");
}

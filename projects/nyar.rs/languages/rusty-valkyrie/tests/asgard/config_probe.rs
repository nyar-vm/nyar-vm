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
    "namespace std.primitive;\n",
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
#[derive(Debug)]
pub(crate) struct ProbeArtifact {
    _temp: tempfile::TempDir,
    pub(crate) output_dir: PathBuf,
    pub(crate) glue_path: PathBuf,
}

/// 构建失败的结构化诊断（保留编译输出原文，失败闭合，不伪造成功）。
pub(crate) struct ProbeFailure {
    pub(crate) message: String,
}

/// 依赖 `std` workspace 成员的探针工程构建：`std::types::Option` 是普通
/// `unite` 声明（带 `OptionKind` 标注），配置读回必须验证它经正式链的行为。
pub(crate) fn build_probe_project_with_std(prefix: &str, source: &str) -> Result<ProbeArtifact, ProbeFailure> {
    let temp = tempfile::Builder::new().prefix(prefix).tempdir().expect("探针临时目录");
    let root = temp.path();
    let project_dir = root.join("app");
    let source_dir = project_dir.join("source");
    fs::create_dir_all(&source_dir).expect("创建探针工程源码目录");
    let std_path = crate::support::valkyrie_v_package(&["projects", "std"]);
    // `std` 以 workspace 版本依赖 `unicode.types`（`projects/std/legion.von`），
    // 工作区成员必须同时注册该底层包，否则规划阶段以
    // `ForcedWorkspaceDependencyMissing { project: "std", dependency: "unicode.types" }` 失败。
    let unicode_types_path = crate::support::valkyrie_v_package(&["projects", "unicode._", "unicode.types"]);
    fs::write(
        root.join("legions.von"),
        format!(
            "{{\n    name: \"asgard-config-probe\",\n    members: [\"{std_path}\", \"{unicode_types_path}\", \"app\"]\n}}\n"
        ),
    )
    .expect("写入 workspace 清单");
    fs::write(
        project_dir.join("legion.von"),
        "{\n    name: \"app\",\n    artifact: \"library\",\n    dependencies: {\n        \"std\": { version: \"workspace\" }\n    },\n    build: [{ target: \"node\" }]\n}\n",
    )
    .expect("写入工程清单");
    fs::write(source_dir.join("main.v"), source).expect("写入探针配置模块源码");
    let output_dir = output_dir_of(&project_dir);
    run_probe_build(temp, project_dir, output_dir)
}

/// 探针工程输出目录。
fn output_dir_of(project_dir: &Path) -> PathBuf {
    project_dir.join("dist").join("probe")
}

/// 执行正式 `legion build` 并定位 glue；两个探针工程构造共用的尾段。
fn run_probe_build(temp: tempfile::TempDir, project_dir: PathBuf, output_dir: PathBuf) -> Result<ProbeArtifact, ProbeFailure> {
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
    // 参数协议：JSON 之外允许 `"<digits>n"` 字符串表示 BigInt（i64/u64 超过 2^53
    // 的入参无法用 JSON Number 无损表达）。返回值：BigInt 按十进制文本输出，
    // 其余按 JSON 序列化，保持既有标量/文本探针输出形状不变。
    fs::write(
        &runner_path,
        r#"import { pathToFileURL } from "node:url";
const glue = await import(pathToFileURL(process.argv[2]).href);
const args = JSON.parse(process.argv[4], (key, value) => (typeof value === "string" && /^-?\d+n$/.test(value) ? BigInt(value.slice(0, -1)) : value));
const value = await glue.callExport(process.argv[3], ...args);
process.stdout.write(typeof value === "bigint" ? value.toString() : JSON.stringify(value ?? null));
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

/// 第三层（聚合）：结构值配置经真实编译 + Node + JS glue 按字段读回。
///
/// 本轮事实：`NyarType::Nominal` 携带 canonical 冻结的实例级 `layout` 身份，
/// WASM 预发射身份门禁只拒绝未注册 layout；ABI schema 从冻结布局派生字段
/// 访问合同（字段名/类型 schema/accessor 导出名），后端为库导出返回签名
/// 可达的聚合布局发射逐字段 accessor 导出；JS glue 按 `nyar.library_invoke`
/// 字段合同把聚合句柄逐字段读回为 JS 对象。缺字段合同仍失败闭合。
#[test]
fn c0_aggregate_layer_roundtrips_through_node_glue() {
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

    let value = call_export_through_glue(&artifact, "configure", "[]")
        .unwrap_or_else(|diagnostic| panic!("聚合配置值必须经 glue 按字段读回，实际失败：{diagnostic}"));
    assert_eq!(value, r#"{"target":1,"entry":2}"#, "glue 读回的聚合配置值必须逐字段等于导出函数返回值");
}

/// `define_config` 形态：含文本、布尔、嵌套聚合与 helper 的结构化配置端到端读回。
///
/// 正例覆盖面：普通分支（顶层文本/整数字段）、helper（非导出函数构造嵌套聚合）、
/// 文本与聚合返回共存于同一模块；字段顺序按声明顺序保留。
#[test]
fn c0_define_config_shape_roundtrips_with_text_helper_and_nested_aggregate() {
    let artifact = build_probe_project(
        "asgard-c0-define-config",
        r#"structure EngineSpec {
    renderer: utf8
    vsync: bool
}

structure AppConfig {
    title: utf8
    version: i32
    engine: EngineSpec
}

micro make_engine() -> EngineSpec {
    return EngineSpec { renderer: "wgpu", vsync: true }
}

[export(name: "configure")]
micro configure() -> AppConfig {
    return AppConfig { title: "asgard", version: 3, engine: make_engine() }
}

[export(name: "label")]
micro label() -> utf8 {
    return "asgard"
}
"#,
    )
    .unwrap_or_else(|failure| panic!("define_config 形态模块必须完成正式构建，实际失败：{}", failure.message));

    let aggregate = call_export_through_glue(&artifact, "configure", "[]")
        .unwrap_or_else(|diagnostic| panic!("define_config 形态聚合必须经 glue 读回，实际失败：{diagnostic}"));
    assert_eq!(
        aggregate, r#"{"title":"asgard","version":3,"engine":{"renderer":"wgpu","vsync":true}}"#,
        "glue 读回的结构化配置值必须逐字段等于源码声明"
    );

    let text = call_export_through_glue(&artifact, "label", "[]")
        .unwrap_or_else(|diagnostic| panic!("同模块文本返回值必须仍可读回，实际失败：{diagnostic}"));
    assert_eq!(text, "\"asgard\"", "文本返回值不得受聚合字段合同发射影响");
}

/// 语言事实固化：省略返回类型的函数被拒绝（返回类型无推断）。
///
/// 模块级尾表达式要成为可导出的配置入口，合成函数必须携带显式返回类型；
/// 本测试锁定当前语言事实。若语言引入返回类型推断，本测试必须翻转为正例。
#[test]
fn c0_language_fact_omitted_return_type_is_rejected() {
    let failure = build_probe_project(
        "asgard-c0-omit-return",
        r#"[export(name: "configure")]
micro configure() {
    return 7
}
"#,
    )
    .expect_err("省略返回类型当前必须被类型检查拒绝");
    assert!(
        failure.message.contains("incompatible with the function return type"),
        "拒绝诊断必须说明返回类型不匹配：{}",
        failure.message
    );
}

/// unite 变体读回合同：无载荷 unite 构造经正式链产出 GC sum 引用，
/// glue 按冻结 sum 布局的 tag 映射读回变体身份。
#[test]
fn c0_gap_unite_value_construction_lowers_to_unit() {
    let artifact = build_probe_project(
        "asgard-c0-unite",
        r#"unite Choice {
    On
    Off
}

[export(name: "configure")]
micro configure() -> Choice {
    return Choice.On
}
"#,
    )
    .unwrap_or_else(|failure| panic!("unite 值构造必须完成正式构建，实际失败：{}", failure.message));

    let variant = call_export_through_glue(&artifact, "configure", "[]")
        .unwrap_or_else(|diagnostic| panic!("unite 返回值必须经 glue 读回变体身份，实际失败：{diagnostic}"));
    assert_eq!(variant, "\"On\"", "glue 读回的变体身份必须等于源码构造的变体名");
}

/// 单变体 unite（产物类型枚举的最小形态）同样必须构造并读回变体身份。
#[test]
fn c0_gap_single_variant_unite_construction_lowers_to_unit() {
    let artifact = build_probe_project(
        "asgard-c0-unite-single",
        r#"unite Target {
    Static
}

[export(name: "configure")]
micro configure() -> Target {
    return Target.Static
}
"#,
    )
    .unwrap_or_else(|failure| panic!("单变体 unite 构造必须完成正式构建，实际失败：{}", failure.message));

    let variant = call_export_through_glue(&artifact, "configure", "[]")
        .unwrap_or_else(|diagnostic| panic!("单变体 unite 返回值必须经 glue 读回变体身份，实际失败：{diagnostic}"));
    assert_eq!(variant, "\"Static\"", "glue 读回的变体身份必须等于源码构造的变体名");
}

/// 结构体字段持有 unite 类型：字段布局元数据与构造值身份必须一致
/// （`Nominal` + 声明级 tagged 布局），聚合构造经 SMIR010 闸门并读回字段变体身份。
#[test]
fn c0_gap_struct_field_with_unite_type_resolves_aggregate_layout() {
    let artifact = build_probe_project(
        "asgard-c0-struct-unite-field",
        r#"unite Choice {
    On
    Off
}

structure Holder {
    choice: Choice
}

[export(name: "configure")]
micro configure() -> Holder {
    let choice: Choice = Choice.Off
    return Holder { choice }
}
"#,
    )
    .unwrap_or_else(|failure| panic!("unite 字段聚合必须完成正式构建，实际失败：{}", failure.message));

    let value = call_export_through_glue(&artifact, "configure", "[]")
        .unwrap_or_else(|diagnostic| panic!("聚合字段必须经 glue 读回变体身份，实际失败：{diagnostic}"));
    assert_eq!(value, "{\"choice\":\"Off\"}", "聚合字段读回值必须携带源码构造的变体身份");
}

/// 已闭合缺口（原 C0 固化项翻转）：std 依赖在场时，类型位置的 `utf8` 与字符串
/// 字面量的原始 `utf8` 归一到同一文本载体身份，返回类型检查通过并完成正式构建，
/// 导出的文本值经 Node + JS glue 原样读回。
#[test]
fn c0_gap_std_dependency_drifts_utf8_type_resolution() {
    let artifact = build_probe_project_with_std(
        "asgard-c0-std-utf8",
        r#"[export(name: "title")]
micro title() -> utf8 {
    return "asgard"
}
"#,
    )
    .unwrap_or_else(|failure| panic!("std 在场时 utf8 返回类型必须归一到同一载体身份，实际失败：{}", failure.message));

    let value = call_export_through_glue(&artifact, "title", "[]")
        .unwrap_or_else(|diagnostic| panic!("std 在场时 utf8 返回值必须经 glue 读回，实际失败：{diagnostic}"));
    assert_eq!(value, "\"asgard\"", "glue 读回的文本配置值必须是导出函数返回的字符串");
}

/// 已闭合缺口（原 C0 固化项翻转）：`Option<utf8>`（配置内 `output` / `base` /
/// `assets` 的形态）在 std 在场时完成正式构建。`Some` 载荷按文本载体同一值空间
/// 接受内建 `Utf8` 实参，导出值经 Node + JS glue 按 sum 投影读回变体与载荷。
#[test]
fn c0_gap_std_option_utf8_resolves_after_carrier_equivalence() {
    let artifact = build_probe_project_with_std(
        "asgard-c0-std-option-utf8",
        r#"using std::types::{Option, Some, None};

[export(name: "with_value")]
micro with_value() -> Option<utf8> {
    return Some("dist")
}
"#,
    )
    .unwrap_or_else(|failure| panic!("`Option<utf8>` 的 `Some` 构造必须完成正式构建，实际失败：{}", failure.message));

    let value = call_export_through_glue(&artifact, "with_value", "[]")
        .unwrap_or_else(|diagnostic| panic!("`Option<utf8>` 导出值必须经 glue 读回，实际失败：{diagnostic}"));
    assert_eq!(value, r#"{"variant":"Some","payload":"dist"}"#, "`Some` 变体身份与文本载荷都必须读回");
}

/// 已闭合缺口（原 C0 固化项翻转）：dev `e9f0aa75` 冻结 ABI 载体后，std 依赖在场时
/// `Option<结构体>` 的 `Some` 构造与 `[export]` 返回位置完成正式构建，构造出的变体
/// 在 wasm 语义里确实是 `Some`（`is_some` 经 Node + glue 读回 `true`）。
///
/// 已知缺口（X16 宿主边界序列化/物化一族）：`Option<Endpoint>` 的 payload **尚未物化**。
/// 冻结布局把它派生为引用存储名义实例 `InstanceHandle("Option")`，`Some` 与 `None`
/// 都返回不透明 GC 引用；JS glue 只能把 symbol 键句柄 JSON 投影成空对象，既读不到
/// 字段，也区分不出变体（`None` 在本载体下同样读回空对象，**不**失败闭合）。
/// 因此本测试不断言该形状——那会把缺陷固化成规格；当前只断言读回成功且不是 `null`。
///
/// TODO(X16 宿主边界序列化/物化)：payload 物化落地后，把本测试里的占位断言
/// （`tests/asgard/config_probe.rs` 中 `assert_ne!(value, "null", ..)` 那行，第 501 行）
/// 换成真实字段断言：
///   `assert_eq!(value, r#"{"host":"127.0.0.1","port":3000}"#)`（Option wire：
///   `None → null`、`Some(v) → v`）
#[test]
fn c0_gap_std_option_struct_constructor_resolves_but_payload_readback_is_pending() {
    let artifact = build_probe_project_with_std(
        "asgard-c0-std-option-struct",
        r#"using std::types::{Option, Some, None};

structure Endpoint {
    host: utf8
    port: u32
}

[export(name: "with_value")]
micro with_value() -> Option<Endpoint> {
    return Some(Endpoint { host: "127.0.0.1", port: 3000 })
}

[export(name: "has_value")]
micro has_value() -> bool {
    return with_value().is_some()
}
"#,
    )
    .unwrap_or_else(|failure| panic!("`Option<结构体>` 构造必须完成正式构建，实际失败：{}", failure.message));

    let has_value = call_export_through_glue(&artifact, "has_value", "[]")
        .unwrap_or_else(|diagnostic| panic!("`Option<结构体>` 的 `is_some` 必须经 glue 读回，实际失败：{diagnostic}"));
    assert_eq!(has_value, "true", "构造 `Some(Endpoint {{ .. }})` 后 `is_some` 必须是 `true`");

    // payload 物化未实现：只验证 `Some` 产出非空引用，不断言不透明句柄的 JSON 形状。
    let value = call_export_through_glue(&artifact, "with_value", "[]")
        .unwrap_or_else(|diagnostic| panic!("`Option<结构体>` 必须经 glue 读回，实际失败：{diagnostic}"));
    assert_ne!(value, "null", "`Some` 必须读回非空引用；payload 物化未实现，不透明句柄形状不作断言");
}

/// 缺口固化：裸对象字面量不是合法表达式，模块尾表达式作者形态尚不可解析。
#[test]
fn c0_gap_bare_object_literal_tail_is_not_parseable() {
    let failure = build_probe_project(
        "asgard-c0-von-literal",
        r#"{
    artifact: {
        target: "static"
        entry: "source/index.w"
    }
}
"#,
    )
    .expect_err("裸对象字面量尾表达式当前必须解析失败");
    assert!(
        failure.message.contains("valkyrie.frontend.parse.syntax"),
        "诊断必须携带解析阶段代码：{}",
        failure.message
    );
}

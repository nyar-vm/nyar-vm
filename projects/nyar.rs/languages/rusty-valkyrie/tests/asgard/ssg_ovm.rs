//! `codegen::ssg_ovm` 块 1 测试：准入门裁决、OVM 端到端渲染与新旧路径对照。
//!
//! 准入子集（E1）：静态元素/文本、单段插值与属性绑定（字面量、已解析绑定）。
//! 越出子集一律结构化诊断；`isActive(...)` 任何时期显式拒绝；
//! 未解析标识符按 V 语义失败闭合，不返回空值（与旧解释器的语义收紧点，逐条记录）。

use rusty_valkyrie::asgard::{
    awsl::{LoweringOptions, compile::compile_awsl_source},
    codegen::ssg_ovm::{AdmissionRule, check_component, render_static_ovm},
    ssg::render_awsl_static,
};
use tempfile::TempDir;

use crate::support::write_legion_project;

fn lower(source: &str) -> rusty_valkyrie::asgard::awsl::LoweredComponent {
    compile_awsl_source(source, "Test", "test.awsl", &LoweringOptions::default()).expect("AWSL 降级必须成功")
}

fn rules(source: &str) -> Vec<AdmissionRule> {
    check_component(&lower(source)).into_iter().map(|diagnostic| diagnostic.rule).collect()
}

/// 准入子集内的最小模板：元素、静态属性、单段插值与动态属性。
const E1_FIXTURE: &str = r#"<widget test>
<Column class="hero" id="landing">
    <Text class="hero-title">{greeting}</Text>
    <a class="cta" :href="link">进入</a>
    <input class="flag" :value="flag" />
</Column>
</widget>
<script>
let greeting: utf8 = "你好「世界」 & 大家"
let link: utf8 = "/docs"
let flag: utf8 = ""
</script>
"#;

#[test]
fn e1_subset_template_is_admitted() {
    assert!(rules(E1_FIXTURE).is_empty(), "E1 模板必须准入: {:?}", rules(E1_FIXTURE));
}

#[test]
fn if_is_admitted_while_loop_and_component_are_rejected_as_out_of_subset() {
    // `<if>`/`<else>` 已迁入准入子集：bool 条件（字面量或 bool 绑定）放行。
    assert!(rules("<widget test><if cond=\"{true}\">x</if></widget>").is_empty());
    assert!(
        rules("<widget test><if cond=\"{flag}\">x</if></widget>\n<script>\nlet flag: bool = true\n</script>").is_empty(),
        "<if> 的 bool 绑定条件必须准入"
    );
    assert!(rules("<widget test><loop item=\"x\" :each=\"{items}\">x</loop></widget>").contains(&AdmissionRule::OutOfSubsetConstruct));
    assert!(rules("<widget test><Other /></widget>").contains(&AdmissionRule::OutOfSubsetConstruct));
}

#[test]
fn is_active_is_always_explicitly_rejected() {
    assert!(rules("<widget test><Text>{isActive(\"/\")}</Text></widget>").contains(&AdmissionRule::IsActiveRejected));
}

#[test]
fn unknown_identifier_fails_closed_instead_of_silent_null() {
    let diagnostics = check_component(&lower("<widget test><Text>{missing}</Text></widget>"));
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.rule == AdmissionRule::UnknownIdentifier && diagnostic.target == "missing"));
}

#[test]
fn operators_ternary_and_calls_are_out_of_subset_expressions() {
    assert!(rules("<widget test><Text>{!a}</Text></widget>").contains(&AdmissionRule::UnsupportedExpression));
    // 三元不是 V 表达式：插值内容必须由共享表达式解析器给出真实表达式树，
    // 非法写法在解析期结构化失败，不得产出半成品 AST（§4.10 E）。
    let ternary = oak_awsl::AwslParser::parse_root(r#"<widget test><Text>{a ? "x" : "y"}</Text></widget>"#);
    assert!(ternary.is_err(), "非法插值表达式必须在解析期失败");
    assert!(rules("<widget test><Text>{a + \"!\"}</Text></widget>").contains(&AdmissionRule::UnsupportedExpression));
    assert!(rules("<widget test><Text>{user.name}</Text></widget>").contains(&AdmissionRule::UnsupportedExpression));
}

#[test]
fn mixed_text_is_admitted_as_sibling_nodes_but_attr_template_concat_is_rejected() {
    // 静态与插值混排的文本在 RenderIR 中是兄弟文本节点，逐段准入。
    assert!(rules("<widget test><Text>你好 {name}</Text></widget>\n<script>\nlet name: utf8 = \"世界\"\n</script>").is_empty());
    // 属性模板值的混合段需要字符串拼接（块 4），当前显式拒绝。
    let diagnostics =
        check_component(&lower("<widget test><a href=\"/u/{name}\">进入</a></widget>\n<script>\nlet name: utf8 = \"x\"\n</script>"));
    assert!(diagnostics.iter().any(|diagnostic| diagnostic.rule == AdmissionRule::UnsupportedConcat), "混合属性模板必须拒绝: {diagnostics:?}");
}

#[test]
fn aggregate_binding_initializer_reports_type_system_gap() {
    let diagnostics =
        check_component(&lower("<widget test><Text>{tabs}</Text></widget>\n<script>\nlet tabs: list = [\"a\", \"b\"]\n</script>"));
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic.rule == AdmissionRule::UnsupportedBindingInit && diagnostic.target.contains("tabs"))
    );
}

#[test]
fn nesting_beyond_limit_is_rejected() {
    let mut source = String::from("<widget test>");
    for _ in 0..70 {
        source.push_str("<div>");
    }
    source.push_str("leaf");
    for _ in 0..70 {
        source.push_str("</div>");
    }
    source.push_str("</widget>");
    assert!(rules(&source).contains(&AdmissionRule::NestingDepthExceeded));
}

/// 端到端：同一模板在新旧两条路径的产物必须逐字节相同（交付形状对照）。
#[test]
fn ovm_render_matches_legacy_oracle_byte_for_byte() {
    let temp = TempDir::new().expect("创建临时目录失败");
    write_workspace_fixture(temp.path());
    let scratch = temp.path().join(".cache").join("asgard-ssg");

    let component = lower(E1_FIXTURE);
    let rendered = render_static_ovm(&component, temp.path(), &scratch, "test").expect("OVM SSG 渲染必须成功");
    let oracle = render_awsl_static(E1_FIXTURE, "Test", "test.awsl", &serde_json::Value::Null).expect("render awsl static");

    assert_eq!(rendered.html, oracle.html, "新旧路径页体必须逐字节一致");
    assert_eq!(rendered.css, oracle.css, "样式必须一致");
    assert_eq!(rendered.scope, oracle.scope, "作用域前缀必须一致");
    assert!(rendered.diagnostics.is_empty());
    assert!(rendered.html.contains("你好「世界」 &amp; 大家"), "文本转义必须保持: {}", rendered.html);
    assert!(rendered.html.contains("<input class=\"flag\" />"), "空值 input 属性必须省略: {}", rendered.html);
}

/// 门禁反例：不声明 `asgard.ssg.dom` 的普通 ovm 构建引用合同族必须失败。
/// 该行为由独立探针工程验证（feature 只能经 `BuildRequest` 声明，CLI 无此入口）；
/// 这里验证 SSG 路径自身声明 feature 后 provider 绑定存在。
#[test]
fn ssg_build_binds_the_gated_provider() {
    let temp = TempDir::new().expect("创建临时目录失败");
    write_workspace_fixture(temp.path());
    let scratch = temp.path().join(".cache").join("asgard-ssg");
    let component = lower(E1_FIXTURE);
    render_static_ovm(&component, temp.path(), &scratch, "test").expect("SSG 构建必须选中门禁 provider");
    assert!(
        scratch.join("test.odin").exists() || scratch.read_dir().map(|mut entries| entries.next().is_some()).unwrap_or(false),
        "SSG 编译必须产生中间产物"
    );
}

fn write_workspace_fixture(project: &std::path::Path) {
    write_legion_project(project, "ssg-ovm-test");
}

use nyar_analyzer::report::{ColSeriesItem, series_to_init_literal};
use oak_awsl::AwslParser;
use rusty_valkyrie::asgard::{
    awsl::{LoweringOptions, lower_component},
    codegen::{ScalarBootstrap, build_awsl_wasm_source},
};

fn chart_script(series_literal: &str) -> String {
    format!(
        r#"<widget chart_status>
<Column>
    <loop item in series>
        <Text class="row">{{item.label}}</Text>
    </loop>
</Column>
</widget>
<script>
let title: utf8 = "T"
let series: [asgard.plotter::ColSeriesItem] = {series_literal}
</script>"#
    )
}

fn sample_series() -> Vec<ColSeriesItem> {
    vec![ColSeriesItem { key: "pass".into(), label: "pass".into(), value_text: "2.0 tests".into(), fill: "#22c55e".into(), height_pct: 100.0 }]
}

/// 报告构建注入的 `series` 字面量必须是具名结构字面量，且元素类型来自声明。
///
/// 匿名结构字面量（`[{ key: … }]`）在 oak-vx 侧不是合法表达式：整体 ABI 提取失败，
/// `series` 派生绑定随之消失，引用处只剩未声明标识符。此测试锁定生成字面量与 `series`
/// 声明组合后必须保留派生绑定。
#[test]
fn named_series_literal_keeps_derived_binding() {
    let literal = series_to_init_literal(&sample_series(), "asgard.plotter::ColSeriesItem").expect("具名字面量");
    let source = chart_script(&literal);
    let root = AwslParser::parse_root(&source).expect("具名字面量 script 必须可解析");
    let lowered = lower_component(&root, "chart_status", "chart-status.awsl", &LoweringOptions::default()).expect("lower");
    assert!(
        lowered.script_bindings.iter().any(|binding| binding.name == "series"),
        "具名字面量必须保留 `series` 派生绑定，实际绑定: {:?}",
        lowered.script_bindings.iter().map(|binding| binding.name.as_str()).collect::<Vec<_>>()
    );
    let generated = build_awsl_wasm_source(std::slice::from_ref(&lowered), ScalarBootstrap::ClosedUnit).expect("生成 V 源码").source;
    assert!(
        generated.contains("asgard.plotter::ColSeriesItem {"),
        "生成的 V 源码必须保留具名结构字面量（绑定按使用点内联）"
    );
}

/// 反向锚点：匿名字面量形态会导致 `series` 绑定丢失，禁止回退到该形态。
#[test]
fn anonymous_series_literal_drops_derived_binding() {
    let source = chart_script(r##"[{ key: "pass", label: "pass", value_text: "2.0 tests", fill: "#22c55e", height_pct: 100.0 }]"##);
    let Ok(root) = AwslParser::parse_root(&source)
    else {
        return;
    };
    let Ok(lowered) = lower_component(&root, "chart_status", "chart-status.awsl", &LoweringOptions::default())
    else {
        return;
    };
    assert!(
        !lowered.script_bindings.iter().any(|binding| binding.name == "series"),
        "匿名字面量若仍能解析出 `series` 绑定，说明反向锚点失效，需要重新取证"
    );
}

/// 生成源码没有模块级 `let`（HIR 会静默丢弃模块级绑定）；非响应式绑定在模板表达式之外的
/// `micro` 函数体内同样必须按使用点内联，否则函数体留下未声明标识符。
#[test]
fn non_reactive_binding_is_inlined_inside_script_micro() {
    let source = r#"<widget index>
<Column>
    <Text>{series[0]}</Text>
</Column>
</widget>
<script>
let series = ["第一条", "第二条"]
micro show(index) {
    return series[index]
}
</script>"#;
    let root = AwslParser::parse_root(source).expect("parse");
    let lowered = lower_component(&root, "index", "index.awsl", &LoweringOptions::default()).expect("lower");
    let generated = build_awsl_wasm_source(&[lowered], ScalarBootstrap::ClosedUnit).expect("generate awsl wasm source").source;
    let start = generated.find("micro show").expect("script micro 必须生成");
    let end = start + generated[start..].find("\n}\n").expect("script micro 必须闭合");
    let block = &generated[start..end];
    assert!(!block.contains("series"), "`micro` 体内不得残留未声明的非响应式绑定:\n{block}");
    assert!(block.contains("\"第一条\""), "`micro` 体内必须内联绑定初始化表达式:\n{block}");
}

/// 绑定替换按字节扫描目标表达式，但复制时必须按 UTF-8 字符边界前进。
/// 逐字节 `as char` 重编码会把 `⁅`/`⁆`、中文等非 ASCII 文本破坏成乱码。
#[test]
fn non_ascii_expression_survives_binding_inlining() {
    let source = r#"<widget index>
<Column>
    <Text>{series[0]}</Text>
</Column>
</widget>
<script>
let series = ["1", "2"]
micro show(index: usize) {
    return series⁅index⁆
}
</script>"#;
    let root = AwslParser::parse_root(source).expect("parse");
    let lowered = lower_component(&root, "index", "index.awsl", &LoweringOptions::default()).expect("lower");
    let generated = build_awsl_wasm_source(&[lowered], ScalarBootstrap::ClosedUnit).expect("generate awsl wasm source").source;
    assert!(generated.contains("⁅index⁆"), "非 ASCII 运算符必须按 UTF-8 原样保留:\n{generated}");
}

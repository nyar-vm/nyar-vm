use oak_awsl::AwslParser;
use rusty_valkyrie::asgard::{
    awsl::{LoweringOptions, lower_component},
    codegen::build_awsl_wasm_source,
};

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
    let generated = build_awsl_wasm_source(&[lowered]).expect("generate awsl wasm source");
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
    let generated = build_awsl_wasm_source(&[lowered]).expect("generate awsl wasm source");
    assert!(generated.contains("⁅index⁆"), "非 ASCII 运算符必须按 UTF-8 原样保留:\n{generated}");
}

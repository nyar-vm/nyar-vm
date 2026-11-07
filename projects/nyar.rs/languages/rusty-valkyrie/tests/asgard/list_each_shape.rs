//! `each` 列表求值回归：模板列表/聚合表达式必须在有限步内收敛，
//! 未解析的属性路径与裸标识符不得触发无界递归；深嵌套必须返回结构化诊断。

use rusty_valkyrie::asgard::{
    codegen::{
        expr_eval::{EvalDiagnosticKind, ExprContext, MAX_EXPRESSION_EVALUATION_DEPTH},
        html_render::MAX_RENDER_NESTING_DEPTH,
    },
    ssg::render_awsl_static,
};
use serde_json::{Value, json};

fn render(source: &str, data: &Value) -> rusty_valkyrie::asgard::codegen::StaticRenderResult {
    render_awsl_static(source, "Test", "test.awsl", data).expect("render awsl static")
}

/// 最小复现：内层 `each` 引用的属性在部分条目上缺失。
#[test]
fn nested_each_with_missing_property_terminates() {
    let source = r#"<widget>
<loop section in sections>
    <loop page in section.pages>
        <span>{page.title}</span>
    </loop>
</loop>
</widget>"#;
    let data = json!({ "sections": [{ "title": "没有 pages 字段" }] });
    let result = render(source, &data);
    assert_eq!(result.html.matches("<span>").count(), 0, "缺失列表属性的条目必须收敛为空列表:\n{}", result.html);
    assert!(result.diagnostics.is_empty(), "缺失属性不是深度问题，不得产生诊断: {:?}", result.diagnostics);
}

/// 未解析的裸标识符同样必须收敛为空值，不能递归回自身。
#[test]
fn unresolved_bare_identifier_terminates() {
    let ctx = ExprContext::from_bindings(&[], &Value::Null);
    assert_eq!(ctx.eval_value("missing"), Value::Null);
    assert!(ctx.diagnostics().is_empty(), "收敛路径不得产生诊断");
}

/// 已解析对象的缺失属性路径必须返回空值。
#[test]
fn unresolved_dotted_path_terminates() {
    let ctx = ExprContext::from_bindings(&[], &json!({ "section": { "title": "x" } }));
    assert_eq!(ctx.eval_value("section.pages"), Value::Null);
    assert!(ctx.diagnostics().is_empty(), "收敛路径不得产生诊断");
}

/// 50 项平铺列表：元素数量、文本内容与顺序逐项成立。
#[test]
fn flat_each_renders_fifty_items_in_order() {
    let items = (0..50).map(|index| format!("item-{index:02}")).collect::<Vec<_>>();
    let source = r#"<widget>
<loop item="entry" :each="{items}">
<span class="entry">{entry}</span>
</loop>
</widget>"#;
    let result = render(source, &json!({ "items": items }));
    assert_eq!(result.html.matches("<span class=\"entry\">").count(), 50, "50 项列表必须逐项展开:\n{}", result.html);
    let expected = (0..50).map(|index| format!("<span class=\"entry\">item-{index:02}</span>")).collect::<String>();
    assert!(result.html.contains(&expected), "列表必须按声明顺序输出全部文本:\n{}", result.html);
}

/// 空列表不得产出任何迭代 DOM。
#[test]
fn empty_each_renders_no_items() {
    let source = r#"<widget>
<loop item="entry" :each="{items}">
<span class="entry">{entry}</span>
</loop>
</widget>"#;
    let result = render(source, &json!({ "items": [] }));
    assert!(!result.html.contains("class=\"entry\""), "空列表不得产出迭代节点:\n{}", result.html);
}

/// 两层嵌套 `each`：外层顺序、内层顺序与归属关系都必须是确定的。
#[test]
fn nested_each_preserves_section_and_entry_order() {
    let source = r#"<widget>
<loop item="section" :each="{sections}">
<div class="section"><span class="section-title">{section.title}</span><loop item="entry" :each="{section.entries}"><span class="entry">{entry}</span></loop></div>
</loop>
</widget>"#;
    let data = json!({
        "sections": [
            { "title": "A", "entries": ["a1", "a2", "a3"] },
            { "title": "B", "entries": ["b1"] },
            { "title": "C", "entries": [] }
        ]
    });
    let result = render(source, &data);
    let expected = concat!(
        "<div class=\"section\"><span class=\"section-title\">A</span>",
        "<span class=\"entry\">a1</span><span class=\"entry\">a2</span><span class=\"entry\">a3</span></div>",
        "<div class=\"section\"><span class=\"section-title\">B</span><span class=\"entry\">b1</span></div>",
        "<div class=\"section\"><span class=\"section-title\">C</span></div>"
    );
    assert!(result.html.contains(expected), "嵌套 each 必须保持外层与内层的声明顺序:\n{}", result.html);
    assert_eq!(result.html.matches("class=\"section\"").count(), 3);
    assert_eq!(result.html.matches("class=\"entry\"").count(), 4);
    assert!(result.diagnostics.is_empty(), "正常嵌套列表不得产生诊断: {:?}", result.diagnostics);
}

/// 超过嵌套上限的列表必须结构化失败：跳过超限子树并携带诊断，而不是栈溢出。
#[test]
fn deeply_nested_each_reports_structured_diagnostic() {
    let depth = MAX_RENDER_NESTING_DEPTH + 4;
    let mut source = String::from("<widget>");
    for level in 0..depth {
        let items = if level == 0 { "levels".to_string() } else { format!("l{}", level - 1) };
        source.push_str(&format!("<loop item=\"l{level}\" :each=\"{{{items}}}\">"));
    }
    source.push_str("<span class=\"deep\">leaf</span>");
    for _ in 0..depth {
        source.push_str("</loop>");
    }
    source.push_str("</widget>");

    let mut levels = json!(["leaf"]);
    for _ in 0..depth {
        levels = json!([levels]);
    }
    let result = render(&source, &json!({ "levels": levels }));
    assert!(
        result
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.kind == EvalDiagnosticKind::RenderNestingExceeded && diagnostic.limit == MAX_RENDER_NESTING_DEPTH }),
        "超过嵌套上限必须产生结构化诊断: {:?}",
        result.diagnostics
    );
    assert!(!result.html.contains("leaf"), "超限子树必须被跳过而不是继续展开:\n{}", result.html);
}

/// 超过求值递归上限的表达式必须结构化失败，而不是把宿主进程栈打穿。
#[test]
fn deep_expression_reports_structured_diagnostic() {
    let ctx = ExprContext::from_bindings(&[], &json!({ "active": true }));
    let expression = format!("{}active", "!".repeat(MAX_EXPRESSION_EVALUATION_DEPTH + 2));
    ctx.eval_value(&expression);
    let diagnostics = ctx.diagnostics();
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EvalDiagnosticKind::ExpressionDepthExceeded && diagnostic.limit == MAX_EXPRESSION_EVALUATION_DEPTH
        }),
        "超过求值上限必须产生结构化诊断: {diagnostics:?}"
    );
}

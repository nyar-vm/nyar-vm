use oak_core::Token;
use oak_valkyrie::lexer::{ValkyrieKeywords, token_type::ValkyrieTokenType};
use rusty_valkyrie::compiler::hir::lowering::{eval_const_condition, parse_const_loop_range, parse_const_range_text};

fn tokens_from(_source: &str, kinds: &[(ValkyrieTokenType, std::ops::Range<usize>)]) -> Vec<Token<ValkyrieTokenType>> {
    kinds.iter().map(|(kind, span)| Token { kind: *kind, span: oak_core::Range { start: span.start, end: span.end } }).collect()
}

#[test]
fn range_i64_iterates_twice_for_zero_to_two() {
    let range = parse_const_range_text("0..2").expect("range");
    assert_eq!(range, 0..2);
    assert_eq!(range.count(), 2);
}

#[test]
fn parse_loop_range_from_parsed_header() {
    use oak_valkyrie::ast::{StatementNode, TemplateNode};
    use rusty_valkyrie::compiler::syntax::oak::{ast::Statement, parse_source};
    let source = r#"micro main() -> i32 {
<% loop _ in 0..2 %>
return 1
<% end %>
}"#;
    let root = parse_source(source).expect("parse");
    let StatementNode::Micro(micro) = &root.items[0]
    else {
        panic!("expected micro")
    };
    let Statement::Template(template) = &micro.body.statements[0]
    else {
        panic!("expected template")
    };
    let TemplateNode::Loop(loop_node) = template.as_ref()
    else {
        panic!("expected loop")
    };
    assert_eq!(parse_const_loop_range(source, &loop_node.header, "native").expect("range"), 0..2);
}

#[test]
fn parse_loop_range_from_compact_literal() {
    let source = "_ in 0..2";
    let tokens = tokens_from(
        source,
        &[
            (ValkyrieTokenType::Underscore, 0..1),
            (ValkyrieTokenType::Keyword(ValkyrieKeywords::In), 2..4),
            (ValkyrieTokenType::FloatLiteral, 5..9),
        ],
    );
    assert_eq!(parse_const_loop_range(source, &tokens, "native").expect("range"), 0..2);
}

#[test]
fn eval_arch_equality_predicate() {
    let source = r#"arch == "wasm32""#;
    let tokens = tokens_from(
        source,
        &[(ValkyrieTokenType::Identifier, 0..4), (ValkyrieTokenType::EqEq, 5..7), (ValkyrieTokenType::StringLiteral, 8..16)],
    );
    assert!(eval_const_condition(source, &tokens, "wasm32").expect("eval"));
    assert!(!eval_const_condition(source, &tokens, "native").expect("eval"));
}

/// 模板常量位置必须与表达式捕获消费同一套转义解码，不能只剥首尾引号。
fn eval_arch_predicate_with_literal(literal: &str, arch: &str) -> Result<bool, rusty_valkyrie::HirError> {
    let source = format!("arch == {literal}");
    let literal_start = source.find(literal).expect("literal in source");
    let tokens = tokens_from(
        &source,
        &[
            (ValkyrieTokenType::Identifier, 0..4),
            (ValkyrieTokenType::EqEq, 5..7),
            (ValkyrieTokenType::StringLiteral, literal_start..literal_start + literal.len()),
        ],
    );
    eval_const_condition(&source, &tokens, arch)
}

#[test]
fn eval_arch_equality_decodes_unicode_escapes() {
    // 只剥引号的旧实现会把 `\u0033\u0032` 当字面文本，永远匹配不上 `wasm32`。
    assert!(eval_arch_predicate_with_literal(r#""wasm\u0033\u0032""#, "wasm32").expect("eval"));
    assert!(!eval_arch_predicate_with_literal(r#""wasm\u0033\u0032""#, "wasm").expect("eval"));
}

#[test]
fn eval_arch_equality_decodes_brace_escapes() {
    assert!(eval_arch_predicate_with_literal(r#""a\{b\}""#, "a{b}").expect("eval"));
}

#[test]
fn eval_arch_equality_rejects_invalid_escape() {
    assert!(eval_arch_predicate_with_literal(r#""a\q""#, "a\\q").is_err(), "未知转义必须结构化失败，不得原样比较");
}

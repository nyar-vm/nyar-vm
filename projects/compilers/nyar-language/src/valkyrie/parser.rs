//! Valkyrie 文本解析入口。
//!
//! 正式编译流只能调用 [`parse`] / [`parse_source`]，不得引入第二条 parser 成功路径。

pub use crate::valkyrie::frontend::{self, ValkyrieRoot, parse_source, std_range};
pub use oak_core::OakDiagnostics;

use oak_core::{Builder, ParseSession, SourceText};
use oak_valkyrie::{ValkyrieBuilder, ValkyrieLanguage};

/// Oak 前端返回的解析结果（含 diagnostics）。
pub type ParseOutput = OakDiagnostics<frontend::ast::ValkyrieRoot>;

/// 使用 Oak 解析并构造 Valkyrie AST 根节点。
pub fn parse(source: &str) -> ParseOutput {
    let language = ValkyrieLanguage::default();
    let builder = ValkyrieBuilder::new(&language);
    let text = SourceText::new(source);
    let mut session = ParseSession::<ValkyrieLanguage>::default();
    builder.build(&text, &[], &mut session)
}

#[cfg(test)]
mod tests {
    use super::{frontend, parse};

    #[test]
    fn parses_minimal_micro_with_oak() {
        let output = parse("micro main() -> i32 { return 1 }");
        let root = output.result.expect("Oak 应构造 Valkyrie AST");
        assert!(!root.items.is_empty());
    }

    #[test]
    fn preserves_call_and_numeric_expression_structure() {
        let output = parse("micro main() -> i32 { let value = add(1, 2); return value }");
        let root = output.result.expect("Oak 应构造调用 AST");
        let frontend::ast::StatementNode::Micro(micro) = &root.items[0]
        else {
            panic!("缺少 micro 声明");
        };
        let frontend::ast::Statement::Let(binding) = &micro.body.statements[0]
        else {
            panic!("缺少 let 绑定");
        };
        let Some(frontend::ast::TermExpression::ApplyCall { callee, args, .. }) = binding.expr.as_ref()
        else {
            panic!("调用没有保留为 Oak ApplyCall");
        };
        assert!(matches!(callee.as_ref(), frontend::ast::TermExpression::NamePath(_)));
        assert!(matches!(args[0], frontend::ast::TermExpression::IntegerLiteral { .. }));
        assert!(matches!(args[1], frontend::ast::TermExpression::IntegerLiteral { .. }));
    }

    #[test]
    fn parses_tls_style_uninitialized_let_with_branch_assignments() {
        let source = r#"micro wrap_tls(config: i32) -> i32 {
    let handle: i32;
    if config > 0 {
        handle = 1
    } else {
        handle = 2
    }
    return handle
}"#;
        let output = parse(source);
        output.result.expect("tls 风格的未初始化 let + 分支赋值应可解析");
    }

    #[test]
    fn parses_uninitialized_typed_let_binding() {
        let output = parse("micro main() -> i32 { let handle: i32; handle = 1; return handle }");
        let root = output.result.expect("Oak 应接受类型注解且无初始化表达式的 let");
        let frontend::ast::StatementNode::Micro(micro) = &root.items[0]
        else {
            panic!("缺少 micro 声明");
        };
        let frontend::ast::Statement::Let(binding) = &micro.body.statements[0]
        else {
            panic!("缺少 let 绑定");
        };
        assert!(binding.expr.is_none());
        assert!(binding.ty.is_some());
    }

    #[test]
    fn parses_nullable_suffix_return_type() {
        let output = parse("micro parse_i32(s: utf8) -> i64? { return null }");
        output.result.expect("后缀 `?` 可空返回类型应可解析");
    }

    #[test]
    #[ignore = "contract-gate: SMIR003 / identity pipeline pending"]
    fn parses_generic_and_structure_declarations() {
        let output = parse("class Box[T] { value: T } micro identity[T](value: T) -> T { return value }");
        let root = output.result.expect("Oak 应构造泛型和结构 AST");
        assert!(root.items.iter().any(|item| matches!(item, frontend::ast::StatementNode::Class(_))));
        assert!(root.items.iter().any(|item| matches!(item, frontend::ast::StatementNode::Micro(_))));
    }
}

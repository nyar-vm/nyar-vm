//! 在 HIR lowering 前按 `arch` 展开 Oak 结构化 TGrammar 节点。

use crate::compiler::{
    hir::lowering::HirError,
    syntax::oak::{
        ast::{
            Block, MethodDeclaration, MicroDeclaration, Statement, StatementNode, StructureBody, TemplateIf, TemplateLoop, TemplateMatch,
            TemplateNode, ValkyrieRoot,
        },
        parse_source_with_language_and_identity, production_language,
    },
};

use nyar_types::DiagnosticSpan;

use oak_core::TokenType;

use oak_valkyrie::lexer::token_type::ValkyrieTokenType;

use super::{
    errors::{TemplateConstruct, TemplateFailure},
    template_const::{const_value_to_source, eval_const_condition, eval_const_expr, parse_const_loop_range},
};

/// 默认架构键；单文件 `compile_source` 测试夹具使用。

#[doc(hidden)]
pub const DEFAULT_COMPILE_ARCH: &str = "native";

/// 展开根节点与声明体中的 TGrammar 模板。

#[doc(hidden)]
pub fn expand_templates_in_root(root: &mut ValkyrieRoot, source: &str, arch: &str) -> Result<(), HirError> {
    expand_statement_nodes(&mut root.items, source, arch)?;

    expand_declaration_bodies(&mut root.items, source, arch)?;

    Ok(())
}

fn expand_declaration_bodies(items: &mut [StatementNode], source: &str, arch: &str) -> Result<(), HirError> {
    for item in items {
        match item {
            StatementNode::Micro(micro) => expand_block(&mut micro.body, source, arch)?,

            StatementNode::Class(class) => expand_structure_body(&mut class.body, source, arch)?,

            StatementNode::Structure(structure) => expand_structure_body(&mut structure.body, source, arch)?,

            StatementNode::Singleton(singleton) => expand_structure_body(&mut singleton.body, source, arch)?,

            StatementNode::Trait(trait_declaration) => expand_structure_body(&mut trait_declaration.body, source, arch)?,

            StatementNode::Imply(imply) => {
                for method in &mut imply.methods {
                    if let Some(body) = &mut method.body {
                        expand_block(body, source, arch)?;
                    }
                }
            }

            StatementNode::Namespace(namespace) => {
                if !crate::compiler::syntax::oak::namespace_is_declaration_only(namespace) {
                    expand_declaration_bodies(crate::compiler::syntax::oak::namespace_items_mut(namespace), source, arch)?;
                }
            }

            _ => {}
        }
    }

    Ok(())
}

fn expand_structure_body(body: &mut StructureBody, source: &str, arch: &str) -> Result<(), HirError> {
    let templates = std::mem::take(&mut body.templates);

    // 模板成员与显式成员分列 AST 两处。模板分支体由 Oak 从片段文本重新解析，
    // 分支成员的 span 是片段内偏移；能代表源码位置的是模板节点自身的 span。
    // 因此以显式成员 span 与模板节点 span 为锚点分组，再按锚点恢复源码顺序。
    let mut members: Vec<(usize, Vec<MethodDeclaration>)> =
        std::mem::take(&mut body.methods).into_iter().map(|method| (method.span.start, vec![method])).collect();

    for template in &templates {
        let span = template_span(template);

        let mut expanded = Vec::new();

        for member in expand_template_to_statement_nodes(template, source, arch)? {
            match member {
                StatementNode::Micro(micro) => expanded.push(micro_to_method(*micro)),

                other => {
                    return Err(HirError::template_expansion_failed(
                        TemplateConstruct::DeclarationBody,
                        TemplateFailure::DeclarationMemberUnsupportedNode { kind: statement_node_kind(&other).to_string() },
                        span.clone(),
                    ));
                }
            }
        }

        members.push((span.start, expanded));
    }

    members.sort_by_key(|(start, _)| *start);

    body.methods = members.into_iter().flat_map(|(_, methods)| methods).collect();

    for method in &mut body.methods {
        if let Some(method_body) = &mut method.body {
            expand_block(method_body, source, arch)?;
        }
    }

    Ok(())
}

fn micro_to_method(micro: MicroDeclaration) -> MethodDeclaration {
    MethodDeclaration {
        name: micro.name,
        generics: micro.generics,
        parameters: micro.params,
        return_type: micro.return_type,
        where_constraints: micro.where_constraints,
        body: if micro.is_abstract { None } else { Some(micro.body) },
        annotations: micro.annotations,
        span: micro.span,
    }
}

fn expand_block(block: &mut Block, source: &str, arch: &str) -> Result<(), HirError> {
    expand_statements(&mut block.statements, source, arch)
}

fn expand_statements(statements: &mut Vec<Statement>, source: &str, arch: &str) -> Result<(), HirError> {
    let mut index = 0;

    while index < statements.len() {
        if let Statement::Template(template) = &statements[index] {
            let expanded = expand_template_to_statements(template, source, arch)?;

            let expanded_len = expanded.len();

            statements.remove(index);

            for (offset, statement) in expanded.into_iter().enumerate() {
                statements.insert(index + offset, statement);
            }

            index += expanded_len;
        }
        else {
            index += 1;
        }
    }

    Ok(())
}

fn expand_statement_nodes(items: &mut Vec<StatementNode>, source: &str, arch: &str) -> Result<(), HirError> {
    let mut index = 0;

    while index < items.len() {
        if matches!(items[index], StatementNode::Template(_)) {
            let template = items[index].clone();

            let StatementNode::Template(template) = template
            else {
                unreachable!()
            };

            let expanded = expand_template_to_statement_nodes(&template, source, arch)?;

            let expanded_len = expanded.len();

            items.remove(index);

            for (offset, item) in expanded.into_iter().enumerate() {
                items.insert(index + offset, item);
            }

            index += expanded_len;
        }
        else if let StatementNode::Namespace(namespace) = &mut items[index] {
            if !crate::compiler::syntax::oak::namespace_is_declaration_only(namespace) {
                expand_statement_nodes(crate::compiler::syntax::oak::namespace_items_mut(namespace), source, arch)?;
            }

            index += 1;
        }
        else {
            index += 1;
        }
    }

    Ok(())
}

fn expand_template_to_statement_nodes(template: &TemplateNode, source: &str, arch: &str) -> Result<Vec<StatementNode>, HirError> {
    let span = template_span(template);
    match template {
        TemplateNode::Match(match_node) => {
            let body = select_arch_match_body(match_node, source, arch, span)?;

            let mut items = body;

            expand_statement_nodes(&mut items, source, arch)?;

            Ok(items)
        }

        TemplateNode::If(if_node) => {
            let body = select_if_body(if_node, source, arch)?;

            let mut items = body;

            expand_statement_nodes(&mut items, source, arch)?;

            Ok(items)
        }

        TemplateNode::Loop(loop_node) => expand_loop_body(loop_node, source, arch),

        TemplateNode::Fragment { tokens, .. } => expand_fragment(tokens, source, arch, span),
    }
}

fn expand_template_to_statements(template: &TemplateNode, source: &str, arch: &str) -> Result<Vec<Statement>, HirError> {
    let span = template_span(template);
    expand_template_to_statement_nodes(template, source, arch)?
        .into_iter()
        .map(|item| statement_node_to_statement(item, span.clone()))
        .collect()
}

/// 模板节点的源码跨度。
fn template_span(template: &TemplateNode) -> DiagnosticSpan {
    let span = match template {
        TemplateNode::Fragment { span, .. } => span,
        TemplateNode::If(node) => &node.span,
        TemplateNode::Loop(node) => &node.span,
        TemplateNode::Match(node) => &node.span,
    };
    DiagnosticSpan { start: span.start, end: span.end }
}

fn statement_node_to_statement(item: StatementNode, span: DiagnosticSpan) -> Result<Statement, HirError> {
    match item {
        StatementNode::Let(binding) => Ok(Statement::Let(*binding)),

        StatementNode::ExprStmt(expr_stmt) => Ok(Statement::Expression(expr_stmt)),

        StatementNode::Template(template) => Ok(Statement::Template(template)),

        other => Err(HirError::template_expansion_failed(
            TemplateConstruct::BlockFragment,
            TemplateFailure::BlockFragmentUnsupportedNode { kind: statement_node_kind(&other).to_string() },
            span,
        )),
    }
}

/// 顶层语句节点的稳定类别短名（不使用 `Debug` 输出参与诊断）。
pub(crate) fn statement_node_kind(item: &StatementNode) -> &'static str {
    match item {
        StatementNode::Namespace(_) => "namespace",
        StatementNode::Using(_) => "using",
        StatementNode::Class(_) => "class",
        StatementNode::Structure(_) => "structure",
        StatementNode::Union(_) => "union",
        StatementNode::Singleton(_) => "singleton",
        StatementNode::Flags(_) => "flags",
        StatementNode::Enums(_) => "enums",
        StatementNode::Trait(_) => "trait",
        StatementNode::Widget(_) => "widget",
        StatementNode::Imply(_) => "imply",
        StatementNode::Micro(_) => "micro",
        StatementNode::TypeFunction(_) => "type_function",
        StatementNode::TypeAlias(_) => "type_alias",
        StatementNode::Statement(_) => "statement",
        StatementNode::Variant(_) => "variant",
        StatementNode::Effect(_) => "effect",
        StatementNode::Shader(_) => "shader",
        StatementNode::Component(_) => "component",
        StatementNode::System(_) => "system",
        StatementNode::Let(_) => "let",
        StatementNode::ExprStmt(_) => "expression_statement",
        StatementNode::Template(_) => "template",
    }
}

fn select_if_body(if_node: &TemplateIf, source: &str, arch: &str) -> Result<Vec<StatementNode>, HirError> {
    for arm in &if_node.arms {
        if !arm.header.is_empty() && eval_const_condition(source, &arm.header, arch)? {
            return Ok(arm.body.clone());
        }
    }

    for arm in &if_node.arms {
        if arm.header.is_empty() {
            return Ok(arm.body.clone());
        }
    }

    Ok(Vec::new())
}

fn expand_loop_body(loop_node: &TemplateLoop, source: &str, arch: &str) -> Result<Vec<StatementNode>, HirError> {
    let range = parse_const_loop_range(source, &loop_node.header, arch)?;

    let mut items = Vec::new();

    for _ in range {
        let mut body = loop_node.body.clone();

        expand_statement_nodes(&mut body, source, arch)?;

        items.extend(body);
    }

    Ok(items)
}

fn expand_fragment(
    tokens: &[oak_core::Token<ValkyrieTokenType>],
    source: &str,
    arch: &str,
    span: DiagnosticSpan,
) -> Result<Vec<StatementNode>, HirError> {
    let snippet = const_value_to_source(&eval_const_expr(source, tokens, arch)?);
    if snippet.trim().is_empty() {
        return Ok(Vec::new());
    }
    // 元表达式 splice 发生在语句上下文中，须经块内重解析而非模块根。
    // 包装文本是合成源：诊断身份明确落 `<frontend:template-fragment>`，不冒充真实文件；
    // 模板在来源文件中的站点由上层 `TemplateExpansionFailed` 的 span 单独承担。
    let wrapped = format!("micro __tg_fragment() -> void {{ {snippet}; }}");
    let root = parse_source_with_language_and_identity(&wrapped, &production_language(), "<frontend:template-fragment>")?;
    let fragment_failure = |produced_items: usize| {
        HirError::template_expansion_failed(
            TemplateConstruct::BlockFragment,
            TemplateFailure::FragmentNotMicroDeclaration { produced_items },
            span.clone(),
        )
    };
    let StatementNode::Micro(micro) = root.items.first().ok_or_else(|| fragment_failure(0))?
    else {
        return Err(fragment_failure(root.items.len()));
    };
    Ok(micro.body.statements.iter().cloned().map(statement_to_statement_node).collect())
}

fn statement_to_statement_node(statement: Statement) -> StatementNode {
    match statement {
        Statement::Let(binding) => StatementNode::Let(Box::new(binding)),
        Statement::Expression(expr_stmt) => StatementNode::ExprStmt(expr_stmt),
        Statement::Template(template) => StatementNode::Template(template),
    }
}

fn select_arch_match_body(match_node: &TemplateMatch, source: &str, arch: &str, span: DiagnosticSpan) -> Result<Vec<StatementNode>, HirError> {
    if !is_arch_scrutinee(source, &match_node.header) {
        return Err(HirError::template_expansion_failed(TemplateConstruct::Match, TemplateFailure::NonArchScrutinee, span));
    }

    for arm in &match_node.arms {
        if let Some(pattern) = &arm.pattern {
            if pattern_token_text(source, pattern) == arch {
                return Ok(arm.body.clone());
            }
        }
    }

    for arm in &match_node.arms {
        if arm.pattern.is_none() {
            return Ok(arm.body.clone());
        }
    }

    Ok(Vec::new())
}

fn is_arch_scrutinee(source: &str, header: &[oak_core::Token<ValkyrieTokenType>]) -> bool {
    pattern_token_text(source, header) == "arch"
}

fn pattern_token_text(source: &str, tokens: &[oak_core::Token<ValkyrieTokenType>]) -> String {
    let meaningful: Vec<_> = tokens.iter().filter(|token| !token.kind.is_ignored()).collect();

    match meaningful.as_slice() {
        [token] if token.kind == ValkyrieTokenType::StringLiteral => decode_quoted(source[token.span.clone()].trim()),

        [token] if token.kind == ValkyrieTokenType::Identifier => source[token.span.clone()].trim().to_string(),

        _ => {
            tokens.iter().filter(|token| !token.kind.is_ignored()).map(|token| source[token.span.clone()].trim()).collect::<Vec<_>>().join(" ")
        }
    }
}

fn decode_quoted(text: &str) -> String {
    let text = text.trim();

    if (text.starts_with('"') && text.ends_with('"')) || (text.starts_with('\'') && text.ends_with('\'')) {
        text[1..text.len().saturating_sub(1)].to_string()
    }
    else {
        text.to_string()
    }
}

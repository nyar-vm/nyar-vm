//! Valkyrie 语法前端：Oak 是唯一文本事实源。
//!
//! HIR lowering、Semantic MIR 与跨包链接只能消费本模块产出的 Oak AST，
//! 不得再经 `vcc-data::AstParser` 或任何 legacy AST 桥接层。

mod error;
mod namespace;
mod naming;
mod staging;

pub(crate) use namespace::{namespace_is_declaration_only, namespace_items, namespace_items_mut};

use oak_core::{Builder, ParseSession, SourceText};
use oak_valkyrie::{ValkyrieBuilder, ValkyrieLanguage};

pub use error::ParseError;
pub use naming::{DIAG_ABI_BINDING_NOT_SNAKE_CASE, DIAG_IDENTIFIER_NOT_SNAKE_CASE, NamingViolation, validate_snake_case};
pub use staging::reject_unsupported_template_source;

/// Oak 前端 AST 类型别名，供 lowering 直接引用。
pub use oak_valkyrie::ast;

/// 解析后的 Valkyrie 模块根。
pub type ValkyrieRoot = ast::ValkyrieRoot;

/// 将 Oak span 转为标准区间。
pub fn std_range(span: &oak_core::Range<usize>) -> std::ops::Range<usize> {
    span.start..span.end
}

/// 生产编译使用的 Oak 语言配置。
///
/// `support_t_grammar` 控制 TGrammar 模板节点是否进入 AST；模板展开由
/// Compiler 在结构化节点上按 `CompilerBuildContext.arch` 执行，禁止文本重扫。
pub fn production_language() -> ValkyrieLanguage {
    ValkyrieLanguage { support_t_grammar: true, ..ValkyrieLanguage::default() }
}

/// 使用 Oak 解析源码；失败时返回结构化 `ParseError`，禁止回退旧 parser。
pub fn parse_source(source: &str) -> Result<ValkyrieRoot, ParseError> {
    parse_source_with_language(source, &production_language())
}

/// 使用显式 `ValkyrieLanguage` 配置解析源码。
pub fn parse_source_with_language(source: &str, language: &ValkyrieLanguage) -> Result<ValkyrieRoot, ParseError> {
    if !language.support_t_grammar {
        reject_unsupported_template_source(source)?;
    }
    let builder = ValkyrieBuilder::new(language);
    let text = SourceText::new(source);
    let mut session = ParseSession::<ValkyrieLanguage>::default();
    let output = builder.build(&text, &[], &mut session);
    output.result.map_err(ParseError::from_oak).map(|mut root| {
        normalize_trait_operator_members(&mut root);
        root
    })
}

/// Oak 把 trait 体里的运算符声明（`infix \`==\``）拆成两个成员节点：早先一个名字恰为 fixity
/// 词素、无签名无方法体的占位成员，随后一个以运算符词素命名的签名成员。
///
/// 语言上这是一个成员（`[infix] micro \`==\``），成员名只能由 `declared_member_name` 给出。
/// 在 AST 出口处就地还原，闭包索引/保留与 HIR lowering 才看到同一份 trait 成员；
/// 否则闭包会按 `infix` / `==` 两个错误身份索引，运算符约束的 evidence 永远查不到声明。
fn normalize_trait_operator_members(root: &mut ValkyrieRoot) {
    normalize_operator_members_in_items(&mut root.items);
}

fn normalize_operator_members_in_items(items: &mut [ast::StatementNode]) {
    for item in items {
        match item {
            ast::StatementNode::Trait(trait_decl) => reclaim_trait_operator_members(&mut trait_decl.body.methods),
            ast::StatementNode::Namespace(namespace) => {
                if let Some(body) = namespace.body.as_mut() {
                    normalize_operator_members_in_items(&mut body.items);
                }
            }
            ast::StatementNode::Statement(inner) => normalize_operator_members_in_items(std::slice::from_mut(inner.as_mut())),
            _ => {}
        }
    }
}

/// 按源码顺序把 fixity 占位成员的注解并回紧随其后的签名成员，占位成员本身不再是声明。
fn reclaim_trait_operator_members(methods: &mut Vec<ast::MethodDeclaration>) {
    let mut reclaimed = Vec::with_capacity(methods.len());
    let mut pending_fixity = None;
    for method in std::mem::take(methods) {
        if let Some(fixity) = operator_fixity_placeholder(&method) {
            pending_fixity = Some(fixity);
            continue;
        }
        let mut method = method;
        if let Some(fixity) = pending_fixity.take() {
            method.annotations.insert(0, fixity);
        }
        reclaimed.push(method);
    }
    *methods = reclaimed;
}

/// 只有「fixity 词素 + 空签名 + 空方法体 + 无注解」的成员才是运算符声明的占位。
fn operator_fixity_placeholder(method: &ast::MethodDeclaration) -> Option<ast::Attribute> {
    if !method.parameters.is_empty()
        || !method.generics.is_empty()
        || method.return_type.is_some()
        || method.body.is_some()
        || !method.where_constraints.is_empty()
        || !method.annotations.is_empty()
    {
        return None;
    }
    let fixity = match method.name.name.as_str() {
        "prefix" | "infix" | "postfix" | "suffix" => method.name.name.clone(),
        _ => return None,
    };
    Some(ast::Attribute {
        name: ast::Identifier { name: fixity, span: method.name.span.clone() },
        args: Vec::new(),
        span: method.span.clone(),
    })
}

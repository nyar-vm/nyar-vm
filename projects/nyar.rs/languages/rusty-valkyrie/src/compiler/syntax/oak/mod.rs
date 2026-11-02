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
    // 解析期结构化诊断进同一诊断汇（多诊断报告）。
    // 已核实的 oak 行为（`oak.rs` a202114d）：Valkyrie 文法以 `Result` 早退，实际产出
    // fatal 单错且 `diagnostics` 恒为空；恢复记录接入 oak 后，这里自动带出全部
    // 非致命诊断。此处只转述 oak 已生产的事实，不在外部包装出多错假象。
    for diagnostic in &output.diagnostics {
        crate::compiler::diagnostics::pending::push_compile_diagnostic(compile_diagnostic_from_oak_error(diagnostic));
    }
    output
        .result
        .map_err(|error| {
            // `build` 的 CST→AST 失败分支会把同一错误同时放进 result 与 diagnostics；
            // 只在尚未记录时补记 fatal，避免同一错误出现两次。
            let already_recorded = output
                .diagnostics
                .iter()
                .any(|recorded| recorded.source_offset() == error.source_offset() && recorded.to_string() == error.to_string());
            if !already_recorded {
                crate::compiler::diagnostics::pending::push_compile_diagnostic(compile_diagnostic_from_oak_error(&error));
            }
            let mut primary = ParseError::from_oak(error);
            // 词法诊断（如"未闭合的引号捕获"）携带比兜底解析错误更精确的原因与定位；
            // 失败时必须一并带出，不能只报兜底消息吞掉根因。
            if !output.diagnostics.is_empty() {
                let details = output.diagnostics.iter().map(|diagnostic| diagnostic.to_string()).collect::<Vec<_>>().join("; ");
                if let ParseError::Invalid { message, .. } = &mut primary {
                    *message = format!("{details}; {message}");
                }
            }
            primary
        })
        .map(|mut root| {
            normalize_trait_operator_members(&mut root);
            root
        })
}

/// 把 oak 生产的单条诊断投影为编译诊断汇条目。
///
/// `code` 由 oak 的稳定 i18n key 派生（`error.syntax` → `valkyrie.frontend.parse.syntax`），
/// 不自行枚举错误清单；`phase`/`owner` 按诊断合同诚实记录为语法阶段、oak-valkyrie 生产。
/// 文件身份不在本层可用：source 先落 `<frontend>` 占位（与既有 `FrontendRejected` 行一致），
/// 组管线接入文件身份后应升级为真实路径。
fn compile_diagnostic_from_oak_error(error: &oak_core::OakError) -> crate::compiler::diagnostics::pending::CompileDiagnostic {
    use nyar_types::{DiagnosticOwner, DiagnosticPhase, DiagnosticRow, DiagnosticSeverity, DiagnosticSource, DiagnosticSpan};
    let kind = error.kind();
    let span = match error.source_offset() {
        Some(offset) => DiagnosticSpan { start: offset, end: offset.saturating_add(1) },
        None => DiagnosticSpan { start: 0, end: 0 },
    };
    let code = format!("valkyrie.frontend.parse.{}", kind.key().trim_start_matches("error."));
    let message = match kind {
        oak_core::OakErrorKind::SyntaxError { message, .. }
        | oak_core::OakErrorKind::CustomError { message }
        | oak_core::OakErrorKind::SemanticError { message }
        | oak_core::OakErrorKind::FormatError { message }
        | oak_core::OakErrorKind::ProtocolError { message }
        | oak_core::OakErrorKind::InternalError { message } => message.clone(),
        _ => error.to_string(),
    };
    let row = DiagnosticRow::new(
        code,
        DiagnosticSeverity::Error,
        DiagnosticPhase::Parsing,
        DiagnosticOwner::syntax("oak-valkyrie"),
        DiagnosticSource { id: "<frontend>".into(), revision: None },
        span,
    );
    crate::compiler::diagnostics::pending::CompileDiagnostic::new(row, message)
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
    Some(ast::Attribute { name: ast::Identifier { name: fixity, span: method.name.span.clone() }, args: Vec::new(), span: method.span.clone() })
}

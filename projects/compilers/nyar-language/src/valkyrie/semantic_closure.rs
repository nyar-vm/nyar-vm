//! 入口驱动的语义闭包规划：声明索引与 body lowering 边界。

use std::collections::{BTreeMap, BTreeSet};

use nyar_types::combined_content_hash;

use crate::valkyrie::frontend::{ast, parse_source};

/// 预 HIR 声明种类；与后续 `ItemId` 分配一一对应，不依赖短名认亲。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DeclarationKind {
    Micro,
    Structure,
    Class,
    Trait,
    Enum,
    Union,
    Flags,
    TypeAlias,
    TypeFunction,
    Method,
    ImplyMethod,
}

impl DeclarationKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Micro => "micro",
            Self::Structure => "structure",
            Self::Class => "class",
            Self::Trait => "trait",
            Self::Enum => "enum",
            Self::Union => "union",
            Self::Flags => "flags",
            Self::TypeAlias => "type_alias",
            Self::TypeFunction => "type_function",
            Self::Method => "method",
            Self::ImplyMethod => "imply_method",
        }
    }
}

/// 预 HIR 稳定声明身份：依赖组 + 仓相对路径 + 种类 + 名称。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeclarationKey {
    /// Resolver 分配的依赖组身份。
    pub dependency_key: String,
    /// 仓相对源码路径。
    pub source_path: String,
    /// 顶层声明种类。
    pub kind: DeclarationKind,
    /// 声明短名（namespace 内暂以文件路径消歧）。
    pub name: String,
}

impl DeclarationKey {
    /// 稳定编码身份；Resolver→Compiler 的声明计划键与闭包 digest 使用同一编码。
    pub fn encoded(&self) -> String {
        format!("{}::{}::{}::{}", self.dependency_key, self.source_path, self.kind.as_str(), self.name)
    }
}

/// 单条声明索引：不降低函数体，只记录 import 边界。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationIndexEntry {
    /// 稳定声明身份。
    pub key: DeclarationKey,
    /// 文件级 `using` 路径（仓相对逻辑名）。
    pub import_paths: BTreeSet<String>,
}

/// 单组语义闭包切片。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupSemanticClosure {
    /// 依赖组身份。
    pub dependency_key: String,
    /// 进入 body lowering 的仓相对路径。
    pub body_lowering_source_paths: BTreeSet<String>,
    /// 本组 body lowering 覆盖的声明身份。
    pub body_lowering_declarations: BTreeSet<DeclarationKey>,
}

/// 入口驱动的完整语义闭包：声明索引 + body lowering 边界 + digest。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticClosure {
    /// 各依赖组的 body lowering 切片。
    pub groups: Vec<GroupSemanticClosure>,
    /// 全 inventory 声明索引（含不可达文件）。
    pub declaration_index: BTreeMap<DeclarationKey, DeclarationIndexEntry>,
    /// 入口闭包内被引用的声明身份。
    pub demanded_declarations: BTreeSet<DeclarationKey>,
    /// 闭包稳定摘要。
    pub digest: String,
}

/// 从入口组与其依赖 inventory 规划完整语义闭包。
pub fn plan_semantic_closure(groups: &[(&str, bool, &[(String, String)])]) -> SemanticClosure {
    let indexed_groups = groups
        .iter()
        .map(|(dependency_key, is_entry, files)| {
            let indexed = files
                .iter()
                .map(|(path, source)| index_source_file(dependency_key, path, source))
                .collect::<Vec<_>>();
            (*dependency_key, *is_entry, indexed)
        })
        .collect::<Vec<_>>();

    let mut declaration_index = BTreeMap::new();
    for (_, _, files) in &indexed_groups {
        for file in files {
            for entry in &file.entries {
                declaration_index.insert(entry.key.clone(), entry.clone());
            }
        }
    }

    let mut demanded_symbols = BTreeSet::new();
    for (_, is_entry, files) in &indexed_groups {
        if !*is_entry {
            continue;
        }
        for file in files {
            if file.parse_ok {
                demanded_symbols.extend(file.referenced_symbols.iter().cloned());
            }
        }
    }

    let dependency_groups = indexed_groups.iter().filter(|(_, is_entry, _)| !*is_entry).map(|(_, _, files)| files).collect::<Vec<_>>();
    let mut changed = true;
    while changed {
        changed = false;
        for files in &dependency_groups {
            let body_paths = expand_dependency_body_paths(files, &demanded_symbols);
            for symbol in collect_symbols_from_paths(files, &body_paths) {
                if demanded_symbols.insert(symbol) {
                    changed = true;
                }
            }
        }
    }

    let mut closure_groups = Vec::with_capacity(indexed_groups.len());
    let mut demanded_declarations = BTreeSet::new();
    for (dependency_key, is_entry, files) in indexed_groups {
        let body_lowering_source_paths = if is_entry {
            files.iter().map(|file| file.path.clone()).collect()
        }
        else {
            expand_dependency_body_paths(&files, &demanded_symbols)
        };
        let body_lowering_declarations = declaration_keys_for_paths(&files, &body_lowering_source_paths);
        demanded_declarations.extend(body_lowering_declarations.iter().cloned());
        closure_groups.push(GroupSemanticClosure {
            dependency_key: dependency_key.to_string(),
            body_lowering_source_paths,
            body_lowering_declarations,
        });
    }

    let digest = compute_closure_digest(&closure_groups, &demanded_declarations);
    SemanticClosure { groups: closure_groups, declaration_index, demanded_declarations, digest }
}

fn compute_closure_digest(groups: &[GroupSemanticClosure], demanded: &BTreeSet<DeclarationKey>) -> String {
    let mut parts = Vec::new();
    for group in groups {
        parts.push(group.dependency_key.clone());
        for path in &group.body_lowering_source_paths {
            parts.push(path.clone());
        }
        for key in &group.body_lowering_declarations {
            parts.push(key.encoded());
        }
    }
    for key in demanded {
        parts.push(format!("demanded:{}", key.encoded()));
    }
    let borrowed = parts.iter().map(String::as_str).collect::<Vec<_>>();
    combined_content_hash(&borrowed)
}

fn declaration_keys_for_paths(files: &[IndexedFile], paths: &BTreeSet<String>) -> BTreeSet<DeclarationKey> {
    let mut keys = BTreeSet::new();
    for file in files {
        if paths.contains(&file.path) {
            keys.extend(file.entries.iter().map(|entry| entry.key.clone()));
        }
    }
    keys
}

fn expand_dependency_body_paths(files: &[IndexedFile], seeds: &BTreeSet<String>) -> BTreeSet<String> {
    let declaration_to_paths = build_declaration_index(files);
    let mut workset = BTreeSet::new();
    let mut queue: Vec<String> = seeds
        .iter()
        .flat_map(|symbol| declaration_to_paths.get(symbol).into_iter().flatten().cloned())
        .collect();
    while let Some(path) = queue.pop() {
        if !workset.insert(path.clone()) {
            continue;
        }
        if let Some(file) = files.iter().find(|file| file.path == path) {
            for symbol in &file.referenced_symbols {
                if let Some(next_paths) = declaration_to_paths.get(symbol) {
                    for next in next_paths {
                        if !workset.contains(next) {
                            queue.push(next.clone());
                        }
                    }
                }
            }
        }
    }
    workset
}

fn collect_symbols_from_paths(files: &[IndexedFile], paths: &BTreeSet<String>) -> BTreeSet<String> {
    let mut symbols = BTreeSet::new();
    for file in files {
        if paths.contains(&file.path) {
            symbols.extend(file.referenced_symbols.iter().cloned());
        }
    }
    symbols
}

fn build_declaration_index(files: &[IndexedFile]) -> BTreeMap<String, BTreeSet<String>> {
    let mut index: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in files {
        if !file.parse_ok {
            continue;
        }
        for entry in &file.entries {
            index.entry(entry.key.name.clone()).or_default().insert(file.path.clone());
        }
    }
    index
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexedFile {
    path: String,
    parse_ok: bool,
    entries: Vec<DeclarationIndexEntry>,
    referenced_symbols: BTreeSet<String>,
}

fn index_source_file(dependency_key: &str, path: &str, source: &str) -> IndexedFile {
    match parse_source(source) {
        Ok(root) => {
            let mut entries = Vec::new();
            let mut referenced_symbols = BTreeSet::new();
            let mut import_paths = BTreeSet::new();
            collect_items(dependency_key, path, &root.items, &mut entries, &mut referenced_symbols, &mut import_paths);
            IndexedFile { path: path.to_string(), parse_ok: true, entries, referenced_symbols }
        }
        Err(_) => IndexedFile {
            path: path.to_string(),
            parse_ok: false,
            entries: Vec::new(),
            referenced_symbols: BTreeSet::new(),
        },
    }
}

fn register_declaration(
    dependency_key: &str,
    source_path: &str,
    kind: DeclarationKind,
    name: &str,
    entries: &mut Vec<DeclarationIndexEntry>,
    import_paths: &BTreeSet<String>,
) {
    entries.push(DeclarationIndexEntry {
        key: DeclarationKey {
            dependency_key: dependency_key.to_string(),
            source_path: source_path.to_string(),
            kind,
            name: name.to_string(),
        },
        import_paths: import_paths.clone(),
    });
}

fn collect_items(
    dependency_key: &str,
    source_path: &str,
    items: &[ast::StatementNode],
    entries: &mut Vec<DeclarationIndexEntry>,
    references: &mut BTreeSet<String>,
    import_paths: &mut BTreeSet<String>,
) {
    for item in items {
        collect_item(dependency_key, source_path, item, entries, references, import_paths);
    }
}

fn collect_item(
    dependency_key: &str,
    source_path: &str,
    item: &ast::StatementNode,
    entries: &mut Vec<DeclarationIndexEntry>,
    references: &mut BTreeSet<String>,
    import_paths: &mut BTreeSet<String>,
) {
    match item {
        ast::StatementNode::Namespace(namespace) => collect_items(dependency_key, source_path, &namespace.items, entries, references, import_paths),
        ast::StatementNode::Using(using) => collect_using(using, references, import_paths),
        ast::StatementNode::Class(class) => {
            register_declaration(dependency_key, source_path, DeclarationKind::Class, &class.name.name, entries, import_paths);
            collect_structure_body(dependency_key, source_path, &class.body, entries, references, import_paths);
        }
        ast::StatementNode::Structure(structure) => {
            register_declaration(dependency_key, source_path, DeclarationKind::Structure, &structure.name.name, entries, import_paths);
            collect_structure_body(dependency_key, source_path, &structure.body, entries, references, import_paths);
        }
        ast::StatementNode::Union(union_def) => {
            register_declaration(dependency_key, source_path, DeclarationKind::Union, &union_def.name.name, entries, import_paths);
            for field in &union_def.body.fields {
                collect_type_expression(&field.typing, references);
            }
        }
        ast::StatementNode::Flags(flags) => {
            register_declaration(dependency_key, source_path, DeclarationKind::Flags, &flags.name.name, entries, import_paths);
        }
        ast::StatementNode::Enums(enums) => {
            register_declaration(dependency_key, source_path, DeclarationKind::Enum, &enums.name.name, entries, import_paths);
            for variant in &enums.variants {
                references.insert(variant.name.name.clone());
            }
        }
        ast::StatementNode::Trait(trait_def) => {
            register_declaration(dependency_key, source_path, DeclarationKind::Trait, &trait_def.name.name, entries, import_paths);
            collect_structure_body(dependency_key, source_path, &trait_def.body, entries, references, import_paths);
        }
        ast::StatementNode::Imply(imply) => collect_imply(dependency_key, source_path, imply, entries, references, import_paths),
        ast::StatementNode::Micro(micro) => {
            register_declaration(dependency_key, source_path, DeclarationKind::Micro, &micro.name.name, entries, import_paths);
            collect_micro(micro, references);
        }
        ast::StatementNode::TypeAlias(alias) => {
            register_declaration(dependency_key, source_path, DeclarationKind::TypeAlias, &alias.name.name, entries, import_paths);
            collect_type_expression(&alias.target, references);
        }
        ast::StatementNode::TypeFunction(type_function) => {
            register_declaration(dependency_key, source_path, DeclarationKind::TypeFunction, &type_function.name.name, entries, import_paths);
            collect_type_function(type_function, references);
        }
        ast::StatementNode::Let(let_stmt) => collect_let(let_stmt, references),
        ast::StatementNode::ExprStmt(expr_stmt) => collect_term(&expr_stmt.expr, references),
        ast::StatementNode::Statement(inner) => collect_item(dependency_key, source_path, inner, entries, references, import_paths),
        _ => {}
    }
}

fn collect_using(using: &ast::UsingDeclaration, references: &mut BTreeSet<String>, import_paths: &mut BTreeSet<String>) {
    let path = using.path.parts.iter().map(|part| part.name.as_str()).collect::<Vec<_>>().join("::");
    import_paths.insert(path);
    collect_name_path(&using.path, references);
    for import in &using.imports {
        references.insert(import.name.clone());
    }
    if let Some(alias) = &using.alias {
        references.insert(alias.name.clone());
    }
}

fn collect_imply(
    dependency_key: &str,
    source_path: &str,
    imply: &ast::ImplyDeclaration,
    entries: &mut Vec<DeclarationIndexEntry>,
    references: &mut BTreeSet<String>,
    import_paths: &BTreeSet<String>,
) {
    collect_type_expression(&imply.target_type, references);
    if let Some(trait_type) = &imply.trait_type {
        collect_type_expression(trait_type, references);
    }
    for method in &imply.methods {
        register_declaration(dependency_key, source_path, DeclarationKind::ImplyMethod, &method.name.name, entries, import_paths);
        collect_method(method, references);
    }
}

fn collect_micro(micro: &ast::MicroDeclaration, references: &mut BTreeSet<String>) {
    for param in &micro.params {
        if let Some(ty) = &param.ty {
            collect_type_expression(ty, references);
        }
    }
    if let Some(return_type) = &micro.return_type {
        collect_type_expression(return_type, references);
    }
    collect_block(&micro.body, references);
}

fn collect_method(method: &ast::MethodDeclaration, references: &mut BTreeSet<String>) {
    for param in &method.parameters {
        if let Some(ty) = &param.ty {
            collect_type_expression(ty, references);
        }
    }
    if let Some(return_type) = &method.return_type {
        collect_type_expression(return_type, references);
    }
    if let Some(body) = &method.body {
        collect_block(body, references);
    }
}

fn collect_type_function(type_function: &ast::TypeFunction, references: &mut BTreeSet<String>) {
    for param in &type_function.params {
        if let Some(ty) = &param.ty {
            collect_type_expression(ty, references);
        }
    }
    if let Some(return_type) = &type_function.return_type {
        collect_type_expression(return_type, references);
    }
    collect_block(&type_function.body, references);
}

fn collect_structure_body(
    dependency_key: &str,
    source_path: &str,
    body: &ast::StructureBody,
    entries: &mut Vec<DeclarationIndexEntry>,
    references: &mut BTreeSet<String>,
    import_paths: &BTreeSet<String>,
) {
    for field in &body.fields {
        collect_type_expression(&field.typing, references);
    }
    for method in &body.methods {
        register_declaration(dependency_key, source_path, DeclarationKind::Method, &method.name.name, entries, import_paths);
        collect_method(method, references);
    }
}

fn collect_let(let_stmt: &ast::Let, references: &mut BTreeSet<String>) {
    if let Some(ty) = &let_stmt.ty {
        collect_type_expression(ty, references);
    }
    if let Some(expr) = &let_stmt.expr {
        collect_term(expr, references);
    }
}

fn collect_block(block: &ast::Block, references: &mut BTreeSet<String>) {
    for statement in &block.statements {
        collect_statement(statement, references);
    }
}

fn collect_statement(statement: &ast::Statement, references: &mut BTreeSet<String>) {
    match statement {
        ast::Statement::Let(let_stmt) => collect_let(let_stmt, references),
        ast::Statement::Expression(expr_stmt) => collect_term(&expr_stmt.expr, references),
        _ => {}
    }
}

fn collect_term(term: &ast::TermExpression, references: &mut BTreeSet<String>) {
    match term {
        ast::TermExpression::ArrayLiteral { items, .. } | ast::TermExpression::TupleLiteral { items, .. } => {
            for item in items {
                collect_term(item, references);
            }
        }
        ast::TermExpression::Unary(node) => collect_term(&node.base, references),
        ast::TermExpression::Binary(node) => {
            collect_term(&node.lhs, references);
            collect_term(&node.rhs, references);
        }
        ast::TermExpression::Cast { expr, ty, .. } => {
            collect_term(expr, references);
            collect_type_expression(ty, references);
        }
        ast::TermExpression::Turbofish { expr, arguments, .. } => {
            collect_term(expr, references);
            for argument in arguments {
                collect_type_expression(argument, references);
            }
        }
        ast::TermExpression::ApplyCall { callee, args, .. } => {
            collect_call_target(callee, references);
            for arg in args {
                collect_term(arg, references);
            }
        }
        ast::TermExpression::DotCall { receiver, field, .. } => {
            collect_term(receiver, references);
            references.insert(field.name.clone());
        }
        ast::TermExpression::Index { receiver, index, .. } => {
            collect_term(receiver, references);
            collect_term(index, references);
        }
        ast::TermExpression::Offset { receiver, offset, .. } => {
            collect_term(receiver, references);
            collect_term(offset, references);
        }
        ast::TermExpression::Group { expr, .. } => collect_term(expr, references),
        ast::TermExpression::Block(block) => collect_block(block, references),
        ast::TermExpression::Micro(micro) => collect_block(&micro.body, references),
        ast::TermExpression::Object { callee, fields, .. } => {
            collect_term(callee, references);
            for (_, value) in fields {
                if let Some(value) = value {
                    collect_term(value, references);
                }
            }
        }
        ast::TermExpression::If { condition, then_branch, else_branch, .. } => {
            collect_term(condition, references);
            collect_block(then_branch, references);
            if let Some(else_branch) = else_branch {
                collect_block(else_branch, references);
            }
        }
        ast::TermExpression::Match { scrutinee, arms, .. } => {
            collect_term(scrutinee, references);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_term(guard, references);
                }
                collect_term(&arm.body, references);
            }
        }
        ast::TermExpression::NamePath(path) => collect_name_path(path, references),
        ast::TermExpression::Return(node) => {
            if let Some(expr) = &node.base {
                collect_term(expr, references);
            }
        }
        ast::TermExpression::Break(node) => {
            if let Some(expr) = &node.base {
                collect_term(expr, references);
            }
        }
        ast::TermExpression::Continue(_) => {}
        ast::TermExpression::Raise(node) => {
            if let Some(expr) = &node.base {
                collect_term(expr, references);
            }
        }
        ast::TermExpression::Resume(node) => {
            if let Some(expr) = &node.base {
                collect_term(expr, references);
            }
        }
        ast::TermExpression::Loop { condition, body, .. } => {
            if let Some(condition) = condition {
                collect_term(condition, references);
            }
            collect_block(body, references);
        }
        _ => {}
    }
}

fn collect_call_target(callee: &ast::TermExpression, references: &mut BTreeSet<String>) {
    match callee {
        ast::TermExpression::NamePath(path) => {
            if let Some(name) = path.parts.last() {
                if !is_builtin_symbol(&name.name) {
                    references.insert(name.name.clone());
                }
            }
        }
        ast::TermExpression::DotCall { receiver, field, .. } => {
            collect_term(receiver, references);
            references.insert(field.name.clone());
        }
        _ => collect_term(callee, references),
    }
}

fn collect_type_expression(ty: &ast::TypeExpression, references: &mut BTreeSet<String>) {
    match ty {
        ast::TypeExpression::Binary(node) => {
            collect_type_expression(&node.lhs, references);
            collect_type_expression(&node.rhs, references);
        }
        ast::TypeExpression::Unary(node) => collect_type_expression(&node.base, references),
        ast::TypeExpression::Generic(generic) => {
            references.insert(generic.name.name.clone());
        }
        ast::TypeExpression::Tuple(tuple) => {
            for element in &tuple.elements {
                collect_type_expression(&element.ty, references);
            }
        }
        ast::TypeExpression::Function(function) => {
            for param in &function.params {
                collect_type_expression(param, references);
            }
            collect_type_expression(&function.return_type, references);
        }
        ast::TypeExpression::Optional(optional) => collect_type_expression(&optional.inner, references),
        ast::TypeExpression::AssociatedType(associated) => {
            references.insert(associated.base.name.clone());
            references.insert(associated.name.name.clone());
        }
        ast::TypeExpression::QualifiedAssociatedType(qualified) => {
            collect_type_expression(&qualified.ty, references);
            collect_name_path(&qualified.trait_path, references);
            references.insert(qualified.name.name.clone());
        }
        ast::TypeExpression::Namepath(path) => collect_name_path(path, references),
        ast::TypeExpression::Apply(apply) => {
            collect_type_expression(&apply.base, references);
            for argument in &apply.arguments {
                collect_type_expression(argument, references);
            }
        }
    }
}

fn collect_name_path(path: &ast::NamePath, references: &mut BTreeSet<String>) {
    for part in &path.parts {
        if !is_builtin_symbol(&part.name) {
            references.insert(part.name.clone());
        }
    }
}

fn is_builtin_symbol(name: &str) -> bool {
    matches!(
        name,
        "i8" | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "usize"
            | "f32"
            | "f64"
            | "bool"
            | "utf8"
            | "void"
            | "mut"
            | "self"
            | "Self"
            | "true"
            | "false"
            | "None"
            | "Some"
            | "Fine"
            | "Fail"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan_from_groups(groups: &[(&str, bool, Vec<(String, String)>)]) -> SemanticClosure {
        let refs = groups.iter().map(|(key, is_entry, files)| (*key, *is_entry, files.as_slice())).collect::<Vec<_>>();
        plan_semantic_closure(&refs)
    }

    #[test]
    fn unreachable_inventory_syntax_error_does_not_expand_into_body_paths() {
        let groups = [
            (
                "library",
                false,
                vec![
                    ("source/good.v".into(), "micro answer() -> i32 { return 23 }".into()),
                    ("source/broken.v".into(), "micro broken() { value = }".into()),
                ],
            ),
            (
                "app",
                true,
                vec![("solution.v".into(), "micro run() -> i32 { let value: i32 = answer(); return value }".into())],
            ),
        ];
        let closure = plan_from_groups(&groups);
        assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["source/good.v".into()]));
        assert_eq!(closure.groups[1].body_lowering_source_paths, BTreeSet::from(["solution.v".into()]));
        assert!(!closure.digest.is_empty());
        assert!(closure.demanded_declarations.iter().any(|key| key.name == "answer"));
    }

    #[test]
    fn reachable_inventory_syntax_error_remains_in_body_paths() {
        let groups = [("app", true, vec![("solution.v".into(), "micro broken() { value = }".into())])];
        let closure = plan_from_groups(&groups);
        assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["solution.v".into()]));
    }

    #[test]
    fn transitive_cross_package_symbol_pulls_base_dependency_file() {
        let groups = [
            (
                "base",
                false,
                vec![("source/answer.v".into(), "micro answer() -> i32 { return 23 }".into())],
            ),
            (
                "library",
                false,
                vec![("source/relay.v".into(), "micro relay() -> i32 { return answer() }".into())],
            ),
            ("app", true, vec![("solution.v".into(), "micro run() -> i32 { return relay() }".into())]),
        ];
        let closure = plan_from_groups(&groups);
        assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["source/answer.v".into()]));
        assert_eq!(closure.groups[1].body_lowering_source_paths, BTreeSet::from(["source/relay.v".into()]));
    }

    #[test]
    fn cross_package_reachable_symbol_pulls_dependency_file_into_closure() {
        let groups = [
            (
                "library",
                false,
                vec![
                    ("source/answer.v".into(), "micro answer() -> i32 { return 23 }".into()),
                    ("source/unused.v".into(), "micro unused() -> i32 { return 0 }".into()),
                ],
            ),
            ("app", true, vec![("solution.v".into(), "micro run() -> i32 { return answer() }".into())]),
        ];
        let closure = plan_from_groups(&groups);
        assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["source/answer.v".into()]));
        assert!(!closure.groups[0].body_lowering_source_paths.contains("source/unused.v"));
    }

    #[test]
    fn closure_digest_is_stable_for_identical_input() {
        let groups = [(
            "app",
            true,
            vec![("solution.v".into(), "micro run() -> i32 { return 23 }".into())],
        )];
        let first = plan_from_groups(&groups);
        let second = plan_from_groups(&groups);
        assert_eq!(first.digest, second.digest);
        assert_eq!(first.declaration_index.len(), 1);
    }
}

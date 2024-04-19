//! 入口驱动的语义闭包规划：声明索引与 body lowering 边界。

use std::collections::{BTreeMap, BTreeSet};

use crate::valkyrie::frontend::{ast, parse_source};

/// 单个源码组的 body lowering 计划。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticGroupPlan {
    /// 依赖包 inventory 是否容忍单文件 parse 失败。
    pub tolerate_inventory_parse_failures: bool,
    /// 进入 Oak parse 与 HIR body lowering 的仓相对路径。
    pub body_lowering_source_paths: Option<BTreeSet<String>>,
}

/// 从入口组与其依赖 inventory 规划各组的 body lowering 边界。
pub fn plan_semantic_groups(groups: &[(&str, bool, &[(String, String)])]) -> Vec<SemanticGroupPlan> {
    let indexed_groups = groups
        .iter()
        .map(|(dependency_key, is_entry, files)| {
            let indexed = files.iter().map(|(path, source)| index_source_file(path, source)).collect::<Vec<_>>();
            (*dependency_key, *is_entry, indexed)
        })
        .collect::<Vec<_>>();

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

    indexed_groups
        .into_iter()
        .map(|(_, is_entry, files)| {
            if is_entry {
                SemanticGroupPlan {
                    tolerate_inventory_parse_failures: false,
                    body_lowering_source_paths: Some(files.iter().map(|file| file.path.clone()).collect()),
                }
            }
            else {
                SemanticGroupPlan {
                    tolerate_inventory_parse_failures: true,
                    body_lowering_source_paths: Some(expand_dependency_body_paths(&files, &demanded_symbols)),
                }
            }
        })
        .collect()
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
        for declaration in &file.declarations {
            index.entry(declaration.clone()).or_default().insert(file.path.clone());
        }
    }
    index
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexedFile {
    path: String,
    parse_ok: bool,
    declarations: BTreeSet<String>,
    referenced_symbols: BTreeSet<String>,
}

fn index_source_file(path: &str, source: &str) -> IndexedFile {
    match parse_source(source) {
        Ok(root) => {
            let mut declarations = BTreeSet::new();
            let mut referenced_symbols = BTreeSet::new();
            collect_items(&root.items, &mut declarations, &mut referenced_symbols);
            IndexedFile { path: path.to_string(), parse_ok: true, declarations, referenced_symbols }
        }
        Err(_) => IndexedFile {
            path: path.to_string(),
            parse_ok: false,
            declarations: BTreeSet::new(),
            referenced_symbols: BTreeSet::new(),
        },
    }
}

fn collect_items(items: &[ast::StatementNode], declarations: &mut BTreeSet<String>, references: &mut BTreeSet<String>) {
    for item in items {
        collect_item(item, declarations, references);
    }
}

fn collect_item(item: &ast::StatementNode, declarations: &mut BTreeSet<String>, references: &mut BTreeSet<String>) {
    match item {
        ast::StatementNode::Namespace(namespace) => collect_items(&namespace.items, declarations, references),
        ast::StatementNode::Using(using) => collect_using(using, references),
        ast::StatementNode::Class(class) => {
            declarations.insert(class.name.name.clone());
            collect_structure_body(&class.body, declarations, references);
        }
        ast::StatementNode::Structure(structure) => {
            declarations.insert(structure.name.name.clone());
            collect_structure_body(&structure.body, declarations, references);
        }
        ast::StatementNode::Union(union_def) => {
            declarations.insert(union_def.name.name.clone());
            for field in &union_def.body.fields {
                collect_type_expression(&field.typing, references);
            }
        }
        ast::StatementNode::Flags(flags) => {
            declarations.insert(flags.name.name.clone());
        }
        ast::StatementNode::Enums(enums) => {
            declarations.insert(enums.name.name.clone());
            for variant in &enums.variants {
                references.insert(variant.name.name.clone());
            }
        }
        ast::StatementNode::Trait(trait_def) => {
            declarations.insert(trait_def.name.name.clone());
            collect_structure_body(&trait_def.body, declarations, references);
        }
        ast::StatementNode::Imply(imply) => collect_imply(imply, declarations, references),
        ast::StatementNode::Micro(micro) => {
            declarations.insert(micro.name.name.clone());
            collect_micro(micro, references);
        }
        ast::StatementNode::TypeAlias(alias) => {
            declarations.insert(alias.name.name.clone());
            collect_type_expression(&alias.target, references);
        }
        ast::StatementNode::TypeFunction(type_function) => {
            declarations.insert(type_function.name.name.clone());
            collect_type_function(type_function, references);
        }
        ast::StatementNode::Let(let_stmt) => collect_let(let_stmt, references),
        ast::StatementNode::ExprStmt(expr_stmt) => collect_term(&expr_stmt.expr, references),
        ast::StatementNode::Statement(inner) => collect_item(inner, declarations, references),
        _ => {}
    }
}

fn collect_using(using: &ast::UsingDeclaration, references: &mut BTreeSet<String>) {
    collect_name_path(&using.path, references);
    for import in &using.imports {
        references.insert(import.name.clone());
    }
    if let Some(alias) = &using.alias {
        references.insert(alias.name.clone());
    }
}

fn collect_imply(imply: &ast::ImplyDeclaration, declarations: &mut BTreeSet<String>, references: &mut BTreeSet<String>) {
    collect_type_expression(&imply.target_type, references);
    if let Some(trait_type) = &imply.trait_type {
        collect_type_expression(trait_type, references);
    }
    for method in &imply.methods {
        declarations.insert(method.name.name.clone());
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

fn collect_structure_body(body: &ast::StructureBody, declarations: &mut BTreeSet<String>, references: &mut BTreeSet<String>) {
    for field in &body.fields {
        collect_type_expression(&field.typing, references);
    }
    for method in &body.methods {
        declarations.insert(method.name.name.clone());
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

    fn plan_from_groups(groups: &[(&str, bool, Vec<(String, String)>)]) -> Vec<SemanticGroupPlan> {
        let refs = groups.iter().map(|(key, is_entry, files)| (*key, *is_entry, files.as_slice())).collect::<Vec<_>>();
        plan_semantic_groups(&refs)
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
        let plans = plan_from_groups(&groups);
        assert!(plans[0].tolerate_inventory_parse_failures);
        assert_eq!(plans[0].body_lowering_source_paths, Some(BTreeSet::from(["source/good.v".into()])));
        assert!(!plans[1].tolerate_inventory_parse_failures);
        assert_eq!(plans[1].body_lowering_source_paths, Some(BTreeSet::from(["solution.v".into()])));
    }

    #[test]
    fn reachable_inventory_syntax_error_remains_in_body_paths() {
        let groups = [("app", true, vec![("solution.v".into(), "micro broken() { value = }".into())])];
        let plans = plan_from_groups(&groups);
        assert!(!plans[0].tolerate_inventory_parse_failures);
        assert_eq!(plans[0].body_lowering_source_paths, Some(BTreeSet::from(["solution.v".into()])));
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
        let plans = plan_from_groups(&groups);
        assert_eq!(plans[0].body_lowering_source_paths, Some(BTreeSet::from(["source/answer.v".into()])));
        assert_eq!(plans[1].body_lowering_source_paths, Some(BTreeSet::from(["source/relay.v".into()])));
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
        let plans = plan_from_groups(&groups);
        assert_eq!(plans[0].body_lowering_source_paths, Some(BTreeSet::from(["source/answer.v".into()])));
        assert!(!plans[0].body_lowering_source_paths.as_ref().expect("dependency closure paths").contains("source/unused.v"));
    }
}

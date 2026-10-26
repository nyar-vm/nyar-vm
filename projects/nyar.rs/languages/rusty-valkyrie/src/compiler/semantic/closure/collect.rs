use std::collections::BTreeSet;

use oak_valkyrie::lexer::token_type::ValkyrieTokenType;

use nyar_types::compiler_input::{DeclarationKind, DeclarationLink, EntryPointKind, ReferencePath};

use crate::compiler::{semantic::operator_members, syntax::oak::ast};

use super::{
    index_file::{FileBuilder, References},
    name_path_segments,
};

pub(crate) fn collect_items(builder: &mut FileBuilder, current_namespace: &[String], items: &[ast::StatementNode]) {
    let mut namespace = current_namespace.to_vec();
    for item in items {
        match item {
            ast::StatementNode::Namespace(namespace_decl) => {
                let declared = name_path_segments(&namespace_decl.name);
                if crate::compiler::syntax::oak::namespace_is_declaration_only(namespace_decl) {
                    namespace = declared;
                }
                else {
                    collect_items(builder, &declared, crate::compiler::syntax::oak::namespace_items(namespace_decl));
                }
            }
            other => collect_item(builder, &namespace, other),
        }
    }
}

fn collect_item(builder: &mut FileBuilder, namespace: &[String], item: &ast::StatementNode) {
    match item {
        ast::StatementNode::Using(using) => {
            let mut references = References::default();
            collect_using(using, &mut references);
            builder.register_module_references(namespace, references);
        }
        ast::StatementNode::Class(class) => {
            let mut signature = References::default();
            collect_signature_fields(&class.body.fields, &mut signature);
            builder.register(namespace, DeclarationKind::Class, &class.name.name, None, None, None, signature);
            collect_members(builder, namespace, &class.name.name, &class.body.methods);
        }
        ast::StatementNode::Structure(structure) => {
            let mut signature = References::default();
            collect_signature_fields(&structure.body.fields, &mut signature);
            builder.register(namespace, DeclarationKind::Structure, &structure.name.name, None, None, None, signature);
            collect_members(builder, namespace, &structure.name.name, &structure.body.methods);
        }
        ast::StatementNode::Trait(trait_decl) => {
            let mut signature = References::default();
            collect_signature_fields(&trait_decl.body.fields, &mut signature);
            builder.register(namespace, DeclarationKind::Trait, &trait_decl.name.name, None, None, None, signature);
            collect_members(builder, namespace, &trait_decl.name.name, &trait_decl.body.methods);
        }
        ast::StatementNode::Union(union_def) => {
            let mut signature = References::default();
            collect_signature_fields(&union_def.body.fields, &mut signature);
            builder.register(namespace, DeclarationKind::Union, &union_def.name.name, None, None, None, signature);
        }
        ast::StatementNode::Flags(flags) => {
            builder.register(namespace, DeclarationKind::Flags, &flags.name.name, None, None, None, References::default());
        }
        ast::StatementNode::Enums(enums) => {
            let key = builder.register(namespace, DeclarationKind::Enum, &enums.name.name, None, None, None, References::default());
            for variant in &enums.variants {
                builder.variant_members.push((variant.name.name.clone(), key.clone()));
            }
        }
        ast::StatementNode::Imply(imply) => collect_imply(builder, namespace, imply),
        ast::StatementNode::Micro(micro) => {
            let mut references = References::default();
            collect_micro(micro, &mut references);
            let entry_point = entry_point_annotation(&micro.annotations);
            builder.register(namespace, DeclarationKind::Micro, &micro.name.name, None, None, entry_point, references);
        }
        ast::StatementNode::TypeAlias(alias) => {
            let mut references = References::default();
            collect_type_expression(&alias.target, &mut references);
            let link = type_target_reference(&alias.target).map(DeclarationLink::AliasTarget);
            builder.register(namespace, DeclarationKind::TypeAlias, &alias.name.name, None, link, None, references);
        }
        ast::StatementNode::TypeFunction(type_function) => {
            let mut references = References::default();
            collect_type_function(type_function, &mut references);
            builder.register(namespace, DeclarationKind::TypeFunction, &type_function.name.name, None, None, None, references);
        }
        ast::StatementNode::Statement(inner) => collect_item(builder, namespace, inner),
        ast::StatementNode::Let(let_stmt) => {
            let mut references = References::default();
            collect_let(let_stmt, &mut references);
            builder.register_module_references(namespace, references);
        }
        ast::StatementNode::ExprStmt(expr_stmt) => {
            let mut references = References::default();
            collect_term(&expr_stmt.expr, &mut references);
            builder.register_module_references(namespace, references);
        }
        _ => {}
    }
}

fn collect_members(builder: &mut FileBuilder, namespace: &[String], owner: &str, methods: &[ast::MethodDeclaration]) {
    for method in methods {
        let mut references = References::default();
        collect_method(method, &mut references);
        // 成员身份使用与 HIR 相同的运算符成员名：`[infix] micro \`+\`` 的身份是 `infix +`，
        // 否则调用点（`a + b`）的选择名永远命不中声明。
        let name = operator_members::declared_member_name(&method.name.name, &method.annotations);
        // 拥有者引用相对成员自身 namespace：聚合与该成员同文件同 namespace 时按 namespace 链命中。
        let link = Some(DeclarationLink::MemberOwner(vec![owner.to_string()]));
        builder.register(namespace, DeclarationKind::Method, &name, Some(owner), link, None, references);
    }
}

fn collect_imply(builder: &mut FileBuilder, namespace: &[String], imply: &ast::ImplyDeclaration) {
    let mut signature = References::default();
    collect_type_expression(&imply.target_type, &mut signature);
    if let Some(trait_type) = &imply.trait_type {
        collect_type_expression(trait_type, &mut signature);
    }
    if imply.methods.is_empty() {
        // 没有 witness 方法的 imply 只有外壳，作为文件级引用保存。
        builder.register_module_references(namespace, signature);
        return;
    }
    // witness 方法挂在被扩展聚合的**声明身份**上：拥有者是 imply 目标类型的引用路径，
    // 由索引在整个 inventory 完成后按可见性解析（可以跨依赖组），不按短名认亲。
    let owner = imply_owner_name(imply);
    let owner_reference = type_target_reference(&imply.target_type);
    for method in &imply.methods {
        // imply 的目标类型与 trait 引用附在每个 witness 方法上：需求任一 witness 都需要它们可解析。
        let mut references = signature.clone();
        collect_method(method, &mut references);
        let name = operator_members::declared_member_name(&method.name.name, &method.annotations);
        let link = owner_reference.clone().map(DeclarationLink::MemberOwner);
        builder.register(namespace, DeclarationKind::ImplyMethod, &name, owner.as_deref(), link, None, references);
    }
}

/// imply 目标类型的聚合短名：`imply HashMap<K, V>` / `imply std::collection::HashMap<K, V>` → `HashMap`。
pub(crate) fn imply_owner_name(imply: &ast::ImplyDeclaration) -> Option<String> {
    type_target_reference(&imply.target_type).and_then(|segments| segments.last().cloned())
}

/// 类型表达式被引用的名字路径：`Utf8Text` → `["Utf8Text"]`，`std::collection::HashMap<K, V>` → `["std","collection","HashMap"]`。
///
/// 只剥掉泛型应用取名字路径，不解释类型本身：声明身份由索引按整个 inventory 与可见性解析。
fn type_target_reference(ty: &ast::TypeExpression) -> Option<ReferencePath> {
    match ty {
        ast::TypeExpression::Namepath(path) => Some(name_path_segments(path)),
        ast::TypeExpression::Apply(apply) => type_target_reference(&apply.base),
        _ => None,
    }
}

fn collect_using(using: &ast::UsingDeclaration, references: &mut References) {
    references.imports.insert(using.path.parts.iter().map(|part| part.name.as_str()).collect::<Vec<_>>().join("::"));
    references.paths.insert(name_path_segments(&using.path));
    for import in &using.imports {
        references.paths.insert(vec![import.name.clone()]);
    }
    if let Some(alias) = &using.alias {
        references.paths.insert(vec![alias.name.clone()]);
    }
}

fn collect_signature_fields(fields: &[ast::FieldDeclaration], references: &mut References) {
    for field in fields {
        collect_type_expression(&field.typing, references);
    }
}

/// 声明的显式入口注解：`[export]` 优先于 `[main]`；二者都只提供 seed，不改变声明身份。
fn entry_point_annotation(annotations: &[ast::Attribute]) -> Option<EntryPointKind> {
    let mut entry_point = None;
    for attribute in annotations {
        match attribute.name.name.as_str() {
            "export" => return Some(EntryPointKind::Export),
            "main" => entry_point = Some(EntryPointKind::Main),
            _ => {}
        }
    }
    entry_point
}

fn collect_micro(micro: &ast::MicroDeclaration, references: &mut References) {
    for param in &micro.params {
        if let Some(ty) = &param.ty {
            collect_type_expression(ty, references);
        }
    }
    if let Some(return_type) = &micro.return_type {
        collect_type_expression(return_type, references);
    }
    collect_where_constraints(&micro.where_constraints, references);
    collect_block(&micro.body, references);
}

fn collect_method(method: &ast::MethodDeclaration, references: &mut References) {
    for param in &method.parameters {
        if let Some(ty) = &param.ty {
            collect_type_expression(ty, references);
        }
    }
    if let Some(return_type) = &method.return_type {
        collect_type_expression(return_type, references);
    }
    collect_where_constraints(&method.where_constraints, references);
    if let Some(body) = &method.body {
        collect_block(body, references);
    }
}

fn collect_type_function(type_function: &ast::TypeFunction, references: &mut References) {
    for param in &type_function.params {
        if let Some(ty) = &param.ty {
            collect_type_expression(ty, references);
        }
    }
    if let Some(return_type) = &type_function.return_type {
        collect_type_expression(return_type, references);
    }
    collect_where_constraints(&type_function.where_constraints, references);
    collect_block(&type_function.body, references);
}

/// `where` 约束的受约束类型、trait 路径、类型实参与关联等式右侧都要进入闭包：
/// 它们会被 HIR 冻结成 trait declaration identity，缺一就会出现「trait 约束未解析」。
fn collect_where_constraints(constraints: &[ast::type_nodes::WhereConstraint], references: &mut References) {
    for constraint in constraints {
        collect_type_expression(&constraint.target, references);
        for bound in &constraint.bounds {
            collect_name_path(&bound.trait_path, references);
            for argument in &bound.type_arguments {
                collect_type_expression(argument, references);
            }
            for equation in &bound.associated_types {
                collect_type_expression(&equation.ty, references);
            }
        }
    }
}

fn collect_let(let_stmt: &ast::Let, references: &mut References) {
    if let Some(ty) = &let_stmt.ty {
        collect_type_expression(ty, references);
    }
    collect_term(&let_stmt.expr, references);
}

fn collect_block(block: &ast::Block, references: &mut References) {
    for statement in &block.statements {
        collect_statement(statement, references);
    }
}

fn collect_statement(statement: &ast::Statement, references: &mut References) {
    match statement {
        ast::Statement::Let(let_stmt) => collect_let(let_stmt, references),
        ast::Statement::Expression(expr_stmt) => collect_term(&expr_stmt.expr, references),
        _ => {}
    }
}

fn collect_term(term: &ast::TermExpression, references: &mut References) {
    match term {
        ast::TermExpression::ArrayLiteral { items, .. } => {
            for item in items {
                collect_term(item, references);
            }
        }
        ast::TermExpression::Unary(node) => {
            // 运算符是普通成员调用：`!a` 降低为 `a.prefix !()`，`*a` 降低为 `a.deref_read()`。
            // 成员名与 HIR lowering 共用同一份映射，闭包据此把成员选择名登记进需求。
            if matches!(node.operator, ValkyrieTokenType::Star) {
                references.selectors.insert(operator_members::DEREF_READ_MEMBER.to_string());
            }
            else {
                // 负整数字面量在 HIR 折叠为字面量，不产生成员调用；这里仍登记该成员名，
                // 闭包只做上界判定：究竟走哪一个由 HIR 的降低规则决定。
                references.selectors.insert(operator_members::prefix_member_name(&node.operator));
            }
            collect_term(&node.base, references);
        }
        ast::TermExpression::Binary(node) => {
            collect_term(&node.lhs, references);
            collect_term(&node.rhs, references);
            match node.operator {
                // 赋值不是成员调用；对下标左值赋值降低为 `suffix [ ]=` / `suffix ⁅ ⁆=`。
                ValkyrieTokenType::Eq => match &node.lhs {
                    ast::TermExpression::Index { .. } => {
                        references.selectors.insert(operator_members::SUBSCRIPT_ASSIGN_MEMBER.to_string());
                    }
                    ast::TermExpression::Offset { .. } => {
                        references.selectors.insert(operator_members::CARDINAL_SUBSCRIPT_ASSIGN_MEMBER.to_string());
                    }
                    _ => {}
                },
                // `&&` / `||` 降低为短路控制流，不产生成员调用。
                ValkyrieTokenType::AndAnd | ValkyrieTokenType::OrOr => {}
                _ => {
                    references.selectors.insert(operator_members::infix_member_name(&node.operator));
                }
            }
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
            // 把完整的 qualified path 放进 paths（与 collect_call_target 同构）；
            // receiver 本身可能是 NamePath，也可能是另一个 DotCall。
            if let Some(full_path) = extract_term_path(term) {
                references.paths.insert(full_path);
            }
            collect_term(receiver, references);
            references.selectors.insert(field.name.clone());
        }
        ast::TermExpression::Index { receiver, index, .. } => {
            references.selectors.insert(operator_members::SUBSCRIPT_MEMBER.to_string());
            collect_term(receiver, references);
            collect_term(index, references);
        }
        ast::TermExpression::Offset { receiver, offset, .. } => {
            references.selectors.insert(operator_members::CARDINAL_SUBSCRIPT_MEMBER.to_string());
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

/// 从一个 term 节点里提取完整的 qualified path 段。
///
/// `a.b.c()` 这种全限定调用的 callee 是 `DotCall(DotCall(NamePath("a"), "b"), "c")`；
/// 把它拉平成 `["a", "b", "c"]` 放进 references.paths，demand 图才能沿完整路径
/// 命中目标函数声明。没有这条路径，只有 `["a","b"]`（NamePath receiver）进 paths、
/// `"c"` 进 selectors，resolve 永远命不中被限定的自由函数。
fn extract_term_path(term: &ast::TermExpression) -> Option<Vec<String>> {
    match term {
        ast::TermExpression::NamePath(path) => Some(name_path_segments(path)),
        ast::TermExpression::DotCall { receiver, field, .. } => {
            let mut prefix = extract_term_path(receiver)?;
            prefix.push(field.name.clone());
            Some(prefix)
        }
        _ => None,
    }
}

fn collect_call_target(callee: &ast::TermExpression, references: &mut References) {
    match callee {
        ast::TermExpression::NamePath(path) => collect_name_path(path, references),
        ast::TermExpression::DotCall { receiver, field, .. } => {
            // 先把完整的 qualified path 放进 paths（demand 图沿它解析），
            // 再保留 receiver 的收集（类型引用、副作用等）和 field 的 selector。
            if let Some(full_path) = extract_term_path(callee) {
                references.paths.insert(full_path);
            }
            collect_term(receiver, references);
            references.selectors.insert(field.name.clone());
        }
        _ => collect_term(callee, references),
    }
}

fn collect_type_expression(ty: &ast::TypeExpression, references: &mut References) {
    match ty {
        ast::TypeExpression::Binary(node) => {
            collect_type_expression(&node.lhs, references);
            collect_type_expression(&node.rhs, references);
        }
        ast::TypeExpression::Unary(node) => collect_type_expression(&node.base, references),
        ast::TypeExpression::Generic(generic) => {
            references.paths.insert(vec![generic.name.name.clone()]);
        }
        ast::TypeExpression::Tuple(tuple) => {
            for element in &tuple.elements {
                collect_type_expression(element, references);
            }
        }
        ast::TypeExpression::Function(function) => {
            for param in &function.params {
                collect_type_expression(param, references);
            }
            collect_type_expression(function.return_type.as_ref(), references);
        }
        ast::TypeExpression::Optional(optional) => collect_type_expression(optional.inner.as_ref(), references),
        ast::TypeExpression::AssociatedType(associated) => {
            references.paths.insert(vec![associated.base.name.clone()]);
            references.paths.insert(vec![associated.name.name.clone()]);
        }
        ast::TypeExpression::QualifiedAssociatedType(qualified) => {
            collect_type_expression(qualified.ty.as_ref(), references);
            collect_name_path(&qualified.trait_path, references);
            references.paths.insert(vec![qualified.name.name.clone()]);
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

fn collect_name_path(path: &ast::NamePath, references: &mut References) {
    let segments = name_path_segments(path);
    if segments.is_empty() {
        return;
    }
    // 源码提到的每个名字都登记为引用：名字事实只有一份，就是声明索引。
    // `usize` / `utf8` 等标量类型同样由源码声明（`structure usize` / `type utf8`），
    // 不是「无声明内建」，因此不存在按名字表跳过的分支；未命中的引用自然解析为空。
    references.paths.insert(segments);
}

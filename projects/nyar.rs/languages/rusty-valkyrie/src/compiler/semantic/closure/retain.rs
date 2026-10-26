use std::collections::BTreeSet;

use nyar_types::compiler_input::{DeclarationKey, DeclarationKind};

use crate::compiler::{semantic::operator_members, syntax::oak::ast};

use super::{collect::imply_owner_name, name_path_segments};

/// 按被需求声明身份裁剪 AST：非被需求声明不得进入 HIR 物化。
///
/// 与 `collect_item` 共用同一套「AST 条目 → [`DeclarationKey`]」映射；声明级物化边界
/// 必须与闭包规划使用同一身份，不得用文件名或短名近似。文件级语句（`using`、
/// 顶层 `let`/表达式）与无 body 的 `namespace` 条目不是声明，保留。
pub fn retain_demanded_declarations(
    items: &mut Vec<ast::StatementNode>,
    dependency_key: &str,
    source_path: &str,
    demanded: &BTreeSet<DeclarationKey>,
) {
    let key_of = |namespace: &[String], kind: DeclarationKind, name: &str, owner: Option<&str>| DeclarationKey {
        dependency_key: dependency_key.to_string(),
        source_path: source_path.to_string(),
        namespace: namespace.to_vec(),
        kind,
        owner: owner.map(str::to_string),
        name: name.to_string(),
    };
    retain_items(items, &[], demanded, &key_of);
}

fn retain_items(
    items: &mut Vec<ast::StatementNode>,
    namespace: &[String],
    demanded: &BTreeSet<DeclarationKey>,
    key_of: &impl Fn(&[String], DeclarationKind, &str, Option<&str>) -> DeclarationKey,
) {
    // 与 `collect_items` 保持同一套 namespace 语义：分号式 `namespace X;` 切换后续条目的
    // namespace，因此这里必须顺序推进作用域，不能对每个条目都用同一个起始 namespace，
    // 否则分号式 namespace 下的声明会按空 namespace 查身份并被误裁掉。
    let mut current = namespace.to_vec();
    items.retain_mut(|item| {
        if let ast::StatementNode::Namespace(declaration) = item {
            if !declaration.has_body() {
                current = name_path_segments(&declaration.name);
                return true;
            }
        }
        keep_declaration(item, &current, demanded, key_of)
    });
}

/// 条目保留规则：声明条目按身份保留；作用域与语句条目保留并递归裁剪其声明子项。
fn keep_declaration(
    item: &mut ast::StatementNode,
    namespace: &[String],
    demanded: &BTreeSet<DeclarationKey>,
    key_of: &impl Fn(&[String], DeclarationKind, &str, Option<&str>) -> DeclarationKey,
) -> bool {
    match item {
        // `namespace` 是作用域语句：带 body 时其条目以声明路径为 namespace 递归裁剪。
        ast::StatementNode::Namespace(decl) => {
            if let Some(body) = &mut decl.body {
                let declared = name_path_segments(&decl.name);
                retain_items(&mut body.items, &declared, demanded, key_of);
            }
            true
        }
        ast::StatementNode::Class(class) => {
            retain_members(&mut class.body.methods, namespace, &class.name.name, demanded, key_of);
            demanded.contains(&key_of(namespace, DeclarationKind::Class, &class.name.name, None)) || !class.body.methods.is_empty()
        }
        ast::StatementNode::Structure(structure) => {
            retain_members(&mut structure.body.methods, namespace, &structure.name.name, demanded, key_of);
            demanded.contains(&key_of(namespace, DeclarationKind::Structure, &structure.name.name, None)) || !structure.body.methods.is_empty()
        }
        ast::StatementNode::Trait(trait_decl) => {
            retain_members(&mut trait_decl.body.methods, namespace, &trait_decl.name.name, demanded, key_of);
            demanded.contains(&key_of(namespace, DeclarationKind::Trait, &trait_decl.name.name, None)) || !trait_decl.body.methods.is_empty()
        }
        ast::StatementNode::Union(union_def) => demanded.contains(&key_of(namespace, DeclarationKind::Union, &union_def.name.name, None)),
        ast::StatementNode::Flags(flags) => demanded.contains(&key_of(namespace, DeclarationKind::Flags, &flags.name.name, None)),
        ast::StatementNode::Enums(enums) => demanded.contains(&key_of(namespace, DeclarationKind::Enum, &enums.name.name, None)),
        ast::StatementNode::Micro(micro) => demanded.contains(&key_of(namespace, DeclarationKind::Micro, &micro.name.name, None)),
        ast::StatementNode::TypeAlias(alias) => demanded.contains(&key_of(namespace, DeclarationKind::TypeAlias, &alias.name.name, None)),
        ast::StatementNode::TypeFunction(type_function) => {
            demanded.contains(&key_of(namespace, DeclarationKind::TypeFunction, &type_function.name.name, None))
        }
        // imply 块的每个 witness 方法各自是一份被需求声明；拥有者与成员名都与 `collect_imply` 一致。
        ast::StatementNode::Imply(imply) => {
            let owner = imply_owner_name(imply);
            imply.methods.retain(|method| {
                let name = operator_members::declared_member_name(&method.name.name, &method.annotations);
                demanded.contains(&key_of(namespace, DeclarationKind::ImplyMethod, &name, owner.as_deref()))
            });
            !imply.methods.is_empty()
        }
        // 包装语句按其内部条目的身份保留或丢弃，包装节点本身不是声明。
        ast::StatementNode::Statement(inner) => keep_declaration(inner.as_mut(), namespace, demanded, key_of),
        // 文件级 `using` / `let` / 顶层表达式不是声明；`singleton` / `widget` / `property`
        // 目前不在声明索引里，只能整条保留（缺口见账本）。
        _ => true,
    }
}

fn retain_members(
    methods: &mut Vec<ast::MethodDeclaration>,
    namespace: &[String],
    owner: &str,
    demanded: &BTreeSet<DeclarationKey>,
    key_of: &impl Fn(&[String], DeclarationKind, &str, Option<&str>) -> DeclarationKey,
) {
    methods.retain(|method| {
        let name = operator_members::declared_member_name(&method.name.name, &method.annotations);
        demanded.contains(&key_of(namespace, DeclarationKind::Method, &name, Some(owner)))
    });
}

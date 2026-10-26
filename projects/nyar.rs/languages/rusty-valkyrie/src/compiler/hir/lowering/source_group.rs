use crate::compiler::{
    emitter::backend_contract::interop::{annotations_declare_external_import, function_host_provider_target},
    hir::{ModuleTypeAliasScope, NominalTypeScope, VisibleTypeAlias, lower_type_expression, type_lowering::VisibleNominalType},
    syntax::oak::ast::{StatementNode, TypeAliasDeclaration},
    types::{
        Identifier, NamePath, SourceID,
        hir::{HirDependencySemanticExport, HirModule},
    },
};

use nyar_types::compiler_input::{CompilerSourceFile, CompilerSourceGroup, CompilerSourceInput, GeneratedSourceGroup};

use crate::compiler::hir::lowering::{
    attribute_lowering::lower_name_path,
    diagnostics::{pipeline_rejected, plan_source, plan_span},
    errors::HirError,
};

/// 建立生成源码组；producer 与入口身份都由生成器显式提供，不接受缺省推断。
pub fn build_generated_group(producer: impl Into<String>, group: CompilerSourceGroup) -> Result<GeneratedSourceGroup, HirError> {
    let producer = producer.into();
    if producer.trim().is_empty() {
        return Err(HirError::SourceGroupProducerMissing {
            dependency_key: Some(group.dependency_key.clone()),
            source: plan_source(),
            span: plan_span(),
        });
    }
    if !group.is_entry {
        return Err(HirError::SourceGroupNotEntry { dependency_key: group.dependency_key.clone(), source: plan_source(), span: plan_span() });
    }
    Ok(GeneratedSourceGroup { producer, group })
}

/// 含生成源码的 Compiler 输入；构造时即验证 producer、入口身份、组身份与依赖顺序。
pub fn build_source_input_with_generated(
    groups: Vec<CompilerSourceGroup>,
    generated: Vec<GeneratedSourceGroup>,
) -> Result<CompilerSourceInput, HirError> {
    let input = CompilerSourceInput { groups, generated };
    compose_source_input_groups(&input)?;
    Ok(input)
}

/// 把生成组追加到应用组之后，得到唯一有序源码闭包。
///
/// 生成组必须显式入口、身份唯一，且只能依赖此前已提供的组；其余拓扑与
/// 文件身份校验由 Compiler 的源码快照校验统一承担。
pub fn compose_source_input_groups(input: &CompilerSourceInput) -> Result<Vec<CompilerSourceGroup>, HirError> {
    let mut composed = input.groups.clone();
    let mut provided = input.groups.iter().map(|group| group.dependency_key.clone()).collect::<std::collections::BTreeSet<_>>();
    for generated in &input.generated {
        if generated.producer.trim().is_empty() {
            return Err(HirError::SourceGroupProducerMissing {
                dependency_key: Some(generated.group.dependency_key.clone()),
                source: plan_source(),
                span: plan_span(),
            });
        }
        if !generated.group.is_entry {
            return Err(HirError::SourceGroupNotEntry {
                dependency_key: generated.group.dependency_key.clone(),
                source: plan_source(),
                span: plan_span(),
            });
        }
        if generated.group.dependency_key.is_empty() || !provided.insert(generated.group.dependency_key.clone()) {
            return Err(HirError::SourceGroupIdentityInvalid {
                dependency_key: generated.group.dependency_key.clone(),
                empty: generated.group.dependency_key.is_empty(),
                source: plan_source(),
                span: plan_span(),
            });
        }
        for dependency in &generated.group.direct_dependencies {
            if dependency == &generated.group.dependency_key || !provided.contains(dependency) {
                return Err(HirError::SourceGroupDependencyUnavailable {
                    group: generated.group.dependency_key.clone(),
                    dependency: dependency.clone(),
                    source: plan_source(),
                    span: plan_span(),
                });
            }
        }
        composed.push(generated.group.clone());
    }
    Ok(composed)
}

/// 在任何解析/裁剪前验证不可变输入身份，避免 map 覆盖制造第二份事实。
pub fn validate_source_groups(groups: &[CompilerSourceGroup]) -> Result<(), HirError> {
    let mut completed = std::collections::BTreeSet::new();
    if groups.is_empty() {
        return Err(HirError::SourceInputEntryGroupMissing { explicit: false, source: plan_source(), span: plan_span() });
    }
    if !groups.iter().any(|group| group.is_entry) {
        return Err(HirError::SourceInputEntryGroupMissing { explicit: true, source: plan_source(), span: plan_span() });
    }
    for group in groups {
        if group.dependency_key.is_empty() || completed.contains(&group.dependency_key) {
            return Err(HirError::SourceGroupIdentityInvalid {
                dependency_key: group.dependency_key.clone(),
                empty: group.dependency_key.is_empty(),
                source: plan_source(),
                span: plan_span(),
            });
        }
        for dependency in &group.direct_dependencies {
            if !completed.contains(dependency) {
                return Err(HirError::SourceGroupDependencyUnavailable {
                    group: group.dependency_key.clone(),
                    dependency: dependency.clone(),
                    source: plan_source(),
                    span: plan_span(),
                });
            }
        }
        let mut paths = std::collections::BTreeSet::new();
        for file in &group.sources {
            if file.path.is_empty() || !paths.insert(&file.path) {
                return Err(HirError::SourceFileIdentityInvalid {
                    group: group.dependency_key.clone(),
                    path: file.path.clone(),
                    empty: file.path.is_empty(),
                    source: plan_source(),
                    span: plan_span(),
                });
            }
        }
        if group.is_entry && group.sources.is_empty() {
            return Err(HirError::EntryGroupWithoutFiles { group: group.dependency_key.clone(), source: plan_source(), span: plan_span() });
        }
        completed.insert(group.dependency_key.clone());
    }
    Ok(())
}

/// 集成测试夹具：从 HIR 构造阶段载荷，不作为生产源码闭包入口。
#[doc(hidden)]
pub(crate) fn compiled_program_from_hir_module(hir_module: HirModule) -> Result<nyar_types::CompiledProgram, HirError> {
    let semantic_mir = crate::compiler::mir::MirLowerer::lower_module_semantic(&hir_module);
    let compiled_program = crate::compiler::emitter::pipeline::compile_linked_semantic_mir(&semantic_mir).map_err(pipeline_rejected)?;
    Ok(compiled_program)
}

/// 同名时本组声明遮蔽依赖别名。不可见组的别名不在返回值里，因此既不会被展开，
/// 也不能遮蔽同名内建类型；本组声明此时只有名字可见（`target: None`）。
pub(crate) fn visible_type_aliases(
    roots: &[&crate::compiler::syntax::oak::ValkyrieRoot],
    imported_semantic_exports: &[HirDependencySemanticExport],
) -> Vec<VisibleTypeAlias> {
    let mut visible = Vec::new();
    for export in imported_semantic_exports {
        for alias in &export.type_aliases {
            visible.push(VisibleTypeAlias {
                name: alias.name.to_string(),
                namespace: alias.namespace.iter().map(ToString::to_string).collect(),
                params: alias.generics.iter().map(ToString::to_string).collect(),
                target: Some(alias.target.clone()),
            });
        }
    }
    for root in roots {
        for (namespace, alias) in root_type_alias_declarations(root) {
            visible.push(VisibleTypeAlias { name: alias.name.name.clone(), namespace, params: Vec::new(), target: None });
        }
    }
    visible
}

/// 把本组（或本根）`type` 声明的目标填入已建立的作用域。
///
/// 全部可见名字在填目标之前已经注册，因此别名之间可以互相引用（含前向引用）；
/// 展开在引用点进行，环由 `expand_module_type_alias` 的 visiting 集合拒绝。
pub(crate) fn register_root_type_alias_targets(roots: &[&crate::compiler::syntax::oak::ValkyrieRoot]) -> Result<(), HirError> {
    for root in roots {
        for (namespace, alias) in root_type_alias_declarations(root) {
            let target = lower_type_expression(&alias.target)?;
            ModuleTypeAliasScope::register_alias(&alias.name.name, namespace, Vec::new(), target);
        }
    }
    Ok(())
}

/// 收集 lowering 作用域内可见的名义类型声明身份（本组声明 + 依赖导出）。
///
/// 只登记会进入 `HirStruct`/`HirEnum` 的声明；每条记录保留声明 namespace，
/// 供限定路径按完整身份匹配。
pub(crate) fn visible_nominal_type_names(
    roots: &[&crate::compiler::syntax::oak::ValkyrieRoot],
    imported_semantic_exports: &[HirDependencySemanticExport],
) -> Vec<VisibleNominalType> {
    fn walk(items: &[StatementNode], namespace: &[String], out: &mut Vec<VisibleNominalType>) {
        for item in items {
            match item {
                StatementNode::Class(class_decl) => {
                    out.push(VisibleNominalType { namespace: namespace.to_vec(), name: class_decl.name.name.clone() })
                }
                StatementNode::Structure(structure) => {
                    out.push(VisibleNominalType { namespace: namespace.to_vec(), name: structure.name.name.clone() })
                }
                StatementNode::Enums(enums) => out.push(VisibleNominalType { namespace: namespace.to_vec(), name: enums.name.name.clone() }),
                StatementNode::Variant(variant) => {
                    out.push(VisibleNominalType { namespace: namespace.to_vec(), name: variant.name.name.clone() })
                }
                StatementNode::Namespace(inner) => {
                    if !crate::compiler::syntax::oak::namespace_is_declaration_only(inner) {
                        let mut nested = namespace.to_vec();
                        nested.extend(lower_name_path(&inner.name).parts().iter().map(ToString::to_string));
                        walk(crate::compiler::syntax::oak::namespace_items(inner), &nested, out);
                    }
                }
                StatementNode::Statement(inner) => walk(std::slice::from_ref(inner.as_ref()), namespace, out),
                _ => {}
            }
        }
    }
    let mut names = Vec::new();
    for export in imported_semantic_exports {
        for item in export.structs.iter() {
            names.push(VisibleNominalType { namespace: item.namespace.iter().map(ToString::to_string).collect(), name: item.name.to_string() });
        }
        for item in export.enums.iter() {
            names.push(VisibleNominalType { namespace: Vec::new(), name: item.name.to_string() });
        }
    }
    for root in roots {
        let mut prefix = Vec::new();
        for item in &root.items {
            if let StatementNode::Namespace(namespace) = item {
                if crate::compiler::syntax::oak::namespace_is_declaration_only(namespace) {
                    prefix = lower_name_path(&namespace.name).parts().iter().map(ToString::to_string).collect();
                }
            }
        }
        walk(&root.items, &prefix, &mut names);
    }
    names
}

/// 收集一个源码根内全部 `type` 别名声明（含 namespace body 与包装语句）。
pub(crate) fn root_type_alias_declarations(root: &crate::compiler::syntax::oak::ValkyrieRoot) -> Vec<(Vec<String>, &TypeAliasDeclaration)> {
    fn walk<'a>(items: &'a [StatementNode], namespace: &[String], out: &mut Vec<(Vec<String>, &'a TypeAliasDeclaration)>) {
        for item in items {
            match item {
                StatementNode::TypeAlias(alias) => out.push((namespace.to_vec(), alias)),
                StatementNode::Namespace(inner) => {
                    if !crate::compiler::syntax::oak::namespace_is_declaration_only(inner) {
                        let mut nested = namespace.to_vec();
                        nested.extend(lower_name_path(&inner.name).parts().iter().map(ToString::to_string));
                        walk(crate::compiler::syntax::oak::namespace_items(inner), &nested, out);
                    }
                }
                StatementNode::Statement(inner) => walk(std::slice::from_ref(inner.as_ref()), namespace, out),
                _ => {}
            }
        }
    }
    let mut prefix = Vec::new();
    for item in &root.items {
        if let StatementNode::Namespace(namespace) = item {
            if crate::compiler::syntax::oak::namespace_is_declaration_only(namespace) {
                prefix = lower_name_path(&namespace.name).parts().iter().map(ToString::to_string).collect();
            }
        }
    }
    let mut declarations = Vec::new();
    walk(&root.items, &prefix, &mut declarations);
    declarations
}

/// `Self` 在其声明作用域内解析出的名义 owner。
///
/// `construct_path` 与 `overload::struct_constructor_path` 使用同一组成（namespace + name），
/// 因此 `Self { … }` 会精确命中 owner 自己的构造器候选；`name` 是候选 owner 使用的短名，
/// 供 `Self::member` 限定路径改写。两者都只是名称解析结果，声明身份仍由候选冻结。
pub(crate) struct SelfOwnerPath {
    pub(crate) construct_path: NamePath,
    pub(crate) name: Identifier,
}

impl SelfOwnerPath {
    pub(crate) fn from_struct(item: &crate::compiler::types::hir::HirStruct) -> Self {
        let mut parts = item.namespace.clone();
        parts.push(item.name.clone());
        Self { construct_path: NamePath::new(parts), name: item.name.clone() }
    }

    pub(crate) fn named(namespace: &[Identifier], name: &Identifier) -> Self {
        let mut parts = namespace.to_vec();
        parts.push(name.clone());
        Self { construct_path: NamePath::new(parts), name: name.clone() }
    }
}

/// `[host_provider(...)]` 实现只通过 Resolver 绑定进入 consumer，不作为依赖导出 API。
pub(crate) fn exportable_dependency_functions(
    functions: &[crate::compiler::types::hir::HirFunction],
) -> Vec<crate::compiler::types::hir::HirFunction> {
    functions.iter().filter(|function| function_host_provider_target(function).is_none()).cloned().collect()
}

pub(crate) fn _integer_literal_u32_placeholder() -> Option<u32> {
    None
}

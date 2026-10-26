use std::collections::{BTreeMap, BTreeSet, VecDeque};

use nyar_types::{
    combined_content_hash,
    compiler_input::{DeclarationIndexEntry, DeclarationKey, DeclarationKind},
};

use crate::compiler::input::CompilerSourceGroup;

use super::{
    ClosureGroupInput, GroupSemanticClosure, SemanticClosure, SemanticClosureError,
    index_file::{IndexedFile, index_source_file},
    indexes::DeclarationIndexes,
};

const CLOSURE_PLANNER_VERSION: &str = "semantic-closure-v1";

pub(crate) fn source_group_closure(
    groups: &[crate::compiler::input::CompilerSourceGroup],
    arch: &str,
) -> Result<SemanticClosure, SemanticClosureError> {
    source_group_closure_with_host_bindings(groups, &[], arch)
}

/// 与 [`source_group_closure`] 相同，但把 Resolver 选定的 host provider 绑定作为外部事实：
/// 被本次入口闭包引用的合同，其选定 provider 声明必须进入 body lowering。
///
/// 未被引用的合同不物化 provider：装配层的选择基于声明库存，不掌握入口可达性。
pub(crate) fn source_group_closure_with_host_bindings(
    groups: &[crate::compiler::input::CompilerSourceGroup],
    host_bindings: &[crate::compiler::emitter::pipeline::CompilerHostProviderBinding],
    arch: &str,
) -> Result<SemanticClosure, SemanticClosureError> {
    let owned = groups
        .iter()
        .map(|group| group.sources.iter().map(|file| (file.path.clone(), file.source.clone())).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let dependency_keys =
        groups.iter().map(|group| group.direct_dependencies.iter().map(String::as_str).collect::<Vec<_>>()).collect::<Vec<_>>();
    let inputs = groups
        .iter()
        .zip(&owned)
        .zip(&dependency_keys)
        .map(|((group, files), dependencies)| ClosureGroupInput {
            dependency_key: group.dependency_key.as_str(),
            module_name: group.name.as_str(),
            direct_dependencies: dependencies.as_slice(),
            is_entry: group.is_entry,
            files: files.as_slice(),
        })
        .collect::<Vec<_>>();
    plan_semantic_closure_with_host_bindings(&inputs, host_bindings, arch)
}

/// 从入口组与其依赖 inventory 规划完整语义闭包；`arch` 决定 TGrammar 模板分支。
pub fn plan_semantic_closure(groups: &[ClosureGroupInput<'_>], arch: &str) -> Result<SemanticClosure, SemanticClosureError> {
    plan_semantic_closure_with_host_bindings(groups, &[], arch)
}

/// 从入口组、依赖 inventory 与 Resolver 的 host provider 选择规划完整语义闭包。
pub fn plan_semantic_closure_with_host_bindings(
    groups: &[ClosureGroupInput<'_>],
    host_bindings: &[crate::compiler::emitter::pipeline::CompilerHostProviderBinding],
    arch: &str,
) -> Result<SemanticClosure, SemanticClosureError> {
    let inventory = groups.iter().map(|group| group.dependency_key).collect::<BTreeSet<_>>();
    // 可见性：本组 + 直接依赖组。这是闭包允许跨组命中声明的唯一范围；
    // 装配层未声明的依赖组不构成可见性，跨组全局名字回退一律不存在。
    let visibility = groups
        .iter()
        .map(|group| {
            let mut visible = BTreeSet::from([group.dependency_key.to_string()]);
            visible.extend(
                group
                    .direct_dependencies
                    .iter()
                    .filter(|dependency| inventory.contains(*dependency))
                    .map(|dependency| (*dependency).to_string()),
            );
            (group.dependency_key.to_string(), visible)
        })
        .collect::<BTreeMap<String, BTreeSet<String>>>();

    let mut indexed_files = BTreeMap::<(String, String), IndexedFile>::new();
    let mut declaration_index = BTreeMap::<DeclarationKey, DeclarationIndexEntry>::new();
    let mut indexes = DeclarationIndexes::default();
    let mut unindexed_source_files = BTreeMap::<String, String>::new();

    for group in groups.iter() {
        for (path, source) in group.files.iter() {
            match index_source_file(group.dependency_key, path, source, arch) {
                Ok(indexed) => {
                    for entry in &indexed.declarations {
                        indexes.register(entry.clone());
                        declaration_index.insert(entry.key.clone(), entry.clone());
                    }
                    for (variant, owner) in &indexed.variant_members {
                        indexes.register_variant(variant, owner);
                    }
                    indexed_files.insert((group.dependency_key.to_string(), path.clone()), indexed);
                }
                Err(reason) => {
                    unindexed_source_files.insert(format!("{}::{path}", group.dependency_key), reason);
                }
            }
        }
    }
    // 聚合身份与成员身份、别名与目标的关联必须在整个 inventory 索引完成后统一按声明身份建立：
    // 聚合与它的成员可以在同组的不同文件里，也可以跨依赖组（std 的 `imply i64: Hash` 与 core 的
    // `structure i64`），按文件边界或「短名 + namespace」连接都是名字近似。
    indexes.link(&visibility);

    let mut demanded = BTreeSet::new();
    let mut selectors = BTreeSet::<String>::new();
    let mut queue = VecDeque::new();
    let mut touched_files = BTreeSet::<(String, String)>::new();
    for group in groups.iter() {
        if !group.is_entry {
            continue;
        }
        // 入口组只为显式 `[export]` / `[main]` 声明提供 seed。
        // 「文件属于入口组」或「组顺序末项」都不是需求事实，不能提升组内全部声明。
        let mut seeded = false;
        for (path, _) in group.files.iter() {
            let file_id = (group.dependency_key.to_string(), path.clone());
            let Some(indexed) = indexed_files.get(&file_id)
            else {
                continue;
            };
            if !indexed.declarations.iter().any(|entry| entry.entry_point.is_some()) {
                continue;
            }
            for entry in &indexed.declarations {
                if entry.entry_point.is_some() {
                    queue.push_back(entry.key.clone());
                    seeded = true;
                }
            }
            // 入口声明所在文件的 `using`/顶层语句是程序的一部分，
            // 必须与声明引用一起展开，否则只含模块级语句的文件会缺依赖。
            if touched_files.insert(file_id) {
                push_scoped_references(indexed, group.dependency_key, &visibility, &indexes, &mut selectors, &mut queue);
            }
        }
        if !seeded {
            let prefix = format!("{}::", group.dependency_key);
            let unindexed_source_files = unindexed_source_files
                .range(prefix.clone()..)
                .take_while(|(file, _)| file.starts_with(&prefix))
                .map(|(file, reason)| (file[prefix.len()..].to_string(), reason.clone()))
                .collect();
            return Err(SemanticClosureError::MissingEntryDeclaration {
                dependency_key: group.dependency_key.to_string(),
                unindexed_source_files,
            });
        }
    }
    // Resolver 选定的 host provider 是需求 seed，与入口声明同等对待：必须先进入固定点，
    // 才能把 provider 引用的类型、成员与辅助函数纳入闭包。若在固定点之后才插入 demanded，
    // provider 的引用永远不被遍历，被引用的聚合不会进入依赖组 HIR，随后类型绑定必然失败。
    enqueue_selected_host_providers(host_bindings, &declaration_index, &mut queue)?;

    let empty_visibility = BTreeSet::new();
    loop {
        while let Some(key) = queue.pop_front() {
            if !demanded.insert(key.clone()) {
                continue;
            }
            let entry = match declaration_index.get(&key) {
                Some(entry) => entry.clone(),
                None => continue,
            };
            let visible = visibility.get(&key.dependency_key).unwrap_or(&empty_visibility);
            if touched_files.insert((key.dependency_key.clone(), key.source_path.clone())) {
                if let Some(file) = indexed_files.get(&(key.dependency_key.clone(), key.source_path.clone())) {
                    push_scoped_references(file, &key.dependency_key, &visibility, &indexes, &mut selectors, &mut queue);
                }
            }
            // 成员方法被需求时，其拥有者聚合的声明外壳同样必须进入闭包。
            if let Some(owner) = &key.owner {
                for resolved in indexes.resolve(&[owner.clone()], &key.namespace, visible) {
                    queue.push_back(resolved);
                }
            }
            selectors.extend(entry.selectors.iter().cloned());
            for reference in &entry.references {
                for resolved in indexes.resolve(reference, &key.namespace, visible) {
                    queue.push_back(resolved);
                }
            }
        }
        // 选择名驱动的成员提升：成员调用的接收者类型在索引阶段不可知，
        // 因此只有「该名字确实出现在闭包需求中」且「其拥有聚合已被需求」两条
        // 结构化事实同时成立时，成员才作为该聚合的成员进入闭包。
        let mut promoted = BTreeSet::new();
        for aggregate in &demanded {
            if !indexes.is_owner(aggregate) {
                continue;
            }
            for member in indexes.selected_members(aggregate, &selectors) {
                if !demanded.contains(&member) {
                    promoted.insert(member);
                }
            }
        }
        if promoted.is_empty() {
            break;
        }
        queue.extend(promoted);
    }

    let mut closure_groups = Vec::with_capacity(groups.len());
    for group in groups.iter() {
        let dependency_key = group.dependency_key;
        // body lowering 边界只由被需求声明身份产生；入口组不再按文件整体下沉。
        let mut body_lowering_source_paths = BTreeSet::new();
        let mut body_lowering_declarations = BTreeSet::new();
        for key in &demanded {
            if key.dependency_key == dependency_key {
                body_lowering_declarations.insert(key.clone());
                body_lowering_source_paths.insert(key.source_path.clone());
            }
        }
        closure_groups.push(GroupSemanticClosure {
            dependency_key: dependency_key.to_string(),
            body_lowering_source_paths,
            body_lowering_declarations,
        });
    }

    let digest = compute_closure_digest(&closure_groups, &indexed_files, arch);
    Ok(SemanticClosure { groups: closure_groups, declaration_index, demanded_declarations: demanded, unindexed_source_files, digest })
}

fn enqueue_selected_host_providers(
    host_bindings: &[crate::compiler::emitter::pipeline::CompilerHostProviderBinding],
    declaration_index: &BTreeMap<DeclarationKey, DeclarationIndexEntry>,
    queue: &mut VecDeque<DeclarationKey>,
) -> Result<(), SemanticClosureError> {
    for binding in host_bindings {
        if !declaration_index.contains_key(&binding.provider) {
            return Err(SemanticClosureError::HostProviderNotResolved { symbol: binding.provider.encoded() });
        }
        queue.push_back(binding.provider.clone());
    }
    Ok(())
}

/// 构造顶层 `micro` 的冻结声明身份。
pub fn micro_declaration_key(dependency_key: &str, source_path: &str, namespace: &[String], name: &str) -> DeclarationKey {
    DeclarationKey {
        dependency_key: dependency_key.to_string(),
        source_path: source_path.to_string(),
        namespace: namespace.to_vec(),
        kind: DeclarationKind::Micro,
        owner: None,
        name: name.to_string(),
    }
}

/// 解析一组「namespace 段 + 名字路径」引用并把命中的声明身份入队；
/// 文件级成员选择名只登记为选择事实，不参与自由名解析。
fn push_scoped_references(
    file: &IndexedFile,
    group: &str,
    visibility: &BTreeMap<String, BTreeSet<String>>,
    indexes: &DeclarationIndexes,
    selectors: &mut BTreeSet<String>,
    queue: &mut VecDeque<DeclarationKey>,
) {
    let empty = BTreeSet::new();
    let visible = visibility.get(group).unwrap_or(&empty);
    selectors.extend(file.module_selectors.iter().cloned());
    for (namespace, reference) in &file.module_references {
        for resolved in indexes.resolve(reference, namespace, visible) {
            queue.push_back(resolved);
        }
    }
}
fn compute_closure_digest(groups: &[GroupSemanticClosure], files: &BTreeMap<(String, String), IndexedFile>, arch: &str) -> String {
    let mut parts = vec![format!("planner:{CLOSURE_PLANNER_VERSION}"), format!("arch:{arch}")];
    for group in groups {
        parts.push(format!("group:{}", group.dependency_key));
        for key in &group.body_lowering_declarations {
            parts.push(format!("decl:{}", key.encoded()));
        }
        for path in &group.body_lowering_source_paths {
            parts.push(format!("path:{path}"));
            if let Some(file) = files.get(&(group.dependency_key.clone(), path.clone())) {
                parts.push(format!("content:{}", file.source_digest));
            }
        }
    }
    let borrowed = parts.iter().map(String::as_str).collect::<Vec<_>>();
    combined_content_hash(&borrowed)
}

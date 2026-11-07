use std::collections::{BTreeMap, BTreeSet, VecDeque};

use nyar_types::{
    combined_content_hash,
    compiler_input::{DeclarationIndexEntry, DeclarationKey, DeclarationKind, DeclarationLink, EntryPointKind},
};

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
                    for entry in indexed.declarations.iter().cloned() {
                        let key = entry.key.clone();
                        indexes.register(entry.clone());
                        match declaration_index.get_mut(&key) {
                            Some(existing) => merge_declaration_entry(existing, entry),
                            None => {
                                declaration_index.insert(key, entry);
                            }
                        }
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
    // 首参为 `self` 的自由 `micro` 只能由方法位置调用命中：`.collect()` 只登记
    // selector，不产生名字引用，因此必须按选择名单独提升，否则依赖组裁剪后
    // consumer 的候选池里根本没有该声明。
    let extension_functions = indexed_files.values().flat_map(|file| file.extension_functions.iter().cloned()).collect::<BTreeSet<_>>();
    // Primitive 自举身份表必须在可见性与需求展开之前建立：`[primitive("…")]` 的意义就是
    // 让 std 尚未进入闭包时，编译器仍能按声明携带的身份识别类型、分配 carrier 布局。
    // 记录自带 `DeclarationKey.dependency_key`，即声明所属可见面：同一依赖组内重复身份
    // 在这里结构化失败，不允许下游按短名回退放行；跨依赖组同一身份是别名载体
    //（封闭生成单元为宿主调用重声明自己用到的标量拼写），规范 owner 优先非生成组。
    let primitive_identities = super::primitive_identity::build_primitive_identity_table(
        indexed_files.values().flat_map(|file| file.primitive_identities.iter().cloned()),
    )?;
    // 聚合身份与成员身份、别名与目标的关联必须在整个 inventory 索引完成后统一按声明身份建立：
    // 聚合与它的成员可以在同组的不同文件里，也可以跨依赖组（std 的 `imply i64: Hash` 与 core 的
    // `structure i64`），按文件边界或「短名 + namespace」连接都是名字近似。
    indexes.link(&visibility);

    // `imply` 块是目标聚合的声明面事实：where 证据（`T: Equal`）在 HIR 调用解析时
    // 必须能在 trait 视图里看到实现，与「是否有调用点以成员形式选中 witness 方法」
    // 无关。这里按 `MemberOwner` 链接把每个 witness 方法解析到聚合声明身份，建立
    // owner → impl 方法 的反查表；保留判定在下面的固定点内按「聚合 + trait 双双需求」执行。
    // 该规则只扩展名字解析上界，body 物化仍由可达性门独立收口。
    let empty_visibility = BTreeSet::new();
    let mut impl_evidence_by_owner = BTreeMap::<DeclarationKey, Vec<DeclarationKey>>::new();
    for (key, entry) in &declaration_index {
        if !matches!(key.kind, DeclarationKind::ImplyMethod | DeclarationKind::Imply) {
            continue;
        }
        let Some(DeclarationLink::MemberOwner(owner_reference)) = &entry.link
        else {
            continue;
        };
        let visible = visibility.get(&key.dependency_key).unwrap_or(&empty_visibility);
        for resolved in indexes.resolve(owner_reference, &key.namespace, visible) {
            // 别名目标按其 `AliasTarget` 链接跟进一层：`imply utf8: Hash` 的 `utf8`
            // 常是 `Utf8Text` 的别名，impl 声明的所有者必须是真实聚合身份。
            let aggregate = if resolved.kind == DeclarationKind::TypeAlias {
                declaration_index.get(&resolved).and_then(|alias| match &alias.link {
                    Some(DeclarationLink::AliasTarget(target)) => {
                        indexes.resolve(target, &resolved.namespace, visible).iter().next().cloned()
                    }
                    _ => None,
                })
            }
            else {
                Some(resolved)
            };
            let Some(aggregate) = aggregate
            else {
                continue;
            };
            if matches!(
                aggregate.kind,
                DeclarationKind::Class | DeclarationKind::Structure | DeclarationKind::Union | DeclarationKind::Enum | DeclarationKind::Flags
            ) {
                impl_evidence_by_owner.entry(aggregate).or_default().push(key.clone());
            }
        }
    }

    let mut demanded = BTreeSet::new();
    let mut selectors = BTreeSet::<String>::new();
    let mut queue = VecDeque::new();
    let mut touched_files = BTreeSet::<(String, String)>::new();
    // 测试入口身份就是测试名本身（run contract 的 `logical_entry`）：重名无法归属结果，
    // 必须在闭包边界结构化失败，而不是让下游择一执行。
    let mut test_entries = BTreeMap::<String, Vec<String>>::new();
    let mut test_entry_names = Vec::new();
    for group in groups.iter() {
        for (path, _) in group.files.iter() {
            let Some(indexed) = indexed_files.get(&(group.dependency_key.to_string(), path.clone()))
            else {
                continue;
            };
            for entry in &indexed.declarations {
                if entry.entry_point == Some(EntryPointKind::Test) {
                    test_entries.entry(entry.key.name.clone()).or_default().push(entry.key.encoded());
                }
            }
        }
    }
    if let Some((test_name, declarations)) = test_entries.iter().find(|(_, declarations)| declarations.len() > 1) {
        return Err(SemanticClosureError::DuplicateTestEntry { test_name: test_name.clone(), declarations: declarations.clone() });
    }
    for group in groups.iter() {
        if !group.is_entry {
            continue;
        }
        // 入口组只为显式 `[export]` / `[main]` / `[test]` 声明提供 seed。
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
                    if entry.entry_point == Some(EntryPointKind::Test) {
                        test_entry_names.push(entry.key.name.clone());
                    }
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
                unseeded_member_families: unseeded_member_families(group, &indexed_files),
            });
        }
    }
    // Resolver 选定的 host provider 是需求 seed，与入口声明同等对待：必须先进入固定点，
    // 才能把 provider 引用的类型、成员与辅助函数纳入闭包。若在固定点之后才插入 demanded，
    // provider 的引用永远不被遍历，被引用的聚合不会进入依赖组 HIR，随后类型绑定必然失败。
    enqueue_selected_host_providers(host_bindings, &declaration_index, &mut queue)?;

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
            if let Some(DeclarationLink::AliasTarget(target)) = &entry.link {
                for resolved in indexes.resolve(target, &key.namespace, visible) {
                    queue.push_back(resolved);
                }
            }
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
        // 方法位置的扩展函数保留：`.collect()` 这类成员调用只在索引阶段贡献
        // selector，接收者类型不可知，既不能产生名字引用，也不属于任何聚合的成员。
        // 保留条件仍是两条声明面事实：选择名确实出现在闭包需求中，且该声明所在
        // 依赖组对某个已需求声明可见。是否真正匹配由 HIR 调用解析按接收者类型决定。
        for extension in &extension_functions {
            if !selectors.contains(&extension.name) || demanded.contains(extension) {
                continue;
            }
            let visible_from_demanded = demanded.iter().any(|demanded_key| {
                visibility.get(&demanded_key.dependency_key).is_some_and(|visible| visible.contains(&extension.dependency_key))
            });
            if visible_from_demanded {
                promoted.insert(extension.clone());
            }
        }
        // 证据驱动的 impl 保留：`T: Equal` 这类 where 证据在 HIR 调用解析时必须能看到
        // 目标聚合的实现，即使没有任何调用点以成员形式选中 witness 方法。保留条件要求
        // 两条声明面事实同时成立——目标聚合已被需求，且 impl 实现的 trait 本身已被需求
        // （trait 别名按 `AliasTarget` 跟进到真实 trait 声明）。「外壳始终物化」会破坏
        // §3.1 的需求闭包上界含义，故不以聚合单独决定保留。
        for aggregate in &demanded {
            let Some(impl_methods) = impl_evidence_by_owner.get(aggregate)
            else {
                continue;
            };
            for method in impl_methods {
                if demanded.contains(method) {
                    continue;
                }
                let Some(entry) = declaration_index.get(method)
                else {
                    continue;
                };
                let visible = visibility.get(&method.dependency_key).unwrap_or(&empty_visibility);
                let implements_demanded_trait = entry.references.iter().any(|reference| {
                    indexes
                        .resolve(reference, &method.namespace, visible)
                        .iter()
                        .any(|resolved| {
                            let trait_key = if resolved.kind == DeclarationKind::TypeAlias {
                                declaration_index.get(resolved).and_then(|alias| match &alias.link {
                                    Some(DeclarationLink::AliasTarget(target)) => indexes
                                        .resolve(target, &resolved.namespace, visible)
                                        .iter()
                                        .find(|key| key.kind == DeclarationKind::Trait)
                                        .cloned(),
                                    _ => None,
                                })
                            }
                            else {
                                Some(resolved.clone())
                            };
                            trait_key.is_some_and(|key| key.kind == DeclarationKind::Trait && demanded.contains(&key))
                        })
                });
                if implements_demanded_trait {
                    promoted.insert(method.clone());
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
    Ok(SemanticClosure {
        groups: closure_groups,
        declaration_index,
        demanded_declarations: demanded,
        test_entries: test_entry_names,
        primitive_identities,
        unindexed_source_files,
        digest,
    })
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

/// 入口组内没有入口注解、但声明了 witness 成员族的 `imply`（按文件 + 拥有者归组）。
///
/// 诊断要指名这些族，让「整组没有入口注解」与「实例成员族缺少表面注解」不再混为一谈：
/// 前者是程序合同缺失，后者是实例入口形态尚未进入表面合同（ABI 决策点）。
fn unseeded_member_families(group: &ClosureGroupInput<'_>, indexed_files: &BTreeMap<(String, String), IndexedFile>) -> Vec<String> {
    let mut families = BTreeMap::<(String, String), BTreeSet<String>>::new();
    for (path, _) in group.files.iter() {
        let Some(indexed) = indexed_files.get(&(group.dependency_key.to_string(), path.clone()))
        else {
            continue;
        };
        for entry in &indexed.declarations {
            if entry.key.kind != DeclarationKind::ImplyMethod {
                continue;
            }
            let Some(owner) = entry.key.owner.as_deref()
            else {
                continue;
            };
            families.entry((path.clone(), owner.to_string())).or_default().insert(entry.key.name.clone());
        }
    }
    families
        .into_iter()
        .map(|((path, owner), names)| format!("`imply {owner} {{ {} }}` ({path})", names.into_iter().collect::<Vec<_>>().join(", ")))
        .collect()
}

/// 合并共享同一 `DeclarationKey` 的多条声明索引事实。
///
/// `DeclarationKey` 只编码「依赖组 + 文件 + namespace + 种类 + owner + 名字」，不含签名：
/// 同一文件内的同名同 arity 顶层 `micro` 重载组（以及同 owner 的同名不同 arity 成员）
/// 因此共享一个身份。需求展开按身份进行，同一身份下每条声明的引用/选择名事实都是
/// 「目标模块能看见哪些声明」的上界来源，必须并集保留。只保留任一条会让其余重载
/// 签名与体内的引用从闭包事实中消失（`std.math.abs(f64)` 的 `f64` 类型引用被同文件
/// `abs(i64)` 覆盖），仅被它引用的聚合连同其运算符实现一起被裁掉，最终在下游降成
/// 同文件同名候选无匹配的伪重载失败。
fn merge_declaration_entry(existing: &mut DeclarationIndexEntry, incoming: DeclarationIndexEntry) {
    existing.references.extend(incoming.references);
    existing.selectors.extend(incoming.selectors);
    existing.import_paths.extend(incoming.import_paths);
    if existing.entry_point.is_none() {
        existing.entry_point = incoming.entry_point;
    }
    if existing.link.is_none() {
        existing.link = incoming.link;
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn type_alias_target_enters_closure_with_split_entry_file() {
        let groups = [
            ClosureGroupInput {
                dependency_key: "stdlib",
                module_name: "stdlib",
                direct_dependencies: &[],
                is_entry: false,
                files: &[
                    ("text/Utf8Text.v".into(), "namespace std.text;\nclass Utf8Text { _repr: [u8] }".into()),
                    ("text/_.v".into(), "namespace! std.text;\ntype utf8 = Utf8Text;".into()),
                ],
            },
            ClosureGroupInput {
                dependency_key: "app",
                module_name: "app",
                direct_dependencies: &["stdlib"],
                is_entry: true,
                files: &[("solution.v".into(), "namespace std.text;\n[export] micro run(value: utf8) -> i32 { return 0 }".into())],
            },
        ];
        let closure = plan_semantic_closure(&groups, "wasm32").expect("closure");
        let std_paths = &closure.groups[0].body_lowering_source_paths;
        assert!(std_paths.contains("text/_.v"), "paths: {std_paths:?}");
        assert!(std_paths.contains("text/Utf8Text.v"), "paths: {std_paths:?}");
    }

    #[test]
    fn imply_body_free_function_reference_pulls_same_file_top_level_micro() {
        // 复现 `std.collection.hash_utf8 unresolved`：imply body 里调用的同文件顶层 micro
        // 必须通过 demand graph 被拉进闭包，而不是靠源码顺序或 workaround。
        let groups = [
            ClosureGroupInput {
                dependency_key: "std",
                module_name: "std",
                direct_dependencies: &[],
                is_entry: false,
                files: &[(
                    "source/collection/HashPrimitives.v".into(),
                    r#"namespace std.collection;

micro hash_utf8(s: utf8) -> usize {
    return 0
}

structure utf8 {
    _repr: [u8]
}

imply utf8: Hash {
    micro hash(self) -> usize {
        return hash_utf8(self)
    }
}
"#
                    .into(),
                )],
            },
            ClosureGroupInput {
                dependency_key: "app",
                module_name: "app",
                direct_dependencies: &["std"],
                is_entry: true,
                files: &[(
                    "solution.v".into(),
                    r#"namespace std.text;
[export] micro run(s: std::collection::utf8) -> usize { return s.hash() }
"#
                    .into(),
                )],
            },
        ];
        let closure = plan_semantic_closure(&groups, "wasm32").expect("closure");
        let std_group = &closure.groups[0];
        let demanded = &std_group.body_lowering_declarations;
        // 顶层 `hash_utf8` 必须进入 demanded；否则 retain 会把它从 AST 里裁掉，
        // lowering 自然找不到它，HIR 就会报 unresolved。
        assert!(
            demanded.iter().any(|key| key.kind == DeclarationKind::Micro && key.name == "hash_utf8"),
            "hash_utf8 must be demanded; actual demanded: {demanded:#?}"
        );
    }

    #[test]
    fn same_file_imply_free_func_refs_resolve_even_when_free_func_after_imply() {
        // 额外验证：即使 free function 声明在 imply 之后，demand graph 仍能正确拉它进来。
        // 这就是账本里说的 workaround（把 hash_utf8 移到 imply 之前）要证明不必要。
        let groups = [
            ClosureGroupInput {
                dependency_key: "std",
                module_name: "std",
                direct_dependencies: &[],
                is_entry: false,
                files: &[(
                    "source/collection/HashPrimitives.v".into(),
                    r#"namespace std.collection;

structure utf8 {
    _repr: [u8]
}

# imply 在顶层 micro 之前声明
imply utf8: Hash {
    micro hash(self) -> usize {
        return hash_utf8(self)
    }
}

# 顶层 micro 在 imply 之后声明
micro hash_utf8(s: utf8) -> usize {
    return 0
}
"#
                    .into(),
                )],
            },
            ClosureGroupInput {
                dependency_key: "app",
                module_name: "app",
                direct_dependencies: &["std"],
                is_entry: true,
                files: &[(
                    "solution.v".into(),
                    r#"namespace std.text;
[export] micro run(s: std::collection::utf8) -> usize { return s.hash() }
"#
                    .into(),
                )],
            },
        ];
        let closure = plan_semantic_closure(&groups, "wasm32").expect("closure");
        let std_group = &closure.groups[0];
        let demanded = &std_group.body_lowering_declarations;
        assert!(
            demanded.iter().any(|key| key.kind == DeclarationKind::Micro && key.name == "hash_utf8"),
            "hash_utf8 must be demanded even when declared after imply; actual demanded: {demanded:#?}"
        );
    }
}

use std::collections::{BTreeMap, BTreeSet};

use nyar_types::compiler_input::{DeclarationIndexEntry, DeclarationKey, DeclarationLink, ReferencePath};

/// 声明索引：闭包规划消费的唯一名字事实来源。
///
/// 索引只登记声明身份与结构关系（依赖组、namespace、种类、拥有者），不复制 HIR 事实：
/// namespace 链与可见组回退都只是「目标模块能看见哪些声明」的上界判定，
/// 精确重载解析、类型匹配与歧义判定仍由 HIR 完成。
#[derive(Debug, Default)]
pub(crate) struct DeclarationIndexes {
    /// `(namespace, 短名) → 自由声明`；namespace 链解析优先使用它。成员声明不登记：
    /// 成员选择名不建立全库同名索引，只能由「拥有者聚合身份 + 选择名」产生。
    names: BTreeMap<(Vec<String>, String), BTreeSet<DeclarationKey>>,
    /// `短名 → 自由声明`；只用于可见组内的回退查询。
    by_name: BTreeMap<String, BTreeSet<DeclarationKey>>,
    /// 变体名 → 拥有它的 sum/unite 声明。
    variant_owners: BTreeMap<String, BTreeSet<DeclarationKey>>,
    /// 待连接的成员声明：`(依赖组, namespace, 拥有者引用, 成员身份)`。
    pending_members: Vec<PendingMember>,
    /// 待展开的类型别名目标：别名身份 → `(别名 namespace, 目标引用)`。
    alias_targets: BTreeMap<DeclarationKey, (Vec<String>, ReferencePath)>,
    /// 名义聚合声明 → 它的成员声明。
    members: BTreeMap<DeclarationKey, BTreeSet<DeclarationKey>>,
    /// 名义聚合声明 → 带宿主直通角色标注的成员声明（声明级需求事实）。
    host_abi_members: BTreeMap<DeclarationKey, BTreeSet<DeclarationKey>>,
}

/// 一条尚未连接的成员声明：拥有者只以引用路径记录，连接时按声明身份解析。
#[derive(Debug)]
struct PendingMember {
    group: String,
    namespace: Vec<String>,
    owner_reference: ReferencePath,
    key: DeclarationKey,
    host_abi_role: bool,
}

impl DeclarationIndexes {
    /// 登记一条声明。
    ///
    /// 成员声明（`method` / `imply_method`）只进入待连接的拥有者关联，不进入任何短名索引；
    /// 类型别名记录目标引用，供成员拥有者解析时穿透别名取到真正的名义聚合。
    pub(crate) fn register(&mut self, entry: DeclarationIndexEntry) {
        let DeclarationIndexEntry { key, link, host_abi_role, .. } = entry;
        if key.kind.is_member() {
            // 拥有者引用不可解析（例如 imply 目标不是名字路径）时 fail-closed：
            // 该成员既不会被裸名命中，也不会被提升，只作为不可达声明存在。
            if let Some(DeclarationLink::MemberOwner(owner_reference)) = link {
                self.pending_members.push(PendingMember {
                    group: key.dependency_key.clone(),
                    namespace: key.namespace.clone(),
                    owner_reference,
                    key,
                    host_abi_role,
                });
            }
            return;
        }
        if let Some(DeclarationLink::AliasTarget(target)) = link {
            self.alias_targets.insert(key.clone(), (key.namespace.clone(), target));
        }
        self.names.entry((key.namespace.clone(), key.name.clone())).or_default().insert(key.clone());
        self.by_name.entry(key.name.clone()).or_default().insert(key);
    }

    /// 登记 unity 变体名与它的拥有者 sum/unite 声明。
    pub(crate) fn register_variant(&mut self, variant: &str, owner: &DeclarationKey) {
        self.variant_owners.entry(variant.to_string()).or_default().insert(owner.clone());
    }

    /// 把成员声明挂到其拥有者聚合身份上。
    ///
    /// 连接发生在整个 inventory 索引完成后：拥有者与成员可以在同组的不同文件里，也可以在不同的
    /// 依赖组里。拥有者引用按成员所属组的可见性解析，因此未声明的依赖组不会成为拥有者；
    /// 解析不到拥有者时成员关联不成立，也不会被名字回退补上。
    pub(crate) fn link(&mut self, visibility: &BTreeMap<String, BTreeSet<String>>) {
        let empty = BTreeSet::new();
        for member in std::mem::take(&mut self.pending_members) {
            let visible = visibility.get(&member.group).unwrap_or(&empty);
            for owner in self.resolve_aggregate(&member.owner_reference, &member.namespace, visible) {
                self.members.entry(owner.clone()).or_default().insert(member.key.clone());
                if member.host_abi_role {
                    self.host_abi_members.entry(owner).or_default().insert(member.key.clone());
                }
            }
        }
    }

    /// 该聚合上带宿主直通角色标注的成员：角色由语言声明给出，是声明级需求事实，
    /// 不需要调用点出现同名选择名。
    pub(crate) fn host_abi_members_of(&self, aggregate: &DeclarationKey) -> BTreeSet<DeclarationKey> {
        self.host_abi_members.get(aggregate).cloned().unwrap_or_default()
    }

    /// 解析一个「必须指向名义聚合」的引用：类型别名不是聚合身份，穿透其目标继续解析。
    fn resolve_aggregate(&self, reference: &[String], namespace: &[String], visible: &BTreeSet<String>) -> BTreeSet<DeclarationKey> {
        let mut aggregates = BTreeSet::new();
        let mut frontier = vec![(reference.to_vec(), namespace.to_vec())];
        let mut visited = BTreeSet::new();
        while let Some((reference, namespace)) = frontier.pop() {
            if !visited.insert((reference.clone(), namespace.clone())) {
                continue;
            }
            for key in self.resolve(&reference, &namespace, visible) {
                match self.alias_targets.get(&key) {
                    Some((alias_namespace, target)) => frontier.push((target.clone(), alias_namespace.clone())),
                    None => {
                        aggregates.insert(key);
                    }
                }
            }
        }
        aggregates
    }

    /// 该声明身份是否为携带成员的名义聚合。
    pub(crate) fn is_owner(&self, key: &DeclarationKey) -> bool {
        self.members.contains_key(key)
    }

    /// 该聚合上「确实出现在闭包需求中」的成员：选择名是需求事实，拥有者是声明身份，
    /// 两者都成立才提升，不做全库同名成员回退。
    pub(crate) fn selected_members(&self, aggregate: &DeclarationKey, selectors: &BTreeSet<String>) -> BTreeSet<DeclarationKey> {
        self.members
            .get(aggregate)
            .map(|members| members.iter().filter(|member| selectors.contains(&member.name)).cloned().collect())
            .unwrap_or_default()
    }

    /// 只保留可见组内的声明：可见性是跨组命中的唯一闸门，本组之外的未声明依赖不得命中。
    fn retain_visible(&self, keys: &BTreeSet<DeclarationKey>, visible: &BTreeSet<String>) -> BTreeSet<DeclarationKey> {
        keys.iter().filter(|key| visible.contains(&key.dependency_key)).cloned().collect()
    }

    /// 解析单个名字路径引用，返回命中的声明身份集合。
    ///
    /// 命中规则与 `path-resolution.md` 一致：限定路径先按绝对路径，再按当前 namespace 逐级前缀；
    /// 单段名字沿 namespace 链向外查找。**每一步都受可见性约束**：只接受引用方本组与
    /// 直接依赖组的声明，未声明的依赖组即使名字命中也不进入闭包。
    ///
    /// - 限定路径未命中 namespace 时，前缀按同一规则解析为名义聚合身份，再取该聚合的成员身份；
    /// - 单段名字在 namespace 链未命中时，只在可见组内按短名回退，且**只接受非成员声明**：
    ///   成员调用按接收者类型解析的事实由「聚合身份 + 已出现的选择名」提供；
    /// - 都不是时只允许变体名命中其拥有者 sum/unite 声明这一条结构化路径。
    pub(crate) fn resolve(&self, reference: &[String], namespace: &[String], visible: &BTreeSet<String>) -> BTreeSet<DeclarationKey> {
        let name = match reference.last() {
            Some(name) => name,
            None => return BTreeSet::new(),
        };
        if reference.len() > 1 {
            let prefix = &reference[..reference.len() - 1];
            if let Some(keys) = self.names.get(&(prefix.to_vec(), name.clone())) {
                let resolved = self.retain_visible(keys, visible);
                if !resolved.is_empty() {
                    return resolved;
                }
            }
            for depth in (0..=namespace.len()).rev() {
                let mut scoped = namespace[..depth].to_vec();
                scoped.extend_from_slice(prefix);
                if let Some(keys) = self.names.get(&(scoped, name.clone())) {
                    let resolved = self.retain_visible(keys, visible);
                    if !resolved.is_empty() {
                        return resolved;
                    }
                }
            }
            // `<聚合>.<成员>`：解析聚合身份后按拥有者身份取成员，不按名字认亲。
            let mut members = BTreeSet::new();
            for owner in self.resolve(prefix, namespace, visible) {
                if let Some(keys) = self.members.get(&owner) {
                    members.extend(keys.iter().filter(|key| key.name == *name).cloned());
                }
            }
            return members;
        }
        // 单段名字沿 namespace 链向外查找，链上未命中时只在可见组内按短名回退。
        // 这里不区分「内建」与用户声明：`usize` / `utf8` 等同样是由源码声明、拥有
        // `[primitive]` 注解的普通类型身份，按名字表放行或拒绝都属于第二份声明事实。
        for depth in (0..=namespace.len()).rev() {
            let scoped = namespace[..depth].to_vec();
            if let Some(keys) = self.names.get(&(scoped, name.clone())) {
                let resolved = self.retain_visible(keys, visible);
                if !resolved.is_empty() {
                    return resolved;
                }
            }
        }
        if let Some(keys) = self.by_name.get(name) {
            let resolved = self.retain_visible(keys, visible);
            if !resolved.is_empty() {
                return resolved;
            }
        }
        self.variant_owners.get(name).map(|keys| self.retain_visible(keys, visible)).unwrap_or_default()
    }
}

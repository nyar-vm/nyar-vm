//! 物化可达性门回归合同。
//!
//! semantic-closure-v1 闭包只是名字解析上界；门在 HIR 冻结调用图上从根集合
//! （`[export]` / `[main]` / `[test]`、Resolver 选定 host provider、singleton
//! 生命周期成员）沿 callee 边求可达闭包。
//! 不可达 callable 不得进入泛型实例化、MIR 降低与 backend `item_instances` 物化集合。
//!
//! 锁定三条事实：
//! 1. 选择名驱动提升进闭包上界的 impl 成员（`Wrapper::get`：选择名 `get` 因
//!    `boxed.get()` 出现在需求中、`Wrapper` 作为参数类型被需求，成员随之进入 HIR）
//!    在闭包上界内、但不在门的声明可达集内；被调用的 `Box.new` / `Box.get` 与入口
//!    `run` 在门内。闭包本身只裁「名字从未被需求」的成员，门裁「无 Invoke 边」的 callable。
//! 2. 门不可达成员的实例不得进入 canonical 产物 `linked.item_instances`；
//!    可达成员的铸造实例必须在产物内。
//! 3. Resolver 选定的 `[host_provider]` 声明即使没有任何源码调用点，也必须作为
//!    物化根进入门（否则合同重映射在链接边界找不到 provider 实现）。

use std::collections::BTreeSet;

use rusty_valkyrie::{
    CompilerSourceGroup, ValkyrieCompiler,
    compiler::{
        emitter::pipeline::CompilerHostProviderBinding,
        hir::{compute_hir_materialization_gate, lowering::DEFAULT_COMPILE_ARCH},
        semantic::closure::micro_declaration_key,
        types::hir::HirModule,
    },
};

const TEST_ARCH: &str = "native";

/// 组模式下 `i64` 必须由依赖组声明为 primitive 结构；否则残留 `Named("i64")`
/// 会以 CAN003 拒绝进入 canonical 类型表。
const CORE_PRIMITIVE_SOURCE: &str = "namespace std::primitive;\n\
     [primitive(\"std::primitive::i64\")] structure i64 { }";

fn core_group() -> CompilerSourceGroup {
    CompilerSourceGroup::single_source("core", "core", CORE_PRIMITIVE_SOURCE)
}

/// 依赖组：`Box<T>` 与 `Wrapper<T>` 共享成员名 `get`。入口只调用 `Box` 的成员；
/// `Wrapper::get` 靠选择名提升进入闭包上界（`Wrapper` 作为参数类型被需求），
/// 但在冻结调用图上没有任何 callee 边。
const LIBRARY: &str = r#"
namespace lib;

class Box<T> {
    _value: T
}

class Wrapper<T> {
    _inner: T
}

imply Box<T> {
    micro new(value: T): Self {
        return Box { _value: value }
    }

    micro get(self) -> T {
        return self._value
    }
}

imply Wrapper<T> {
    micro get(self) -> T {
        return self._inner
    }
}
"#;

/// 入口组：参数类型需求 `Wrapper`、调用点需求选择名 `get`；`Wrapper::get` 不得进门。
const ENTRY: &str = r#"
namespace app;

using lib.{Box, Wrapper};

[export] micro run(wrapper: Wrapper<i64>, value: i64) -> i64 {
    let boxed = Box.new(value)
    return boxed.get()
}
"#;

fn gate_groups() -> Vec<CompilerSourceGroup> {
    vec![
        core_group(),
        CompilerSourceGroup::single_source("lib", "lib", LIBRARY).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "app", ENTRY).as_entry().with_direct_dependencies(vec!["core".into(), "lib".into()]),
    ]
}

/// 按拥有聚合短名 + 成员名在 `imply` 表里恢复冻结声明身份。
///
/// struct 的名义声明身份即其 `constructor_declaration`（绑定阶段以该 ItemId 充当
/// `ValkyrieType::Nominal` 的名义身份），impl target 绑定后按该身份归属拥有者。
fn imply_member_declaration(modules: &[HirModule], owner: &str, name: &str) -> nyar_types::ItemId {
    let owner_declarations = modules
        .iter()
        .flat_map(|module| module.structs.iter())
        .filter(|structure| structure.name.as_str() == owner)
        .filter_map(|structure| structure.constructor_declaration)
        .collect::<BTreeSet<_>>();
    let members = modules
        .iter()
        .flat_map(|module| module.impls.iter())
        .filter(|implementation| implementation.target.nominal_declaration().is_some_and(|target| owner_declarations.contains(&target)))
        .flat_map(|implementation| implementation.methods.iter())
        .collect::<Vec<_>>();
    members.iter().find(|method| method.name.as_str() == name).and_then(|method| method.declaration).unwrap_or_else(|| {
        panic!(
            "`{owner}::{name}` 必须留在闭包上界并携带声明身份；实际 `{owner}` 成员: {:?}",
            members.iter().map(|m| (m.name.as_str().to_string(), m.declaration)).collect::<Vec<_>>()
        )
    })
}

fn free_function_declaration(modules: &[HirModule], dependency_key: &str, name: &str) -> nyar_types::ItemId {
    modules
        .iter()
        .find(|module| module.dependency_key == dependency_key)
        .and_then(|module| module.functions.iter().find(|function| function.name.as_str() == name))
        .and_then(|function| function.declaration)
        .unwrap_or_else(|| panic!("组 `{dependency_key}` 的自由函数 `{name}` 必须携带声明身份"))
}

#[test]
fn unreachable_impl_member_stays_in_closure_upper_bound_but_not_in_gate() {
    let groups = gate_groups();
    let hirs = ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH).expect("双组泛型闭包必须完成 HIR 物化");
    // 上界事实：`Wrapper::get` 被选择名提升留在闭包内（helper 找不到即 panic），门不裁上界。
    let wrapper_get = imply_member_declaration(&hirs, "Wrapper", "get");
    let box_get = imply_member_declaration(&hirs, "Box", "get");
    let box_new = imply_member_declaration(&hirs, "Box", "new");
    let run = free_function_declaration(&hirs, "app", "run");

    let gate = compute_hir_materialization_gate(&hirs, &[]).expect("门计算必须结构化成功");
    assert!(!gate.reachable_declarations().contains(&wrapper_get), "没有任何可达调用点 callee 边触达的 `Wrapper::get` 不得进入物化可达集");
    assert!(gate.reachable_declarations().contains(&box_get), "`Box.get` 被可达调用点触达，必须在门内");
    assert!(gate.reachable_declarations().contains(&box_new), "`Box.new` 被可达调用点触达，必须在门内");
    assert!(gate.reachable_declarations().contains(&run), "`[export]` 入口必须在门内");
}

#[test]
fn unreachable_member_is_never_instantiated_nor_materialized_in_program() {
    let groups = gate_groups();
    let hirs = ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH).expect("双组泛型闭包必须完成 HIR 物化");
    let wrapper_get = imply_member_declaration(&hirs, "Wrapper", "get");
    let box_get = imply_member_declaration(&hirs, "Box", "get");
    let box_new = imply_member_declaration(&hirs, "Box", "new");

    // 门内单态化：不可达声明不得铸造任何实例副本。
    let minted_for_wrapper_get = hirs
        .iter()
        .flat_map(|module| module.instantiated_callables.iter())
        .filter(|callable| callable.function.declaration == Some(wrapper_get))
        .count();
    assert_eq!(minted_for_wrapper_get, 0, "门不可达的 `Wrapper::get` 不得进入泛型实例化");

    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], TEST_ARCH)
        .expect("入口导出泛型程序必须完整编译");
    let canonical = program.canonical();
    let materialized_for_wrapper_get = canonical.linked.item_instances.values().filter(|record| record.declaration == wrapper_get).count();
    assert_eq!(materialized_for_wrapper_get, 0, "门不可达的 `Wrapper::get` 不得进入 `item_instances` 物化集合");
    assert!(
        canonical.linked.item_instances.values().any(|record| record.declaration == box_get),
        "可达的 `Box.get` 实例必须在 `item_instances` 内"
    );
    assert!(
        canonical.linked.item_instances.values().any(|record| record.declaration == box_new),
        "可达的 `Box.new` 实例必须在 `item_instances` 内"
    );
}

/// 合同 `demo::write` 无体、provider `adaptor::write` 没有任何源码调用点：
/// provider 只能靠 Resolver 绑定进入物化根，否则链接边界的合同重映射会
/// 报「host provider 未在源码闭包中定义」。
const CONTRACT_APP_SOURCE: &str = "[host_contract] micro write(value: i64) -> i64 { } \
    [main] micro main() -> i64 { return write(1) }";
const PROVIDER_ADAPTOR_SOURCE: &str = "[host_provider(demo::write)] micro write(value: i64) -> i64 { return value }";

#[test]
fn selected_host_provider_declaration_is_a_materialization_root() {
    let groups = [
        core_group(),
        CompilerSourceGroup::single_source("adaptor", "adaptor", PROVIDER_ADAPTOR_SOURCE).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "demo", CONTRACT_APP_SOURCE)
            .as_entry()
            .with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ];
    let bindings = vec![CompilerHostProviderBinding {
        contract: micro_declaration_key("app", "demo.v", &[], "write"),
        provider: micro_declaration_key("adaptor", "adaptor.v", &[], "write"),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    }];
    let hirs = ValkyrieCompiler::default()
        .resolve_source_groups_with_host_bindings(&groups, &bindings, TEST_ARCH)
        .expect("host provider 绑定组必须完成 HIR 物化");
    let gate = compute_hir_materialization_gate(&hirs, &bindings).expect("门计算必须结构化成功");

    let provider = free_function_declaration(&hirs, "adaptor", "write");
    assert!(gate.reachable_declarations().contains(&provider), "Resolver 选定的 host provider 必须作为物化根进入门");
    let contract = free_function_declaration(&hirs, "app", "write");
    assert!(gate.reachable_declarations().contains(&contract), "被 `[main]` 调用的合同声明必须可达");

    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &bindings, &[], TEST_ARCH)
        .expect("provider 作为物化根后合同重映射必须完整编译");
}

/// witness 消解出的 impl 泛型成员（`Wrapper<T>::probe`）只在 `run<P: Prober>` 的
/// 约束调用点被到达：代入闭合前，静态声明可达闭包看不到 `probe` 的声明身份。
/// 该克隆体体内对另一个 impl 泛型成员（`Leaf<T>::tag_of`）的调用必须照样结算实例；
/// 旧实现按声明过滤已铸造克隆体，整段体内调用被跳过，`tag_of` 停在 `instance=None`
/// 并被合同验证误报 `InstanceNotClosed`。
const WITNESS_CHAIN_SOURCE: &str = r#"
namespace app;

class Leaf<T> {
    value: T,
    tag: i64
}

imply Leaf<T> {
    micro new(value: T): Self {
        return Leaf { value: value, tag: 1 }
    }

    micro tag_of(self) -> i64 {
        return self.tag
    }
}

trait Prober {
    micro probe(self) -> i64
}

class Wrapper<T> {
    leaf: Leaf<T>
}

imply Wrapper<T> {
    micro new(leaf: Leaf<T>): Self {
        return Wrapper { leaf: leaf }
    }
}

imply Wrapper<T>: Prober {
    micro probe(self) -> i64 {
        return self.leaf.tag_of()
    }
}

micro run<P>(p: P) -> i64
    where P: Prober
{
    return p.probe()
}

[export]
micro entry() -> i64 {
    let seed: i64 = 1
    let wrapper: Wrapper<i64> = Wrapper.new(Leaf.new(seed))
    return run(wrapper)
}
"#;

fn witness_chain_groups() -> Vec<CompilerSourceGroup> {
    vec![
        core_group(),
        CompilerSourceGroup::single_source("app", "app", WITNESS_CHAIN_SOURCE)
            .as_entry()
            .with_direct_dependencies(vec!["core".into()]),
    ]
}

#[test]
fn witness_minted_clone_body_closes_nested_member_instance() {
    let groups = witness_chain_groups();
    let hirs = ValkyrieCompiler::default()
        .resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH)
        .expect("witness 消解出的克隆体体内嵌套 impl 泛型成员调用必须闭合实例");

    let probe = hirs
        .iter()
        .flat_map(|module| module.instantiated_callables.iter())
        .find(|callable| callable.function.name.as_str() == "probe")
        .expect("`Wrapper<i64>::probe` 必须已由 witness 消解铸造");
    let receiver = format!("{:?}", probe.function.params.first().expect("`probe` 必须有接收者").ty);
    assert!(receiver.contains("Integer64"), "`probe` 接收者必须是 `Wrapper<i64>`: {receiver}");
    assert!(!receiver.contains("binder: Some("), "`probe` 接收者必须闭合: {receiver}");

    let tag_of = hirs
        .iter()
        .flat_map(|module| module.instantiated_callables.iter())
        .find(|callable| callable.function.name.as_str() == "tag_of")
        .expect("witness 克隆体体内对 `Leaf::tag_of` 的调用必须铸造实例");
    let owner = format!("{:?}", tag_of.owner_type.as_ref().expect("`tag_of` 实例必须带 owner"));
    assert!(owner.contains("Integer64"), "`tag_of` 必须闭合到 `Leaf<i64>`: {owner}");
    assert!(!owner.contains("binder: Some("), "`tag_of` owner 必须闭合: {owner}");

    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], TEST_ARCH)
        .expect("witness 克隆体实例闭合后整条编译链必须成功");
}

/// 依赖组里 `Tagged: Base` 的 super trait 证据只在库组可见，入口组只能看到导出面。
const SUPER_TRAIT_LIBRARY_SOURCE: &str = r#"
namespace lib;

trait Base {
    micro base(self) -> i64
    micro doubled(self) -> i64 {
        return self.base()
    }
}
trait Tagged: Base { micro tag(self) -> i64 }

class Thing { value: i64 }

imply Thing {
    micro new(value: i64): Self {
        return Thing { value: value }
    }
}

imply Thing: Base {
    micro base(self) -> i64 {
        return self.value
    }
}

imply Thing: Tagged {
    micro tag(self) -> i64 {
        return self.value
    }
}

[export]
micro run<X>(item: X) -> i64
    where X: Tagged
{
    return item.tag()
}

[export]
micro base_of<X>(item: X) -> i64
    where X: Base
{
    return item.base()
}
"#;

const SUPER_TRAIT_ENTRY_SOURCE: &str = r#"
namespace app;

using lib.{Thing, run, base_of};

[export]
micro entry() -> i64 {
    let seed: i64 = 7
    let thing: Thing = Thing.new(seed)
    let tagged: i64 = run(thing)
    let base: i64 = base_of(thing)
    return tagged
}
"#;

fn super_trait_groups() -> Vec<CompilerSourceGroup> {
    vec![
        core_group(),
        CompilerSourceGroup::single_source("lib", "lib", SUPER_TRAIT_LIBRARY_SOURCE)
            .with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "app", SUPER_TRAIT_ENTRY_SOURCE)
            .as_entry()
            .with_direct_dependencies(vec!["core".into(), "lib".into()]),
    ]
}

#[test]
fn imported_super_trait_evidence_closes_concrete_where_witness() {
    let groups = super_trait_groups();
    ValkyrieCompiler::default()
        .resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH)
        .expect("具体实参 `Thing` 的 super trait 证据必须经导出面解析到同一 trait 声明身份");
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], TEST_ARCH)
        .expect("super trait 证据闭合后整条编译链必须成功");
}

/// 同一 impl 内的成员调用：`peek` 体内的 `self.inner()` 也必须拿到具体实例。
const GENERIC_IMPL_SUPER_TRAIT_SOURCE: &str = r#"
namespace app;

trait Base {
    micro base(self) -> i64
    micro doubled(self) -> i64 {
        return self.base()
    }
}
trait Tagged: Base { micro tag(self) -> i64 }

class Thing { value: i64 }
imply Thing {
    micro new(value: i64): Self {
        return Thing { value: value }
    }
}
imply Thing: Base {
    micro base(self) -> i64 {
        return self.value
    }
}
imply Thing: Tagged {
    micro tag(self) -> i64 {
        return self.value
    }
}

class Holder<T> { marker: i64 }
imply Holder<T> {
    micro new(marker: i64): Self {
        return Holder { marker: marker }
    }
    micro peek(self) -> i64
        where T: Tagged
    {
        return self.inner()
    }
    micro inner(self) -> i64
        where T: Tagged
    {
        return self.marker
    }
}

[export]
micro entry() -> i64 {
    let seed: i64 = 5
    let thing: Thing = Thing.new(seed)
    let holder: Holder<Thing> = Holder.new(seed)
    let peeked: i64 = holder.peek()
    let base: i64 = thing.base()
    return peeked
}
"#;

#[test]
fn super_trait_evidence_closes_nested_impl_member_instance() {
    let groups = vec![
        core_group(),
        CompilerSourceGroup::single_source("app", "app", GENERIC_IMPL_SUPER_TRAIT_SOURCE)
            .as_entry()
            .with_direct_dependencies(vec!["core".into()]),
    ];
    let hirs = ValkyrieCompiler::default()
        .resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH)
        .expect("`Holder<Thing>::peek` 体内对 `inner` 的调用必须在 super trait 证据闭合后铸造实例");

    let inners = hirs
        .iter()
        .flat_map(|module| module.instantiated_callables.iter())
        .filter(|callable| callable.function.name.as_str() == "inner")
        .collect::<Vec<_>>();
    assert!(!inners.is_empty(), "`Holder<Thing>::inner` 克隆体必须已铸造");
    // 泛型 impl 成员的开放克隆体保留 binder 是应有的；这里要求的是 `peek` 的具体实参调用
    // 已经把 `Holder<Thing>::inner` 也铸成闭合实例。
    let closed = inners.iter().any(|callable| {
        callable.owner_type.as_ref().is_some_and(|owner| !format!("{owner:?}").contains("binder: Some("))
    });
    let owners = inners.iter().map(|callable| format!("{:?}", callable.owner_type)).collect::<Vec<_>>();
    assert!(closed, "至少一个 `inner` 实例的 owner 必须闭合: {owners:?}");
}

/// witness 实例边可达的 impl 成员体内调用的顶层实现：声明身份不在可达集，
/// 只有实例身份被可达调用点引用。顶层函数也必须按同一判据放行，否则整段实现
/// 被裁掉，链接期报「调用实例 ... 没有定义或显式导入合同」。
/// 这是 `HashMap<utf8,V>::insert` → `utf8: Hash::hash` → `hash_utf8` 的骨架复现。
const WITNESS_REACHED_TOP_LEVEL_IMPLEMENTATION: &str = r#"
namespace app;

trait Hasher {
    micro hash(self) -> i64
}

class Thing { value: i64 }

imply Thing {
    micro new(value: i64): Self {
        return Thing { value: value }
    }
}

micro digest(value: i64) -> i64 {
    return value
}

imply Thing: Hasher {
    micro hash(self) -> i64 {
        return digest(self.value)
    }
}

class Holder<T> { _value: T }

imply Holder<T> {
    micro new(value: T): Self {
        return Holder { _value: value }
    }

    micro peek(self) -> i64
        where T: Hasher
    {
        return self._value.hash()
    }
}

[export]
micro entry() -> i64 {
    let seed: i64 = 7
    let holder: Holder<Thing> = Holder.new(Thing.new(seed))
    return holder.peek()
}
"#;

#[test]
fn witness_reached_top_level_implementation_is_not_gated_out() {
    let groups = vec![
        core_group(),
        CompilerSourceGroup::single_source("app", "app", WITNESS_REACHED_TOP_LEVEL_IMPLEMENTATION)
            .as_entry()
            .with_direct_dependencies(vec!["core".into()]),
    ];
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], TEST_ARCH)
        .expect("witness 实例边可达的顶层实现 `digest` 必须进入 MIR 并闭合链接");
    let canonical = program.canonical();
    assert!(
        canonical.linked.callable_names.values().any(|name| name.parts().last().is_some_and(|part| part.as_str() == "digest")),
        "`digest` 必须被物化；否则 `hash` 体内的调用会在链接期悬空"
    );
}

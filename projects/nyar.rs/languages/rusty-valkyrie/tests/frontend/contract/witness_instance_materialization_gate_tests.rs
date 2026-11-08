//! witness 消解实例的物化门禁合同。
//!
//! 泛型接收者上的 trait 方法调用（`where T: Equal` 内的 `infix ==`）在 HIR 解析期
//! 冻结 `trait_dispatch` evidence，单态化固定点把它消解为具体 impl 成员实例并回填到
//! 铸造克隆体的调用点。物化可达性门必须按「可达克隆体体内的已回填 callee」放行这些
//! 实例：它们的声明身份（impl 成员）不在声明级可达集内，若只按声明门禁收口，链接期
//! 会以「调用实例没有定义或显式导入合同」结构化失败。
//!
//! 本文件冻结：
//! - 正例一：`Array<T>::contains` 体内 `self.get(i).unwrap() == value` 的 witness 实例
//!   端到端物化成功（`contains` 场景）；
//! - 正例二：委托链 `insert` → `find_slot` 体内的 witness 实例端到端物化成功
//!   （哈希表 `find_slot` / `find_insert_slot` 场景的同构形态）；
//! - 反例：具体代入类型没有相应 trait impl 时，where 证据保持 fail-closed，
//!   以 `UnresolvedCall` 结构化拒绝。

use nyar_types::compiler_input::CompilerSourceGroup;
use rusty_valkyrie::{HirError, ValkyrieCompiler};

const PRIMITIVES: &str = "namespace std.primitive;
[primitive(\"std::primitive::u64\")] structure u64 { }
[primitive(\"std::primitive::i64\")] structure i64 { }
[primitive(\"std::primitive::i32\")] structure i32 { }
[primitive(\"std::primitive::bool\")] structure bool { }

type usize = u64;

trait Equal {
    infix `==`(self, other: Self): bool
    infix `!=`(self, other: Self): bool
}

imply u64: Equal {
    infix `==`(self, other: Self): bool { return true }
    infix `!=`(self, other: Self): bool { return false }
}

imply u64 {
    infix `<`(self, other: usize): bool { return true }
    infix `+`(self, other: usize): usize { return self }
}

imply i64: Equal {
    infix `==`(self, other: Self): bool { return true }
    infix `!=`(self, other: Self): bool { return false }
}

imply i32: Equal {
    infix `==`(self, other: Self): bool { return true }
    infix `!=`(self, other: Self): bool { return false }
}
";

const OPTION: &str = "namespace std.types;

micro panic<T>(message: T): void {
}

[tag(OptionKind)]
unite Option<T> {
    [tag(0)]
    Some { value: T },
    [tag(1, default)]
    None,
}

imply Option<T> {
    micro is_none(self): bool {
        match self {
            case Some(value):
                false
            case None:
                true
        }
    }

    micro is_some(self): bool {
        match self {
            case Some(value):
                true
            case None:
                false
        }
    }

    micro unwrap(self): T {
        match self {
            case Some(value):
                value
            case None:
                panic(\"unwrap on `None` value\")
        }
    }
}
";

const ARRAY: &str = "namespace std.collection;

type Equal = std::primitive::Equal;

class Array<T> {
}

imply Array<T> {
    micro length(self): usize { return 0 }

    micro get(self, index: usize): Option<T> {
        let missing: Option<T> = None
        return missing
    }

    micro contains(self, value: T): bool where T: Equal {
        let mut i: usize = 0
        while i < self.length() {
            if self.get(i).unwrap() == value {
                return true
            }
            i = i + 1
        }
        return false
    }
}
";

const TABLE_CHAIN: &str = "namespace std.collection;

type Equal = std::primitive::Equal;

class Table<K> {
}

imply Table<K> {
    micro find_slot(self, key: K): bool where K: Equal {
        return key == key
    }

    micro insert(self, key: K): bool where K: Equal {
        return self.find_slot(key)
    }
}
";

const APP_CONTAINS: &str = "namespace leetcode.app;

[export]
micro touch(array: Array<i32>): bool {
    return array.contains(1)
}

[main]
micro main() {
}
";

const APP_TABLE: &str = "namespace leetcode.app;

[export]
micro touch_table(table: Table<i64>): bool {
    return table.insert(7)
}

[main]
micro main() {
}
";

const APP_OPAQUE: &str = "namespace leetcode.app;

class Opaque {
}

[export]
micro touch_opaque(array: Array<Opaque>, value: Opaque): bool {
    return array.contains(value)
}

[main]
micro main() {
}
";

const HASH_MAP: &str = "namespace std.collection;

type Equal = std::primitive::Equal;

class HashMap<K, V> {
}

imply HashMap<K, V> {
    micro insert(mut self, key: K, value: V): unit
        where K: Equal
    {
        return
    }

    micro contains_key(self, key: K): bool
        where K: Equal
    {
        return key == key
    }
}
";

const APP_INTERSECTION: &str = "namespace leetcode.app;

[export]
micro intersection(mut table: HashMap<i64, bool>, key: i64): bool {
    table.insert(key, true)
    return table.contains_key(key)
}

[main]
micro main() {
}
";

const BOUND_EVIDENCE: &str = "namespace std.bounds;

trait Outer {
    micro outer_mark(self): bool
}

trait Inner {
    micro inner_mark(self): bool
}

structure Opaque {
}

imply Opaque: Outer {
    micro outer_mark(self): bool { return true }
}
";

const BOUND_EVIDENCE_WITH_INNER: &str = "namespace std.bounds;

trait Outer {
    micro outer_mark(self): bool
}

trait Inner {
    micro inner_mark(self): bool
}

structure Opaque {
}

imply Opaque: Outer {
    micro outer_mark(self): bool { return true }
}

imply Opaque: Inner {
    micro inner_mark(self): bool { return true }
}
";

const APP_DELEGATED_BOUND: &str = "namespace leetcode.app;

using std.bounds;

micro inner<T>(value: T): bool
    where T: Inner
{
    return value.inner_mark()
}

micro outer<T>(value: T): bool
    where T: Outer
{
    return inner(value)
}

[export]
micro touch(value: Opaque): bool {
    if value.outer_mark() {
        return outer(value)
    }
    return false
}

[main]
micro main() {
}
";

fn bound_groups(evidence_source: &str) -> Vec<CompilerSourceGroup> {
    let bounds_group = CompilerSourceGroup::single_source("std.bounds", "std.bounds", evidence_source);
    let app_group = CompilerSourceGroup::single_source("leetcode.app", "leetcode.app", APP_DELEGATED_BOUND)
        .with_direct_dependencies(vec!["std.bounds".to_string()])
        .as_entry();
    vec![bounds_group, app_group]
}

fn source_groups(collection_source: &str, app_source: &str) -> Vec<CompilerSourceGroup> {
    let core_group = CompilerSourceGroup::single_source("std.primitive", "std.primitive", PRIMITIVES);
    let types_group =
        CompilerSourceGroup::single_source("std.types", "std.types", OPTION).with_direct_dependencies(vec!["std.primitive".to_string()]);
    let collection_group = CompilerSourceGroup::single_source("std.collection", "std.collection", collection_source)
        .with_direct_dependencies(vec!["std.primitive".to_string(), "std.types".to_string()]);
    let app_group = CompilerSourceGroup::single_source("leetcode.app", "leetcode.app", app_source)
        .with_direct_dependencies(vec!["std.primitive".to_string(), "std.types".to_string(), "std.collection".to_string()])
        .as_entry();
    vec![core_group, types_group, collection_group, app_group]
}

/// 正例一：`where T: Equal` 内 `infix ==` 的 witness 实例必须物化到程序里。
#[test]
fn generic_receiver_infix_witness_instance_is_materialized() {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program(&source_groups(ARRAY, APP_CONTAINS))
        .expect("`contains` 体内的 witness 实例必须经物化门放行并链接成功");
}

/// 正例二：委托链末端的 witness 实例（`insert` → `find_slot` → `==`）同样物化成功。
#[test]
fn delegated_chain_witness_instance_is_materialized() {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program(&source_groups(TABLE_CHAIN, APP_TABLE))
        .expect("委托链末端的 witness 实例必须经物化门放行并链接成功");
}

/// 反例：具体代入类型没有 `Equal` impl 时，where 证据保持 fail-closed。
#[test]
fn concrete_argument_without_trait_impl_is_rejected() {
    let error = ValkyrieCompiler::default()
        .compile_source_groups_to_program(&source_groups(ARRAY, APP_OPAQUE))
        .expect_err("`Opaque` 没有 `Equal` impl 时 `contains` 不得解析");
    assert!(matches!(&error, HirError::UnresolvedCall { .. }), "got {error:?}");
}

const BAG: &str = "namespace std.collection;

type Equal = std::primitive::Equal;

class Bag<T> {
    _items: [T]
}

imply Bag<T> {
    micro put(self, value: T): bool where T: Equal {
        return value == value
    }
}
";

const APP_BAG: &str = "namespace leetcode.app;

[export]
micro touch_bag(bag: Bag<i32>): bool {
    return bag.put(1)
}

[main]
micro main() {
}
";

const APP_BAG_WRONG_LITERAL: &str = "namespace leetcode.app;

[export]
micro touch_bag(bag: Bag<i32>): bool {
    return bag.put(\"x\")
}

[main]
micro main() {
}
";

const TEXT_GRAPH: &str = "namespace std.graph;

type Equal = std::primitive::Equal;

[primitive(\"std::primitive::text\")]
structure text {
}

trait Hash {
    micro hash(self): usize
}

imply text: Equal {
    infix `==`(self, other: Self): bool { return true }

    infix `!=`(self, other: Self): bool { return false }
}

imply text: Hash {
    micro hash(self): usize { return 0 }
}

class Graph<N> {
}

imply Graph<N> {
    micro add_edge(mut self, from: N, to: N): unit where N: Equal & Hash {
        return
    }
}
";

const APP_TEXT_GRAPH: &str = "namespace leetcode.app;

[export]
micro touch_graph(graph: Graph<text>): unit {
    graph.add_edge(\"a\", \"b\")
}

[main]
micro main() {
}
";

/// `DirectedGraph<utf8>` 的同构形态：双 bound（`Equal & Hash`）+ 字符串字面量，
/// receiver 必须把 `N` 钉到 `text`，字面量按文本形参特化，双份证据都按 declaration 结算。
#[test]
#[ignore = "HIR 已通过；剩余 CAN015 文本字面量与名义文本类型的 MIR 身份差异属族 I（HIR 显式转换节点；06 条目 3）"]
fn receiver_seeded_binder_specializes_text_literal_with_two_bounds() {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program(&source_groups(TEXT_GRAPH, APP_TEXT_GRAPH))
        .expect("`Graph<text>` 上的 `add_edge(\"a\", \"b\")` 必须经 receiver 播种与双证据解析");
}

const CLASS_EVIDENCE: &str = "namespace std.evidence;

type Equal = std::primitive::Equal;

class Plain {
}

imply Plain: Equal {
    infix `==`(self, other: Self): bool { return true }

    infix `!=`(self, other: Self): bool { return false }
}

micro touch<T>(x: T): bool where T: Equal {
    return true
}
";

const APP_CLASS_EVIDENCE: &str = "namespace leetcode.app;

using std.evidence::{Plain, touch};

[export]
micro touch_plain(plain: Plain): bool {
    return touch(plain)
}

[main]
micro main() {
}
";

/// 类（class）目标上的 trait impl 必须和结构体一样提供 where 证据：
/// `touch<Plain>` 代入具体类类型时 `Equal` 证据不得缺席。
#[test]
fn class_target_trait_impl_satisfies_where_evidence() {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program(&source_groups(CLASS_EVIDENCE, APP_CLASS_EVIDENCE))
        .expect("class 目标的 `Equal` impl 必须满足 where 证据");
}

/// receiver 实参必须把类泛型 binder 先钉住：`Bag<i32>` 上的 `put(1)`，
/// `T` 由 receiver 播种为 `i32` 名义，字面量再按形参特化并通过 `Equal` 证据；
/// 否则开放 binder 被字面量绑成编码原语，证据必然拒绝。
#[test]
fn receiver_seeded_generic_binder_specializes_literal_argument() {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program(&source_groups(BAG, APP_BAG))
        .expect("`Bag<i32>` 上的 `put(1)` 必须经 receiver 播种解析");
}

/// 反例：receiver 已把 `T` 钉到 `i32` 时，字符串字面量不得塞进同一形参。
#[test]
fn receiver_seeded_generic_binder_rejects_wrong_literal() {
    let error = ValkyrieCompiler::default()
        .compile_source_groups_to_program(&source_groups(BAG, APP_BAG_WRONG_LITERAL))
        .expect_err("`Bag<i32>` 上的 `put(\"x\")` 必须失败闭合");
    assert!(matches!(&error, HirError::UnresolvedCall { .. }), "got {error:?}");
}

/// D4：`intersection-of-two-arrays` 语料形态——`HashMap<i64, bool>` 的
/// `insert` / `contains_key` 都带 `where K: Equal`，具体代入 `i64` 时证据必须进入实例化。
#[test]
fn intersection_of_two_arrays_shape_compiles() {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program(&source_groups(HASH_MAP, APP_INTERSECTION))
        .expect("`HashMap<i64, bool>` 的 `insert` / `contains_key` 必须以 `i64: Equal` 证据实例化");
}

/// D3/D4 反例：解析期被延后的内层约束，必须在具体类型实例化时结算。
///
/// `outer<T>` 只声明 `T: Outer`，体内调用 `inner` 要求 `T: Inner`。解析 `outer` 时 `T`
/// 仍是开放 binder，内层义务只能延后；`Opaque` 只有 `Outer` 证据，实例化 `outer<Opaque>`
/// 后必须结构化失败，不得静默生成一个无 `Inner` 证据的实例。
#[test]
fn delegated_bound_missing_at_instantiation_is_rejected() {
    let error = ValkyrieCompiler::default()
        .compile_source_groups_to_program(&bound_groups(BOUND_EVIDENCE))
        .expect_err("`Opaque` 没有 `Inner` impl 时 `inner` 不得实例化");
    match &error {
        HirError::UnresolvedCall { selector, .. } => {
            assert_eq!(selector.to_string(), "inner", "缺证据必须落在内层委托调用 `inner` 上: {error:?}")
        }
        other => panic!("expected unresolved call, got {other:?}"),
    }
}

/// 正例对照：补上 `imply Opaque: Inner` 后同一委托链必须成功。
#[test]
fn delegated_bound_present_at_instantiation_compiles() {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program(&bound_groups(BOUND_EVIDENCE_WITH_INNER))
        .expect("`Opaque` 具备 `Inner` impl 时委托链必须实例化成功");
}

const DEFAULT_BODY_CHAIN: &str = "namespace std.collection;

trait Sorted {
    micro cmp(self, other: Self): bool

    micro keep_left(self, other: Self): bool {
        return self.cmp(other)
    }
}

imply u64: Sorted {
    micro cmp(self, other: Self): bool { return self < other }
}
";

const APP_DEFAULT_BODY_CHAIN: &str = "namespace leetcode.app;

using std.collection::Sorted;

[export]
micro touch(value: u64): bool {
    return value.keep_left(value)
}

[main]
micro main() {
}
";

/// 正例三：trait 默认体克隆到具体 `Self` 后，体内已回填的内部调用（`keep_left` → impl
/// `cmp`）以及 impl 体内再派生的固有算子实例（`cmp` 体内 `u64 <`）都必须物化。
/// 这两级 callee 的声明身份都不在声明级可达集内，只能按实例可达闭包收口。
#[test]
fn default_body_internal_call_on_concrete_self_is_materialized() {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program(&source_groups(DEFAULT_BODY_CHAIN, APP_DEFAULT_BODY_CHAIN))
        .expect("默认体克隆体内已回填的 callee 实例必须物化并链接成功");
}

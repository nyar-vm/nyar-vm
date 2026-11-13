//! 具体接收者的 trait 成员 witness 必须在进入目标私有计划前完成物化。
//!
//! 回归面：泛型 impl 成员（`imply ArrayList<T>: Length` 这类）被具体接收者调用时，
//! 解析期冻结的 witness 携带的是已选定 impl 成员的声明身份；若单态化固定点不认
//! 这种身份，witness 会存活到 `TargetExecutionPlan`，以「缺少 declaration 实例」
//! 失败。这里锁定整条链：跨包闭包 → 单态化放电 → canonical → 目标私有计划。

use nyar_emitter::TargetExecutionPlan;
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

fn witness_groups() -> Vec<CompilerSourceGroup> {
    vec![
        CompilerSourceGroup::single_source(
            "core",
            "core",
            r#"
namespace std.primitive;

[primitive("std::primitive::i64")]
structure i64 { }

namespace std.operator;

trait Length {
    micro length(self): i64
}

namespace std.collection;

class ArrayList<T> {
    _length: i64
}

imply ArrayList<T> {
    micro new(capacity: i64): Self {
        return ArrayList { _length: capacity }
    }
}

imply ArrayList<T>: std.operator::Length {
    micro length(self): i64 {
        return self._length
    }
}
"#,
        ),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            r#"
namespace demo;

using std.collection::{ArrayList}

[export]
micro probe() -> i64 {
    let list: ArrayList<i64> = ArrayList::new(7)
    return list.length()
}
"#,
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into()]),
    ]
}

/// 泛型 std 形制（跨包 + `imply` 成员 + 具体接收者实例化）必须产出目标私有计划，
/// 不得以「trait witness 缺少 declaration 实例」失败。
#[test]
fn concrete_receiver_trait_member_witness_materializes_into_target_plan() {
    let groups = witness_groups();
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program(&groups)
        .expect("跨包泛型 witness 闭包必须完成 Compiler 链路");
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .expect("具体接收者的 trait 成员 witness 必须物化进目标私有计划");
}

const DEFAULT_METHOD_CORE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i64")]
structure i64 { }

namespace demo.traits;

trait Sum {
    micro base(self): i64

    micro bonus(self): i64 {
        return self.base()
    }

    micro total(self): i64 {
        let direct: i64 = self.base()
        let derived: i64 = self.bonus()
        return direct
    }
}

structure Sample {
    seed: i64
}

imply Sample: Sum {
    micro base(self): i64 {
        return self.seed
    }
}
"#;

const DEFAULT_METHOD_APP: &str = r#"
namespace demo.app;

using demo.traits::{Sample}

[export]
micro probe(sample: Sample) -> i64 {
    return sample.total()
}
"#;

/// 默认方法体（`total`）调用同 trait 的必实现成员（`base`）与另一个默认方法
/// （`bonus`）时，具体 `structure` 接收者必须各自铸造闭合实例；witness 不得以
/// `(trait 默认槽, 具体 self 类型)` 存活到目标私有计划。
#[test]
fn default_method_body_on_concrete_structure_self_materializes() {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", DEFAULT_METHOD_CORE),
        CompilerSourceGroup::single_source("app", "demo", DEFAULT_METHOD_APP).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program(&groups)
        .expect("默认方法体 witness 闭包必须完成 Compiler 链路");
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .expect("默认方法体的具体 self witness 必须物化进目标私有计划");
}

const SORT_CORE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i64")]
structure i64 { }

namespace demo.collection;

trait Comparable {
    micro compare(self, other: Self): bool

    micro less(self, other: Self): bool {
        return self.compare(other)
    }
}

structure Rank {
    value: i64
}

imply Rank: Comparable {
    micro compare(self, other: Rank): bool {
        return true
    }
}

class ArrayList<T> {
    first: T
    second: T
}

imply ArrayList<T> {
    micro new(first: T, second: T): Self {
        return ArrayList { first: first, second: second }
    }
}

[export]
micro sort<X>(items: ArrayList<X>): X
    where X: Comparable
{
    if items.first.less(items.second) {
        return items.first
    }
    return items.second
}
"#;

const SORT_APP: &str = r#"
namespace demo.app;

using demo.collection::{ArrayList, Rank, sort}

[export]
micro probe(rank: Rank) -> Rank {
    let items: ArrayList<Rank> = ArrayList.new(rank, rank)
    return sort(items)
}
"#;

/// 泛型 `sort<T>(items: ArrayList<T>) where T: Comparable` 的体内经 trait 默认方法
/// `less` 派发到必实现成员 `compare`；`T = Rank` 闭包后默认方法实例与体内 witness
/// 都必须按具体 self 类型物化，不得以「trait witness 缺少 declaration 实例」失败。
#[test]
fn generic_sort_default_method_witness_materializes_for_concrete_t() {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", SORT_CORE),
        CompilerSourceGroup::single_source("app", "demo", SORT_APP).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program(&groups)
        .expect("泛型 sort 的 witness 闭包必须完成 Compiler 链路");
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .expect("`sort<Rank>` 的默认方法 witness 必须物化进目标私有计划");
}

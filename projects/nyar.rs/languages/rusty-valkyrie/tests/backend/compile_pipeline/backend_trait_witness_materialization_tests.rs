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

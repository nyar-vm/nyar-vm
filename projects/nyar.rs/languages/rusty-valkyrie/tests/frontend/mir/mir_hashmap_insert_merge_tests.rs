//! `HashMap.insert` 位于 `else` 臂、`if` 臂内嵌套 early return：语句位置的 `if`
//! 不得把 `insert` 被丢弃的返回值带进 merge 块。
//!
//! 回归 G7：修复前 merge 块期待一个实参，而嵌套 `return` 的臂没有产出，
//! 报 `SMIR007 jump arity differs`（block 6）。

use nyar_emitter::TargetExecutionPlan;
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

fn hashmap_insert_groups() -> Vec<CompilerSourceGroup> {
    vec![
        CompilerSourceGroup::single_source(
            "core",
            "core",
            r#"
namespace std.primitive;

[primitive("std::primitive::i64")]
structure i64 { }

namespace std.collection;

class HashMap<K, V> {
    _capacity: i64
}

imply HashMap<K, V> {
    micro new(capacity: i64): Self {
        return HashMap { _capacity: capacity }
    }

    micro insert(self, key: i64, value: i64): i64 {
        self._capacity = key
        return value
    }
}
"#,
        ),
        CompilerSourceGroup::single_source(
            "app",
            "demo",
            r#"
namespace demo;

using std.collection::{HashMap, i64}

[export]
micro probe(flag: bool, flag2: bool) -> bool {
    let mut m: HashMap<i64, i64> = HashMap::new(4)
    if flag {
        if flag2 { return false }
    } else {
        m.insert(1, 2)
    }
    return true
}
"#,
        )
        .as_entry()
        .with_direct_dependencies(vec!["core".into()]),
    ]
}

/// 语句位置 `if` 的 `else` 臂尾是带返回值的 `insert` 调用，该值必须被丢弃，
/// merge 边不得携带实参；整条链必须通过 SMIR 并产出目标私有计划。
#[test]
fn hashmap_insert_in_else_with_nested_early_return_merges_without_value() {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program(&hashmap_insert_groups())
        .expect("G7 fixture 必须完成 Compiler 链路（不得报 jump arity differs）");
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .expect("G7 fixture 必须通过目标私有计划");
}

/// 反例（G7 侧）：人为构造目标块声明了一个参数、跳边却少给一个值，
/// 校验必须继续报 `SMIR007`。语义模型收紧后不得变成"arity 校验放宽"。
#[test]
fn hand_built_jump_missing_argument_is_still_rejected() {
    use rusty_valkyrie::{
        Identifier, MirBlock, MirBlockRef, MirFunction, MirTerminator, MirValue, MirValueOrigin, MirValueRef, NamePath, mir::MirModule,
        validate_semantic_module,
    };
    use rusty_valkyrie::types::hir::ValkyrieType;

    let parameter = MirValueRef(0);
    let mut module: MirModule = rusty_valkyrie::compiler::mir::ssa::test_support::lower_test_module(Vec::new(), Vec::new());
    module.functions.push(MirFunction {
        display_symbol: "fixture.jump_missing_argument".into(),
        symbol: NamePath::new(vec![Identifier::new("fixture"), Identifier::new("jump_missing_argument")]),
        declaration: None,
        instance: None,
        return_type: ValkyrieType::Unit,
        param_types: Vec::new(),
        value_types: [(parameter, ValkyrieType::Boolean)].into_iter().collect(),
        entry: MirBlockRef(0),
        values: vec![MirValue { id: parameter, origin: MirValueOrigin::Temporary }],
        blocks: vec![
            MirBlock {
                id: MirBlockRef(0),
                label: "entry".into(),
                parameters: Vec::new(),
                instructions: Vec::new(),
                terminator: MirTerminator::Jump { target: MirBlockRef(1), arguments: Vec::new() },
            },
            MirBlock {
                id: MirBlockRef(1),
                label: "needs_one".into(),
                parameters: vec![parameter],
                instructions: Vec::new(),
                terminator: MirTerminator::Return { value: None },
            },
        ],
    });

    let error = validate_semantic_module(&module).expect_err("目标块参数未获实参时必须拒绝");
    assert_eq!(error.code, "SMIR007", "必须落在 SMIR007，实际 {error:?}");
    assert!(error.detail.contains("arity"), "必须报 arity 不一致，实际 {}", error.detail);
    assert_eq!(error.function, "fixture.jump_missing_argument");
}

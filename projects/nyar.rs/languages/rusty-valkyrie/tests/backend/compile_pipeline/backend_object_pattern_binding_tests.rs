//! 对象模式按声明字段名绑定的 OVM 真实执行回归。
//!
//! 同一多字段 unite 载荷分别用正序、逆序对象模式与位置解构绑定：
//! 对象模式必须按声明字段名取值，位置解构必须保持逐位语义。
//! 绑定类型由 `tests/frontend/hir/variant_auto_constructor_pattern_tests.rs`
//! 的槽位类型探针覆盖，这里补齐执行层的逐 binder 值断言。
#![cfg(feature = "ovm-lane")]

use nyar_emitter::{FragmentSubmission, TargetExecutionPlan};
use odin_vm::{OdinVm, Value};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

const PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }
"#;

const APP: &str = r#"
namespace demo.app;

unite Pair {
    [constructor]
    Both { left: i32, right: i32 },
    Neither,
}

micro make_pair(left: i32, right: i32) -> Pair {
    return Both(left, right)
}

micro forward_left() -> i32 {
    let pair: Pair = make_pair(1, 2)
    match pair {
        case Both { left, right }:
            left
        case Neither:
            0
    }
}

micro forward_right() -> i32 {
    let pair: Pair = make_pair(1, 2)
    match pair {
        case Both { left, right }:
            right
        case Neither:
            0
    }
}

micro reversed_left() -> i32 {
    let pair: Pair = make_pair(1, 2)
    match pair {
        case Both { right, left }:
            left
        case Neither:
            0
    }
}

micro reversed_right() -> i32 {
    let pair: Pair = make_pair(1, 2)
    match pair {
        case Both { right, left }:
            right
        case Neither:
            0
    }
}

micro positional_right() -> i32 {
    let pair: Pair = make_pair(1, 2)
    match pair {
        case Both(left, right):
            right
        case Neither:
            0
    }
}

micro check_all() -> i32 {
    let a: i32 = forward_left()
    let b: i32 = forward_right()
    let c: i32 = reversed_left()
    let d: i32 = reversed_right()
    let e: i32 = positional_right()
    return a
}

[main]
micro main() -> i32 {
    return check_all()
}
"#;

fn compile_app() -> nyar_types::CompiledProgram {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", PRIMITIVES),
        CompilerSourceGroup::single_source("app", "demo", APP).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], "native")
        .expect("对象模式绑定 fixture 必须完成编译")
}

/// 宿主只能调用**声明导出面**的成员；被测函数不是入口，因此这里用集成测试钩子
/// 显式把它登记为导出（不是「可达即导出」）。冻结 ABI 与正式导出走同一合同。
fn declare_test_export(plan: &mut TargetExecutionPlan, program: &nyar_types::CompiledProgram, symbol_suffix: &str) {
    let instance = program
        .canonical()
        .linked
        .callable_names
        .iter()
        .find(|(_, name)| name.to_string().ends_with(symbol_suffix))
        .map(|(instance, _)| *instance)
        .unwrap_or_else(|| panic!("缺少 `{symbol_suffix}` 的 callable 实例"));
    plan.testing_export_names_mut().insert(instance, symbol_suffix.to_string());
}

fn run_export(program: &nyar_types::CompiledProgram, name: &str) -> i32 {
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let mut plan = TargetExecutionPlan::from_compiled_program(program, &fragment, nyar::TheoryBundle::default(), &roots)
        .expect("ovm 目标计划");
    declare_test_export(&mut plan, program, name);
    let module = nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .expect("ovm 模块必须完成降低");
    let entry = module
        .exports
        .iter()
        .map(|export| export.symbol_name.clone())
        .find(|symbol| symbol.ends_with(name))
        .unwrap_or_else(|| panic!("必须导出 `{name}`"));
    let bytes = acorn_odin::encode_module(&module);
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("对象模式绑定模块必须通过加载期校验");
    let value = vm.run(&loaded, &entry, Vec::new()).expect("被测函数必须在 OVM 内真实执行成功");
    match value {
        Value::I32(value) => value,
        other => panic!("`{name}` 必须返回 i32，实际 {other:?}"),
    }
}

/// `left = 1`、`right = 2`：正序/逆序对象模式都必须按声明字段名读到对应槽位，
/// 位置解构 `(left, right)` 必须保持逐位语义。
///
/// 若对象模式退化为按书写位置绑定，`reversed_left` 会读到 `right` 的值 2，
/// `reversed_right` 会读到 `left` 的值 1，两条断言立即失败。
#[test]
fn object_pattern_binds_by_declared_field_name_on_ovm() {
    let program = compile_app();
    assert_eq!(run_export(&program, "forward_left"), 1, "正序对象模式槽位 0 必须是 `left`");
    assert_eq!(run_export(&program, "forward_right"), 2, "正序对象模式槽位 1 必须是 `right`");
    assert_eq!(run_export(&program, "reversed_left"), 1, "逆序对象模式里的 `left` 必须按字段名读到槽位 0");
    assert_eq!(run_export(&program, "reversed_right"), 2, "逆序对象模式里的 `right` 必须按字段名读到槽位 1");
    assert_eq!(run_export(&program, "positional_right"), 2, "位置解构必须保持逐位语义");
}

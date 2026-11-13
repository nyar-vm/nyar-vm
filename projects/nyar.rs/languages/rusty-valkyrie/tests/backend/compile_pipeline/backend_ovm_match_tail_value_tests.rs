//! 尾 `match` 产值的 OVM 加载/执行回归。
//!
//! 返回 `unite` 的函数以值 `match` 结尾时，穷举 match 的 `no_match` 块是
//! `Unreachable` 终结；它不得物化为裸 `Return` 与值返回在加载期汇合冲突
//! （`return type mismatch: Void vs Value`）。模块必须可加载并在 Odin VM 真实执行。
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

[tag(ProbeOptionKind)]
unite ProbeOption<T> {
    [tag(0)]
    [constructor]
    Some { value: T },
    [tag(1, default)]
    None,
}

micro make_some(value: i32) -> ProbeOption<i32> {
    return Some(value)
}

micro make_none() -> ProbeOption<i32> {
    return None
}

micro tail_match_ref(self: ProbeOption<i32>) -> ProbeOption<i32> {
    match self {
        case Some(value):
            make_some(value)
        case None:
            make_none()
    }
}

micro zero() -> i32 {
    return 0
}

micro unwrap_or_zero(self: ProbeOption<i32>) -> i32 {
    match self {
        case Some(value):
            value
        case None:
            zero()
    }
}

[main]
micro main() -> i32 {
    let input: ProbeOption<i32> = make_some(41)
    let output: ProbeOption<i32> = tail_match_ref(input)
    return unwrap_or_zero(output)
}
"#;

fn compile_app() -> Result<nyar_types::CompiledProgram, String> {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", PRIMITIVES),
        CompilerSourceGroup::single_source("app", "demo", APP).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], "native")
        .map_err(|error| error.to_string())
}

#[test]
fn tail_match_sum_value_loads_and_executes_on_ovm() {
    let program = compile_app().expect("尾 match 产值程序必须完成编译");
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .map_err(|error| error.to_string())
        .expect("ovm 目标计划");
    let module = nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .expect("ovm 模块必须完成降低");
    let entry =
        module.exports.iter().map(|export| export.symbol_name.clone()).find(|name| name.ends_with("main")).expect("必须导出 main 入口");
    let bytes = acorn_odin::encode_module(&module);
    let mut vm = OdinVm::new();
    let loaded = vm.load(&bytes).expect("尾 match 产值模块必须通过加载期校验");
    let value = vm.run(&loaded, &entry, Vec::new()).expect("main 必须在 OVM 内真实执行成功");
    assert_eq!(value, Value::I32(41), "尾 match 的值必须沿汇合槽返回");
}

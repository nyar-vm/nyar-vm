//! 块作用域写回在 OVM 后端的对偶实测。
//!
//! `172b944a` 修的是 MIR builder 的块退出语义（块内对块外已存在绑定的写回必须
//! 保留到后继读取）。本用例不依赖 phi 形状推断，直接把同一最小程序编译到 OVM，
//! 在 Odin VM 里真实执行并按返回值断言，确认后端没有同款缺陷。
//!
//! 前端不接受裸块语句，因此这里用值位置的块表达式承载与 for-in 脱糖相同的
//! `HirExprKind::Block` lowering 路径。
#![cfg(feature = "ovm-lane")]

use nyar_emitter::{FragmentSubmission, TargetExecutionPlan};
use odin_vm::{OdinVm, Value};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

const PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::bool")]
structure bool { }
"#;

const APP: &str = r#"
namespace demo.app;

micro block_writeback(seed: bool) -> bool {
    let mut x: bool = seed
    let ignored: bool = {
        let mut go: bool = true
        while go {
            x = true
            go = false
        }
        true
    }
    return x
}

micro nested_block_writeback(seed: bool) -> bool {
    let mut x: bool = seed
    let ignored: bool = {
        let inner: bool = {
            let mut go: bool = true
            while go {
                x = true
                go = false
            }
            true
        }
        inner
    }
    return x
}

micro break_writeback(seed: bool) -> bool {
    let mut x: bool = seed
    let ignored: bool = {
        loop {
            x = true
            break
        }
        true
    }
    return x
}

micro continue_writeback(seed: bool) -> bool {
    let mut x: bool = seed
    let ignored: bool = {
        let mut go: bool = true
        while go {
            go = false
            x = true
            continue
        }
        true
    }
    return x
}

micro branch_writeback(seed: bool) -> bool {
    let mut x: bool = seed
    let ignored: bool = {
        let mut go: bool = true
        while go {
            if seed {
                x = false
            } else {
                x = true
            }
            go = false
        }
        true
    }
    return x
}

micro shadow_writeback(seed: bool) -> bool {
    let mut x: bool = seed
    let ignored: bool = {
        let mut x: bool = true
        x = false
        x
    }
    return x
}

micro check_all() -> bool {
    let a: bool = block_writeback(false)
    let b: bool = nested_block_writeback(false)
    let c: bool = break_writeback(false)
    let d: bool = continue_writeback(false)
    let e: bool = branch_writeback(false)
    let f: bool = shadow_writeback(true)
    if a {
        if b {
            if c {
                if d {
                    if e {
                        if f {
                            return true
                        }
                    }
                }
            }
        }
    }
    return false
}

[main]
micro main() -> bool {
    return check_all()
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

fn run_export(program: &nyar_types::CompiledProgram, name: &str, seed: bool) -> Value {
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let mut plan = TargetExecutionPlan::from_compiled_program(program, &fragment, nyar::TheoryBundle::default(), &roots)
        .map_err(|error| error.to_string())
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
    let loaded = vm.load(&bytes).expect("模块必须通过 OVM 加载期校验");
    vm.run(&loaded, &entry, vec![Value::Bool(seed)]).expect("被测函数必须在 OVM 内真实执行成功")
}

#[test]
fn block_scope_writeback_returns_loop_value_on_ovm() {
    let program = compile_app().expect("块写回程序必须完成编译");
    assert_eq!(run_export(&program, "block_writeback", false), Value::Bool(true), "块内循环写回必须在块退出后对 return 可见");
}

#[test]
fn nested_block_scope_writeback_returns_loop_value_on_ovm() {
    let program = compile_app().expect("嵌套块写回程序必须完成编译");
    assert_eq!(run_export(&program, "nested_block_writeback", false), Value::Bool(true), "多层块退出都必须把写回传递到最外层");
}

#[test]
fn break_inside_block_scope_writeback_returns_loop_value_on_ovm() {
    let program = compile_app().expect("break 写回程序必须完成编译");
    assert_eq!(run_export(&program, "break_writeback", false), Value::Bool(true), "块内 break 前的写回必须在块退出后对 return 可见");
}

#[test]
fn continue_inside_block_scope_writeback_returns_loop_value_on_ovm() {
    let program = compile_app().expect("continue 写回程序必须完成编译");
    assert_eq!(run_export(&program, "continue_writeback", false), Value::Bool(true), "块内 continue 前的写回必须在块退出后对 return 可见");
}

#[test]
fn branch_merge_inside_block_scope_writeback_returns_on_ovm() {
    let program = compile_app().expect("分支合流写回程序必须完成编译");
    assert_eq!(run_export(&program, "branch_writeback", false), Value::Bool(true), "块内两条分支的写回必须合流后传到 return");
}

#[test]
fn shadowed_block_let_does_not_overwrite_outer_binding_on_ovm() {
    let program = compile_app().expect("遮蔽写回程序必须完成编译");
    assert_eq!(run_export(&program, "shadow_writeback", true), Value::Bool(true), "块内同名 `let` 的赋值不得泄漏为块外绑定");
}

//! 大 `i64` 字面量：源码值必须按位精确进入 OVM 常量池。
//!
//! 旧实现用 `Integer32(value as i32)` + `i32_to_i64`，超出 `i32` 范围的字面量会保留
//! 低 32 位再符号扩展（`i64::MAX` → `-1`）。本文件用 V 源真跑取证，不是指令级断言。

use nyar::TheoryBundle;
use nyar_emitter::{FragmentSubmission, TargetExecutionPlan};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

const PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i64")]
structure i64 { }

[primitive("std::primitive::i32")]
structure i32 { }
"#;

fn groups(statements: &str) -> Vec<CompilerSourceGroup> {
    let core = CompilerSourceGroup::single_source("core", "core", PRIMITIVES);
    let source = format!("namespace demo.primitive.i64_literal;\n\n[main]\nmicro main() -> i64 {{\n{statements}\n}}\n");
    let app = CompilerSourceGroup::single_source("app", "demo", &source).as_entry().with_direct_dependencies(vec!["core".into()]);
    vec![core, app]
}

fn compile(statements: &str) -> Result<nyar_types::CompiledProgram, String> {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups(statements), &[], &[], "native")
        .map_err(|error| error.to_string())
}

fn plan_for(program: &nyar_types::CompiledProgram) -> Result<TargetExecutionPlan, String> {
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    TargetExecutionPlan::from_compiled_program(program, &fragment, TheoryBundle::default(), &roots).map_err(|error| error.to_string())
}

#[cfg(feature = "ovm-lane")]
mod ovm {
    use super::*;
    use nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module;
    use acorn_odin::{OdinConstant, OdinExportKind, encode_module};
    use odin_vm::{executor::Executor, module::LoadedModule, value::Value};

    fn lower_ovm(statements: &str) -> Result<acorn_odin::OdinModuleData, String> {
        let program = compile(statements)?;
        let plan = plan_for(&program)?;
        lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan)).map_err(|error| error.to_string())
    }

    fn run_main(statements: &str) -> Result<Value, String> {
        let module = lower_ovm(statements)?;
        let bytes = encode_module(&module);
        let loaded = LoadedModule::from_bytes(&bytes).map_err(|error| error.to_string())?;
        let entry = loaded
            .exports
            .iter()
            .find(|export| export.kind == OdinExportKind::Function && export.symbol_name.ends_with("main"))
            .ok_or_else(|| "导出表必须包含 main".to_string())?;
        let mut executor = Executor::new();
        executor.run(&loaded, entry.function_index as usize, Vec::new()).map_err(|error| error.to_string())
    }

    /// `i64::MIN` 的源码写法必须原样到达运行时：`-9223372036854775808` 是合法字面量。
    #[test]
    fn i64_lowest_min_literal_executes_exactly() {
        let value = run_main("    return -9223372036854775808").expect("i64::MIN 字面量必须可运行");
        assert_eq!(value, Value::I64(i64::MIN), "`-9223372036854775808` 必须是 i64::MIN");
    }

    /// `i64::MAX` 必须原样到达运行时；旧实现会得到 `-1`。
    #[test]
    fn i64_max_literal_executes_exactly() {
        let value = run_main("    return 9223372036854775807").expect("i64::MAX 字面量必须可运行");
        assert_eq!(value, Value::I64(i64::MAX), "`9223372036854775807` 必须是 i64::MAX");
    }

    /// 刚超出 `i32` 的 `2147483648` 必须精确，不能截断。
    #[test]
    fn just_above_i32_max_executes_exactly() {
        let value = run_main("    return 2147483648").expect("2147483648 必须可运行");
        assert_eq!(value, Value::I64(2_147_483_648), "`2147483648` 必须是精确的 2^31");
    }

    /// `i32` 边界数值精确。
    ///
    /// OVM 是未类型化栈；门禁要求返回操作数与函数声明返回类型一致，因此 `i64` 返回槽
    /// 承载的常量以 `I64` 到达运行时（`emit_load_constant` 先发 `Integer32`，
    /// 返回边界再按槽类型拓宽）。数值必须精确。
    #[test]
    fn i32_boundaries_stay_exact() {
        assert_eq!(run_main("    return 2147483647").expect("i32::MAX 必须可运行"), Value::I64(2_147_483_647));
        assert_eq!(run_main("    return -2147483648").expect("i32::MIN 必须可运行"), Value::I64(-2_147_483_648));
    }
}

//! `f64` 目标合同端到端：V 源 → canonical → `TargetExecutionPlan` → OVM 模块 → 真实执行。
//!
//! 锁定的核心事实：`Float { bits: 64 }` 在计划期必须被登记为 `NyarType::Float64`，
//! 否则 `__f64_add` / `__f64_lt` / `math_sqrt` / `f64_to_i32` 的签名在计划期就被拒，
//! 宿主能力一条都到不了 OVM 运行时。
//!
//! 浮点**字面量**当前在 canonical 层被 `CAN012` 拒绝（"浮点常量没有 canonical 形状"），
//! 因此本测试用 `i64_to_f64` + `f64_div` 构造 2.5、4.0、NaN 与 1.5/2.25，
//! 不绕过 canonical 边界，也不把 f64 降成 i64 位模式。

use nyar::TheoryBundle;
use nyar_emitter::{FragmentSubmission, TargetExecutionPlan};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

const CONTRACTS: &str = r#"
namespace demo.primitive.f64_ops;

[primitive("std::primitive::bool")]
structure bool { }

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::i64")]
structure i64 { }

[primitive("std::primitive::f64")]
structure f64 { }

[host_contract]
micro __i64_to_f64(value: i64): f64

[host_contract]
micro __f64_add(lhs: f64, rhs: f64): f64

[host_contract]
micro __f64_div(lhs: f64, rhs: f64): f64

[host_contract]
micro __f64_lt(lhs: f64, rhs: f64): bool

[host_contract]
micro __f64_to_i32(value: f64): i32

[host_contract]
micro __math_sqrt(value: f64): f64
"#;

const PROVIDERS: &str = r#"
namespace demo.adaptor.f64;

[host_provider(demo::primitive::f64_ops::__i64_to_f64)]
[ovm("i64_to_f64")]
micro i64_to_f64(value: i64): f64

[host_provider(demo::primitive::f64_ops::__f64_add)]
[ovm("f64_add")]
micro f64_add(lhs: f64, rhs: f64): f64

[host_provider(demo::primitive::f64_ops::__f64_div)]
[ovm("f64_div")]
micro f64_div(lhs: f64, rhs: f64): f64

[host_provider(demo::primitive::f64_ops::__f64_lt)]
[ovm("f64_lt")]
micro f64_lt(lhs: f64, rhs: f64): bool

[host_provider(demo::primitive::f64_ops::__f64_to_i32)]
[ovm("f64_to_i32")]
micro f64_to_i32(value: f64): i32

[host_provider(demo::primitive::f64_ops::__math_sqrt)]
[ovm("math_sqrt")]
micro math_sqrt(value: f64): f64
"#;

fn provider_bindings() -> Vec<rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding> {
    use rusty_valkyrie::compiler::semantic::closure::micro_declaration_key;
    let ops_ns = ["demo".to_owned(), "primitive".to_owned(), "f64_ops".to_owned()];
    let adaptor_ns = ["demo".to_owned(), "adaptor".to_owned(), "f64".to_owned()];
    [
        ("i64_to_f64", "i64_to_f64"),
        ("f64_add", "f64_add"),
        ("f64_div", "f64_div"),
        ("f64_lt", "f64_lt"),
        ("f64_to_i32", "f64_to_i32"),
        ("math_sqrt", "math_sqrt"),
    ]
    .into_iter()
    .map(|(contract, provider)| rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding {
        contract: micro_declaration_key("core", "core.v", &ops_ns, &format!("__{contract}")),
        provider: micro_declaration_key("adaptor", "adaptor.v", &adaptor_ns, provider),
        required_capabilities: vec![],
        foreign_bindings: vec![],
    })
    .collect()
}

fn app(return_type: &str, statements: &str) -> String {
    format!("namespace demo.primitive.f64_ops;\n\n[main]\nmicro main() -> {return_type} {{\n{statements}\n}}\n")
}

fn groups(return_type: &str, statements: &str) -> Vec<CompilerSourceGroup> {
    let core = CompilerSourceGroup::single_source("core", "core", CONTRACTS);
    let adaptor = CompilerSourceGroup::single_source("adaptor", "adaptor", PROVIDERS).with_direct_dependencies(vec!["core".into()]);
    let app = CompilerSourceGroup::single_source("app", "demo", &app(return_type, statements))
        .as_entry()
        .with_direct_dependencies(vec!["core".into(), "adaptor".into()]);
    vec![core, adaptor, app]
}

fn compile(return_type: &str, statements: &str) -> Result<nyar_types::CompiledProgram, String> {
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups(return_type, statements), &provider_bindings(), &[], "native")
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
    use odin_bytecode::{OdinExportKind, OdinHeadCode, encode_module};
    use odin_vm::{executor::Executor, module::LoadedModule, value::Value};

    fn lower_ovm(return_type: &str, statements: &str) -> Result<odin_bytecode::OdinModuleData, String> {
        let program = compile(return_type, statements)?;
        let plan = plan_for(&program)?;
        lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan)).map_err(|error| error.to_string())
    }

    fn run_main(return_type: &str, statements: &str) -> Result<Value, String> {
        let module = lower_ovm(return_type, statements)?;
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

    /// 计划期必须接受 `Float { bits: 64 }`，且三个符号都留在 `nyar.host` 导入表。
    #[test]
    fn f64_plan_accepts_float64_and_keeps_host_imports() {
        let module = lower_ovm(
            "f64",
            "    let lhs = __f64_div(__i64_to_f64(3), __i64_to_f64(2))\n    let rhs = __f64_div(__i64_to_f64(9), __i64_to_f64(4))\n    return __f64_add(lhs, rhs)",
        )
        .expect("`Float { bits: 64 }` 必须有目标合同，f64 宿主符号才能进入 OVM");
        for symbol in ["i64_to_f64", "f64_div", "f64_add"] {
            assert!(
                module.imports.iter().any(|import| import.module_name == "nyar.host" && import.symbol_name == symbol),
                "`{symbol}` 必须保留 nyar.host 导入: {:?}",
                module.imports
            );
        }
        assert!(module.code_bytes.contains(&(OdinHeadCode::CallImport as u8)), "f64 调用必须发射 CallImport");
    }

    /// 1.5 + 2.25 = 3.75，走真实 `Value::F64` 载体（不是 i64 位模式）。
    #[test]
    fn f64_add_executes_to_three_point_seventy_five() {
        let value = run_main(
            "f64",
            "    let lhs = __f64_div(__i64_to_f64(3), __i64_to_f64(2))\n    let rhs = __f64_div(__i64_to_f64(9), __i64_to_f64(4))\n    return __f64_add(lhs, rhs)",
        )
        .expect("OVM 必须完成 f64 加法");
        assert_eq!(value, Value::F64(3.75), "1.5 + 2.25 必须是 f64 3.75");
    }

    /// `math_sqrt(4.0)` = 2.0，负数与 NaN 的 IEEE 语义由运行时宿主符号保证。
    #[test]
    fn math_sqrt_of_four_executes_to_two() {
        let value = run_main("f64", "    return __math_sqrt(__i64_to_f64(4))").expect("OVM 必须完成 math_sqrt");
        assert_eq!(value, Value::F64(2.0), "sqrt(4.0) 必须是 f64 2.0");
    }

    /// `f64_to_i32(2.5)` 朝零截断为 2。
    #[test]
    fn f64_to_i32_of_two_point_five_executes_to_two() {
        let value = run_main("i32", "    return __f64_to_i32(__f64_div(__i64_to_f64(5), __i64_to_f64(2)))").expect("OVM 必须完成 f64_to_i32");
        assert_eq!(value, Value::I32(2), "f64_to_i32(2.5) 必须是 i32 2（朝零截断）");
    }

    /// 有序比较：`1.0 < 2.0` 为真，证明比较语义确实穿透到运行时。
    #[test]
    fn f64_lt_ordered_comparison_executes_true() {
        let value = run_main("bool", "    return __f64_lt(__i64_to_f64(1), __i64_to_f64(2))").expect("OVM 必须完成 f64_lt");
        assert_eq!(value, Value::Bool(true), "1.0 < 2.0 必须为 true");
    }

    /// `NaN < 1.0` 必须为 false：NaN 在运行时产生（`0.0 / 0.0`），不是计划期常量。
    #[test]
    fn f64_lt_with_nan_executes_false() {
        let value = run_main("bool", "    let nan = __f64_div(__i64_to_f64(0), __i64_to_f64(0))\n    return __f64_lt(nan, __i64_to_f64(1))")
            .expect("OVM 必须完成 NaN 比较");
        assert_eq!(value, Value::Bool(false), "NaN < 1.0 必须为 false");
    }
}

//! 未捕获 `raise` 的底类型建模：当前块直接以 `Unreachable` 终结，不冒充值返回。
//!
//! 正例：值返回函数的尾 `raise` 必须完成编译，且 MIR 中不得出现把 `Unit` 常量当
//! 返回值的 `Return`。
//! 反例：普通 `unit` 函数正常返回后被当值使用，必须保持结构化失败——本修复只把
//! 「不可恢复 `raise`」建模为不可达，没有放宽 `Unit → T`。

use rusty_valkyrie::{
    CompilerSourceGroup, MirLowerer, ValkyrieCompiler,
    compiler::{
        mir::{
            ssa::{MirConstant, MirModule, MirOperand, MirTerminator},
            validation::validate_module,
        },
        types::hir::ValkyrieType,
    },
    types::hir::HirModule,
};

const PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::bool")]
structure bool { }

[primitive("std::primitive::i64")]
structure i64 { }
"#;

/// 与 `panic-explicit` 同一形态：文本载体进入 `Resume` 形参分支。
const TEXT_RAISE_APP: &str = r#"
namespace demo;

[export(name: "explicitPanic")]
micro explicit_panic() -> i64 {
    raise "probe panic"
}

micro numeric_panic() -> i64 {
    raise 42
}
"#;

/// 名义载体：`Resume = !`，恢复块不声明形参（`void` 不占物理槽）。
const NOMINAL_RAISE_APP: &str = r#"
namespace demo;

class ProbeError {
    code: i64
}

[export(name: "nominalPanic")]
micro nominal_panic() -> i64 {
    raise ProbeError { code: 1 }
}
"#;

fn source(primitives: &str, app: &str) -> String {
    format!("{primitives}\n{app}")
}

fn lower(text: &str) -> HirModule {
    ValkyrieCompiler::default().compile_source(text).expect("fixture 必须完成 HIR 分析")
}

fn function<'a>(module: &'a MirModule, name: &str) -> &'a rusty_valkyrie::compiler::mir::ssa::MirFunction {
    module.functions.iter().find(|function| function.display_symbol.contains(name)).unwrap_or_else(|| panic!("fixture 缺少 `{name}`"))
}

/// 断言该函数的未捕获 `raise` 以 `Unreachable` 终结，且没有任何 `Unit` 返回值，
/// 也不保留 `PerformEffect { Raise }`（那会要求尚未验证的 suspend 合同）。
fn assert_uncaught_raise_diverges(module: &MirModule, name: &str) {
    let function = function(module, name);
    assert_eq!(function.return_type, ValkyrieType::Integer64 { signed: true }, "`{name}` 必须保持声明返回类型");
    let mut unreachable_blocks = 0usize;
    for block in &function.blocks {
        match &block.terminator {
            MirTerminator::Unreachable => unreachable_blocks += 1,
            MirTerminator::PerformEffect { effect: rusty_valkyrie::MirEffectKind::Raise, .. } => {
                panic!("`{name}` 的未捕获 raise 不得保留 suspend 通道");
            }
            MirTerminator::Return { value: Some(MirOperand::Constant(MirConstant::Unit)) } => {
                panic!("`{name}` 不得把 `Unit` 常量当返回值");
            }
            _ => {}
        }
    }
    assert!(unreachable_blocks >= 1, "`{name}` 的未捕获 raise 必须终止为 `Unreachable`");
}

/// 值返回函数的尾 `raise` 必须编译通过：`Unit` 常量不得作为返回值进入 `Return`。
#[test]
fn tail_uncaught_raise_in_value_returning_function_compiles() {
    let group = CompilerSourceGroup::single_source("app", "demo", &source(PRIMITIVES, TEXT_RAISE_APP)).as_entry();
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&[group], &[], &[], "native")
        .map(|_| ())
        .map_err(|error| error.to_string())
        .expect("值返回函数的尾 `raise` 不得因 `Unit` 返回值被拒绝");
}

/// MIR 形状：文本载体与基本载体都留下不可达恢复块，没有 `Unit` 返回值。
#[test]
fn uncaught_raise_resume_block_terminates_unreachable() {
    let hir = lower(&source(PRIMITIVES, TEXT_RAISE_APP));
    let module = MirLowerer::lower_module_semantic(&hir);
    validate_module(&module).expect("尾 `raise` 的 MIR 必须通过语义合同校验");
    assert_uncaught_raise_diverges(&module, "explicit_panic");
    assert_uncaught_raise_diverges(&module, "numeric_panic");
}

/// `Resume = !`（名义载体）同样不得把 `Unit` 当返回值。
#[test]
fn nominal_carrier_raise_does_not_return_unit() {
    let hir = lower(&source(PRIMITIVES, NOMINAL_RAISE_APP));
    let module = MirLowerer::lower_module_semantic(&hir);
    validate_module(&module).expect("名义载体的尾 `raise` 必须通过 MIR 语义合同校验");
    assert_uncaught_raise_diverges(&module, "nominal_panic");
}

/// 反例：普通 `unit` 函数正常返回后被当值使用，必须保持结构化失败。
#[test]
fn unit_return_as_value_is_still_rejected() {
    let app = r#"
namespace demo;

micro unit_result() -> unit {
    return
}

[export]
micro unit_used_as_value() -> i64 {
    return unit_result()
}
"#;
    let error = ValkyrieCompiler::default().compile_source(&source(PRIMITIVES, app)).err().expect("`unit` 返回值不得冒充 `i64`").to_string();
    assert!(error.contains("unit") || error.contains("ReturnValueTypeMismatch"), "必须是返回类型合同失败，实际：{error}");
}

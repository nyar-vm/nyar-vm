//! `unit` 臂不参与 `match` 值合流。
//!
//! 语句位置（或块尾）的 `match` 各臂都以赋值等 `unit` 表达式结束时，`match` 本身没有
//! 可物化的值。修复前 `match_exit` 仍建立 `unit` 参数，臂入边把 `Constant(Unit)` 当 phi
//! 实参，canonical 以 CAN010「终结符操作数不是 SSA 值」拒绝。修复后与 `if` / `if let`
//! 的 `branch_merge_type` 同模型：`Unit` 不占 merge 槽，但控制流仍正常续接到 exit，
//! 不得把正常完成的 `Unit` 臂当成发散臂。

use rusty_valkyrie::{
    CompilerSourceGroup, MirLowerer, ValkyrieCompiler,
    compiler::mir::{ssa::MirModule, validation::validate_module},
};

const PRIMITIVES: &str = r#"
namespace std::primitive;
[primitive("std::primitive::bool")] structure bool { }
[primitive("std::primitive::i32")] structure i32 { }
"#;

const OPTION_DECL: &str = r#"
[tag(ProbeOptionKind)]
unite ProbeOption<T> {
    [tag(0)]
    [constructor]
    Some { value: T },
    [tag(1, default)]
    None,
}
"#;

const APP: &str = r#"
[export]
micro accumulate(value: ProbeOption<i32>) -> i32 {
    let mut total: i32 = 0
    let mut keep: bool = true
    while keep {
        match value {
            case Some(item):
                total = item
            case None:
                total = 0
        }
    }
    return total
}
"#;

fn source() -> String {
    format!("{PRIMITIVES}\n{OPTION_DECL}\n{APP}")
}

fn compile_to_canonical() -> Result<(), String> {
    let group = CompilerSourceGroup::single_source("app", "demo", &source()).as_entry();
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&[group], &[], &[], "native")
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn lower_module() -> MirModule {
    let hir = ValkyrieCompiler::new(rusty_valkyrie::compiler::types::SourceID { version_id: 9930 })
        .compile_source(&source())
        .expect("fixture 必须完成 HIR 分析");
    let mir = MirLowerer::lower_module_semantic(&hir);
    validate_module(&mir).expect("fixture 必须通过 MIR 校验");
    mir
}

/// 回归：`unit` 臂不建立 `match_exit` 值参数，也不把 `Constant(Unit)` 当 phi 实参。
#[test]
fn unit_arms_do_not_occupy_the_match_merge_slot() {
    let module = lower_module();
    let function = module
        .functions
        .iter()
        .find(|function| function.display_symbol.contains("accumulate"))
        .expect("fixture 缺少 `accumulate`");
    let exit = function
        .blocks
        .iter()
        .find(|block| block.label == "match_exit")
        .expect("fixture 缺少 `match_exit` 块");
    assert!(exit.parameters.is_empty(), "`unit` 合流不得建立 exit 参数");
    for block in &function.blocks {
        if let rusty_valkyrie::compiler::mir::ssa::MirTerminator::Jump { arguments, .. } = &block.terminator {
            assert!(
                !arguments.iter().any(|argument| matches!(argument, rusty_valkyrie::MirOperand::Constant(rusty_valkyrie::MirConstant::Unit))),
                "块 `{}` 的 Jump 不得把 `Constant(Unit)` 当值实参",
                block.label
            );
        }
    }
}

/// 回归：整个 fixture 必须编译到 canonical（CAN010 不得再因 `unit` 合流触发）。
#[test]
fn unit_arm_match_compiles_to_canonical() {
    compile_to_canonical().expect("`unit` 臂的 `match` 不得被 canonical 拒绝");
}

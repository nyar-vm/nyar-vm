//! `null` 的可空空值链：HIR 字面量身份 → SSA `LoadConstant{Null, ty:T?}`。
//!
//! 本用例只断言结构与身份，不比较回报数：空值不是名义 `Option` 变体，也不是 Unit。
//! 若 `null` 退化成标识符调用（CAN033）或 Unit 常量（SMIR007 actual=Unit），
//! 断言会失败。

use rusty_valkyrie::compiler::{
    hir::ValkyrieCompiler,
    mir::ssa::{MirConstant, MirFunction, MirLowerer, MirModule, MirOperation, MirTerminator},
    types::SourceID,
};
use rusty_valkyrie::compiler::types::hir::ValkyrieType;

fn compile(source: &str) -> MirModule {
    let hir = ValkyrieCompiler::new(SourceID { version_id: 9901 }).compile_source(source).expect("fixture 必须完成 HIR 分析");
    MirLowerer::lower_module_semantic(&hir)
}

fn function<'a>(module: &'a MirModule, name: &str) -> &'a MirFunction {
    module.functions.iter().find(|function| function.display_symbol.contains(name)).unwrap_or_else(|| panic!("fixture 缺少函数 `{name}`"))
}

#[test]
fn null_returns_nullable_typed_empty_reference_constant() {
    let module = compile(
        r#"
namespace null_fixture;

micro answer() -> i64? {
    return null
}
"#,
    );
    let function = function(&module, "answer");
    let expected = function.return_type.clone();
    assert!(matches!(expected, ValkyrieType::Nullable(_)), "返回类型必须是可空类型，实际 {expected:?}");

    let mut saw_null = false;
    let mut saw_return = false;
    for block in &function.blocks {
        for instruction in &block.instructions {
            if let MirOperation::LoadConstant { constant: MirConstant::Null, ty } = &instruction.kind {
                saw_null = true;
                assert_eq!(ty.as_ref(), Some(&expected), "空值常量必须携带声明返回的 `T?` 上下文类型");
                let result = instruction.results.first().expect("空值常量必须有结果");
                assert_eq!(function.value_types.get(result), Some(&expected), "空值结果的 SSA 类型必须等于上下文类型");
            }
        }
        if let MirTerminator::Return { value: Some(operand) } = &block.terminator {
            if let rusty_valkyrie::compiler::mir::ssa::MirOperand::Value(value) = operand {
                if matches!(function.value_types.get(value), Some(ValkyrieType::Nullable(_))) {
                    saw_return = true;
                }
            }
        }
    }
    assert!(saw_null, "`return null` 必须降低为 `LoadConstant{{Null}}`");
    assert!(saw_return, "`return null` 的返回操作数必须携带 `T?` SSA 类型");
}

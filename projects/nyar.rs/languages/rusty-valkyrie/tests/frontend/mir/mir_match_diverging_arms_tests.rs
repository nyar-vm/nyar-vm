//! 发散分支不参与 `match` 值合流。
//!
//! `match` 各臂都以 `return` / `panic` 结束时，`match` 自身没有值；函数尾不能再把
//! `Unit` 占位当成返回值（SMIR007 `return operand type differs (actual=Unit)`）。有值
//! 分支的合流仍按正常类型推导，不得按分支数或是否 `return` 特判。

use rusty_valkyrie::{
    MirLowerer, ValkyrieCompiler,
    compiler::mir::{
        ssa::{MirModule, MirTerminator},
        validation::validate_module,
    },
    types::hir::ValkyrieType,
};

const PRIMITIVES: &str = r#"
namespace std::primitive;
[primitive("std::primitive::bool")] structure bool { }
[primitive("std::primitive::i32")] structure i32 { }
imply i32 {
    infix `==`(self, rhs: Self): bool { true }
}
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

fn compile(source: &str) -> Result<MirModule, String> {
    let source = format!("{PRIMITIVES}\n{OPTION_DECL}\n{source}");
    let hir = ValkyrieCompiler::new(rusty_valkyrie::compiler::types::SourceID { version_id: 9920 })
        .compile_source(&source)
        .map_err(|error| format!("{error:?}"))?;
    let mir = MirLowerer::lower_module_semantic(&hir);
    validate_module(&mir).map_err(|error| format!("{error:?}"))?;
    Ok(mir)
}

fn function<'a>(module: &'a MirModule, name: &str) -> &'a rusty_valkyrie::compiler::mir::ssa::MirFunction {
    module.functions.iter().find(|function| function.display_symbol.contains(name)).unwrap_or_else(|| panic!("fixture 缺少 `{name}`"))
}

fn assert_returns_match_type(module: &MirModule, name: &str, expected: &ValkyrieType) {
    let function = function(module, name);
    for block in &function.blocks {
        if let MirTerminator::Return { value: Some(value) } = &block.terminator {
            let ty = match value {
                rusty_valkyrie::MirOperand::Value(value) => function.value_types.get(value).cloned(),
                rusty_valkyrie::MirOperand::Constant(constant) => Some(mir_constant_type(constant)),
                _ => None,
            }
            .unwrap_or_else(|| panic!("`{name}` 的 return 操作数缺少类型"));
            assert_eq!(&ty, expected, "`{name}` 块 `{}` 的 return 类型错误", block.label);
        }
    }
}

fn mir_constant_type(constant: &rusty_valkyrie::MirConstant) -> ValkyrieType {
    match constant {
        rusty_valkyrie::MirConstant::Bool(_) => ValkyrieType::Boolean,
        rusty_valkyrie::MirConstant::Utf8(_) => ValkyrieType::Utf8,
        rusty_valkyrie::MirConstant::Utf16(_) => ValkyrieType::Utf16,
        rusty_valkyrie::MirConstant::Int(_) => ValkyrieType::Integer32 { signed: true },
        rusty_valkyrie::MirConstant::Float64(_) => ValkyrieType::Float64,
        rusty_valkyrie::MirConstant::Unit | rusty_valkyrie::MirConstant::Null => ValkyrieType::Unit,
    }
}

/// 回归：所有臂都 `return` 的 `match` 作为函数尾时，函数仍按声明的 `bool` 返回。
#[test]
fn all_returning_match_tail_keeps_function_return_type() {
    let module = compile(
        r#"
micro equals_opt(value: ProbeOption<i32>) -> bool {
    match value {
        case Some(item):
            return item == 0
        case None:
            return false
    }
}
"#,
    )
    .expect("发散分支不得把 `bool` 返回合流成 `Unit`");
    assert_returns_match_type(&module, "equals_opt", &ValkyrieType::Boolean);
}

/// 回归：有值分支正常合流，发散分支被排除。
#[test]
fn diverging_arm_does_not_pollute_value_merge() {
    let module = compile(
        r#"
micro pick(value: ProbeOption<i32>, fallback: i32) -> i32 {
    match value {
        case Some(item):
            if item == 0 {
                return fallback
            } else {
                item
            }
        case None:
            fallback
    }
}
"#,
    )
    .expect("发散臂不得污染有值合流");
    assert_returns_match_type(&module, "pick", &ValkyrieType::Integer32 { signed: true });
}

/// 回归：`return match {…}` 且所有臂都发散时，`return` 站点不得再发 `Return { Unit }`。
#[test]
fn explicit_return_of_all_diverging_match_keeps_divergence() {
    let module = compile(
        r#"
micro classify(value: ProbeOption<i32>) -> bool {
    return match value {
        case Some(item):
            return item == 0
        case None:
            return false
    }
}
"#,
    )
    .expect("显式 return 的 `match` 不得把发散改写成 `Unit` 返回");
    assert_returns_match_type(&module, "classify", &ValkyrieType::Boolean);
}

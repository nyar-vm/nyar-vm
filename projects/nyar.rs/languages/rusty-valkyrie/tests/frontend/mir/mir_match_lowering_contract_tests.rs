//! `match` lowering 的三条合同，共用同一个 `ProbeOption` fixture，只在验证阶段上不同：
//!
//! 1. 判别块不得急切提取载荷（`SumPayloadGet` 必须在判别成功的后继块顶部物化）；
//! 2. `unit` 臂不占 `match` 值合流槽，也不得把 `Constant(Unit)` 当 phi 实参；
//! 3. 发散臂不污染有值合流，函数尾不得把发散分支改写成 `Unit` 返回。

use rusty_valkyrie::{
    CompilerSourceGroup, MirLowerer, ValkyrieCompiler,
    compiler::{
        mir::{
            ssa::{MirModule, MirOperation, MirTerminator},
            validation::validate_module,
        },
        types::SourceID,
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

fn source_with_fixture(source: &str) -> String {
    format!("{PRIMITIVES}\n{OPTION_DECL}\n{source}")
}

fn lower(source: &str, version_id: u32) -> MirModule {
    let hir = ValkyrieCompiler::new(SourceID { version_id })
        .compile_source(&source_with_fixture(source))
        .expect("fixture 必须完成 HIR 分析");
    MirLowerer::lower_module_semantic(&hir)
}

fn lower_validated(source: &str, version_id: u32) -> Result<MirModule, String> {
    let hir = ValkyrieCompiler::new(SourceID { version_id })
        .compile_source(&source_with_fixture(source))
        .map_err(|error| format!("{error:?}"))?;
    let mir = MirLowerer::lower_module_semantic(&hir);
    validate_module(&mir).map_err(|error| format!("{error:?}"))?;
    Ok(mir)
}

fn function<'a>(module: &'a MirModule, name: &str) -> &'a rusty_valkyrie::compiler::mir::ssa::MirFunction {
    module.functions.iter().find(|function| function.display_symbol.contains(name)).unwrap_or_else(|| panic!("fixture 缺少 `{name}`"))
}

/// 同一 SSA 块不得同时承担 tag 判别（`SumVariantIs`）与载荷提取（`SumPayloadGet`）。
fn assert_variant_is_never_shares_block_with_payload_get(module: &MirModule, context: &str) {
    let mut saw_variant_is = false;
    let mut saw_payload_get = false;
    for function in &module.functions {
        for block in &function.blocks {
            let has_variant_is =
                block.instructions.iter().any(|instruction| matches!(instruction.kind, MirOperation::SumVariantIs { .. }));
            let has_payload_get =
                block.instructions.iter().any(|instruction| matches!(instruction.kind, MirOperation::SumPayloadGet { .. }));
            saw_variant_is |= has_variant_is;
            saw_payload_get |= has_payload_get;
            assert!(
                !(has_variant_is && has_payload_get),
                "{context}: 块 `{}` 同时承担 tag 判别与载荷提取，未匹配路径会读 null 载荷",
                block.label
            );
        }
    }
    assert!(saw_variant_is, "{context}: fixture 必须真的产生 `SumVariantIs`");
    assert!(saw_payload_get, "{context}: fixture 必须真的产生 `SumPayloadGet`");
}

#[test]
fn match_check_block_never_extracts_payload_eagerly() {
    let module = lower(
        r#"
micro pick_or(input: ProbeOption<i32>, fallback: i32) -> i32 {
    match input {
        case Some(value):
            value
        case None:
            fallback
    }
}
"#,
        9710,
    );
    assert_variant_is_never_shares_block_with_payload_get(&module, "match 表达式");
}

/// 反例：嵌套 extractor（`Some(Some(x))`）当前只消费外层 tag 判别与外层载荷；
/// 内层 tag 不构成匹配条件。锁定现状为显式边界，不允许静默扩展语义。
#[test]
fn nested_extractor_matches_on_outer_tag_only() {
    let module = lower(
        r#"
micro nested(input: ProbeOption<ProbeOption<i32>>, fallback: i32) -> i32 {
    match input {
        case Some(Some(value)):
            value
        else:
            fallback
    }
}
"#,
        9710,
    );
    let function = function(&module, "nested");
    let variant_is_count = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| matches!(instruction.kind, MirOperation::SumVariantIs { .. }))
        .count();
    assert_eq!(variant_is_count, 1, "嵌套 extractor 当前只判别外层 tag；内层判别语义未定案前不得静默扩展");
}

/// 回归：`unit` 臂不建立 `match_exit` 值参数，也不把 `Constant(Unit)` 当 phi 实参。
#[test]
fn unit_arms_do_not_occupy_the_match_merge_slot() {
    let module = lower_validated(
        r#"
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
"#,
        9930,
    )
    .expect("fixture 必须通过 MIR 校验");
    let function = function(&module, "accumulate");
    let exit = function
        .blocks
        .iter()
        .find(|block| block.label == "match_exit")
        .expect("fixture 缺少 `match_exit` 块");
    assert!(exit.parameters.is_empty(), "`unit` 合流不得建立 exit 参数");
    for block in &function.blocks {
        if let MirTerminator::Jump { arguments, .. } = &block.terminator {
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
    let group = CompilerSourceGroup::single_source("app", "demo", &source_with_fixture(
        r#"
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
"#,
    ))
    .as_entry();
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&[group], &[], &[], "native")
        .map(|_| ())
        .map_err(|error| error.to_string())
        .expect("`unit` 臂的 `match` 不得被 canonical 拒绝");
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
    let module = lower_validated(
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
        9920,
    )
    .expect("发散分支不得把 `bool` 返回合流成 `Unit`");
    assert_returns_match_type(&module, "equals_opt", &ValkyrieType::Boolean);
}

/// 回归：有值分支正常合流，发散分支被排除。
#[test]
fn diverging_arm_does_not_pollute_value_merge() {
    let module = lower_validated(
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
        9920,
    )
    .expect("发散臂不得污染有值合流");
    assert_returns_match_type(&module, "pick", &ValkyrieType::Integer32 { signed: true });
}

/// 回归：`return match {…}` 且所有臂都发散时，`return` 站点不得再发 `Return { Unit }`。
#[test]
fn explicit_return_of_all_diverging_match_keeps_divergence() {
    let module = lower_validated(
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
        9920,
    )
    .expect("显式 return 的 `match` 不得把发散改写成 `Unit` 返回");
    assert_returns_match_type(&module, "classify", &ValkyrieType::Boolean);
}

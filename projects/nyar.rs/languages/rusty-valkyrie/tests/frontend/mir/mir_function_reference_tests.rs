//! 具名函数作值的 Semantic MIR 合同：`FunctionReference` lower 为 `LoadFunctionRef`，
//! 只消费 HIR 冻结的 `ItemInstanceId`，结果 SSA 值携带冻结 `Function` 签名类型。
//!
//! 验收面：实参位 lower 出 `LoadFunctionRef` 且实例与 HIR 一致、结果类型登记为
//! `Function`、泛型代入后调用合同通过 `validate_semantic_module`、实例缺失结构化失败
//! （`UnresolvedCallableIdentity`，不产 `LoadFunctionRef`）、identity 表未登记的实例被
//! SMIR003 拒绝。

use nyar_types::ItemInstanceId;
use rusty_valkyrie::{
    ValkyrieCompiler,
    compiler::mir::validation::validate_semantic_module,
    mir::ssa::{MirDiagnostic, MirFunction, MirInstruction, MirLowerer, MirOperand, MirOperation, MirValueRef},
    types::hir::{HirExprKind, HirFunction, HirFunctionReference, HirModule, ValkyrieType},
};

const NON_GENERIC_CONSUMER_SOURCE: &str = r#"
micro cmp_i32(a: i32, b: i32) -> i32 {
    return 0
}

micro apply(a: i32, b: i32, cmp: micro(i32, i32) -> i32) -> i32 {
    return cmp(a, b)
}

micro run() -> i32 {
    return apply(3, 5, cmp_i32)
}
"#;

const GENERIC_CONSUMER_SOURCE: &str = r#"
micro cmp_i64(a: i64, b: i64) -> i32 {
    return 0
}

micro choose<T>(a: T, b: T, cmp: micro(T, T) -> i32) -> i32 {
    return cmp(a, b)
}

micro run() -> i32 {
    let x: i64 = 3
    let y: i64 = 5
    return choose(x, y, cmp_i64)
}
"#;

fn compile(source: &str) -> HirModule {
    ValkyrieCompiler::default().compile_source(source).expect("函数作值源码必须完成 HIR")
}

fn function<'a>(module: &'a HirModule, name: &str) -> &'a HirFunction {
    module.functions.iter().find(|function| function.name.as_str() == name).expect(name)
}

/// 取 `run` 尾调用最后一个实参上的 `FunctionReference`。
fn reference(module: &HirModule) -> &HirFunctionReference {
    let run = function(module, "run");
    let expression = match &run.body.expr.as_deref().expect("run 体尾表达式").kind {
        HirExprKind::Return(Some(expression)) => expression,
        kind => panic!("期望 return 表达式: {kind:?}"),
    };
    let HirExprKind::Call { args, .. } = &expression.kind else {
        panic!("期望调用表达式: {:?}", expression.kind)
    };
    let HirExprKind::FunctionReference(reference) = &args.last().expect("最后一个实参").value.kind else {
        panic!("最后一个实参必须是 FunctionReference")
    };
    reference
}

/// 把裸名实参引用上的冻结实例抹掉（模拟上游未回填的畸形 HIR）。
fn erase_reference_instance(module: &mut HirModule) {
    let run = module.functions.iter_mut().find(|function| function.name.as_str() == "run").expect("run");
    let HirExprKind::Return(Some(expression)) = &mut run.body.expr.as_mut().expect("run 体尾表达式").kind else {
        panic!("期望 return 表达式");
    };
    let HirExprKind::Call { args, .. } = &mut expression.kind else {
        panic!("期望调用表达式");
    };
    let HirExprKind::FunctionReference(reference) = &mut args.last_mut().expect("最后一个实参").value.kind else {
        panic!("最后一个实参必须是 FunctionReference");
    };
    reference.instance = None;
}

/// 按程序序收集一个 MIR 函数里的全部 `Call` 指令。
fn collect_calls(mir_function: &MirFunction) -> Vec<(&MirOperand, &Vec<MirOperand>, &[MirValueRef])> {
    mir_function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match &instruction.kind {
            MirOperation::Call { callee, arguments } => Some((callee, arguments, instruction.results.as_slice())),
            _ => None,
        })
        .collect()
}

/// 回查定义某个值引用的指令。
fn defining_instruction<'a>(mir_function: &'a MirFunction, value: &MirValueRef) -> &'a MirInstruction {
    mir_function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find(|instruction| instruction.results.contains(value))
        .unwrap_or_else(|| panic!("值引用 {value:?} 必须由本函数内指令定义"))
}

fn collect_function<'a>(mir: &'a rusty_valkyrie::mir::ssa::MirModule, suffix: &str) -> &'a MirFunction {
    mir.functions.iter().find(|f| f.display_symbol.ends_with(suffix)).unwrap_or_else(|| panic!("{suffix} 函数"))
}

/// `apply(3, 5, cmp_i32)` 的函数实参 lower 为 `LoadFunctionRef`，
/// 实例与 HIR 冻结一致，结果 SSA 类型就是冻结的 `Function` 签名。
#[test]
fn function_reference_argument_lowers_to_load_function_ref() {
    let hir = compile(NON_GENERIC_CONSUMER_SOURCE);
    let instance = reference(&hir).instance.expect("固定点必须为函数作值铸造实例");
    let signature = reference(&hir).signature.clone();

    let mir = MirLowerer::lower_module_semantic(&hir);
    assert!(
        !mir.diagnostics.iter().any(|diagnostic| matches!(diagnostic, MirDiagnostic::UnsupportedExpression { .. })),
        "FunctionReference 不得落 UnsupportedExpression：{:?}",
        mir.diagnostics
    );
    assert!(validate_semantic_module(&mir).is_ok(), "函数作值展开后的 Semantic MIR 必须通过合同校验");

    let run = collect_function(&mir, "::run");
    let calls = collect_calls(run);
    assert_eq!(calls.len(), 1, "run 体只有一个 apply 调用");
    let MirOperand::Value(argument) = &calls[0].1[2] else {
        panic!("函数实参必须是 SSA 值，实际 {:?}", calls[0].1[2]);
    };
    match &defining_instruction(run, argument).kind {
        MirOperation::LoadFunctionRef { instance: loaded } => {
            assert_eq!(*loaded, instance, "MIR 只消费 HIR 冻结实例，不重新铸造");
        }
        other => panic!("函数实参必须由 LoadFunctionRef 定义，实际 {other:?}"),
    }
    assert_eq!(
        run.value_types.get(argument),
        Some(&ValkyrieType::Function(signature)),
        "LoadFunctionRef 结果 SSA 类型必须是冻结 Function 签名"
    );
}

/// 泛型代入路径：`choose<i64>` 合同里函数形参与实参类型闭合，体内间接调用
/// 以函数值 callee 形态通过 `validate_semantic_module`。
#[test]
fn generic_consumer_lowers_function_argument_and_indirect_call() {
    let hir = compile(GENERIC_CONSUMER_SOURCE);
    let instance = reference(&hir).instance.expect("固定点必须铸造实例");

    let mir = MirLowerer::lower_module_semantic(&hir);
    assert!(validate_semantic_module(&mir).is_ok(), "泛型代入后的函数作值合同必须通过校验");

    let run = collect_function(&mir, "::run");
    let calls = collect_calls(run);
    let last = calls.first().expect("run 体调用").1.last().expect("函数实参");
    let MirOperand::Value(argument) = last else {
        panic!("函数实参必须是 SSA 值");
    };
    match &defining_instruction(run, argument).kind {
        MirOperation::LoadFunctionRef { instance: loaded } => assert_eq!(*loaded, instance),
        other => panic!("期望 LoadFunctionRef，实际 {other:?}"),
    }

    // `choose` 实例体内 `cmp(a, b)` 必须是函数值间接调用（Value callee 携带 Function 类型）。
    let indirect = mir
        .functions
        .iter()
        .flat_map(|function| {
            function.blocks.iter().flat_map(|block| &block.instructions).filter_map(move |instruction| match &instruction.kind {
                MirOperation::Call { callee: MirOperand::Value(callee), .. } => Some((function, callee)),
                _ => None,
            })
        })
        .find(|(function, callee)| matches!(function.value_types.get(callee), Some(ValkyrieType::Function(_))));
    assert!(indirect.is_some(), "泛型实例体内必须保留函数值间接调用形态");
}

/// 实例缺失（上游未回填）即结构化失败：产出 `UnresolvedCallableIdentity` 诊断，
/// 不发射 `LoadFunctionRef`，实参退化为 Unit 常量而非猜测身份。
#[test]
fn missing_instance_lowers_to_fail_closed_diagnostic() {
    let mut hir = compile(NON_GENERIC_CONSUMER_SOURCE);
    erase_reference_instance(&mut hir);

    let mir = MirLowerer::lower_module_semantic(&hir);
    assert!(
        mir.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            MirDiagnostic::UnresolvedCallableIdentity { symbol } if symbol == "main.cmp_i32"
        )),
        "实例缺失必须产出 UnresolvedCallableIdentity：{:?}",
        mir.diagnostics
    );
    assert!(
        !mir.functions.iter().any(|function| function
            .blocks
            .iter()
            .any(|block| block.instructions.iter().any(|instruction| matches!(instruction.kind, MirOperation::LoadFunctionRef { .. })))),
        "实例缺失时不得发射 LoadFunctionRef"
    );
}

/// identity 表未登记的实例被 SMIR003 拒绝：下游不得按短名回查救活。
#[test]
fn unregistered_instance_rejected_by_validation() {
    let hir = compile(NON_GENERIC_CONSUMER_SOURCE);
    let mut mir = MirLowerer::lower_module_semantic(&hir);
    let forged = ItemInstanceId::from_index(999_999).expect("构造越界实例身份");
    let mut replaced = false;
    for function in &mut mir.functions {
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                if let MirOperation::LoadFunctionRef { instance } = &mut instruction.kind {
                    *instance = forged;
                    replaced = true;
                }
            }
        }
    }
    assert!(replaced, "夹具必须先产出 LoadFunctionRef");
    let error = validate_semantic_module(&mir).expect_err("未登记实例必须被拒绝");
    assert_eq!(error.code, "SMIR003", "必须按 callable identity 缺失失败：{error:?}");
}

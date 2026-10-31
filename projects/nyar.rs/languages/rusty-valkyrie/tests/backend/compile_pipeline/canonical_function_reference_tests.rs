//! 具名函数作值的 canonical 合同：`LoadFunctionRef` 1:1 保留 HIR 冻结实例身份，
//! 结果 SSA 值的 TypeId 必须是 `Function` 类型 kind；未链接实例失败关闭（CAN015）。
//!
//! 验收面：非泛型消费方 canonical 成功且 run 体内 `LoadFunctionRef` 实例与 HIR 冻结一致、
//! 结果类型登记为 `CanonicalTypeKind::Function`、冻结实例已进 `item_instances` 链接闭包；
//! 泛型代入路径 canonical 成功且实例体内保留 `CanonicalCallee::Value` 间接调用；
//! 伪造未链接实例被 `program.validate()` 拒绝，对外诊断 code 统一 `CAN015`。

use nyar_types::{CanonicalCallee, CanonicalOperation, CanonicalTypeKind, ItemInstanceId};
use rusty_valkyrie::{
    ValkyrieCompiler,
    compiler::emitter::pipeline::canonical_program_from_semantic_mir,
    mir::ssa::{MirLowerer, MirModule, MirOperation},
    types::hir::{HirExprKind, HirModule},
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

/// 取 `run` 尾调用最后一个实参上 HIR 冻结的实例身份。
fn frozen_instance(hir: &HirModule) -> ItemInstanceId {
    let run = hir.functions.iter().find(|function| function.name.as_str() == "run").expect("run");
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
    reference.instance.expect("固定点必须为函数作值铸造实例")
}

fn mir_function<'a>(mir: &'a MirModule, suffix: &str) -> &'a rusty_valkyrie::mir::ssa::MirFunction {
    mir.functions.iter().find(|function| function.display_symbol.ends_with(suffix)).unwrap_or_else(|| panic!("{suffix} 函数"))
}

/// 非泛型消费方：canonical 成功，`LoadFunctionRef` 保留冻结实例，结果类型是 Function kind，
/// 冻结实例进入链接闭包并携带限定名称。
#[test]
fn function_reference_survives_canonical_with_frozen_identity_and_function_type() {
    let hir = compile(NON_GENERIC_CONSUMER_SOURCE);
    let frozen = frozen_instance(&hir);
    let mir = MirLowerer::lower_module_semantic(&hir);
    let run_instance = mir_function(&mir, "::run").instance.expect("run 必须有实例 identity");

    let program = canonical_program_from_semantic_mir(&mir).expect("函数作值必须通过 canonical");
    let run = program.mir.functions.get(&run_instance).expect("canonical run 函数");

    let (loaded, results) = run
        .blocks
        .values()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match &instruction.operation {
            CanonicalOperation::LoadFunctionRef { instance } => Some((*instance, instruction.results.clone())),
            _ => None,
        })
        .expect("canonical 必须 1:1 保留 LoadFunctionRef");
    assert_eq!(loaded, frozen, "canonical 只保留 HIR 冻结实例，不重新铸造");
    assert_eq!(results.len(), 1, "LoadFunctionRef 结果必须恰好一个 SSA 值");

    let type_id = run.value_types.get(&results[0]).expect("结果 SSA 值必须登记 TypeId");
    let kind = &program.linked.types.get(type_id).expect("TypeId 必须在 linked types 中").kind;
    assert!(
        matches!(kind, CanonicalTypeKind::Function { parameters, return_type }
            if parameters.len() == 2 && program.linked.types.contains_key(return_type)),
        "LoadFunctionRef 结果类型必须是完整登记的 Function kind，实际 {kind:?}"
    );

    assert!(program.linked.item_instances.contains_key(&frozen), "被引用的冻结实例必须在链接闭包内");
    let name = program.linked.callable_names.get(&frozen).expect("冻结 callable 必须有限定名称");
    assert!(name.to_string().ends_with("cmp_i32"), "限定名称应指向 cmp_i32，实际 {name}");
}

/// 泛型代入路径：canonical 成功，`run` 体内保留冻结实例的 `LoadFunctionRef`，
/// 消费方实例体内保留 `CanonicalCallee::Value` 间接调用且 callee 类型是 Function kind。
#[test]
fn generic_consumer_reaches_canonical_with_indirect_invoke() {
    let hir = compile(GENERIC_CONSUMER_SOURCE);
    let frozen = frozen_instance(&hir);
    let mir = MirLowerer::lower_module_semantic(&hir);
    let run_instance = mir_function(&mir, "::run").instance.expect("run 必须有实例 identity");

    let program = canonical_program_from_semantic_mir(&mir).expect("泛型代入后的函数作值必须通过 canonical");
    let run = program.mir.functions.get(&run_instance).expect("canonical run 函数");
    let loaded = run
        .blocks
        .values()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match &instruction.operation {
            CanonicalOperation::LoadFunctionRef { instance } => Some(*instance),
            _ => None,
        })
        .expect("canonical 必须保留 LoadFunctionRef");
    assert_eq!(loaded, frozen, "泛型路径同样只消费 HIR 冻结实例");

    let indirect = program
        .mir
        .functions
        .values()
        .flat_map(|function| {
            function.blocks.values().flat_map(|block| &block.instructions).filter_map(move |instruction| {
                match &instruction.operation {
                    CanonicalOperation::Invoke { callee: CanonicalCallee::Value(callee), .. } => Some((function, *callee)),
                    _ => None,
                }
            })
        })
        .find(|(function, callee)| {
            function.value_types.get(callee).and_then(|ty| program.linked.types.get(ty)).is_some_and(|record| {
                matches!(record.kind, CanonicalTypeKind::Function { .. })
            })
        });
    assert!(indirect.is_some(), "消费方实例体内必须保留函数值间接调用形态");
}

/// 伪造未链接实例失败关闭：canonical 末尾 `program.validate()` 拒绝，
/// 对外诊断 code 统一映射为 CAN015，禁止按短名回查救活。
#[test]
fn unlinked_function_ref_instance_fails_closed_in_canonical() {
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
    let error = canonical_program_from_semantic_mir(&mir).expect_err("未链接实例必须被 canonical 拒绝");
    assert_eq!(error.records[0].code, "CAN015", "canonical 验证失败必须映射统一诊断码：{error:?}");
}

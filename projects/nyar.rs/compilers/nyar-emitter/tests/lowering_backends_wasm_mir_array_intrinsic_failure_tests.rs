//! WASM `ArrayGet` / `ArraySet` 消费侧回退查键失败的结构化失败合同：没有任何
//! `ArrayNew` 登记对应 gc arraytype 时，消费侧必须在编码期返回 `Err`（与 `Cast`
//! 臂同构），禁止 panic 把可执行合同缺陷升级成编译器崩溃。

use acorn_wasm::WasmBinaryModule;
use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef},
    testing::lower_fragment_to_wasm_mir_module,
};
use nyar_types::NyarType;
use std::collections::BTreeMap;

const I32: NyarType = NyarType::Integer32 { signed: true };

fn main_function(instructions: Vec<Instruction>, return_value: ValueRef, value_types: Vec<(ValueRef, NyarType)>) -> ExecutableFunction {
    let block = Block {
        id: BlockRef(0),
        label: "entry".to_string(),
        parameters: Vec::new(),
        instructions,
        terminator: Terminator::Return { value: Some(Operand::Value(return_value)) },
    };
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: I32,
        param_types: Vec::new(),
        value_types: BTreeMap::from_iter(value_types),
        value_layouts: BTreeMap::new(),
        return_layout: None,
        entry: BlockRef(0),
        values: Vec::new(),
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        #[allow(deprecated)]
        state_machine: None,
        suspend_plan: None,
        blocks: vec![block],
        diagnostics: Vec::new(),
    }
}

/// `main()`：%0 = 常量 7（i32 槽，即误标为标量的数组 receiver）；
/// %1 = `ArrayGet(array=%0, index=%0)`；return %1。模块内无任何 `ArrayNew`，
/// 消费侧回退到 output 元素类型 `i32` 查键必然 miss。
fn array_get_main() -> ExecutableFunction {
    let load = Instruction::from_kind(InstructionKind::LoadConstant { constant: Constant::Int(7), ty: Some(I32) });
    let get = Instruction::from_kind(InstructionKind::ArrayGet { array: Operand::Value(ValueRef(0)), index: Operand::Value(ValueRef(0)) });
    main_function(
        vec![Instruction { results: vec![ValueRef(0)], ..load }, Instruction { results: vec![ValueRef(1)], ..get }],
        ValueRef(1),
        vec![(ValueRef(0), I32), (ValueRef(1), I32)],
    )
}

/// `main()`：%0 = 常量 7（i32 槽，即误标为标量的数组 receiver）；
/// `ArraySet(array=%0, index=%0, value=%0)`；return %0。同样无 `ArrayNew` 登记 arraytype。
fn array_set_main() -> ExecutableFunction {
    let load = Instruction::from_kind(InstructionKind::LoadConstant { constant: Constant::Int(7), ty: Some(I32) });
    let set = Instruction::from_kind(InstructionKind::ArraySet {
        array: Operand::Value(ValueRef(0)),
        index: Operand::Value(ValueRef(0)),
        value: Operand::Value(ValueRef(0)),
    });
    main_function(
        vec![Instruction { results: vec![ValueRef(0)], ..load }, Instruction { results: Vec::new(), ..set }],
        ValueRef(0),
        vec![(ValueRef(0), I32)],
    )
}

fn lower(function: ExecutableFunction) -> nyar_emitter::Result<WasmBinaryModule> {
    let main_name = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main_name.clone(), function)]));
    plan.testing_module_name("array_intrinsic_failure").testing_entry_operation(main_name);
    let submission = FragmentSubmission::from_backend_plan(plan);
    lower_fragment_to_wasm_mir_module(&submission, "main")
}

/// fail-closed：`ArrayGet` 消费侧查不到已登记 gc arraytype 时结构化失败，
/// 禁止 panic 或回退发射运行期 trap 占位。
#[test]
fn wasm_array_get_without_registered_arraytype_fails_structured() {
    let error = lower(array_get_main()).expect_err("ArrayGet 消费侧查键失败必须返回 Err，不得 panic");
    assert!(error.contains("ArrayGet missing gc arraytype"), "错误必须指明 arraytype 登记事实缺失: {error}");
}

/// fail-closed：`ArraySet` 消费侧查不到已登记 gc arraytype 时结构化失败，
/// 禁止 panic 或回退发射运行期 trap 占位。
#[test]
fn wasm_array_set_without_registered_arraytype_fails_structured() {
    let error = lower(array_set_main()).expect_err("ArraySet 消费侧查键失败必须返回 Err，不得 panic");
    assert!(error.contains("ArraySet missing gc arraytype"), "错误必须指明 arraytype 登记事实缺失: {error}");
}

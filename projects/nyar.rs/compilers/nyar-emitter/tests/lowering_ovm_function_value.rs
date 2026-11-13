//! OVM 函数值 lowering：`LoadFunctionRef` 落成“本模块函数表下标”的 `i32`，
//! 间接调用落成 `CallIndirect`（先压实参、再压 callee）。
//!
//! 反例钉死 fail-closed：不属于本模块函数表的实例（跨模块/导入函数值）
//! 必须在 lowering 期结构化失败，不许回落符号、不许猜下标。

#![cfg(feature = "ovm-lane")]

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef},
    testing::lower_fragment_to_nyar_module,
};
use nyar_types::{ItemInstanceId, NyarFunctionType, NyarType};
use acorn_odin::{OdinHeadCode, OdinInstruction, OdinModuleData, decode_at};

fn function_type() -> NyarType {
    NyarType::Function(Box::new(NyarFunctionType { params: Vec::new(), return_type: NyarType::Integer32 { signed: true } }))
}

fn callee_function() -> ExecutableFunction {
    ExecutableFunction {
        symbol: "callee".to_string(),
        return_type: NyarType::Integer32 { signed: true },
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), NyarType::Integer32 { signed: true })]),
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
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: Vec::new(),
            instructions: vec![Instruction {
                results: vec![ValueRef(0)],
                ..Instruction::from_kind(InstructionKind::LoadConstant {
                    constant: Constant::Int(7),
                    ty: Some(NyarType::Integer32 { signed: true }),
                })
            }],
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(0))) },
        }],
        diagnostics: Vec::new(),
    }
}

/// `main`：`LoadFunctionRef(callee)` → 以函数值为 callee 的零实参调用。
fn caller_function(instance: ItemInstanceId) -> ExecutableFunction {
    ExecutableFunction {
        symbol: "main".to_string(),
        return_type: NyarType::Integer32 { signed: true },
        param_types: Vec::new(),
        value_types: BTreeMap::from([(ValueRef(0), function_type()), (ValueRef(1), NyarType::Integer32 { signed: true })]),
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
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: Vec::new(),
            instructions: vec![
                Instruction {
                    results: vec![ValueRef(0)],
                    ..Instruction::from_kind(InstructionKind::LoadFunctionRef {
                        instance,
                        environment: None,
                        captured: Vec::new(),
                    })
                },
                Instruction {
                    results: vec![ValueRef(1)],
                    ..Instruction::from_kind(InstructionKind::Call { callee: Operand::Value(ValueRef(0)), arguments: Vec::new() })
                },
            ],
            terminator: Terminator::Return { value: Some(Operand::Value(ValueRef(1))) },
        }],
        diagnostics: Vec::new(),
    }
}

fn main_name() -> QualifiedName {
    QualifiedName::new(vec![Identifier::new("main")])
}

fn lower(function: ExecutableFunction) -> nyar_emitter::Result<OdinModuleData> {
    let name = main_name();
    let functions = BTreeMap::from([(name.clone(), function), (QualifiedName::new(vec![Identifier::new("callee")]), callee_function())]);
    let mut plan = TargetExecutionPlan::from_functions(functions);
    plan.testing_module_name("ovm_function_value").testing_entry_operation(name);
    lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
}

fn decoded_at(module: &OdinModuleData, offset: i32, length: i32) -> Vec<OdinInstruction> {
    let start = offset as usize;
    let end = start + length as usize;
    let mut pc = start;
    let mut instructions = Vec::new();
    while pc < end {
        let instruction = decode_at(&module.code_bytes, pc);
        assert!(instruction.size > 0, "无法解码的字节码 pc={pc}");
        pc += usize::from(instruction.size);
        instructions.push(instruction);
    }
    instructions
}

/// 正：函数值落成两槽闭包对象（槽 0 = 常量函数下标，槽 1 = null 环境），
/// 间接调用落成 `CallIndirect`（argc = 可见实参 + 环境槽 = 1）。
#[test]
fn ovm_function_value_lowers_to_closure_object_and_call_indirect() {
    // `from_functions` 按名称序分配身份：`callee` -> 0，`main` -> 1。
    let callee_instance = ItemInstanceId::from_index(0).expect("callee 身份");
    let module = lower(caller_function(callee_instance)).expect("函数值与间接调用必须能 lowering");

    let main = module.functions.iter().find(|function| function.name == "main").expect("main 必须导出");
    let instructions = decoded_at(&module, main.code_offset, main.code_length);
    assert!(
        instructions.iter().any(|instruction| instruction.code == OdinHeadCode::ObjectNew),
        "无捕获函数值也必须用统一闭包对象载体，不能退回裸 i32"
    );
    let constant = instructions
        .iter()
        .filter(|instruction| instruction.code == OdinHeadCode::Constant)
        .find(|instruction| module.constants.get(instruction.operand1 as usize) == Some(&acorn_odin::OdinConstant::Integer32(0)))
        .expect("LoadFunctionRef 必须把 callee 函数表下标写进闭包槽 0");
    assert_eq!(constant.code, OdinHeadCode::Constant);
    assert!(
        instructions
            .iter()
            .any(|instruction| instruction.code == OdinHeadCode::Constant
                && module.constants.get(instruction.operand1 as usize) == Some(&acorn_odin::OdinConstant::Null)),
        "无捕获闭包的环境槽必须显式写 null"
    );

    let indirect =
        instructions.iter().find(|instruction| instruction.code == OdinHeadCode::CallIndirect).expect("间接调用必须发射 CallIndirect");
    assert_eq!(indirect.operand1, 1, "零可见实参调用的 argc 必须包含环境槽");
    assert_eq!(module.functions.len(), 2, "callee 必须进入同一模块函数表");
}

/// 反：实例不属于本模块函数表（跨模块/导入）必须 lowering 期失败，不得 panic。
#[test]
fn ovm_function_value_rejects_out_of_module_instance() {
    let unknown = ItemInstanceId::from_index(7).expect("测试身份");
    let error = lower(caller_function(unknown)).expect_err("跨模块函数值必须结构化失败");
    assert!(error.contains("跨模块/导入函数值未支持"), "错误必须点名跨模块函数值: {error}");
}

//! JVM managed ABI 契约与 argv launcher 的结构化验证。
//!
//! 用例直接构造已冻结的片段计划：每个导出入口必须对应一个已经降低的
//! `public static` 方法；契约摘要必须稳定，缺失入口或不可表达的返回类型必须
//! 返回结构化错误，不得静默跳过或补造入口。

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    nyar_backend_jvm::{JvmInstruction, JvmMethodDescriptor, JvmTypeDescriptor},
    testing::lower_fragment_to_jvm_managed_surface,
};
use nyar_types::{Block, BlockRef, ExecutableFunction, Instruction, InstructionKind, ItemInstanceId, NyarType, Operand, Terminator, ValueRef};

const LONG: NyarType = NyarType::Integer64 { signed: true };

fn function(symbol: &str, return_type: NyarType, parameters: Vec<NyarType>) -> ExecutableFunction {
    let value_types = parameters.iter().enumerate().map(|(index, ty)| (ValueRef(index as u32), ty.clone())).collect();
    let block_parameters = (0..parameters.len() as u32).map(ValueRef).collect();
    ExecutableFunction {
        return_layout: None,
        value_layouts: BTreeMap::new(),
        symbol: symbol.to_string(),
        return_type,
        param_types: parameters,
        value_types,
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
            parameters: block_parameters,
            instructions: Vec::new(),
            terminator: Terminator::Return { value: None },
        }],
        diagnostics: Vec::new(),
    }
}

fn instruction(kind: InstructionKind, results: Vec<ValueRef>) -> Instruction {
    let mut instruction = Instruction::from_kind(kind);
    instruction.results = results;
    instruction
}

/// 构造 `add(i64) -> i64` 的冻结片段，并把唯一函数登记为导出名。
fn identity_plan(symbol: &str, export_name: &str) -> FragmentSubmission {
    let mut fixture = function(symbol, LONG, vec![LONG]);
    fixture.value_types.insert(ValueRef(1), LONG);
    fixture.blocks[0].instructions.push(instruction(InstructionKind::Copy { source: Operand::Value(ValueRef(0)) }, vec![ValueRef(1)]));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(1))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("managed_abi_jvm").testing_entry_operation(main);
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), export_name.to_string());
    FragmentSubmission::from_backend_plan(plan)
}

fn main_descriptor() -> JvmMethodDescriptor {
    JvmMethodDescriptor::new(vec![JvmTypeDescriptor::array(JvmTypeDescriptor::Object("java/lang/String".to_string()))], JvmTypeDescriptor::Void)
}

#[test]
fn exported_long_identity_produces_managed_entry_and_argv_launcher() {
    let submission = identity_plan("managed.abi.add", "add");
    let (class_file, companion_classes, managed_abi) =
        lower_fragment_to_jvm_managed_surface(&submission).expect("managed ABI lowering must succeed");

    let method = class_file.methods.iter().find(|method| method.name == "add").expect("lowered add method");
    assert_eq!(method.descriptor, JvmMethodDescriptor::new(vec![JvmTypeDescriptor::Long], JvmTypeDescriptor::Long));

    let contract = managed_abi.expect("exported entry must produce a managed ABI contract");
    assert_eq!(contract.schema_version, 1);
    assert_eq!(contract.lane, "jvm");
    assert_eq!(contract.calling_convention, "static-argv-launcher");
    assert_eq!(contract.entries.len(), 1);
    let entry = &contract.entries[0];
    assert_eq!(entry.logical_entry, "add");
    assert_eq!(entry.owner, class_file.internal_name);
    assert_eq!(entry.method, "add");
    assert_eq!(entry.signature, "(J)J");
    assert_eq!(entry.parameter_types, vec!["i64".to_string()]);
    assert_eq!(entry.return_type, "i64");
    assert_eq!(entry.launcher_class, "nyar/invoke/add");

    assert_eq!(companion_classes.len(), 1);
    let launcher = &companion_classes[0];
    assert_eq!(launcher.internal_name, "nyar/invoke/add");
    let launcher_main = launcher.methods.iter().find(|method| method.name == "main").expect("launcher main");
    assert_eq!(launcher_main.descriptor, main_descriptor());
    assert_eq!(launcher_main.access_flags & (0x0001 | 0x0008), 0x0001 | 0x0008);
    let instructions = launcher_main.code.as_ref().expect("launcher code").instructions.clone();
    assert!(
        instructions.iter().any(
            |instruction| matches!(instruction, JvmInstruction::GetStatic(field) if field.owner == "java/lang/System" && field.name == "out")
        ),
        "{instructions:?}"
    );
    assert!(
        instructions
            .iter()
            .any(|instruction| matches!(instruction, JvmInstruction::InvokeVirtual(reference) if reference.owner == "java/io/PrintStream" && reference.name == "println")),
        "{instructions:?}"
    );
    launcher.to_bytes().expect("launcher class encoding");

    assert_eq!(contract.contract_digest.len(), 64, "digest must be 64 hex chars: {}", contract.contract_digest);
    assert!(contract.contract_digest.chars().all(|character| character.is_ascii_hexdigit()), "{}", contract.contract_digest);

    let rerun = identity_plan("managed.abi.add", "add");
    let (_, _, rerun_contract) = lower_fragment_to_jvm_managed_surface(&rerun).expect("rerun lowering");
    assert_eq!(rerun_contract.expect("rerun contract").contract_digest, contract.contract_digest);
}

#[test]
fn different_export_names_produce_different_contract_digests() {
    let (_, _, first) = lower_fragment_to_jvm_managed_surface(&identity_plan("managed.abi.add", "add")).expect("first lowering");
    let (_, _, second) = lower_fragment_to_jvm_managed_surface(&identity_plan("managed.abi.sum", "sum")).expect("second lowering");
    let first = first.expect("first contract");
    let second = second.expect("second contract");
    assert_eq!(first.entries[0].logical_entry, "add");
    assert_eq!(second.entries[0].logical_entry, "sum");
    assert_ne!(first.contract_digest, second.contract_digest);
}

#[test]
fn export_without_lowered_method_is_rejected_structurally() {
    let mut submission = identity_plan("managed.abi.add", "add");
    let plan = std::sync::Arc::get_mut(&mut submission.backend_plan).expect("unique plan");
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(1).expect("second instance"), "missing".to_string());

    let error = match lower_fragment_to_jvm_managed_surface(&submission) {
        Ok(_) => panic!("missing export must not be silently skipped"),
        Err(error) => error,
    };
    assert!(error.contains("missing"), "{error}");
    assert!(error.contains("没有已降低方法") || error.contains("缺少函数体") || error.contains("缺少方法计划"), "{error}");
}

#[test]
fn unsigned_logical_entry_is_rejected_instead_of_signed_remap() {
    // `u64` 在物理上可以合法承载于 JVM `Long`，但 launcher 只有 `parseLong`
    // 有符号解析器；不得因此把逻辑 ABI 类型改写成 `i64`。
    let unsigned = NyarType::Integer64 { signed: false };
    let mut fixture = function("managed.abi.unsigned", unsigned.clone(), vec![unsigned.clone()]);
    fixture.value_types.insert(ValueRef(1), unsigned.clone());
    fixture.blocks[0].instructions.push(instruction(InstructionKind::Copy { source: Operand::Value(ValueRef(0)) }, vec![ValueRef(1)]));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(1))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("managed_abi_jvm").testing_entry_operation(main);
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "unsigned".to_string());
    let submission = FragmentSubmission::from_backend_plan(plan);

    let error = match lower_fragment_to_jvm_managed_surface(&submission) {
        Ok(_) => panic!("unsigned 入口没有编组能力时必须结构化拒绝，不得改成 signed"),
        Err(error) => error,
    };
    assert!(error.contains("unsigned"), "错误必须说明缺少 unsigned 编组能力: {error}");
}

#[test]
fn unsupported_return_type_does_not_fabricate_entry() {
    let fixture = function("managed.abi.aggregate", NyarType::Tuple(vec![LONG]), Vec::new());

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("managed_abi_jvm").testing_entry_operation(main);
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "aggregate".to_string());
    let submission = FragmentSubmission::from_backend_plan(plan);

    let error = match lower_fragment_to_jvm_managed_surface(&submission) {
        Ok((_, companions, managed_abi)) => panic!("unsupported return type must fail: companions={companions:?} abi={managed_abi:?}"),
        Err(error) => error,
    };
    assert!(error.contains("裸 tuple") || error.contains("不支持类型"), "{error}");
}

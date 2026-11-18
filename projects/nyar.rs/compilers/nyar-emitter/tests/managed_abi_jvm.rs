//! JVM managed ABI 契约与 argv launcher 的结构化验证。
//!
//! 用例直接构造已冻结的片段计划：每个导出入口必须对应一个已经降低的
//! `public static` 方法；契约摘要必须稳定，缺失入口或不可表达的返回类型必须
//! 返回结构化错误，不得静默跳过或补造入口。

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    emitter_backend_jvm::{JvmInstruction, JvmMethodDescriptor, JvmTypeDescriptor},
    testing::lower_fragment_to_jvm_managed_surface,
};
use nyar_emitter::contracts::{Block, BlockRef, Constant, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef};
use nyar_types::{ItemInstanceId, SemanticType};

const LONG: SemanticType = SemanticType::Integer64 { signed: true };

fn function(symbol: &str, return_type: SemanticType, parameters: Vec<SemanticType>) -> ExecutableFunction {
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
        #[allow(deprecated)]
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: block_parameters,
            instructions: Vec::new(),
            terminator: Terminator::Return { value: None },
        }],
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
    plan.testing_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), export_name.to_string());
    FragmentSubmission::from_backend_plan(plan)
}

fn main_descriptor() -> JvmMethodDescriptor {
    JvmMethodDescriptor::new(vec![JvmTypeDescriptor::array(JvmTypeDescriptor::Object("java/lang/String".to_string()))], JvmTypeDescriptor::Void)
}

#[test]
fn exported_long_identity_produces_managed_entry_and_argv_launcher() {
    let submission = identity_plan("managed.abi.add", "add");
    let (class_file, companion_classes, managed_abi, _) =
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
    let (_, _, rerun_contract, _) = lower_fragment_to_jvm_managed_surface(&rerun).expect("rerun lowering");
    assert_eq!(rerun_contract.expect("rerun contract").contract_digest, contract.contract_digest);
}

#[test]
fn different_export_names_produce_different_contract_digests() {
    let (_, _, first, _) = lower_fragment_to_jvm_managed_surface(&identity_plan("managed.abi.add", "add")).expect("first lowering");
    let (_, _, second, _) = lower_fragment_to_jvm_managed_surface(&identity_plan("managed.abi.sum", "sum")).expect("second lowering");
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
    plan.testing_export_names_mut().insert(ItemInstanceId::from_index(1).expect("second instance"), "missing".to_string());

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
    let unsigned = SemanticType::Integer64 { signed: false };
    let mut fixture = function("managed.abi.unsigned", unsigned.clone(), vec![unsigned.clone()]);
    fixture.value_types.insert(ValueRef(1), unsigned.clone());
    fixture.blocks[0].instructions.push(instruction(InstructionKind::Copy { source: Operand::Value(ValueRef(0)) }, vec![ValueRef(1)]));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(1))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("managed_abi_jvm").testing_entry_operation(main);
    plan.testing_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "unsigned".to_string());
    let submission = FragmentSubmission::from_backend_plan(plan);

    let error = match lower_fragment_to_jvm_managed_surface(&submission) {
        Ok(_) => panic!("unsigned 入口没有编组能力时必须结构化拒绝，不得改成 signed"),
        Err(error) => error,
    };
    assert!(error.contains("unsigned"), "错误必须说明缺少 unsigned 编组能力: {error}");
}

/// 从已构造的计划中按符号名解析函数实例身份。
fn instance_of(plan: &TargetExecutionPlan, symbol: &str) -> ItemInstanceId {
    plan.testing_functions()
        .iter()
        .find(|(_, function)| function.symbol == symbol)
        .map(|(instance, _)| *instance)
        .unwrap_or_else(|| panic!("fixture function `{symbol}` 不在计划中"))
}

/// `f(x: i64) -> i64 = x` 的冻结函数体。
fn identity_function(symbol: &str) -> ExecutableFunction {
    let mut fixture = function(symbol, LONG, vec![LONG]);
    fixture.value_types.insert(ValueRef(1), LONG);
    fixture.blocks[0].instructions.push(instruction(InstructionKind::Copy { source: Operand::Value(ValueRef(0)) }, vec![ValueRef(1)]));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(1))) };
    fixture
}

/// 重命名入口：launcher 身份只来自计划冻结的 `entry_operation`，不按方法名扫描。
#[test]
fn renamed_entry_freezes_launcher_from_frozen_entry_identity() {
    let mut fixture = function("app.start", LONG, Vec::new());
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Constant(Constant::Int(7))) };

    let start = QualifiedName::new(vec![Identifier::new("start")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(start.clone(), fixture)]));
    plan.testing_module_name("managed_abi_jvm").testing_entry_operation(start);
    let (_, companions, managed_abi, entry_launcher) =
        lower_fragment_to_jvm_managed_surface(&FragmentSubmission::from_backend_plan(plan)).expect("重命名入口必须降低");

    assert!(managed_abi.is_none(), "无导出面不产 managed ABI 契约");
    assert_eq!(entry_launcher.as_deref(), Some("nyar/invoke/start"), "launcher 必须来自冻结入口身份，不按方法名扫描补造");
    let launcher = companions.iter().find(|class| class.internal_name == "nyar/invoke/start").expect("冻结 launcher 类必须存在");
    assert!(launcher.methods.iter().any(|method| method.name == "main"), "launcher 类持有 argv main 分发");
}

/// 入口与导出并存：入口同时被导出时，ABI 契约与程序入口复用同一个 launcher。
#[test]
fn entry_coexisting_with_exports_reuses_single_frozen_launcher() {
    let run = identity_function("app.run");
    let helper = identity_function("app.helper");
    let run_name = QualifiedName::new(vec![Identifier::new("run")]);
    let helper_name = QualifiedName::new(vec![Identifier::new("helper")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(run_name.clone(), run), (helper_name, helper)]));
    plan.testing_module_name("managed_abi_jvm").testing_entry_operation(run_name);
    let run_instance = instance_of(&plan, "app.run");
    let helper_instance = instance_of(&plan, "app.helper");
    plan.testing_export_names_mut().insert(run_instance, "run".to_string());
    plan.testing_export_names_mut().insert(helper_instance, "helper".to_string());
    let (_, companions, managed_abi, entry_launcher) =
        lower_fragment_to_jvm_managed_surface(&FragmentSubmission::from_backend_plan(plan)).expect("入口与导出并存必须降低");

    let contract = managed_abi.expect("导出面必须冻结 managed ABI 契约");
    assert_eq!(contract.entries.len(), 2, "两个导出入口都进入契约: {:?}", contract.entries);
    assert_eq!(entry_launcher.as_deref(), Some("nyar/invoke/run"), "程序入口复用导出 `run` 的 launcher 身份");
    let launcher_names = companions.iter().map(|class| class.internal_name.clone()).collect::<Vec<_>>();
    assert_eq!(
        launcher_names.iter().filter(|name| **name == "nyar/invoke/run").count(),
        1,
        "同一入口的 launcher 不得构建两份: {launcher_names:?}"
    );
    assert!(launcher_names.contains(&"nyar/invoke/helper".to_string()), "{launcher_names:?}");
}

/// 无入口的库交付：只有导出 launcher，不伪造程序入口。
#[test]
fn library_without_entry_freezes_exports_without_application_launcher() {
    let add = identity_function("app.add");
    let add_name = QualifiedName::new(vec![Identifier::new("add")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(add_name, add)]));
    plan.testing_module_name("managed_abi_jvm");
    let add_instance = instance_of(&plan, "app.add");
    plan.testing_export_names_mut().insert(add_instance, "add".to_string());
    let (_, _, managed_abi, entry_launcher) =
        lower_fragment_to_jvm_managed_surface(&FragmentSubmission::from_backend_plan(plan)).expect("无入口库必须按导出面降低");

    let contract = managed_abi.expect("库导出面必须冻结 managed ABI 契约");
    assert_eq!(contract.entries.len(), 1);
    assert!(entry_launcher.is_none(), "无 `entry_operation` 的库不得伪造程序 launcher");
}

#[test]
fn unsupported_return_type_does_not_fabricate_entry() {
    let fixture = function("managed.abi.aggregate", SemanticType::Tuple(vec![LONG]), Vec::new());

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("managed_abi_jvm").testing_entry_operation(main);
    plan.testing_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "aggregate".to_string());
    let submission = FragmentSubmission::from_backend_plan(plan);

    let error = match lower_fragment_to_jvm_managed_surface(&submission) {
        Ok((_, companions, managed_abi, _)) => panic!("unsupported return type must fail: companions={companions:?} abi={managed_abi:?}"),
        Err(error) => error,
    };
    assert!(error.contains("裸 tuple") || error.contains("不支持类型"), "{error}");
}

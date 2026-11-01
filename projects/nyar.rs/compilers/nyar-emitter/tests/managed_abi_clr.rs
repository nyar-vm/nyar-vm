//! CLR managed ABI 契约与 argv 分发入口的结构化验证。
//!
//! 用例直接构造已冻结的片段计划：每个导出入口必须对应一个已经降低的全局方法；
//! 契约摘要必须稳定，不可表达的标量必须返回结构化错误，不得静默跳过或补造入口。

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, ManagedAbiContract, ManagedAbiEntry, TargetExecutionPlan,
    nyar_backend_clr::{MsilInstruction, MsilInstructionOperand, MsilMethodSignature, MsilModule, MsilOpcode, MsilType},
    testing::lower_fragment_to_clr_managed_surface,
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

fn identity_fixture(symbol: &str, ty: NyarType) -> ExecutableFunction {
    let mut fixture = function(symbol, ty.clone(), vec![ty.clone()]);
    fixture.value_types.insert(ValueRef(1), ty);
    let mut copy = Instruction::from_kind(InstructionKind::Copy { source: Operand::Value(ValueRef(0)) });
    copy.results = vec![ValueRef(1)];
    fixture.blocks[0].instructions.push(copy);
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(1))) };
    fixture
}

/// 构造导出 `add(i64) -> i64` 的冻结片段（无 `[main]` 入口，模拟库构建）。
fn identity_plan(export_name: &str) -> FragmentSubmission {
    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main, identity_fixture("managed.abi.add", LONG))]));
    plan.testing_module_name("managed_abi_clr");
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), export_name.to_string());
    FragmentSubmission::from_backend_plan(plan)
}

fn managed_entry_signature() -> MsilMethodSignature {
    MsilMethodSignature::new(MsilType::Int32 { signed: true }, vec![MsilType::sz_array(MsilType::String)])
}

fn method_by_name<'a>(module: &'a MsilModule, name: &str) -> &'a nyar_emitter::nyar_backend_clr::MsilMethodBody {
    module.global_methods.iter().find(|method| method.method.name == name).unwrap_or_else(|| panic!("lowered method `{name}`"))
}

#[test]
fn exported_long_identity_produces_managed_entry_and_abi_contract() {
    let submission = identity_plan("add");
    let (module, managed_abi) = lower_fragment_to_clr_managed_surface(&submission).expect("managed ABI lowering must succeed");

    let add = method_by_name(&module, "add");
    assert_eq!(add.method.signature, MsilMethodSignature::new(MsilType::Int64 { signed: true }, vec![MsilType::Int64 { signed: true }]));

    let entry_method = method_by_name(&module, "managed_entry");
    assert!(entry_method.is_entry_point, "managed_entry must be the module entry point");
    assert_eq!(entry_method.method.signature, managed_entry_signature());
    let instructions: &[MsilInstruction] = &entry_method.instructions;
    assert!(
        instructions.iter().any(|instruction| matches!(
            &instruction.operand,
            Some(MsilInstructionOperand::Method(target))
                if target.owner.as_deref() == Some("[mscorlib]System.String") && target.name == "op_Equality"
        )),
        "{instructions:?}"
    );
    assert!(
        instructions.iter().any(|instruction| matches!(
            &instruction.operand,
            Some(MsilInstructionOperand::Method(target))
                if target.owner.as_deref() == Some("[mscorlib]System.Int64") && target.name == "Parse"
        )),
        "{instructions:?}"
    );
    assert!(
        instructions.iter().any(|instruction| matches!(
            &instruction.operand,
            Some(MsilInstructionOperand::Method(target))
                if target.owner.is_none() && target.name == "add"
                    && target.signature == MsilMethodSignature::new(MsilType::Int64 { signed: true }, vec![MsilType::Int64 { signed: true }])
        )),
        "{instructions:?}"
    );
    assert!(
        instructions.iter().any(|instruction| matches!(
            &instruction.operand,
            Some(MsilInstructionOperand::Method(target))
                if target.owner.as_deref() == Some("[mscorlib]System.Console") && target.name == "WriteLine"
                    && target.signature == MsilMethodSignature::new(MsilType::Void, vec![MsilType::Int64 { signed: true }])
        )),
        "{instructions:?}"
    );
    assert!(instructions.iter().any(|instruction| instruction.opcode == MsilOpcode::LdcI4_0));
    assert!(instructions.iter().any(|instruction| instruction.opcode == MsilOpcode::Ret));

    let contract = managed_abi.expect("exported entry must produce a managed ABI contract");
    assert_eq!(contract.schema_version, 1);
    assert_eq!(contract.lane, "clr");
    assert_eq!(contract.calling_convention, "static-argv-launcher");
    assert_eq!(contract.entries.len(), 1);
    let entry = &contract.entries[0];
    assert_eq!(entry.logical_entry, "add");
    assert_eq!(entry.owner, "_GlobalMethods");
    assert_eq!(entry.method, "add");
    assert_eq!(entry.signature, "int64(int64)");
    assert_eq!(entry.parameter_types, vec!["i64".to_string()]);
    assert_eq!(entry.return_type, "i64");
    assert_eq!(entry.launcher_class, "managed_entry");

    assert_eq!(contract.contract_digest.len(), 64, "digest must be 64 hex chars: {}", contract.contract_digest);
    assert!(contract.contract_digest.chars().all(|character| character.is_ascii_hexdigit()), "{}", contract.contract_digest);

    let rerun = identity_plan("add");
    let (_, rerun_contract) = lower_fragment_to_clr_managed_surface(&rerun).expect("rerun lowering");
    assert_eq!(rerun_contract.expect("rerun contract").contract_digest, contract.contract_digest);
}

#[test]
fn different_export_names_produce_different_contract_digests() {
    let (_, first) = lower_fragment_to_clr_managed_surface(&identity_plan("add")).expect("first lowering");
    let (_, second) = lower_fragment_to_clr_managed_surface(&identity_plan("sum")).expect("second lowering");
    let first = first.expect("first contract");
    let second = second.expect("second contract");
    assert_eq!(first.entries[0].logical_entry, "add");
    assert_eq!(second.entries[0].logical_entry, "sum");
    assert_ne!(first.contract_digest, second.contract_digest);
}

#[test]
fn unsupported_utf8_carrier_does_not_fabricate_entry() {
    let fixture = identity_fixture("managed.abi.echo", NyarType::Utf8);
    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main, fixture)]));
    plan.testing_module_name("managed_abi_clr");
    plan.testing_wasm_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "echo".to_string());
    let submission = FragmentSubmission::from_backend_plan(plan);

    let error = match lower_fragment_to_clr_managed_surface(&submission) {
        Ok((module, managed_abi)) => panic!("unsupported carrier must fail: module={module:?} abi={managed_abi:?}"),
        Err(error) => error,
    };
    assert!(error.contains("managed ABI"), "{error}");
    assert!(error.contains("不支持类型"), "{error}");
}

#[test]
fn managed_abi_json_serialization_carries_clr_lane_and_dll_artifact() {
    let contract = ManagedAbiContract::new(
        "clr",
        "leetcode-palindrome-number.dll",
        vec![ManagedAbiEntry {
            logical_entry: "isPalindrome".to_string(),
            owner: "_GlobalMethods".to_string(),
            method: "isPalindrome".to_string(),
            signature: "bool(int64)".to_string(),
            parameter_types: vec!["i64".to_string()],
            return_type: "bool".to_string(),
            launcher_class: "managed_entry".to_string(),
        }],
    );

    let directory = tempfile::tempdir().expect("temp dir");
    let output_path = directory.path().join("managed-abi.json");
    let content = serde_json::to_string_pretty(&contract).expect("serialize managed ABI");
    std::fs::write(&output_path, format!("{content}\n")).expect("write managed-abi.json");
    let written = std::fs::read_to_string(&output_path).expect("read managed-abi.json");

    assert!(written.contains("\"lane\": \"clr\""), "{written}");
    assert!(written.contains("\"artifact\": \"leetcode-palindrome-number.dll\""), "{written}");
    let decoded: ManagedAbiContract = serde_json::from_str(&written).expect("deserialize managed ABI");
    assert_eq!(decoded.contract_digest, contract.contract_digest);
    assert_eq!(decoded.entries[0].logical_entry, "isPalindrome");
}

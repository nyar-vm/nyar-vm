use nyar::{Identifier, NamePath};
use nyar_emitter::{
    contracts::{Block, BlockRef, ExecutableFunction, Instruction, InstructionKind, Operand, Terminator, ValueRef},
    *,
};
use nyar_types::NyarType;
use std::collections::BTreeMap;

/// 从 terminator 推断 `return_type`，避免 `Return { Some(...) }` 与 Unit 返回类型触发 SMIR007。
fn function(instructions: Vec<Instruction>, terminator: Terminator, value_types: BTreeMap<ValueRef, NyarType>) -> ExecutableFunction {
    let return_type = match &terminator {
        Terminator::Return { value: Some(Operand::Value(v)) } => value_types.get(v).cloned().unwrap_or(NyarType::Unit),
        Terminator::Return { value: Some(Operand::Constant(c)) } => match c {
            nyar_emitter::contracts::Constant::Bool(_) => NyarType::Boolean,
            nyar_emitter::contracts::Constant::Int(_) => NyarType::Integer64 { signed: true },
            nyar_emitter::contracts::Constant::Float64(_) => NyarType::Float64,
            nyar_emitter::contracts::Constant::Utf8(_) => NyarType::Utf8,
            nyar_emitter::contracts::Constant::Utf16(_) => NyarType::Utf16,
            nyar_emitter::contracts::Constant::Unit => NyarType::Unit,
        },
        _ => NyarType::Unit,
    };
    ExecutableFunction {
        return_layout: None,
        value_layouts: BTreeMap::new(),
        symbol: "contract_fixture".to_string(),
        return_type,
        param_types: Vec::new(),
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
        blocks: vec![Block { id: BlockRef(0), label: "entry".to_string(), parameters: Vec::new(), instructions, terminator }],
        diagnostics: Vec::new(),
    }
}

fn instr(kind: InstructionKind, results: Vec<ValueRef>) -> Instruction {
    let mut instruction = Instruction::from_kind(kind);
    instruction.results = results;
    instruction
}

#[test]
fn rejects_named_option_return_alias() {
    let value = ValueRef(1);
    let actual = NyarType::Nullable(Box::new(NyarType::Integer32 { signed: true }));
    let mut fixture = function(Vec::new(), Terminator::Return { value: Some(Operand::Value(value)) }, BTreeMap::from([(value, actual)]));
    fixture.return_type = NyarType::Apply(Box::new(NyarType::Named(Identifier::new("Option"))), vec![NyarType::Integer32 { signed: true }]);
    assert_eq!(validate_function(&fixture).unwrap_err().code, "SMIR007");
}

#[test]
fn rejects_platform_alias_inside_nullable_return() {
    let value = ValueRef(1);
    let actual = NyarType::Nullable(Box::new(NyarType::Named(Identifier::new("usize"))));
    let mut fixture = function(Vec::new(), Terminator::Return { value: Some(Operand::Value(value)) }, BTreeMap::from([(value, actual)]));
    fixture.return_type = NyarType::Nullable(Box::new(NyarType::Integer32 { signed: false }));
    assert_eq!(validate_function(&fixture).unwrap_err().code, "SMIR007");
}

#[test]
fn accepts_exact_structured_nullable_return() {
    let value = ValueRef(1);
    let actual = NyarType::Nullable(Box::new(NyarType::Integer32 { signed: true }));
    let fixture = function(Vec::new(), Terminator::Return { value: Some(Operand::Value(value)) }, BTreeMap::from([(value, actual)]));
    assert!(validate_function(&fixture).is_ok());
}

#[test]
fn rejects_instruction_output_without_semantic_type() {
    let result = validate_function(&function(
        vec![instr(InstructionKind::Copy { source: Operand::Constant(nyar_emitter::contracts::Constant::Unit) }, vec![ValueRef(7)])],
        Terminator::Return { value: None },
        BTreeMap::new(),
    ));
    assert_eq!(result.unwrap_err().code, "SMIR001");
}

#[test]
fn rejects_residual_pattern_match() {
    let result = validate_function(&function(
        vec![instr(
            InstructionKind::PatternMatch {
                value: Operand::Constant(nyar_emitter::contracts::Constant::Unit),
                pattern_debug: "fixture".to_string(),
            },
            Vec::new(),
        )],
        Terminator::Return { value: None },
        BTreeMap::new(),
    ));
    assert_eq!(result.unwrap_err().code, "SMIR008");
}

#[test]
fn accepts_minimal_semantic_function() {
    let result = validate_function(&function(Vec::new(), Terminator::Return { value: None }, BTreeMap::new()));
    assert!(result.is_ok());
}

#[test]
fn observation_is_stable_and_does_not_include_backend_names() {
    let result = validate_function(&function(Vec::new(), Terminator::Return { value: None }, BTreeMap::new()));
    assert_eq!(observation("valid_minimal", result.as_ref().map(|_| ()).map_err(|error| error)), "valid_minimal|accept||");

    let result = validate_function(&function(
        vec![instr(InstructionKind::Copy { source: Operand::Constant(nyar_emitter::contracts::Constant::Unit) }, vec![ValueRef(7)])],
        Terminator::Return { value: None },
        BTreeMap::new(),
    ));
    assert_eq!(
        observation("missing_value_type", result.as_ref().map(|_| ()).map_err(|error| error)),
        "missing_value_type|reject|SMIR001|instruction"
    );
}

#[test]
fn accepts_structured_array_length() {
    let receiver = ValueRef(10);
    let output = ValueRef(11);
    let mut value_types = BTreeMap::new();
    value_types.insert(receiver, NyarType::Array(Box::new(NyarType::Integer32 { signed: true })));
    value_types.insert(output, NyarType::Integer32 { signed: true });
    let result = validate_function(&function(
        vec![instr(InstructionKind::ArrayLength { array: Operand::Value(receiver) }, vec![output])],
        Terminator::Return { value: Some(Operand::Value(output)) },
        value_types,
    ));
    assert_eq!(
        observation("aggregate_array_sum.valid_array_len", result.as_ref().map(|_| ()).map_err(|error| error)),
        "aggregate_array_sum.valid_array_len|accept||"
    );
}

#[test]
fn rejects_array_length_without_result_type() {
    let receiver = ValueRef(10);
    let output = ValueRef(11);
    let mut value_types = BTreeMap::new();
    value_types.insert(receiver, NyarType::Array(Box::new(NyarType::Integer32 { signed: true })));
    let result = validate_function(&function(
        vec![instr(InstructionKind::ArrayLength { array: Operand::Value(receiver) }, vec![output])],
        Terminator::Return { value: None },
        value_types,
    ));
    assert_eq!(
        observation("aggregate_array_sum.missing_result_type", result.as_ref().map(|_| ()).map_err(|error| error)),
        "aggregate_array_sum.missing_result_type|reject|SMIR001|instruction"
    );
}

#[test]
fn accepts_thin_call_with_typed_results() {
    let left = ValueRef(20);
    let right = ValueRef(21);
    let result_v = ValueRef(22);
    let ty = NyarType::Named(Identifier::new("TokenKind"));
    let mut value_types = BTreeMap::new();
    value_types.insert(left, ty.clone());
    value_types.insert(right, ty);
    value_types.insert(result_v, NyarType::Boolean);
    let result = validate_function(&function(
        vec![instr(
            InstructionKind::Call {
                callee: Operand::Symbol(NamePath::new(vec![Identifier::new("primitive"), Identifier::new("sum_equal")])),
                arguments: vec![Operand::Value(left), Operand::Value(right)],
            },
            vec![result_v],
        )],
        Terminator::Return { value: Some(Operand::Value(result_v)) },
        value_types,
    ));
    assert!(result.is_ok());
}

#[test]
fn rejects_call_result_without_semantic_type() {
    let left = ValueRef(20);
    let right = ValueRef(21);
    let result_v = ValueRef(22);
    let ty = NyarType::Named(Identifier::new("TokenKind"));
    let mut value_types = BTreeMap::new();
    value_types.insert(left, ty.clone());
    value_types.insert(right, ty);
    let result = validate_function(&function(
        vec![instr(
            InstructionKind::Call {
                callee: Operand::Symbol(NamePath::new(vec![Identifier::new("primitive"), Identifier::new("sum_equal")])),
                arguments: vec![Operand::Value(left), Operand::Value(right)],
            },
            vec![result_v],
        )],
        Terminator::Return { value: None },
        value_types,
    ));
    assert_eq!(result.unwrap_err().code, "SMIR001");
}

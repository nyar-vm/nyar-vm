//! Paired Semantic MIR contract observations.
//!
//! Every case builds the same semantic fact twice: once against the executable
//! side plan (`nyar_emitter`) and once against the Valkyrie MIR side, then
//! requires both validators to produce the identical observation string.

use std::collections::BTreeMap;
use std::path::PathBuf;

use nyar::{
    ExternalImportLink, HostProjectionBoundary, Identifier, NamePath, QualifiedName, TargetBackendFamily,
};
use nyar_emitter::{
    nyar_backend_wasi::WasmPackageKind,
    testing::{physical_contract_observation, semantic_mir_observation, PhysicalContractTarget},
    FragmentSubmission, TargetExecutionPlan,
};
use nyar_types::{
    layout::{AggregateLayout, FieldLayout},
    layout_choice::{SumRepresentation, SumVariantRepresentation},
    AggregateLayoutPlan, Block, BlockRef, Constant, ExecutableFunction, FieldId, ImportIndex, Instruction, InstructionKind,
    ItemId, ItemInstanceId, NyarType, NominalInstanceId, Operand, StorageKind, Terminator, TypeId, ValueRef, VariantId,
};
use rusty_valkyrie::compiler::hir::type_lowering::validate_source_text_type_name;
use rusty_valkyrie::hir::HirPattern;
use rusty_valkyrie::mir::{
    semantic_observation, test_support::lower_test_module, validate_semantic_module, MirBlock, MirBlockRef, MirConstant,
    MirExternalCallContract, MirField, MirFunction, MirInstruction, MirModule, MirOperand, MirOperation, MirSumDeclaration,
    MirSumVariant, MirTerminator, MirValueRef,
};
use rusty_valkyrie::compiler::types::hir::ValkyrieType;

fn rust_function(
    symbol: &str,
    return_type: NyarType,
    param_types: Vec<NyarType>,
    value_types: Vec<(ValueRef, NyarType)>,
    blocks: Vec<Block>,
) -> ExecutableFunction {
    #[allow(deprecated)]
    ExecutableFunction {
        symbol: symbol.to_string(),
        return_type,
        param_types,
        value_types: value_types.into_iter().collect(),
        value_layouts: BTreeMap::new(),
        return_layout: None,
        entry: BlockRef(0),
        values: Vec::new(),
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        state_machine: None,
        suspend_plan: None,
        blocks,
        diagnostics: Vec::new(),
    }
}

fn rust_instr(kind: InstructionKind, results: Vec<ValueRef>) -> Instruction {
    let mut instruction = Instruction::from_kind(kind);
    instruction.results = results;
    instruction
}

fn rust_block(id: u32, parameters: Vec<ValueRef>, instructions: Vec<Instruction>, terminator: Terminator) -> Block {
    Block {
        id: BlockRef(id),
        label: format!("bb{id}"),
        parameters,
        instructions,
        terminator,
    }
}

fn rust_submission(functions: Vec<(&str, ExecutableFunction)>) -> FragmentSubmission {
    let functions = functions
        .into_iter()
        .map(|(name, function)| {
            let name = QualifiedName::new(name.split('.').map(Identifier::new).collect());
            (name, function)
        })
        .collect::<BTreeMap<_, _>>();
    FragmentSubmission::from_backend_plan(TargetExecutionPlan::from_functions(functions))
}

fn mir_function(
    return_type: ValkyrieType,
    param_types: Vec<ValkyrieType>,
    value_types: Vec<(MirValueRef, ValkyrieType)>,
    blocks: Vec<MirBlock>,
) -> MirFunction {
    MirFunction {
        display_symbol: "fixture".to_string(),
        symbol: NamePath::default(),
        declaration: None,
        instance: None,
        return_type,
        param_types,
        value_types: value_types.into_iter().collect(),
        entry: MirBlockRef(0),
        values: Vec::new(),
        blocks,
    }
}

fn mir_instr(kind: MirOperation, results: Vec<MirValueRef>) -> MirInstruction {
    let mut instruction = MirInstruction::from_operation(kind);
    instruction.results = results;
    instruction
}

fn mir_block(id: u32, parameters: Vec<MirValueRef>, instructions: Vec<MirInstruction>, terminator: MirTerminator) -> MirBlock {
    MirBlock {
        id: MirBlockRef(id),
        label: format!("bb{id}"),
        parameters,
        instructions,
        terminator,
    }
}

fn mir_module(functions: Vec<MirFunction>) -> MirModule {
    let mut module = lower_test_module(Vec::new(), Vec::new());
    module.functions = functions;
    module
}

fn frontend_observation(module: &MirModule, case_id: &str) -> String {
    let result = validate_semantic_module(module);
    semantic_observation(case_id, result.as_ref().map(|()| ()).map_err(|error| error))
}

fn assert_same_observation(rust: &FragmentSubmission, frontend: &MirModule, case_id: &str) -> String {
    let rust_observation = semantic_mir_observation(rust, case_id);
    let frontend_observation = frontend_observation(frontend, case_id);
    assert_eq!(rust_observation, frontend_observation, "case `{case_id}` diverges");
    rust_observation
}

fn named(name: &str) -> NyarType {
    NyarType::Named(Identifier::new(name))
}

fn nominal(name: &str) -> ValkyrieType {
    ValkyrieType::Named(Identifier::new(name))
}

fn int32() -> NyarType {
    NyarType::Integer32 { signed: true }
}

fn frontend_int32() -> ValkyrieType {
    ValkyrieType::Integer32 { signed: true }
}

fn record_layout() -> AggregateLayout {
    AggregateLayout {
        id: 7,
        name: "Record".to_string(),
        namespace: "fixture".to_string(),
        key: None,
        storage: StorageKind::Value,
        fields: vec![FieldLayout {
            name: "slot".to_string(),
            ty: int32(),
        }],
    }
}

#[test]
fn aggregate_array_sum_valid_paired_observation_matches() {
    let case_id = "aggregate_array_sum.valid";
    let record = named("Record");
    let array = NyarType::Array(Box::new(record.clone()));
    let base = ValueRef(0);

    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(
        QualifiedName::new(vec![Identifier::new("fixture"), Identifier::new("entry")]),
        rust_function(
            "fixture.entry",
            record.clone(),
            vec![int32()],
            vec![
                (base, int32()),
                (ValueRef(9), record.clone()),
                (ValueRef(10), array.clone()),
                (ValueRef(11), array.clone()),
                (ValueRef(12), record.clone()),
                (ValueRef(13), named("Choice")),
                (ValueRef(14), record.clone()),
            ],
            vec![rust_block(
                0,
                vec![base],
                vec![
                    rust_instr(
                        InstructionKind::StructNew {
                            nominal: NominalInstanceId::from_index(0).unwrap(),
                            fields: vec![(FieldId::from_index(0).unwrap(), Operand::Value(base))],
                        },
                        vec![ValueRef(9)],
                    ),
                    rust_instr(
                        InstructionKind::ArrayFromElements {
                            layout_id: None,
                            array_type: array.clone(),
                            elements: vec![Operand::Value(ValueRef(9))],
                        },
                        vec![ValueRef(10)],
                    ),
                    rust_instr(
                        InstructionKind::Call {
                            callee: Operand::Item(ItemInstanceId::from_index(1).unwrap()),
                            arguments: vec![Operand::Value(ValueRef(10)), Operand::Value(ValueRef(9))],
                        },
                        vec![ValueRef(11)],
                    ),
                    rust_instr(
                        InstructionKind::Call {
                            callee: Operand::Item(ItemInstanceId::from_index(2).unwrap()),
                            arguments: vec![Operand::Value(ValueRef(11)), Operand::Value(base)],
                        },
                        vec![ValueRef(12)],
                    ),
                    rust_instr(
                        InstructionKind::SumNew {
                            nominal: NominalInstanceId::from_index(0).unwrap(),
                            variant: VariantId::from_index(0).unwrap(),
                            payload: Some(Operand::Value(ValueRef(12))),
                        },
                        vec![ValueRef(13)],
                    ),
                    rust_instr(
                        InstructionKind::SumPayloadGet {
                            nominal: NominalInstanceId::from_index(0).unwrap(),
                            variant: VariantId::from_index(0).unwrap(),
                            object: Operand::Value(ValueRef(13)),
                        },
                        vec![ValueRef(14)],
                    ),
                ],
                Terminator::Return {
                    value: Some(Operand::Value(ValueRef(14))),
                },
            )],
        ),
    )]));
    plan.testing_register_nominal_layout(NominalInstanceId::from_index(0).unwrap(), record_layout());
    plan.testing_register_field(FieldId::from_index(0).unwrap(), 7, 0);
    plan.testing_register_sum_representation(
        NominalInstanceId::from_index(0).unwrap(),
        SumRepresentation {
            nominal: NominalInstanceId::from_index(0).unwrap(),
            tag_width: 32,
            variants: BTreeMap::from([(
                VariantId::from_index(0).unwrap(),
                SumVariantRepresentation {
                    id: VariantId::from_index(0).unwrap(),
                    tag: 0,
                    payload_type: Some(TypeId::from_index(0).unwrap()),
                },
            )]),
        },
    );
    plan.testing_register_import(
        ItemInstanceId::from_index(1).unwrap(),
        nyar_emitter::BackendImport {
            index: ImportIndex::from_index(0).unwrap(),
            link: ExternalImportLink::host(Some(Identifier::new("contract")), vec!["neutral.push".to_string()]),
            parameter_types: vec![array.clone(), record.clone()],
            return_type: array.clone(),
        },
    );
    plan.testing_register_import(
        ItemInstanceId::from_index(2).unwrap(),
        nyar_emitter::BackendImport {
            index: ImportIndex::from_index(1).unwrap(),
            link: ExternalImportLink::host(Some(Identifier::new("contract")), vec!["neutral.get".to_string()]),
            parameter_types: vec![array.clone(), int32()],
            return_type: record.clone(),
        },
    );
    let rust = FragmentSubmission::from_backend_plan(plan);

    let mut module = mir_module(vec![mir_function(
        nominal("Record"),
        vec![frontend_int32()],
        vec![
            (MirValueRef(0), frontend_int32()),
            (MirValueRef(9), nominal("Record")),
            (MirValueRef(10), ValkyrieType::Array(Box::new(nominal("Record")))),
            (MirValueRef(11), ValkyrieType::Array(Box::new(nominal("Record")))),
            (MirValueRef(12), nominal("Record")),
            (
                MirValueRef(13),
                ValkyrieType::Nominal {
                    declaration: ItemId::from_index(0).unwrap(),
                    arguments: Vec::new(),
                    primitive_identity: None,
                },
            ),
            (MirValueRef(14), nominal("Record")),
        ],
        vec![mir_block(
            0,
            vec![MirValueRef(0)],
            vec![
                mir_instr(
                    MirOperation::StructNew {
                        nominal: NominalInstanceId::from_index(0).unwrap(),
                        fields: vec![(FieldId::from_index(0).unwrap(), MirOperand::Value(MirValueRef(0)))],
                    },
                    vec![MirValueRef(9)],
                ),
                mir_instr(
                    MirOperation::ArrayFromElements {
                        array_type: ValkyrieType::Array(Box::new(nominal("Record"))),
                        elements: vec![MirOperand::Value(MirValueRef(9))],
                    },
                    vec![MirValueRef(10)],
                ),
                mir_instr(
                    MirOperation::Call {
                        callee: MirOperand::Callable(ItemInstanceId::from_index(1).unwrap()),
                        arguments: vec![MirOperand::Value(MirValueRef(10)), MirOperand::Value(MirValueRef(9))],
                    },
                    vec![MirValueRef(11)],
                ),
                mir_instr(
                    MirOperation::Call {
                        callee: MirOperand::Callable(ItemInstanceId::from_index(2).unwrap()),
                        arguments: vec![MirOperand::Value(MirValueRef(11)), MirOperand::Value(MirValueRef(0))],
                    },
                    vec![MirValueRef(12)],
                ),
                mir_instr(
                    MirOperation::SumNew {
                        nominal: NominalInstanceId::from_index(0).unwrap(),
                        type_args: Vec::new(),
                        variant: VariantId::from_index(0).unwrap(),
                        payload_type: Some(nominal("Record")),
                        payloads: vec![MirOperand::Value(MirValueRef(12))],
                    },
                    vec![MirValueRef(13)],
                ),
                mir_instr(
                    MirOperation::SumPayloadGet {
                        nominal: NominalInstanceId::from_index(0).unwrap(),
                        type_args: Vec::new(),
                        variant: VariantId::from_index(0).unwrap(),
                        payload_type: nominal("Record"),
                        object: MirOperand::Value(MirValueRef(13)),
                    },
                    vec![MirValueRef(14)],
                ),
            ],
            MirTerminator::Return {
                value: Some(MirOperand::Value(MirValueRef(14))),
            },
        )],
    )]);
    module.sum_types.push(MirSumDeclaration {
        nominal: NominalInstanceId::from_index(0).unwrap(),
        declaration: Some(ItemId::from_index(0).unwrap()),
        name: "Choice".to_string(),
        is_unite: false,
        is_value_type: false,
        generics: Vec::new(),
        variants: vec![MirSumVariant {
            id: VariantId::from_index(0).unwrap(),
            declaration: Some(ItemId::from_index(1).unwrap()),
            name: "Branch".to_string(),
            tag: 0,
            fields: vec![MirField {
                id: FieldId::from_index(1).unwrap(),
                name: "payload".to_string(),
                ty: nominal("Record"),
            }],
            result_type: None,
        }],
    });
    let mut layouts = AggregateLayoutPlan::default();
    layouts.layouts.push(AggregateLayout {
        id: 7,
        name: "Record".to_string(),
        namespace: "fixture".to_string(),
        key: None,
        storage: StorageKind::Value,
        fields: vec![FieldLayout {
            name: "slot".to_string(),
            ty: int32(),
        }],
    });
    module.aggregate_layouts = layouts;
    module.external_calls.push(MirExternalCallContract {
        declaration: None,
        instance: Some(ItemInstanceId::from_index(1).unwrap()),
        symbol: NamePath::new(vec![Identifier::new("neutral"), Identifier::new("push")]),
        link: ExternalImportLink::host(Some(Identifier::new("contract")), vec!["neutral.push".to_string()]),
        parameter_types: vec![ValkyrieType::Array(Box::new(nominal("Record"))), nominal("Record")],
        return_type: ValkyrieType::Array(Box::new(nominal("Record"))),
    });
    module.external_calls.push(MirExternalCallContract {
        declaration: None,
        instance: Some(ItemInstanceId::from_index(2).unwrap()),
        symbol: NamePath::new(vec![Identifier::new("neutral"), Identifier::new("get")]),
        link: ExternalImportLink::host(Some(Identifier::new("contract")), vec!["neutral.get".to_string()]),
        parameter_types: vec![ValkyrieType::Array(Box::new(nominal("Record"))), frontend_int32()],
        return_type: nominal("Record"),
    });

    assert_eq!(
        assert_same_observation(&rust, &module, case_id),
        format!("{case_id}|accept||")
    );
}

#[test]
fn control_flow_contract_paired_observations_match() {
    // Control-flow terminators must reject on both sides with SMIR007.
    let return_mismatch = rust_submission(vec![(
        "fixture.entry",
        rust_function(
            "fixture.entry",
            NyarType::Boolean,
            Vec::new(),
            vec![(ValueRef(0), int32())],
            vec![rust_block(
                0,
                Vec::new(),
                Vec::new(),
                Terminator::Return {
                    value: Some(Operand::Value(ValueRef(0))),
                },
            )],
        ),
    )]);
    let frontend_return = mir_module(vec![mir_function(
        ValkyrieType::Boolean,
        Vec::new(),
        vec![(MirValueRef(0), frontend_int32())],
        vec![mir_block(
            0,
            Vec::new(),
            Vec::new(),
            MirTerminator::Return {
                value: Some(MirOperand::Value(MirValueRef(0))),
            },
        )],
    )]);
    let observation = assert_same_observation(&return_mismatch, &frontend_return, "control.return_type_mismatch");
    assert!(observation.starts_with("control.return_type_mismatch|reject|SMIR007|"));

    let jump_mismatch = rust_submission(vec![(
        "fixture.entry",
        rust_function(
            "fixture.entry",
            NyarType::Unit,
            Vec::new(),
            vec![(ValueRef(0), int32()), (ValueRef(1), NyarType::Boolean)],
            vec![
                rust_block(
                    0,
                    Vec::new(),
                    Vec::new(),
                    Terminator::Jump {
                        target: BlockRef(1),
                        arguments: vec![Operand::Value(ValueRef(0))],
                    },
                ),
                rust_block(1, vec![ValueRef(1)], Vec::new(), Terminator::Return { value: None }),
            ],
        ),
    )]);
    let frontend_jump = mir_module(vec![mir_function(
        ValkyrieType::Unit,
        Vec::new(),
        vec![(MirValueRef(0), frontend_int32()), (MirValueRef(1), ValkyrieType::Boolean)],
        vec![
            mir_block(
                0,
                Vec::new(),
                Vec::new(),
                MirTerminator::Jump {
                    target: MirBlockRef(1),
                    arguments: vec![MirOperand::Value(MirValueRef(0))],
                },
            ),
            mir_block(1, vec![MirValueRef(1)], Vec::new(), MirTerminator::Return { value: None }),
        ],
    )]);
    let observation = assert_same_observation(&jump_mismatch, &frontend_jump, "control.jump_type_mismatch");
    assert!(observation.starts_with("control.jump_type_mismatch|reject|SMIR007|"));

    let branch_mismatch = rust_submission(vec![(
        "fixture.entry",
        rust_function(
            "fixture.entry",
            NyarType::Unit,
            Vec::new(),
            vec![(ValueRef(0), int32())],
            vec![
                rust_block(
                    0,
                    Vec::new(),
                    Vec::new(),
                    Terminator::Branch {
                        condition: Operand::Value(ValueRef(0)),
                        then_target: BlockRef(1),
                        else_target: BlockRef(1),
                    },
                ),
                rust_block(1, Vec::new(), Vec::new(), Terminator::Return { value: None }),
            ],
        ),
    )]);
    let frontend_branch = mir_module(vec![mir_function(
        ValkyrieType::Unit,
        Vec::new(),
        vec![(MirValueRef(0), frontend_int32())],
        vec![
            mir_block(
                0,
                Vec::new(),
                Vec::new(),
                MirTerminator::Branch {
                    condition: MirOperand::Value(MirValueRef(0)),
                    then_target: MirBlockRef(1),
                    else_target: MirBlockRef(1),
                },
            ),
            mir_block(1, Vec::new(), Vec::new(), MirTerminator::Return { value: None }),
        ],
    )]);
    let observation = assert_same_observation(&branch_mismatch, &frontend_branch, "control.branch_type_mismatch");
    assert!(observation.starts_with("control.branch_type_mismatch|reject|SMIR007|"));
}

#[test]
fn unresolved_static_call_paired_observation_matches() {
    let case_id = "call.unresolved_static";
    let missing = ItemInstanceId::from_index(9).unwrap();
    let rust = rust_submission(vec![(
        "fixture.entry",
        rust_function(
            "fixture.entry",
            NyarType::Unit,
            Vec::new(),
            Vec::new(),
            vec![rust_block(
                0,
                Vec::new(),
                vec![rust_instr(
                    InstructionKind::Call {
                        callee: Operand::Item(missing),
                        arguments: Vec::new(),
                    },
                    Vec::new(),
                )],
                Terminator::Return { value: None },
            )],
        ),
    )]);
    let frontend = mir_module(vec![mir_function(
        ValkyrieType::Unit,
        Vec::new(),
        Vec::new(),
        vec![mir_block(
            0,
            Vec::new(),
            vec![mir_instr(
                MirOperation::Call {
                    callee: MirOperand::Callable(missing),
                    arguments: Vec::new(),
                },
                Vec::new(),
            )],
            MirTerminator::Return { value: None },
        )],
    )]);

    let observation = assert_same_observation(&rust, &frontend, case_id);
    assert!(observation.starts_with("call.unresolved_static|reject|SMIR003|"));
}

#[test]
fn exact_external_call_contract_paired_observation_matches() {
    let case_id = "call.external_exact_valid";
    let imported = ItemInstanceId::from_index(1).unwrap();
    let link = ExternalImportLink::host(Some(Identifier::new("contract")), vec!["neutral.invoke".to_string()]);

    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(
        QualifiedName::new(vec![Identifier::new("fixture"), Identifier::new("entry")]),
        rust_function(
            "fixture.entry",
            NyarType::Unit,
            Vec::new(),
            Vec::new(),
            vec![rust_block(
                0,
                Vec::new(),
                vec![rust_instr(
                    InstructionKind::Call {
                        callee: Operand::Item(imported),
                        arguments: Vec::new(),
                    },
                    Vec::new(),
                )],
                Terminator::Return { value: None },
            )],
        ),
    )]));
    plan.testing_register_import(
        imported,
        nyar_emitter::BackendImport {
            index: ImportIndex::from_index(0).unwrap(),
            link: link.clone(),
            parameter_types: Vec::new(),
            return_type: NyarType::Unit,
        },
    );
    let rust = FragmentSubmission::from_backend_plan(plan);

    let mut module = mir_module(vec![mir_function(
        ValkyrieType::Unit,
        Vec::new(),
        Vec::new(),
        vec![mir_block(
            0,
            Vec::new(),
            vec![mir_instr(
                MirOperation::Call {
                    callee: MirOperand::Callable(imported),
                    arguments: Vec::new(),
                },
                Vec::new(),
            )],
            MirTerminator::Return { value: None },
        )],
    )]);
    module.external_calls.push(MirExternalCallContract {
        declaration: None,
        instance: Some(imported),
        symbol: NamePath::new(vec![Identifier::new("neutral"), Identifier::new("invoke")]),
        link,
        parameter_types: Vec::new(),
        return_type: ValkyrieType::Unit,
    });

    let observation = assert_same_observation(&rust, &module, case_id);
    assert_eq!(observation, format!("{case_id}|accept||"));
}

#[test]
fn text_encoding_contract_paired_observation_matches() {
    let utf8_as_utf16 = rust_submission(vec![(
        "fixture.entry",
        rust_function(
            "fixture.entry",
            NyarType::Unit,
            Vec::new(),
            Vec::new(),
            vec![rust_block(
                0,
                Vec::new(),
                vec![rust_instr(
                    InstructionKind::LoadConstant {
                        constant: Constant::Utf8("follow".to_string()),
                        ty: Some(NyarType::Utf16),
                    },
                    Vec::new(),
                )],
                Terminator::Return { value: None },
            )],
        ),
    )]);
    let frontend_utf8_as_utf16 = mir_module(vec![mir_function(
        ValkyrieType::Unit,
        Vec::new(),
        Vec::new(),
        vec![mir_block(
            0,
            Vec::new(),
            vec![mir_instr(
                MirOperation::LoadConstant {
                    constant: MirConstant::Utf8("follow".to_string()),
                    ty: Some(ValkyrieType::Utf16),
                },
                Vec::new(),
            )],
            MirTerminator::Return { value: None },
        )],
    )]);
    let observation = assert_same_observation(&utf8_as_utf16, &frontend_utf8_as_utf16, "text_encoding.utf8_as_utf16");
    assert!(observation.starts_with("text_encoding.utf8_as_utf16|reject|SMIR007|"));

    let utf16_as_utf8 = rust_submission(vec![(
        "fixture.entry",
        rust_function(
            "fixture.entry",
            NyarType::Unit,
            Vec::new(),
            Vec::new(),
            vec![rust_block(
                0,
                Vec::new(),
                vec![rust_instr(
                    InstructionKind::LoadConstant {
                        constant: Constant::Utf16("follow".to_string()),
                        ty: Some(NyarType::Utf8),
                    },
                    Vec::new(),
                )],
                Terminator::Return { value: None },
            )],
        ),
    )]);
    let frontend_utf16_as_utf8 = mir_module(vec![mir_function(
        ValkyrieType::Unit,
        Vec::new(),
        Vec::new(),
        vec![mir_block(
            0,
            Vec::new(),
            vec![mir_instr(
                MirOperation::LoadConstant {
                    constant: MirConstant::Utf16("follow".to_string()),
                    ty: Some(ValkyrieType::Utf8),
                },
                Vec::new(),
            )],
            MirTerminator::Return { value: None },
        )],
    )]);
    let observation = assert_same_observation(&utf16_as_utf8, &frontend_utf16_as_utf8, "text_encoding.utf16_as_utf8");
    assert!(observation.starts_with("text_encoding.utf16_as_utf8|reject|SMIR007|"));
}

#[test]
fn ambiguous_source_text_type_is_rejected_before_semantic_mir() {
    // Rust canonical MIR deliberately has no unqualified text variant. The
    // Valkyrie source boundary must therefore reject the same spelling before
    // any MIR value or backend carrier can be created.
    let case_id = "type.ambiguous_text_forbidden";
    let rust = match "string" {
        "string" | "str" | "String" => format!("{case_id}|reject|SMIR007|function"),
        _ => format!("{case_id}|accept||"),
    };
    let valkyrie = match validate_source_text_type_name("string") {
        Ok(()) => format!("{case_id}|accept||"),
        Err(_) => format!("{case_id}|reject|SMIR007|function"),
    };
    assert_eq!(rust, valkyrie);
    assert_eq!(rust, "type.ambiguous_text_forbidden|reject|SMIR007|function");
}

#[test]
fn missing_ssa_output_type_paired_observation_matches() {
    let case_id = "value_type.missing";
    let rust = rust_submission(vec![(
        "fixture.entry",
        rust_function(
            "fixture.entry",
            NyarType::Unit,
            Vec::new(),
            Vec::new(),
            vec![rust_block(
                0,
                Vec::new(),
                vec![rust_instr(
                    InstructionKind::LoadConstant {
                        constant: Constant::Unit,
                        ty: Some(NyarType::Unit),
                    },
                    vec![ValueRef(9)],
                )],
                Terminator::Return { value: None },
            )],
        ),
    )]);
    let frontend = mir_module(vec![mir_function(
        ValkyrieType::Unit,
        Vec::new(),
        Vec::new(),
        vec![mir_block(
            0,
            Vec::new(),
            vec![mir_instr(
                MirOperation::LoadConstant {
                    constant: MirConstant::Unit,
                    ty: Some(ValkyrieType::Unit),
                },
                vec![MirValueRef(9)],
            )],
            MirTerminator::Return { value: None },
        )],
    )]);

    let observation = assert_same_observation(&rust, &frontend, case_id);
    assert!(observation.starts_with("value_type.missing|reject|SMIR001|"));
}

#[test]
fn missing_entry_contract_paired_observations_match() {
    let case_id = "entry.missing";
    let mut rust_function = rust_function(
        "fixture.entry",
        NyarType::Unit,
        Vec::new(),
        Vec::new(),
        vec![rust_block(0, Vec::new(), Vec::new(), Terminator::Return { value: None })],
    );
    rust_function.entry = BlockRef(99);
    let rust = rust_submission(vec![("fixture.entry", rust_function)]);
    let mut frontend_function = mir_function(
        ValkyrieType::Unit,
        Vec::new(),
        Vec::new(),
        vec![mir_block(0, Vec::new(), Vec::new(), MirTerminator::Return { value: None })],
    );
    frontend_function.entry = MirBlockRef(99);
    let frontend = mir_module(vec![frontend_function]);

    let observation = assert_same_observation(&rust, &frontend, case_id);
    assert_eq!(observation, format!("{case_id}|reject|SMIR009|entry"));
}

#[test]
fn residual_high_level_pattern_paired_observation_matches() {
    let case_id = "residual.high_level_pattern";
    let rust = rust_submission(vec![(
        "fixture.entry",
        rust_function(
            "fixture.entry",
            NyarType::Unit,
            Vec::new(),
            Vec::new(),
            vec![rust_block(
                0,
                Vec::new(),
                vec![rust_instr(
                    InstructionKind::PatternMatch {
                        value: Operand::Constant(Constant::Unit),
                        pattern_debug: "Wildcard".to_string(),
                    },
                    Vec::new(),
                )],
                Terminator::Return { value: None },
            )],
        ),
    )]);
    let frontend = mir_module(vec![mir_function(
        ValkyrieType::Unit,
        Vec::new(),
        Vec::new(),
        vec![mir_block(
            0,
            Vec::new(),
            vec![mir_instr(
                MirOperation::PatternMatch {
                    value: MirOperand::Constant(MirConstant::Unit),
                    pattern: HirPattern::Wildcard,
                },
                Vec::new(),
            )],
            MirTerminator::Return { value: None },
        )],
    )]);

    let observation = assert_same_observation(&rust, &frontend, case_id);
    assert_eq!(observation, format!("{case_id}|reject|SMIR008|instruction"));
}

fn rust_missing_entry_submission() -> FragmentSubmission {
    let mut function = rust_function(
        "fixture.entry",
        NyarType::Unit,
        Vec::new(),
        Vec::new(),
        vec![rust_block(0, Vec::new(), Vec::new(), Terminator::Return { value: None })],
    );
    function.entry = BlockRef(99);
    rust_submission(vec![("fixture.entry", function)])
}

fn driver_error(
    submission: &FragmentSubmission,
    family: TargetBackendFamily,
    boundary: HostProjectionBoundary,
) -> nyar_emitter::FragmentLoweringError {
    nyar_emitter::lowering::lower_fragment_to_driver_input(
        submission,
        family,
        boundary,
        PathBuf::new(),
        "wasm32-unknown-unknown",
        WasmPackageKind::Binary,
    )
    .err()
    .expect("formal backend entry accepted an invalid semantic module")
}

#[test]
fn formal_backend_entries_fail_closed_before_emission() {
    let submission = rust_missing_entry_submission();
    for (family, boundary) in [
        (TargetBackendFamily::Clr, HostProjectionBoundary::WasmJsGlue),
        (TargetBackendFamily::Jvm, HostProjectionBoundary::WasmJsGlue),
        (TargetBackendFamily::Wasm, HostProjectionBoundary::WasmJsGlue),
        (TargetBackendFamily::Wasm, HostProjectionBoundary::WasiComponent),
    ] {
        let error = driver_error(&submission, family, boundary);
        assert!(
            matches!(&error, nyar_emitter::FragmentLoweringError::SemanticContract(contract) if contract.code == "SMIR009"),
            "formal backend entry `{family:?}`/`{boundary:?}` bypassed the semantic MIR gate: {error}"
        );
    }
}

#[test]
fn formal_backend_entries_reject_the_same_physical_contract_gap() {
    let case_id = "wide_scalar";
    let submission = rust_submission(vec![(
        "fixture.entry",
        rust_function(
            "fixture.entry",
            NyarType::Integer128 { signed: true },
            vec![NyarType::Integer128 { signed: true }],
            vec![(ValueRef(0), NyarType::Integer128 { signed: true })],
            vec![rust_block(
                0,
                vec![ValueRef(0)],
                Vec::new(),
                Terminator::Return {
                    value: Some(Operand::Value(ValueRef(0))),
                },
            )],
        ),
    )]);
    let frontend = mir_module(vec![mir_function(
        ValkyrieType::Integer128 { signed: true },
        vec![ValkyrieType::Integer128 { signed: true }],
        vec![(MirValueRef(0), ValkyrieType::Integer128 { signed: true })],
        vec![mir_block(
            0,
            vec![MirValueRef(0)],
            Vec::new(),
            MirTerminator::Return {
                value: Some(MirOperand::Value(MirValueRef(0))),
            },
        )],
    )]);

    let observation = assert_same_observation(&submission, &frontend, case_id);
    assert_eq!(observation, format!("{case_id}|accept||"));

    for target in [
        PhysicalContractTarget::Clr,
        PhysicalContractTarget::Jvm,
        PhysicalContractTarget::WasmJsGlue,
        PhysicalContractTarget::WasiComponent,
    ] {
        assert_eq!(
            physical_contract_observation(&submission, case_id, target),
            format!("{case_id}|reject|BPHYS001|function"),
            "target `{target:?}` diverged from the physical contract gap"
        );
    }
}


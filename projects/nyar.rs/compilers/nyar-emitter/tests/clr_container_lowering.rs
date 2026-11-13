//! CLR 容器对象模型与容器指令的结构化验证。
//!
//! 用例直接构造已冻结的片段计划，验证布局/sum 类型定义、数组/聚合构造与存取
//! 指令，以及未支持元素载体的结构化拒绝；不经过前端。

use std::collections::BTreeMap;

use nyar::{Identifier, QualifiedName};
use nyar_emitter::{
    FragmentSubmission, TargetExecutionPlan,
    contracts::ArrayInitialization,
    nyar_backend_clr::{MsilInstruction, MsilInstructionOperand, MsilModule, MsilOpcode, MsilType},
    testing::lower_fragment_to_clr_managed_surface,
};
use nyar_types::{
    AggregateLayout, AggregateLayoutPlan, Block, BlockRef, Constant, ExecutableFunction, FieldId, FieldLayout, Instruction, InstructionKind,
    ItemInstanceId, NominalInstanceId, NyarType, Operand, StorageKind, Terminator, ValueRef,
};

const INTEGER64: NyarType = NyarType::Integer64 { signed: true };
const INTEGER32: NyarType = NyarType::Integer32 { signed: true };

fn array_of_integer64() -> NyarType {
    NyarType::Array(Box::new(INTEGER64))
}

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

fn instr(kind: InstructionKind, results: Vec<ValueRef>) -> Instruction {
    let mut instruction = Instruction::from_kind(kind);
    instruction.results = results;
    instruction
}

fn method_code<'a>(module: &'a MsilModule, name: &str) -> &'a [MsilInstruction] {
    module.global_methods.iter().find(|method| method.method.name == name).expect("lowered global method").instructions.as_slice()
}

fn exported_plan(fixture: ExecutableFunction, name: &str) -> FragmentSubmission {
    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("clr_container").testing_entry_operation(main);
    plan.testing_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), name.to_string());
    FragmentSubmission::from_backend_plan(plan)
}

/// `ArrayList` 引用布局：字段 `items: [i64]`、`length: i32`。
fn array_list_layout() -> AggregateLayout {
    AggregateLayout {
        id: 0,
        name: "ArrayList".to_string(),
        namespace: "core".to_string(),
        key: None,
        storage: StorageKind::Reference,
        size: 16,
        align: 8,
        fields: vec![
            FieldLayout { name: "items".to_string(), ty: array_of_integer64(), offset: 0, size: 8, align: 8 },
            FieldLayout { name: "length".to_string(), ty: INTEGER32, offset: 8, size: 4, align: 4 },
        ],
    }
}

#[test]
fn lowers_array_list_construction_length_and_index_with_typed_elements() {
    let layout = array_list_layout();
    let field_items = FieldId::from_index(0).expect("items field id");
    let field_length = FieldId::from_index(1).expect("length field id");
    let nominal = NominalInstanceId::from_index(0).expect("nominal id");

    let mut fixture = function("clr.array_list", INTEGER64, Vec::new());
    fixture.value_types.insert(ValueRef(0), array_of_integer64());
    fixture.value_types.insert(ValueRef(1), NyarType::Nominal { layout: layout.id, spelling: Identifier::new("ArrayList") });
    fixture.value_types.insert(ValueRef(2), INTEGER32);
    fixture.value_types.insert(ValueRef(3), INTEGER64);
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::ArrayFromElements {
            layout_id: layout.id,
            array_type: array_of_integer64(),
            elements: vec![Operand::Constant(Constant::Int(1)), Operand::Constant(Constant::Int(2))],
        },
        vec![ValueRef(0)],
    ));
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::StructNew {
            nominal,
            fields: vec![
                (field_items, Operand::Value(ValueRef(0))),
                (field_length, Operand::Constant(Constant::Int(2))),
            ],
        },
        vec![ValueRef(1)],
    ));
    fixture.blocks[0].instructions.push(instr(InstructionKind::ArrayLength { array: Operand::Value(ValueRef(0)) }, vec![ValueRef(2)]));
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::ArrayGet { array: Operand::Value(ValueRef(0)), index: Operand::Constant(Constant::Int(0)) },
        vec![ValueRef(3)],
    ));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(ValueRef(3))) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("clr_container").testing_entry_operation(main);
    plan.testing_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), "main".to_string());
    plan.testing_register_nominal_layout(nominal, layout);
    plan.testing_register_field(field_items, 0, 0);
    plan.testing_register_field(field_length, 0, 1);

    let (module, _) = lower_fragment_to_clr_managed_surface(&FragmentSubmission::from_backend_plan(plan)).expect("array list must lower");
    let layout_type = module.types.iter().find(|ty| ty.full_name == "nyar::layout::ArrayList$0").expect("layout type definition");
    assert_eq!(layout_type.fields.len(), 2, "{layout_type:?}");
    assert_eq!(layout_type.fields[0].name, "field0");
    assert_eq!(layout_type.fields[0].ty, MsilType::sz_array(MsilType::Int64 { signed: true }));
    assert_eq!(layout_type.fields[1].ty, MsilType::Int32 { signed: true });
    assert!(layout_type.methods.iter().any(|method| method.method.name == ".ctor"), "{layout_type:?}");

    let instructions = method_code(&module, "main");
    assert!(
        instructions.iter().any(|instruction| matches!(
            &instruction.operand,
            Some(MsilInstructionOperand::Method(reference))
                if reference.owner.as_deref() == Some("nyar::layout::ArrayList$0") && reference.name == ".ctor"
        )),
        "{instructions:?}"
    );
    assert!(
        instructions.iter().any(|instruction| matches!(
            &instruction.operand,
            Some(MsilInstructionOperand::Field(owner, name))
                if owner == "nyar::layout::ArrayList$0" && name == "field0"
        )),
        "{instructions:?}"
    );
    assert!(
        instructions.iter().any(|instruction| matches!(
            &instruction.operand,
            Some(MsilInstructionOperand::Field(owner, name))
                if owner == "nyar::layout::ArrayList$0" && name == "field1"
        )),
        "{instructions:?}"
    );
    assert!(
        instructions.iter().any(|instruction| instruction.opcode == MsilOpcode::Newarr
            && matches!(&instruction.operand, Some(MsilInstructionOperand::Type(name)) if name == "int64")),
        "`[i64]` must allocate `int64[]`: {instructions:?}"
    );
    assert!(
        instructions.iter().any(|instruction| instruction.opcode == MsilOpcode::StelemAny
            && matches!(&instruction.operand, Some(MsilInstructionOperand::Type(name)) if name == "int64")),
        "`[i64]` elements must store with `stelem.any int64`: {instructions:?}"
    );
    assert!(
        instructions.iter().any(|instruction| instruction.opcode == MsilOpcode::LdelemAny
            && matches!(&instruction.operand, Some(MsilInstructionOperand::Type(name)) if name == "int64")),
        "`[i64]` elements must load with `ldelem.any int64`: {instructions:?}"
    );
    assert!(instructions.iter().any(|instruction| instruction.opcode == MsilOpcode::Ldlen), "{instructions:?}");
}

#[test]
fn zero_field_reference_aggregate_copy_does_not_allocate() {
    let layout = AggregateLayout {
        id: 1,
        name: "Marker".to_string(),
        namespace: "core".to_string(),
        key: None,
        storage: StorageKind::Reference,
        size: 1,
        align: 1,
        fields: Vec::new(),
    };
    let nominal_type = NyarType::Nominal { layout: layout.id, spelling: Identifier::new("Marker") };
    let mut fixture = function("clr.marker", NyarType::Unit, vec![nominal_type.clone(), nominal_type]);
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::AggregateCopy { layout_id: layout.id, source: Operand::Value(ValueRef(0)), dest: Operand::Value(ValueRef(1)) },
        Vec::new(),
    ));

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("clr_container").testing_entry_operation(main);
    plan.testing_aggregate_layouts(AggregateLayoutPlan { layouts: vec![layout], ..AggregateLayoutPlan::default() });

    let (module, _) =
        lower_fragment_to_clr_managed_surface(&FragmentSubmission::from_backend_plan(plan)).expect("zero-field reference copy must lower");
    let instructions = method_code(&module, "main");
    assert!(instructions.iter().all(|instruction| instruction.opcode != MsilOpcode::Newobj), "{instructions:?}");
    assert!(
        instructions
            .iter()
            .any(|instruction| instruction.opcode == MsilOpcode::Ldloc
                && matches!(&instruction.operand, Some(MsilInstructionOperand::Integer(0)))),
        "{instructions:?}"
    );
    assert!(
        instructions
            .iter()
            .any(|instruction| instruction.opcode == MsilOpcode::Stloc
                && matches!(&instruction.operand, Some(MsilInstructionOperand::Integer(1)))),
        "{instructions:?}"
    );
}

#[test]
fn integer128_array_element_is_rejected_structurally() {
    let element = NyarType::Integer128 { signed: true };
    let array_type = NyarType::Array(Box::new(element));
    let mut fixture = function("clr.invalid_array", NyarType::Unit, Vec::new());
    fixture.value_types.insert(ValueRef(0), array_type.clone());
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::ArrayFromElements {
            layout_id: 0,
            array_type,
            elements: vec![Operand::Constant(Constant::Int(1))],
        },
        vec![ValueRef(0)],
    ));

    let error = lower_fragment_to_clr_managed_surface(&exported_plan(fixture, "main")).expect_err("`i128` array element must fail");
    assert!(error.contains("i128"), "{error}");
    assert!(error.contains("数组元素类型"), "{error}");
}

#[test]
fn array_fill_initialization_is_rejected_structurally() {
    let array_type = NyarType::Array(Box::new(INTEGER32));
    let mut fixture = function("clr.fill", NyarType::Unit, vec![INTEGER32]);
    fixture.value_types.insert(ValueRef(1), array_type.clone());
    fixture.blocks[0].instructions.push(instr(
        InstructionKind::ArrayNew {
            array_type,
            length: Operand::Value(ValueRef(0)),
            initialization: ArrayInitialization::Fill(Operand::Constant(Constant::Int(0))),
        },
        vec![ValueRef(1)],
    ));

    let error = lower_fragment_to_clr_managed_surface(&exported_plan(fixture, "main")).expect_err("`Fill` must fail");
    assert!(error.contains("Fill"), "{error}");
}

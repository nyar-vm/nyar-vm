use nyar_types::{
    CanonicalProgram, CanonicalSemanticMir, CanonicalTypeKind, NominalValueSemantics, ValueIdentity, layout_choice::ValueRepresentation,
    pipeline::RepresentationPlanStage,
};
use nyar_types::layout::{AggregateLayout, AggregateLayoutPlan, FieldLayout, StorageKind};
use rusty_valkyrie::{compiler::emitter::pipeline::CanonicalRepresentationPlanner, *};
use std::collections::BTreeSet;

use crate::primitives::with_primitives;

#[test]
fn source_aggregate_semantics_reach_the_representation_plan_without_construction() {
    let output = rusty_valkyrie::ValkyrieCompiler::default()
        .compile_source_to_program(&with_primitives(
            "structure Point { value: i32 } class Node { value: i32 } \
             micro value_identity(value: Point) -> Point { return value } \
             micro reference_identity(value: Node) -> Node { return value }",
        ))
        .expect("声明与参数源码必须完成编译，不依赖构造指令补布局");
    let program = output.canonical();
    let plan = output.representation();
    assert_eq!(program.linked.nominal_instances.len(), 2);
    assert_eq!(plan.adt_reps.len(), 2);
    let semantic_mir = rusty_valkyrie::compiler::mir::MirLowerer::lower_module_semantic(&rusty_valkyrie::ValkyrieCompiler::default().compile_source(&with_primitives("structure Point { value: i32 } class Node { value: i32 } micro value_identity(value: Point) -> Point { return value } micro reference_identity(value: Node) -> Node { return value }")).expect("source for representation test"));
    for aggregate in &semantic_mir.structs {
        // canonical 类型表按前端冻结的 `Nominal { declaration }` 形状收录聚合，禁止按短名回查。
        let item = aggregate.declaration.expect("structure/class 声明必须在前端冻结 declaration ItemId");
        let ty = rusty_valkyrie::compiler::types::hir::ValkyrieType::Nominal {
            declaration: item,
            arguments: Vec::new(),
            // 查找键的身份必须与聚合声明一致，否则命不中类型表。
            primitive_identity: aggregate.primitive_identity.clone(),
        };
        let declaration = semantic_mir.type_identities[&ty];
        let record = program.linked.nominal_instances.values().find(|record| record.declaration == declaration).unwrap();
        let expected = if aggregate.is_value_type { NominalValueSemantics::Value } else { NominalValueSemantics::Reference };
        assert_eq!(record.semantics, expected);
        assert_eq!(record.ty, declaration);
    }
    let mut observed = BTreeSet::new();
    for function in program.mir.functions.values() {
        let (value, ty) = function.parameters[0];
        let CanonicalTypeKind::Nominal { declaration, .. } = &program.linked.types[&ty].kind
        else {
            panic!("源码参数必须保留名义类型");
        };
        let nominal = program.linked.nominal_instances.values().find(|record| record.declaration == *declaration).unwrap();
        let representation = &plan.value_reps[&ValueIdentity::new(function.instance, value)];
        match nominal.semantics {
            NominalValueSemantics::Value => {
                assert_eq!(*representation, ValueRepresentation::Specialized);
                observed.insert("value");
            }
            NominalValueSemantics::Reference => {
                assert_eq!(*representation, ValueRepresentation::Reified);
                observed.insert("reference");
            }
        }
    }
    assert_eq!(observed, BTreeSet::from(["value", "reference"]));
    let mut invalid = program.clone();
    invalid.linked.nominal_instances.clear();
    invalid.linked.fields.clear();
    let error = CanonicalRepresentationPlanner.plan(&invalid).expect_err("缺名义声明语义不能默认成值聚合");
    assert_eq!(error.records[0].code, "PLAN001");
    assert_eq!(error.records[0].stage, nyar_types::CompileStage::ValidateMir);
}

#[test]
fn planner_requires_each_complete_nominal_type_before_representation() {
    use nyar_types::{NominalInstanceId, TypeId, TypeRecord};

    let output = rusty_valkyrie::ValkyrieCompiler::default()
        .compile_source_to_program(&with_primitives(
            "class Node {} micro first(value: Node, tag: bool) -> Node { return value } \
             micro second(value: Node, tag: i32) -> Node { return value }",
        ))
        .expect("源码声明与函数合同必须完成编译");
    let mut program = output.canonical().clone();
    let original = program.linked.nominal_instances.values().next().unwrap().clone();
    let functions = program.mir.functions.keys().copied().collect::<Vec<_>>();
    let mut instantiated_types = Vec::new();
    for (index, function_id) in functions.iter().enumerate() {
        let ty = TypeId::from_index(100 + index as u32).unwrap();
        let nominal = NominalInstanceId::from_index(100 + index as u32).unwrap();
        let function = program.mir.functions.get_mut(function_id).unwrap();
        let argument = function.parameters[1].1;
        program.linked.types.insert(
            ty,
            TypeRecord {
                declaration: original.declaration,
                kind: CanonicalTypeKind::Nominal { declaration: original.declaration, arguments: vec![argument] },
            },
        );
        let mut record = original.clone();
        record.ty = ty;
        program.linked.nominal_instances.insert(nominal, record);
        function.parameters[0].1 = ty;
        function.return_type = ty;
        function.value_types.insert(function.parameters[0].0, ty);
        // canonical 校验要求入口块参数类型与函数参数表逐位一致，夹具必须同步代入新实例类型。
        let entry = function.entry;
        function.blocks.get_mut(&entry).expect("入口块必须存在").parameters[0].1 = ty;
        let signature = program.linked.item_instances.get_mut(function_id).unwrap();
        signature.parameter_types[0] = ty;
        signature.return_type = ty;
        instantiated_types.push((nominal, ty));
    }
    let plan = CanonicalRepresentationPlanner.plan(&program).expect("完整实例夹具独立规划，非泛型源码生产证明");
    for (function_id, (nominal, _)) in functions.iter().zip(&instantiated_types) {
        let value = program.mir.functions[function_id].parameters[0].0;
        assert_eq!(plan.value_reps[&ValueIdentity::new(*function_id, value)], ValueRepresentation::Reified);
        assert!(plan.adt_reps.contains_key(nominal));
    }
    program.linked.nominal_instances.remove(&instantiated_types[1].0);
    let error = CanonicalRepresentationPlanner.plan(&program).expect_err("同声明另一个实例不能代替缺失的完整实例");
    assert_eq!(error.records[0].code, "PLAN001");
    assert_eq!(error.records[0].stage, nyar_types::CompileStage::ValidateMir);
}

#[test]
fn planner_accepts_only_validated_canonical_program() {
    let program = CanonicalProgram { linked: Default::default(), mir: CanonicalSemanticMir::default() };
    let plan = CanonicalRepresentationPlanner.plan(&program).expect("空 canonical 程序是合法成功值");
    assert!(plan.invoke_lowerings.is_empty());
}

#[test]
fn source_function_values_keep_their_owners_through_representation_planning() {
    let output = rusty_valkyrie::ValkyrieCompiler::default()
        .compile_source_to_program(&with_primitives(
            "micro boolean_identity(value: bool) -> bool { return value } \
                 micro integer_identity(value: i32) -> i32 { return value }",
        ))
        .expect("源码必须完成前端分析");
    let program = output.canonical();
    let plan = CanonicalRepresentationPlanner.plan(&program).expect("完整语义合同必须完成表示规划");
    let functions = program.mir.functions.values().collect::<Vec<_>>();
    assert_eq!(functions.len(), 2);
    let first_value = functions[0].parameters[0].0;
    let second_value = functions[1].parameters[0].0;
    assert_eq!(first_value, second_value);
    assert_ne!(functions[0].parameters[0].1, functions[1].parameters[0].1);
    let expected_values = functions.iter().map(|function| function.value_types.len()).sum::<usize>();
    assert_eq!(plan.value_reps.len(), expected_values);
    for function in &functions {
        for value in function.value_types.keys() {
            assert!(plan.value_reps.contains_key(&ValueIdentity::new(function.instance, *value)));
        }
    }

    let mut invalid = program.clone();
    invalid.mir.functions.get_mut(&functions[1].instance).unwrap().value_types.remove(&second_value);
    let error = CanonicalRepresentationPlanner.plan(&invalid).expect_err("缺类型必须在表示规划前失败");
    assert_eq!(error.records[0].code, "PLAN001");
    assert_eq!(error.records[0].stage, nyar_types::CompileStage::ValidateMir);
}

fn layout_item(index: u32) -> nyar_types::ItemId {
    nyar_types::ItemId::from_index(index).expect("ItemId")
}

fn inner_layout(id: u32) -> AggregateLayout {
    AggregateLayout {
        id,
        name: "Inner".into(),
        namespace: "example".into(),
        key: None,
        storage: StorageKind::Value,
        fields: Vec::new(),
    }
}

fn record_layout(id: u32, inner_id: u32) -> AggregateLayout {
    AggregateLayout {
        id,
        name: "Record".into(),
        namespace: "example".into(),
        key: None,
        storage: StorageKind::Value,
        fields: vec![FieldLayout {
            name: "inner".into(),
            ty: nyar_types::NyarType::Nominal { layout: inner_id, spelling: nyar_types::Identifier::new("Inner") },
        }],
    }
}

fn modular_layout_consumer() -> rusty_valkyrie::mir::MirModule {
    let hir = rusty_valkyrie::ValkyrieCompiler::default()
        .compile_source("micro entry() -> unit { return }")
        .expect("源码解析");
    let mut consumer = rusty_valkyrie::compiler::mir::MirLowerer::lower_module_semantic(&hir);
    consumer.name = "consumer".into();
    consumer.aggregate_layouts = AggregateLayoutPlan::default();
    // consumer 本地编号：Record=5 的字段引用 Inner=7。
    consumer.aggregate_layouts.layouts.push(record_layout(5, 7));
    consumer.aggregate_layouts.layouts.push(inner_layout(7));
    consumer.aggregate_layouts.declaration_to_layout.insert(layout_item(1), 5);
    consumer.aggregate_layouts.declaration_to_layout.insert(layout_item(2), 7);
    consumer.aggregate_layouts.value_type_declarations.insert(layout_item(1));
    consumer
}

#[test]
fn repeated_layout_declaration_merges_by_identity_with_remapped_local_ids() {
    let consumer = modular_layout_consumer();
    let mut dependency = consumer.clone();
    dependency.name = "dependency".into();
    dependency.aggregate_layouts = AggregateLayoutPlan::default();
    // 依赖本地编号不同：Record=1 的字段引用 Inner=3，但声明身份相同。
    dependency.aggregate_layouts.layouts.push(record_layout(1, 3));
    dependency.aggregate_layouts.layouts.push(inner_layout(3));
    dependency.aggregate_layouts.declaration_to_layout.insert(layout_item(1), 1);
    dependency.aggregate_layouts.declaration_to_layout.insert(layout_item(2), 3);
    dependency.aggregate_layouts.value_type_declarations.insert(layout_item(1));

    let merged = rusty_valkyrie::compiler::emitter::pipeline::merge_aggregate_layouts(&consumer, &[dependency])
        .expect("同一声明合同必须合并");
    assert_eq!(merged.layouts.len(), 2);
    assert_eq!(merged.declaration_to_layout[&layout_item(2)], 7);
    let record = merged.layouts.iter().find(|layout| layout.id == 5).expect("consumer Record 保留");
    assert!(
        matches!(&record.fields[0].ty, nyar_types::NyarType::Nominal { layout: 7, .. }),
        "依赖侧本地布局编号必须重写到合并身份：{:?}",
        record.fields[0].ty
    );
}

#[test]
fn layout_contract_conflict_across_modules_still_fails() {
    let consumer = modular_layout_consumer();
    let mut dependency = consumer.clone();
    dependency.name = "dependency".into();
    dependency.aggregate_layouts = AggregateLayoutPlan::default();
    // 同一 declaration，依赖侧存储类不同 —— 语义合同冲突仍必须闭合。
    dependency.aggregate_layouts.layouts.push(record_layout(1, 3));
    dependency.aggregate_layouts.layouts.push(inner_layout(3));
    dependency
        .aggregate_layouts
        .layouts
        .iter_mut()
        .find(|layout| layout.id == 3)
        .expect("dependency Inner 必须已登记")
        .storage = StorageKind::Reference;
    dependency.aggregate_layouts.declaration_to_layout.insert(layout_item(1), 1);
    dependency.aggregate_layouts.declaration_to_layout.insert(layout_item(2), 3);
    dependency.aggregate_layouts.value_type_declarations.insert(layout_item(1));

    let error = rusty_valkyrie::compiler::emitter::pipeline::merge_aggregate_layouts(&consumer, &[dependency])
        .expect_err("存储类不同不能合并");
    assert!(error.to_string().contains("合同冲突"), "实际错误：{error}");
}

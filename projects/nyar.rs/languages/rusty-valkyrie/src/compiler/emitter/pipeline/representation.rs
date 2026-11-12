//! CanonicalProgram 到目标无关 RepresentationPlan 的唯一规划器。

use std::collections::BTreeSet;

use nyar_types::{
    CanonicalConstant, CanonicalOperation, CanonicalPrimitiveType, CanonicalProgram, CanonicalTypeKind, NominalValueSemantics, StageResult,
    ValueIdentity,
    layout_choice::{
        AdtRepresentation, InvokeLowering, RepresentationPlan, SumRepresentation, SumVariantRepresentation, TextRepresentation,
        ValueRepresentation,
    },
    pipeline::RepresentationPlanStage,
};

use super::diagnostics::fail_stage;

/// 只消费 CanonicalProgram 的目标无关表示规划器。
#[derive(Debug, Clone, Copy, Default)]
pub struct CanonicalRepresentationPlanner;

impl RepresentationPlanStage for CanonicalRepresentationPlanner {
    fn plan(&self, program: &CanonicalProgram) -> StageResult<RepresentationPlan> {
        program.validate().map_err(|error| {
            fail_stage::<()>(
                nyar_types::CompileStage::ValidateMir,
                "PLAN001",
                &program.linked.module_name,
                format!("planner 收到未验证的 CanonicalProgram: {error:?}"),
            )
            .err()
            .unwrap()
        })?;

        let mut plan = RepresentationPlan::default();
        plan.text_representation = plan_text_representation(program);
        for nominal in program.linked.nominal_instances.keys() {
            plan.adt_reps.insert(*nominal, AdtRepresentation::TypedAggregate);
        }
        for layout in &program.linked.sum_types {
            if !program.linked.nominal_instances.contains_key(&layout.nominal) {
                return fail_stage(
                    nyar_types::CompileStage::RepresentationPlan,
                    "PLAN003",
                    &program.linked.module_name,
                    format!("sum 布局引用未知 nominal identity: {:?}", layout.nominal),
                );
            }
            let mut variants = std::collections::BTreeMap::new();
            for variant in &layout.variants {
                let Some(record) = program.linked.variants.get(&(layout.nominal, variant.id))
                else {
                    return fail_stage(
                        nyar_types::CompileStage::RepresentationPlan,
                        "PLAN004",
                        &program.linked.module_name,
                        format!("sum 布局引用未知 variant identity: {:?}/{:?}", layout.nominal, variant.id),
                    );
                };
                let payload_type = record.payload_type;
                if payload_type.is_some() != variant.payload_type.is_some() {
                    return fail_stage(
                        nyar_types::CompileStage::RepresentationPlan,
                        "PLAN005",
                        &program.linked.module_name,
                        format!("sum variant payload 合同不一致: {:?}/{:?}", layout.nominal, variant.id),
                    );
                }
                if variants.insert(variant.id, SumVariantRepresentation { id: variant.id, tag: variant.tag, payload_type }).is_some() {
                    return fail_stage(
                        nyar_types::CompileStage::RepresentationPlan,
                        "PLAN006",
                        &program.linked.module_name,
                        format!("sum variant identity 重复: {:?}/{:?}", layout.nominal, variant.id),
                    );
                }
            }
            if plan
                .sum_reps
                .insert(layout.nominal, SumRepresentation { nominal: layout.nominal, tag_width: layout.tag_width, variants })
                .is_some()
            {
                return fail_stage(
                    nyar_types::CompileStage::RepresentationPlan,
                    "PLAN007",
                    &program.linked.module_name,
                    format!("sum nominal identity 重复: {:?}", layout.nominal),
                );
            }
        }
        let nominal_semantics =
            program.linked.nominal_instances.values().map(|record| (record.ty, record.semantics)).collect::<std::collections::BTreeMap<_, _>>();
        let mut instruction_ids = BTreeSet::new();
        for function in program.mir.functions.values() {
            for value in function.value_types.keys() {
                let ty = &program.linked.types[&function.value_types[value]].kind;
                let representation = match ty {
                    CanonicalTypeKind::Nominal { .. } => match nominal_semantics[&function.value_types[value]] {
                        NominalValueSemantics::Value => ValueRepresentation::Specialized,
                        NominalValueSemantics::Reference => ValueRepresentation::Reified,
                    },
                    _ => ValueRepresentation::Specialized,
                };
                plan.value_reps.insert(ValueIdentity::new(function.instance, *value), representation);
            }
            for block in function.blocks.values() {
                for instruction in &block.instructions {
                    if !instruction_ids.insert(instruction.id) {
                        return fail_stage(
                            nyar_types::CompileStage::RepresentationPlan,
                            "PLAN002",
                            &program.linked.module_name,
                            format!("InstructionId 重复: {}", instruction.id.index()),
                        );
                    }
                    match &instruction.operation {
                        CanonicalOperation::Invoke { callee, .. } => {
                            let lowering = match callee {
                                nyar_types::CanonicalCallee::Item(_) => InvokeLowering::Direct,
                                nyar_types::CanonicalCallee::Value(_) => InvokeLowering::TypedReference,
                                // witness 分派只登记类型化 witness 降低选择；具体 self 的消去
                                // 与开放泛型的字典 ABI 留给目标合同，不在此选 adaptor。
                                nyar_types::CanonicalCallee::Witness(_) => InvokeLowering::TypedWitness,
                            };
                            plan.invoke_lowerings.insert(instruction.id, lowering);
                        }
                        CanonicalOperation::StructNew { .. }
                        | CanonicalOperation::Copy { .. }
                        | CanonicalOperation::Cast { .. }
                        | CanonicalOperation::StoreVar { .. }
                        | CanonicalOperation::AggregateCopy { .. }
                        | CanonicalOperation::LoadConstant { .. }
                        | CanonicalOperation::SumNew { .. }
                        | CanonicalOperation::SumPayloadGet { .. }
                        | CanonicalOperation::SumVariantIs { .. }
                        | CanonicalOperation::FieldGet { .. }
                        | CanonicalOperation::FieldSet { .. }
                        | CanonicalOperation::ArrayGet { .. }
                        | CanonicalOperation::ArrayNew { .. }
                        | CanonicalOperation::ArrayFromElements { .. }
                        | CanonicalOperation::ArraySet { .. }
                        | CanonicalOperation::ArrayLength { .. }
                        | CanonicalOperation::NullIsNull { .. }
                        | CanonicalOperation::NullUnwrap { .. }
                        | CanonicalOperation::LoadFunctionRef { .. }
                        | CanonicalOperation::TupleNew { .. }
                        | CanonicalOperation::TupleGet { .. } => {}
                    }
                }
            }
        }
        Ok(plan)
    }
}

/// 程序级文本表示事实的唯一规划点。
///
/// 文本值空间只有两种正式来源：源程序在 `std.text` 下声明并登记了 canonical
/// 文本载体（`text_encoding_types`）时，文本值由该载体布局承载，规划为
/// [`TextRepresentation::Managed`]；未声明任何载体时，`utf8` / `utf16` 值就是
/// 宿主编码基元本身，规划为 [`TextRepresentation::Raw`]。判定只读声明与值类型
/// 事实，不读布局注册表——布局缺失必须作为目标层的 missing 事实失败闭合，
/// 不能反过来成为 raw 的依据。程序没有任何文本值时保持 `None`。
fn plan_text_representation(program: &CanonicalProgram) -> Option<TextRepresentation> {
    if !program.linked.text_encoding_types.is_empty() {
        return Some(TextRepresentation::Managed);
    }
    let is_text_type = |type_id: nyar_types::TypeId| {
        matches!(
            program.linked.types.get(&type_id).map(|record| &record.kind),
            Some(CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Utf8 | CanonicalPrimitiveType::Utf16))
        )
    };
    let has_text_values = program.mir.functions.values().any(|function| {
        function.value_types.values().any(|ty| is_text_type(*ty))
            || function.parameters.iter().any(|(_, ty)| is_text_type(*ty))
            || is_text_type(function.return_type)
            || function.blocks.values().any(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(
                        &instruction.operation,
                        CanonicalOperation::LoadConstant { constant: CanonicalConstant::Utf8(_) | CanonicalConstant::Utf16(_) }
                    )
                })
            })
    });
    has_text_values.then_some(TextRepresentation::Raw)
}

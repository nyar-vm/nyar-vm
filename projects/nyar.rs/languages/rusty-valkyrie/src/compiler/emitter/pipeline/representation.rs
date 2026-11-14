//! ValkyrieMirProgram 到目标无关 RepresentationPlan 的唯一规划器。

use std::collections::BTreeSet;

use nyar_types::{CanonicalTypeKind, NominalValueSemantics, StageResult, ValueIdentity};

use crate::compiler::valkyrie_mir::{
    AdtRepresentation, InvokeLowering, RepresentationPlan, ValueRepresentation, ValkyrieMirOperation, ValkyrieMirProgram,
    pipeline::RepresentationPlanStage,
};

use super::diagnostics::fail_stage;

/// 只消费 ValkyrieMirProgram 的目标无关表示规划器。
#[derive(Debug, Clone, Copy, Default)]
pub struct CanonicalRepresentationPlanner;

impl RepresentationPlanStage for CanonicalRepresentationPlanner {
    fn plan(&self, program: &ValkyrieMirProgram) -> StageResult<RepresentationPlan> {
        program.validate().map_err(|error| {
            fail_stage::<()>(
                nyar_types::CompileStage::ValidateMir,
                "PLAN001",
                &program.linked.module_name,
                format!("planner 收到未验证的 ValkyrieMirProgram: {error:?}"),
            )
            .err()
            .unwrap()
        })?;

        let mut plan = RepresentationPlan::default();
        plan.text_representation = crate::compiler::valkyrie_mir::declared_text_representation(program);
        for nominal in program.linked.nominal_instances.keys() {
            plan.adt_reps.insert(*nominal, AdtRepresentation::TypedAggregate);
        }
        // sum 的 variant/payload 事实由链接程序唯一拥有；平台 tag 宽度与槽位
        // 由各目标在适配器内决定，这里不复制也不提前选择。
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
                        ValkyrieMirOperation::Invoke { callee, .. } => {
                            let lowering = match callee {
                                crate::compiler::valkyrie_mir::ValkyrieMirCallee::Item(_) => InvokeLowering::Direct,
                                crate::compiler::valkyrie_mir::ValkyrieMirCallee::Value(_) => InvokeLowering::TypedReference,
                                // witness 分派只登记类型化 witness 降低选择；具体 self 的消去
                                // 与开放泛型的字典 ABI 留给目标合同，不在此选 adaptor。
                                crate::compiler::valkyrie_mir::ValkyrieMirCallee::Witness(_) => InvokeLowering::TypedWitness,
                            };
                            plan.invoke_lowerings.insert(instruction.id, lowering);
                        }
                        ValkyrieMirOperation::StructNew { .. }
                        | ValkyrieMirOperation::Copy { .. }
                        | ValkyrieMirOperation::Cast { .. }
                        | ValkyrieMirOperation::StoreVar { .. }
                        | ValkyrieMirOperation::AggregateCopy { .. }
                        | ValkyrieMirOperation::LoadConstant { .. }
                        | ValkyrieMirOperation::SumNew { .. }
                        | ValkyrieMirOperation::SumPayloadGet { .. }
                        | ValkyrieMirOperation::SumVariantIs { .. }
                        | ValkyrieMirOperation::FieldGet { .. }
                        | ValkyrieMirOperation::FieldSet { .. }
                        | ValkyrieMirOperation::ArrayGet { .. }
                        | ValkyrieMirOperation::ArrayNew { .. }
                        | ValkyrieMirOperation::ArrayFromElements { .. }
                        | ValkyrieMirOperation::ArraySet { .. }
                        | ValkyrieMirOperation::ArrayLength { .. }
                        | ValkyrieMirOperation::NullIsNull { .. }
                        | ValkyrieMirOperation::NullUnwrap { .. }
                        | ValkyrieMirOperation::LoadFunctionRef { .. }
                        | ValkyrieMirOperation::TupleNew { .. }
                        | ValkyrieMirOperation::TupleGet { .. } => {}
                    }
                }
            }
        }
        Ok(plan)
    }
}

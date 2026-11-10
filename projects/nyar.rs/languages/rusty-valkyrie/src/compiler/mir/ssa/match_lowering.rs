use crate::compiler::types::hir::{HirMatchArm, ValkyrieType};

use super::{
    MirBlockRef, MirBuilder, MirCaseArm, MirCaseChain, MirConstant, MirOperand, MirTerminator, control_flow_context::MirFallthroughContext,
    control_flow_lowering::BranchArmExit, infer_builder_operand_type,
};

fn is_never_type(ty: &ValkyrieType) -> bool {
    matches!(ty, ValkyrieType::Void)
}

#[doc(hidden)]
pub fn arm_contributes_to_match_merge(
    arm_result: &MirOperand,
    actual_type: Option<&ValkyrieType>,
    expected_type: Option<&ValkyrieType>,
) -> bool {
    if actual_type.is_some_and(is_never_type) {
        return false;
    }
    // `unit` 没有可物化值槽：无论期望类型是否写成 `unit`，都不建立 merge 参数。
    // 与 `branch_merge_type` 过滤 `Unit` 是同一模型，避免把 `Constant(Unit)` 当成 phi 实参。
    if actual_type.is_some_and(|ty| matches!(ty, ValkyrieType::Unit)) {
        return false;
    }
    if expected_type.is_some_and(|ty| matches!(ty, ValkyrieType::Unit)) {
        return false;
    }
    !matches!(arm_result, MirOperand::Constant(MirConstant::Unit))
}
impl MirBuilder {
    /// `no_match` 入边回填分支前的变量值：该入边只在 `match` 未命中时可达，
    /// 语义上等于保持进入分支前的绑定。
    fn exit_binding_arguments(
        &self,
        exit_block: MirBlockRef,
        pre_bindings: &std::collections::BTreeMap<String, MirOperand>,
    ) -> Vec<MirOperand> {
        self.blocks[exit_block.0 as usize]
            .parameters
            .iter()
            .map(|parameter| {
                self.values
                    .iter()
                    .find(|value| value.id == *parameter)
                    .and_then(|value| match &value.origin {
                        super::MirValueOrigin::BlockParameter { name, .. } => pre_bindings.get(name).cloned(),
                        _ => None,
                    })
                    .unwrap_or(MirOperand::Constant(MirConstant::Unit))
            })
            .collect()
    }

    pub(super) fn lower_match_expr(
        &mut self,
        scrutinee: &crate::compiler::hir::HirExpr,
        arms: &[HirMatchArm],
        expected_type: Option<&crate::compiler::hir::ValkyrieType>,
    ) -> MirOperand {
        self.lower_case_like_expr(scrutinee, arms, true, expected_type)
    }

    pub(super) fn lower_case_expr(
        &mut self,
        scrutinee: &crate::compiler::hir::HirExpr,
        arms: &[HirMatchArm],
        expected_type: Option<&crate::compiler::hir::ValkyrieType>,
    ) -> MirOperand {
        self.lower_case_like_expr(scrutinee, arms, false, expected_type)
    }

    fn lower_case_like_expr(
        &mut self,
        scrutinee: &crate::compiler::hir::HirExpr,
        arms: &[HirMatchArm],
        produce_value: bool,
        expected_type: Option<&crate::compiler::hir::ValkyrieType>,
    ) -> MirOperand {
        let scrutinee_operand = self.lower_expr_to_operand(scrutinee);
        if arms.is_empty() {
            return MirOperand::Constant(MirConstant::Unit);
        }

        let entry_block = self.current_block;
        let entry_label = self.current_label.clone();
        let entry_instructions = self.instructions.clone();
        let entry_terminator = self.terminator.clone();

        let arm_blocks: Vec<MirBlockRef> = arms.iter().enumerate().map(|(index, _)| self.new_block(&format!("match_arm_{index}"))).collect();
        let no_match_block = self.new_block("match_no_match");
        let exit_block = self.new_block("match_exit");

        self.current_block = entry_block;
        self.current_label = entry_label.clone();
        self.instructions = entry_instructions;
        self.terminator = entry_terminator;
        self.terminate(MirTerminator::Jump { target: arm_blocks[0], arguments: Vec::new() });
        self.flush_block(&entry_label);

        let saved_bindings = self.bindings.clone();
        let saved_static_bindings = self.static_bindings.clone();
        let mut exit_value = None;
        let mut any_arm_joins_exit = false;
        let exit_label = if produce_value { "match_exit" } else { "case_exit" };
        let no_match_label = if produce_value { "match_no_match" } else { "case_no_match" };
        let mut chain_arms = Vec::with_capacity(arms.len());
        let mut arm_exits: Vec<BranchArmExit> = Vec::with_capacity(arms.len());

        for (index, arm) in arms.iter().enumerate() {
            let arm_block = arm_blocks[index];
            let next_target = arm_blocks.get(index + 1).copied().unwrap_or(no_match_block);
            let arm_label = if produce_value { format!("match_arm_{index}") } else { format!("case_arm_{index}") };
            let needs_pattern_check = !matches!(
                arm.pattern,
                crate::compiler::hir::HirPattern::Wildcard
                    | crate::compiler::hir::HirPattern::Variable(_)
                    | crate::compiler::hir::HirPattern::Else
            );
            let mut check_block = None;
            let mut guard_block = None;
            let mut extractor_payload = None;

            self.current_block = arm_block;
            self.current_label = arm_label.clone();
            self.instructions.clear();
            self.terminator = None;
            self.bindings = saved_bindings.clone();
            self.static_bindings = saved_static_bindings.clone();

            if needs_pattern_check {
                let body_or_guard_block = self.new_block(&format!("{arm_label}_check"));
                check_block = Some(body_or_guard_block);
                self.current_block = arm_block;
                self.current_label = arm_label.clone();
                self.instructions.clear();
                self.terminator = None;
                self.bindings = saved_bindings.clone();
                self.static_bindings = saved_static_bindings.clone();
                let (matched, payload) = self.lower_pattern_match_probe(&arm.pattern, scrutinee_operand.clone());
                self.terminate(MirTerminator::Branch { condition: matched, then_target: body_or_guard_block, else_target: next_target });
                self.flush_block(&arm_label);
                self.current_block = body_or_guard_block;
                self.current_label = format!("{arm_label}_check");
                self.instructions.clear();
                self.terminator = None;
                self.bindings = saved_bindings.clone();
                self.static_bindings = saved_static_bindings.clone();
                // sum 载荷延后在判别成功的后继块物化：check 块只承担 tag 判定，
                // nullary 变体的 null 载荷槽不会在未匹配路径被读（wasm `ref.cast` trap）。
                extractor_payload = payload.map(|payload| self.materialize_probe_payload(payload));
            }

            if let Some(guard) = &arm.guard {
                let current_guard_block = self.current_block;
                guard_block = Some(current_guard_block);
                let guard_label = self.current_label.clone();
                let body_block = self.new_block(&format!("{arm_label}_body"));
                self.current_block = current_guard_block;
                self.current_label = guard_label;
                self.instructions.clear();
                self.terminator = None;
                self.bindings = saved_bindings.clone();
                self.static_bindings = saved_static_bindings.clone();
                self.bind_pattern_from_operand_with_payload(&arm.pattern, scrutinee_operand.clone(), None, extractor_payload.clone());
                let guard_value = self.lower_expr_to_operand(guard);
                self.terminate(MirTerminator::Branch { condition: guard_value, then_target: body_block, else_target: next_target });
                let flushed_guard_label = self.current_label.clone();
                self.flush_block(&flushed_guard_label);
                self.current_block = body_block;
                self.current_label = format!("{arm_label}_body");
                self.instructions.clear();
                self.terminator = None;
            }

            let body_block = self.current_block;
            self.bind_pattern_from_operand_with_payload(&arm.pattern, scrutinee_operand.clone(), None, extractor_payload.clone());
            self.control_flow.push_fallthrough(MirFallthroughContext { target: next_target });
            let arm_result = self.lower_expr_to_operand_with_hint(&arm.body, expected_type);
            let _ = self.control_flow.pop_fallthrough();
            if self.terminator.is_none() {
                if produce_value {
                    let ty = infer_builder_operand_type(&arm_result, &self.value_types).or_else(|| expected_type.cloned());
                    if arm_contributes_to_match_merge(&arm_result, ty.as_ref(), expected_type) {
                        if let (Some(merge_ty), MirOperand::Value(value)) = (ty.as_ref(), &arm_result) {
                            self.value_types.entry(*value).or_insert(merge_ty.clone());
                        }
                        let exit_parameter = self.ensure_match_exit_parameter(exit_block, &mut exit_value, ty);
                        debug_assert_eq!(self.blocks[exit_block.0 as usize].parameters.first().copied(), Some(exit_parameter));
                        self.terminate(MirTerminator::Jump { target: exit_block, arguments: vec![arm_result] });
                        any_arm_joins_exit = true;
                    }
                    else if ty.as_ref().is_some_and(is_never_type) || expected_type.is_some_and(|expected| !matches!(expected, ValkyrieType::Unit)) {
                        // `panic` / `raise` arms resume on `Never`, or the arm produced a `Unit`
                        // placeholder for a non-`Unit` signature: they never reach the join.
                        self.terminate(MirTerminator::Unreachable);
                    }
                    else {
                        // `Unit` 臂正常完成但不携带值：续接到 exit，不能当成分散臂。
                        // 否则 `match` 之后的正常控制流会被错误地标成不可达。
                        self.terminate(MirTerminator::Jump { target: exit_block, arguments: Vec::new() });
                        any_arm_joins_exit = true;
                    }
                }
                else {
                    self.terminate(MirTerminator::Jump { target: exit_block, arguments: Vec::new() });
                }
            }
            // 臂内对既有绑定的写回必须合流：语句位置直接合到 exit 块，值位置由
            // `materialize_match_binding_merge` 合到 exit 之前的独立 phi 块。
            arm_exits.push((self.current_block, self.bindings.clone()));
            let current_arm_label = self.current_label.clone();
            self.flush_block(&current_arm_label);
            chain_arms.push(MirCaseArm {
                entry_block: arm_block,
                check_block,
                guard_block,
                body_block,
                next_arm_target: next_target,
                exit_target: exit_block,
                fallthrough_target: arm_blocks.get(index + 1).copied(),
            });
        }

        if produce_value {
            // 值位置的 `match` exit 只承载结果槽；臂内绑定写回在 exit 之前的独立
            // phi 块合流，使合流后的绑定值支配后续读取。
            self.materialize_match_binding_merge(exit_block, &saved_bindings, &arm_exits, "match_bindings");
        }
        else {
            self.materialize_branch_bindings(exit_block, &saved_bindings, &arm_exits);
        }

        self.current_block = no_match_block;
        self.current_label = no_match_label.to_string();
        self.instructions.clear();
        self.terminator = None;
        if produce_value {
            // The fallthrough edge never carries a merge value: either previous arms
            // already fixed the merge slot type (`bool`, `Option<T>`, …) or every arm
            // diverged and there is no value to merge at all. Feeding `Unit` into the
            // exit phi would pollute the merge type (SMIR007).
            self.terminate(MirTerminator::Unreachable);
        }
        else {
            let arguments = self.exit_binding_arguments(exit_block, &saved_bindings);
            self.terminate(MirTerminator::Jump { target: exit_block, arguments });
        }
        self.flush_block(no_match_label);

        let all_arms_diverge = produce_value && !any_arm_joins_exit;
        self.current_block = exit_block;
        self.current_label = exit_label.to_string();
        self.instructions.clear();
        // 所有臂都发散（`return` / `panic` / `raise`）时，`match` 没有值可合流，
        // 合流点也不可达。把发散状态留在当前块上，调用方（函数尾、`return match …`）
        // 才不会再拿 `Unit` 占位符伪造一个返回值。这与 `if` 的 then/else 均发散时
        // 在 merge 块留下 `Unreachable` 是同一模型，不按臂数或“是否 return”特判。
        self.terminator = all_arms_diverge.then_some(MirTerminator::Unreachable);
        self.case_chains.push(MirCaseChain {
            dispatch_block: entry_block,
            first_arm: arm_blocks[0],
            no_match_block,
            exit_block,
            produce_value,
            arms: chain_arms,
        });
        if produce_value {
            exit_value
                .map(|value| {
                    if let Some(expected) = expected_type {
                        self.value_types.entry(value).or_insert(expected.clone());
                    }
                    MirOperand::Value(value)
                })
                .unwrap_or(MirOperand::Constant(MirConstant::Unit))
        }
        else {
            MirOperand::Constant(MirConstant::Unit)
        }
    }

    pub(super) fn lower_fallthrough_expr(&mut self) -> MirOperand {
        if let Some(context) = self.control_flow.current_fallthrough().cloned() {
            self.terminate(MirTerminator::Jump { target: context.target, arguments: Vec::new() });
            let label = self.current_label.clone();
            self.flush_block(&label);
            self.new_block("after_fallthrough");
            self.instructions.clear();
            self.terminator = Some(MirTerminator::Unreachable);
        }
        else {
            self.terminate(MirTerminator::Unreachable);
            let label = self.current_label.clone();
            self.flush_block(&label);
            self.new_block("after_invalid_fallthrough");
            self.instructions.clear();
            self.terminator = Some(MirTerminator::Unreachable);
        }
        MirOperand::Constant(MirConstant::Unit)
    }

    fn ensure_match_exit_parameter(
        &mut self,
        exit_block: MirBlockRef,
        exit_value: &mut Option<super::MirValueRef>,
        ty: Option<crate::compiler::hir::ValkyrieType>,
    ) -> super::MirValueRef {
        if let Some(value) = *exit_value {
            if let Some(ty) = ty {
                self.value_types.entry(value).or_insert(ty);
            }
            return value;
        }
        let value = self.ensure_block_parameter(exit_block, "match_result", ty);
        *exit_value = Some(value);
        value
    }
}

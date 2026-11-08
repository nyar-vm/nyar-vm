impl MirBuilder {
    pub(crate) fn lower_object_pattern_match_operand(
        &mut self,
        name: &Option<NamePath>,
        type_declaration: Option<nyar_types::ItemId>,
        fields: &[(Identifier, HirPattern)],
        field_ids: &[Option<nyar_types::FieldId>],
        named_fields: bool,
        value: MirOperand,
    ) -> MirOperand {
        let fallback = || HirPattern::Object {
            name: name.clone(),
            type_declaration,
            fields: fields.to_vec(),
            field_ids: field_ids.to_vec(),
            named_fields,
            rest: None,
        };
        let Some(actual_type) = infer_builder_operand_type(&value, &self.value_types)
        else {
            return self.lower_fallback_pattern_match(fallback(), value);
        };

        if name.is_some() {
            let Some(declaration) = type_declaration
            else {
                return self.lower_fallback_pattern_match(fallback(), value);
            };
            if !self.type_pattern_matches(&actual_type, declaration) {
                return MirOperand::Constant(MirConstant::Bool(false));
            }
        }

        if fields.is_empty() {
            return MirOperand::Constant(MirConstant::Bool(true));
        }

        if !self.has_known_struct_layout(&actual_type) {
            return self.lower_fallback_pattern_match(fallback(), value);
        }

        let mut accumulated: Option<MirOperand> = None;
        for (index, (field_name, field_pattern)) in fields.iter().enumerate() {
            // 字段身份已由 HIR 冻结；缺失即结构化失败，不按字段名回查布局。
            let Some(field_id) = field_ids.get(index).copied().flatten()
            else {
                return MirOperand::Constant(MirConstant::Bool(false));
            };
            let field_operand = self.lower_object_field_get_operand(value.clone(), field_id, field_name);
            let field_match = self.lower_pattern_match_operand(field_pattern, field_operand);
            accumulated = Some(self.merge_pattern_match_operands(accumulated, field_match));
        }

        accumulated.unwrap_or(MirOperand::Constant(MirConstant::Bool(true)))
    }

    pub(crate) fn lower_or_pattern_match_operand(&mut self, patterns: &[HirPattern], value: MirOperand) -> MirOperand {
        let mut accumulated: Option<MirOperand> = None;
        for pattern in patterns {
            let candidate = self.lower_pattern_match_operand(pattern, value.clone());
            accumulated = Some(self.merge_pattern_match_any_operands(accumulated, candidate));
        }
        accumulated.unwrap_or(MirOperand::Constant(MirConstant::Bool(false)))
    }

    pub(crate) fn has_known_struct_layout(&self, actual_type: &ValkyrieType) -> bool {
        match actual_type {
            ValkyrieType::Nominal { declaration, .. } => {
                crate::compiler::mir::ssa::MirStruct::find_by_declaration(&self.field_declarations, *declaration).is_some()
            }
            _ => false,
        }
    }

    fn lower_extractor_nested_pattern_match_operand(
        &mut self,
        extractor: &HirExtractorPattern,
        payload: MirOperand,
        matched: MirOperand,
        resolved: &HirResolvedCall,
    ) -> MirOperand {
        let mut accumulated = Some(matched);
        match extractor {
            HirExtractorPattern::Constructor { fields, .. } => {
                for (slot, field_pattern) in fields.iter().enumerate() {
                    let slot_type = self.infer_extractor_payload_field_type(&payload, slot).or_else(|| self.infer_extractor_slot_type(resolved, slot));
                    let extracted = self.lower_extractor_payload_field_operand(payload.clone(), slot, slot_type);
                    let item_match = self.lower_pattern_match_operand(field_pattern, extracted);
                    accumulated = Some(self.merge_pattern_match_operands(accumulated, item_match));
                }
            }
            HirExtractorPattern::Array { prefix, suffix, .. } => {
                for (slot, field_pattern) in prefix.iter().enumerate() {
                    let extracted =
                        self.lower_extractor_payload_slot_operand(payload.clone(), slot, self.infer_extractor_slot_type(resolved, slot));
                    let item_match = self.lower_pattern_match_operand(field_pattern, extracted);
                    accumulated = Some(self.merge_pattern_match_operands(accumulated, item_match));
                }
                let suffix_base = prefix.len()
                    + match extractor {
                        HirExtractorPattern::Array { rest, .. } if rest.is_some() => 1,
                        _ => 0,
                    };
                for (index, field_pattern) in suffix.iter().enumerate() {
                    let slot = suffix_base + index;
                    let extracted =
                        self.lower_extractor_payload_slot_operand(payload.clone(), slot, self.infer_extractor_slot_type(resolved, slot));
                    let item_match = self.lower_pattern_match_operand(field_pattern, extracted);
                    accumulated = Some(self.merge_pattern_match_operands(accumulated, item_match));
                }
            }
        }
        accumulated.unwrap_or(MirOperand::Constant(MirConstant::Bool(true)))
    }

    /// Layout id for unite `tag`/`payload` FieldGet: 只认 Nominal declaration，禁止按 sum 短名登记。
    fn layout_id_for_sum_field_access(&mut self, actual_type: &ValkyrieType, sum_name: &str) -> Option<LayoutId> {
        let declaration = actual_type.nominal_declaration()?;
        let ensured = ensure_unite_tagged_layout(&mut self.aggregate_layouts, declaration, sum_name);
        layout_id_for_type(actual_type, &self.aggregate_layouts)
            .or(Some(ensured))
            .and_then(|id| {
                let layout = self.aggregate_layouts.layouts.iter().find(|layout| layout.id == id)?;
                if layout.fields.iter().any(|field| field.name == "tag") || layout.fields.iter().any(|field| field.name == "payload") {
                    Some(id)
                }
                else {
                    None
                }
            })
            .or(Some(ensured))
    }

    pub(crate) fn lower_extractor_call_operand(&mut self, resolved: &HirResolvedCall, value: MirOperand) -> MirOperand {
        use crate::compiler::types::hir::HirCallableDomain;
        if resolved.domain == HirCallableDomain::Extractor {
            if let Some(payload) = self.try_lower_result_option_extractor(resolved, value.clone()) {
                return payload;
            }
            self.diagnostics.push(MirDiagnostic::UnresolvedFieldIdentity { field: "sum extractor payload".to_string() });
            return MirOperand::Constant(MirConstant::Unit);
        }
        let Some(callee) = lower_resolved_callee(resolved)
        else {
            self.diagnostics.push(MirDiagnostic::UnresolvedCallableIdentity { symbol: resolved.symbol.to_string() });
            return MirOperand::Constant(MirConstant::Unit);
        };
        let output = self.next_value(MirValueOrigin::Temporary);
        self.push_instruction(MirOperation::Call { callee, arguments: vec![value.clone()] }, vec![output]);
        self.value_types.insert(output, resolved.return_type.clone());
        MirOperand::Value(output)
    }

    fn lower_nullable_some_operand(&mut self, nullable: MirOperand) -> MirOperand {
        let is_null_operand = self.emit_null_is_null_operand(nullable);
        self.lower_logical_not_operand(is_null_operand)
    }

    fn lower_extractor_matched_operand(&mut self, nullable: MirOperand) -> MirOperand {
        self.lower_nullable_some_operand(nullable)
    }

    /// 从已登记变体记录 payload 按声明槽位取字段。
    ///
    /// 返回的是变体自身冻结的字段身份与代入后的字段类型；记录身份来自
    /// `variant.declaration`，同一枚举的不同变体各自独立。
    fn variant_record_slot(&self, payload_ty: &ValkyrieType, slot: usize) -> Option<(nyar_types::FieldId, ValkyrieType)> {
        let ValkyrieType::Nominal { declaration, arguments, .. } = payload_ty
        else {
            return None;
        };
        let (sum, variant) = crate::compiler::mir::MirSumDeclaration::find_record_variant(&self.sum_types, *declaration)?;
        let field = variant.fields.get(slot)?;
        let field_types = sum.instantiate_field_types(variant, arguments)?;
        Some((field.id, field_types.get(slot)?.clone()))
    }

    pub(crate) fn variant_record_field_by_name(
        &self,
        payload_ty: &ValkyrieType,
        name: &nyar_types::Identifier,
    ) -> Option<(nyar_types::FieldId, ValkyrieType)> {
        let ValkyrieType::Nominal { declaration, arguments, .. } = payload_ty
        else {
            return None;
        };
        let (sum, variant) = crate::compiler::mir::MirSumDeclaration::find_record_variant(&self.sum_types, *declaration)?;
        let target = name.to_string();
        let slot = variant.fields.iter().position(|field| field.name == target)?;
        let field_types = sum.instantiate_field_types(variant, arguments)?;
        Some((variant.fields[slot].id, field_types.get(slot)?.clone()))
    }

    /// 按变体记录字段身份取 payload 槽位；没有记录身份时只允许单字段 payload 本身。
    fn lower_extractor_payload_field_operand(
        &mut self,
        payload: MirOperand,
        slot: usize,
        expected_type: Option<ValkyrieType>,
    ) -> MirOperand {
        let payload_type = infer_builder_operand_type(&payload, &self.value_types);
        if let Some((field, field_type)) = payload_type.as_ref().and_then(|payload_ty| self.variant_record_slot(payload_ty, slot)) {
            let output = self.next_value(MirValueOrigin::Temporary);
            self.push_instruction(MirOperation::FieldGet { object: payload, field }, vec![output]);
            self.value_types.insert(output, field_type);
            return MirOperand::Value(output);
        }
        self.lower_extractor_payload_single_slot_operand(payload, slot, expected_type)
    }

    fn lower_extractor_payload_single_slot_operand(
        &mut self,
        payload: MirOperand,
        slot: usize,
        expected_type: Option<ValkyrieType>,
    ) -> MirOperand {
        if slot == 0 {
            // Single-field extractor payload: the value is the payload itself — no synthetic projection.
            // Do not clobber a concrete SSA type (e.g. VonDiagnostic from Result Apply) with the
            // generic unite parameter (`Named("E")`) from HIR extractor_payload_type.
            if let (Some(expected_type), MirOperand::Value(value)) = (expected_type, &payload) {
                self.value_types.entry(*value).or_insert(expected_type);
            }
            return payload;
        }
        // 没有变体记录身份的后续槽位一律失败关闭；不再制造 tuple projection。
        let extracted = self.reject_unmodeled_static_operation("extractor payload slot", MirValueOrigin::Temporary);
        if let Some(expected_type) = expected_type {
            self.value_types.insert(extracted, expected_type);
        }
        MirOperand::Value(extracted)
    }

    fn lower_extractor_payload_slot_operand(&mut self, payload: MirOperand, slot: usize, expected_type: Option<ValkyrieType>) -> MirOperand {
        self.lower_extractor_payload_single_slot_operand(payload, slot, expected_type)
    }

    fn infer_extractor_payload_field_type(&self, payload: &MirOperand, slot: usize) -> Option<ValkyrieType> {
        let payload_ty = infer_builder_operand_type(payload, &self.value_types)?;
        self.variant_record_slot(&payload_ty, slot).map(|(_, field_type)| field_type)
    }

    fn infer_extractor_slot_type(&self, resolved: &HirResolvedCall, slot: usize) -> Option<ValkyrieType> {
        let payload_type = resolved.extractor_payload_type.clone().or_else(|| nullable_payload_type(&resolved.return_type))?;
        if slot == 0 { Some(payload_type) } else { None }
    }

    /// 表达式字段读取：身份由 HIR 冻结，MIR 只按 `FieldId` 发出 `FieldGet`。
    pub(crate) fn lower_object_field_get_operand(
        &mut self,
        object: MirOperand,
        field: nyar_types::FieldId,
        field_name: &Identifier,
    ) -> MirOperand {
        let field_type = self.field_type_for_object_operand(&object, field_name);
        let output = self.next_value(MirValueOrigin::Temporary);
        self.push_instruction(MirOperation::FieldGet { object, field }, vec![output]);
        if let Some(field_type) = field_type {
            self.value_types.insert(output, field_type);
        }
        MirOperand::Value(output)
    }

    pub(crate) fn lower_eq_constant_operand(&mut self, lhs: MirOperand, rhs: MirConstant, ty: &ValkyrieType) -> MirOperand {
        let rhs_operand = self.materialize_pattern_constant_operand(rhs, ty);
        self.lower_binary_operator_operand("infix ==", lhs, rhs_operand, ty)
    }

    pub(crate) fn lower_lt_constant_operand(&mut self, lhs: MirOperand, rhs: MirConstant, ty: &ValkyrieType) -> MirOperand {
        let rhs_operand = self.materialize_pattern_constant_operand(rhs, ty);
        self.lower_lt_operands(lhs, rhs_operand, ty)
    }

    pub(crate) fn lower_lt_operands(&mut self, lhs: MirOperand, rhs: MirOperand, ty: &ValkyrieType) -> MirOperand {
        self.lower_binary_operator_operand("infix <", lhs, rhs, ty)
    }

    fn materialize_pattern_constant_operand(&mut self, constant: MirConstant, ty: &ValkyrieType) -> MirOperand {
        if matches!(constant, MirConstant::Unit) && matches!(ty, ValkyrieType::Unit) {
            return MirOperand::Constant(constant);
        }
        let value = self.next_value(MirValueOrigin::Literal);
        self.push_instruction(MirOperation::LoadConstant { constant, ty: Some(ty.clone()) }, vec![value]);
        self.value_types.insert(value, ty.clone());
        MirOperand::Value(value)
    }

    fn lower_binary_operator_operand(&mut self, member: &str, lhs: MirOperand, rhs: MirOperand, subject: &ValkyrieType) -> MirOperand {
        let member_id = Identifier::new(member);
        let Some(instance) = crate::compiler::mir::ssa::generic_call_instantiation::lookup_member_dispatch(&self.member_dispatch, subject, &member_id, None)
        else {
            return self.reject_unbound_member_call(member);
        };
        self.push_call_returning(MirOperand::Callable(instance), vec![lhs, rhs], ValkyrieType::Boolean)
    }

    /// 模式比较所需的成员 callable 未绑定：按普通 callable 身份缺失失败关闭。
    ///
    /// 比较只是接收者的普通成员调用，不存在运算符专用身份或专用诊断。
    fn reject_unbound_member_call(&mut self, member: &str) -> MirOperand {
        self.diagnostics.push(MirDiagnostic::UnresolvedCallableIdentity { symbol: member.to_string() });
        MirOperand::Constant(MirConstant::Unit)
    }

    /// 把作为控制流操作数的布尔常量落到 SSA 值上。
    ///
    /// 规范化后端要求 `Branch` 条件与 `Jump` 实参都是 SSA 值（CAN010 拒绝常量），
    /// 而模式比较的短路 helpers 会直接产生 `true`/`false` 终结符操作数，
    /// 因此统一经 `LoadConstant` 物化，不在终结符里携带常量。
    fn materialize_control_operand(&mut self, operand: MirOperand) -> MirOperand {
        match operand {
            MirOperand::Constant(MirConstant::Bool(value)) => {
                self.materialize_pattern_constant_operand(MirConstant::Bool(value), &ValkyrieType::Boolean)
            }
            other => other,
        }
    }

    /// 物化一个布尔常量并返回其 SSA 值操作数。
    fn materialize_bool_operand(&mut self, value: bool) -> MirOperand {
        self.materialize_pattern_constant_operand(MirConstant::Bool(value), &ValkyrieType::Boolean)
    }

    /// Short-circuit `!value`: `if value { false } else { true }`.
    pub(crate) fn lower_logical_not_operand(&mut self, value: MirOperand) -> MirOperand {
        let pre_bindings = self.bindings.clone();
        let cond_block = self.current_block;
        let then_block = self.new_block("not_then");
        let else_block = self.new_block("not_else");
        let merge_block = self.new_block("not_merge");
        let mut exit_value = None;

        self.current_block = cond_block;
        let condition = self.materialize_control_operand(value);
        self.terminate(MirTerminator::Branch { condition, then_target: then_block, else_target: else_block });
        self.flush_block("not_cond");

        self.current_block = then_block;
        self.bindings = pre_bindings.clone();
        self.ensure_branch_exit_parameter(merge_block, &mut exit_value, Some(ValkyrieType::Boolean));
        let taken = self.materialize_bool_operand(false);
        self.terminate(MirTerminator::Jump { target: merge_block, arguments: vec![taken] });
        self.flush_block("not_then");

        self.current_block = else_block;
        self.bindings = pre_bindings.clone();
        self.ensure_branch_exit_parameter(merge_block, &mut exit_value, Some(ValkyrieType::Boolean));
        let taken = self.materialize_bool_operand(true);
        self.terminate(MirTerminator::Jump { target: merge_block, arguments: vec![taken] });
        self.flush_block("not_else");

        self.current_block = merge_block;
        self.bindings = pre_bindings;
        exit_value.map(MirOperand::Value).unwrap_or(MirOperand::Constant(MirConstant::Bool(false)))
    }

    /// Short-circuit `lhs && rhs`: `if lhs { rhs } else { false }`.
    pub(crate) fn lower_logical_and_operand(&mut self, lhs: MirOperand, rhs: MirOperand) -> MirOperand {
        let pre_bindings = self.bindings.clone();
        let cond_block = self.current_block;
        let then_block = self.new_block("and_then");
        let else_block = self.new_block("and_else");
        let merge_block = self.new_block("and_merge");
        let mut exit_value = None;

        self.current_block = cond_block;
        let condition = self.materialize_control_operand(lhs);
        self.terminate(MirTerminator::Branch { condition, then_target: then_block, else_target: else_block });
        self.flush_block("and_cond");

        self.current_block = then_block;
        self.bindings = pre_bindings.clone();
        self.ensure_branch_exit_parameter(merge_block, &mut exit_value, Some(ValkyrieType::Boolean));
        let taken = self.materialize_control_operand(rhs);
        self.terminate(MirTerminator::Jump { target: merge_block, arguments: vec![taken] });
        self.flush_block("and_then");

        self.current_block = else_block;
        self.bindings = pre_bindings.clone();
        self.ensure_branch_exit_parameter(merge_block, &mut exit_value, Some(ValkyrieType::Boolean));
        let taken = self.materialize_bool_operand(false);
        self.terminate(MirTerminator::Jump { target: merge_block, arguments: vec![taken] });
        self.flush_block("and_else");

        self.current_block = merge_block;
        self.bindings = pre_bindings;
        exit_value.map(MirOperand::Value).unwrap_or(MirOperand::Constant(MirConstant::Bool(false)))
    }

    /// Short-circuit `lhs || rhs`: `if lhs { true } else { rhs }`.
    pub(crate) fn lower_logical_or_operand(&mut self, lhs: MirOperand, rhs: MirOperand) -> MirOperand {
        let pre_bindings = self.bindings.clone();
        let cond_block = self.current_block;
        let then_block = self.new_block("or_then");
        let else_block = self.new_block("or_else");
        let merge_block = self.new_block("or_merge");
        let mut exit_value = None;

        self.current_block = cond_block;
        let condition = self.materialize_control_operand(lhs);
        self.terminate(MirTerminator::Branch { condition, then_target: then_block, else_target: else_block });
        self.flush_block("or_cond");

        self.current_block = then_block;
        self.bindings = pre_bindings.clone();
        self.ensure_branch_exit_parameter(merge_block, &mut exit_value, Some(ValkyrieType::Boolean));
        let taken = self.materialize_bool_operand(true);
        self.terminate(MirTerminator::Jump { target: merge_block, arguments: vec![taken] });
        self.flush_block("or_then");

        self.current_block = else_block;
        self.bindings = pre_bindings.clone();
        self.ensure_branch_exit_parameter(merge_block, &mut exit_value, Some(ValkyrieType::Boolean));
        let taken = self.materialize_control_operand(rhs);
        self.terminate(MirTerminator::Jump { target: merge_block, arguments: vec![taken] });
        self.flush_block("or_else");

        self.current_block = merge_block;
        self.bindings = pre_bindings;
        exit_value.map(MirOperand::Value).unwrap_or(MirOperand::Constant(MirConstant::Bool(false)))
    }

    pub(crate) fn merge_pattern_match_operands(&mut self, lhs: Option<MirOperand>, rhs: MirOperand) -> MirOperand {
        match (lhs, rhs) {
            (None, rhs) => rhs,
            (Some(MirOperand::Constant(MirConstant::Bool(false))), _) => MirOperand::Constant(MirConstant::Bool(false)),
            (Some(_), MirOperand::Constant(MirConstant::Bool(false))) => MirOperand::Constant(MirConstant::Bool(false)),
            (Some(MirOperand::Constant(MirConstant::Bool(true))), rhs) => rhs,
            (Some(lhs), MirOperand::Constant(MirConstant::Bool(true))) => lhs,
            (Some(lhs), rhs) => self.lower_logical_and_operand(lhs, rhs),
        }
    }

    pub(crate) fn merge_pattern_match_any_operands(&mut self, lhs: Option<MirOperand>, rhs: MirOperand) -> MirOperand {
        match (lhs, rhs) {
            (None, rhs) => rhs,
            (Some(MirOperand::Constant(MirConstant::Bool(true))), _) => MirOperand::Constant(MirConstant::Bool(true)),
            (Some(_), MirOperand::Constant(MirConstant::Bool(true))) => MirOperand::Constant(MirConstant::Bool(true)),
            (Some(MirOperand::Constant(MirConstant::Bool(false))), rhs) => rhs,
            (Some(lhs), MirOperand::Constant(MirConstant::Bool(false))) => lhs,
            (Some(lhs), rhs) => self.lower_logical_or_operand(lhs, rhs),
        }
    }

    /// 类型模式只按已冻结的声明身份判定；缺失时由调用方返回结构化失败，禁止按拼写重建继承关系。
    pub(crate) fn type_pattern_matches(&self, actual_type: &ValkyrieType, expected: nyar_types::ItemId) -> bool {
        let Some(actual) = actual_type.nominal_declaration()
        else {
            return false;
        };
        self.declaration_inherits_from(actual, expected, &mut BTreeSet::new())
    }

    fn declaration_inherits_from(
        &self,
        actual: nyar_types::ItemId,
        expected: nyar_types::ItemId,
        visiting: &mut BTreeSet<nyar_types::ItemId>,
    ) -> bool {
        if actual == expected {
            return true;
        }
        if !visiting.insert(actual) {
            return false;
        }
        let found = crate::compiler::mir::ssa::MirStruct::find_by_declaration(&self.field_declarations, actual).is_some_and(|structure| {
            structure.parent_declarations.iter().any(|parent| self.declaration_inherits_from(*parent, expected, visiting))
        });
        visiting.remove(&actual);
        found
    }

    /// 把 extractor payload 绑定到 extractor 各字段的模式绑定上。
    pub(crate) fn bind_extractor_pattern_from_payload(&mut self, extractor: &HirExtractorPattern, payload: MirOperand) {
        let resolved = extractor_resolved_call(extractor);
        match extractor {
            HirExtractorPattern::Constructor { fields, .. } => {
                for (slot, field_pattern) in fields.iter().enumerate() {
                    let payload_ty = infer_builder_operand_type(&payload, &self.value_types);
                    let record_slot = payload_ty.as_ref().and_then(|payload_ty| self.variant_record_slot(payload_ty, slot));
                    let mut slot_type = record_slot
                        .as_ref()
                        .map(|(_, field_type)| field_type.clone())
                        .or_else(|| resolved.and_then(|resolved| self.infer_extractor_slot_type(resolved, slot)));
                    if slot_type.as_ref().is_some_and(is_generic_parameter) {
                        slot_type = payload_ty.clone();
                    }
                    else if slot_type.is_none() {
                        slot_type = payload_ty;
                    }
                    let extracted = self.lower_extractor_payload_field_operand(payload.clone(), slot, slot_type.clone());
                    // Preserve extractor payload/slot types on bindings (`Fine(plan)` → `plan`
                    // must stay `LegionSourceClosurePlan`) so later `plan.files.length()` can
                    // recover ArrayLength instead of emitting an untyped Call (SMIR002).
                    self.bind_pattern_from_operand(field_pattern, extracted, slot_type);
                }
            }
            HirExtractorPattern::Array { prefix, rest, suffix, .. } => {
                let item_hint = resolved.and_then(|resolved| {
                    resolved.extractor_payload_type.as_ref().and_then(|ty| match ty {
                        ValkyrieType::Tuple(items) => items.first().cloned(),
                        ValkyrieType::Array(item) => Some(*item.clone()),
                        other => Some(other.clone()),
                    })
                });
                for (slot, field_pattern) in prefix.iter().enumerate() {
                    let extracted = self.lower_extractor_payload_slot_operand(payload.clone(), slot, item_hint.clone());
                    self.bind_pattern_from_operand(field_pattern, extracted, item_hint.clone());
                }
                let mut next_slot = prefix.len();
                if let Some(rest) = rest {
                    let rest_type = item_hint.clone().map(|item| ValkyrieType::Array(Box::new(item)));
                    let extracted = self.lower_extractor_payload_slot_operand(payload.clone(), next_slot, rest_type.clone());
                    self.bind_pattern_from_operand(&HirPattern::Variable(rest.clone()), extracted, rest_type);
                    next_slot += 1;
                }
                for (index, field_pattern) in suffix.iter().enumerate() {
                    let extracted = self.lower_extractor_payload_slot_operand(payload.clone(), next_slot + index, item_hint.clone());
                    self.bind_pattern_from_operand(field_pattern, extracted, item_hint.clone());
                }
            }
        }
    }

    pub(crate) fn bind_object_pattern_from_operand(
        &mut self,
        fields: &[(Identifier, HirPattern)],
        field_ids: &[Option<nyar_types::FieldId>],
        rest: Option<&crate::compiler::types::hir::HirIdentifier>,
        operand: MirOperand,
    ) {
        let Some(actual_type) = infer_builder_operand_type(&operand, &self.value_types)
        else {
            for (_, field_pattern) in fields {
                self.bind_unknown_pattern_bindings(field_pattern);
            }
            if let Some(rest) = rest {
                self.bind_unknown_pattern_bindings(&HirPattern::Variable(rest.clone()));
            }
            return;
        };

        if !self.has_known_struct_layout(&actual_type) {
            for (_, field_pattern) in fields {
                self.bind_unknown_pattern_bindings(field_pattern);
            }
            if let Some(rest) = rest {
                self.bind_unknown_pattern_bindings(&HirPattern::Variable(rest.clone()));
            }
            return;
        }

        for (index, (field_name, field_pattern)) in fields.iter().enumerate() {
            // 字段身份已由 HIR 冻结；缺失即结构化失败，不按字段名回查布局。
            let Some(field_id) = field_ids.get(index).copied().flatten()
            else {
                self.bind_unknown_pattern_bindings(field_pattern);
                continue;
            };

            let extracted = self.lower_object_field_get_operand(operand.clone(), field_id, field_name);
            self.bind_pattern_from_operand(field_pattern, extracted, None);
        }
        if let Some(rest) = rest {
            self.bind_pattern_from_operand(&HirPattern::Variable(rest.clone()), operand, Some(actual_type));
        }
    }

    pub(crate) fn bind_unknown_pattern_bindings(&mut self, pattern: &HirPattern) {
        match pattern {
            HirPattern::Wildcard | HirPattern::Else => {}
            HirPattern::Variable(identifier) => {
                // Unknown pattern bindings have no semantic operand. Do not
                // manufacture a symbol that a backend could mistake for a
                // real declaration; keep the failure structured and fail
                // closed at the shared contract boundary.
                let _ = identifier;
            }
            HirPattern::Tuple(items) => {
                for item in items {
                    self.bind_unknown_pattern_bindings(item);
                }
            }
            HirPattern::Extractor(extractor) => match extractor {
                HirExtractorPattern::Array { prefix, rest, suffix, .. } => {
                    for item in prefix {
                        self.bind_unknown_pattern_bindings(item);
                    }
                    if let Some(rest) = rest {
                        self.bind_unknown_pattern_bindings(&HirPattern::Variable(rest.clone()));
                    }
                    for item in suffix {
                        self.bind_unknown_pattern_bindings(item);
                    }
                }
                HirExtractorPattern::Constructor { fields, .. } => {
                    for field in fields {
                        self.bind_unknown_pattern_bindings(field);
                    }
                }
            },
            HirPattern::Object { fields, rest, .. } => {
                for (_, field_pattern) in fields {
                    self.bind_unknown_pattern_bindings(field_pattern);
                }
                if let Some(rest) = rest {
                    self.bind_unknown_pattern_bindings(&HirPattern::Variable(rest.clone()));
                }
            }
            HirPattern::TypedBind { identifier, .. } => {
                self.bind_unknown_pattern_bindings(&HirPattern::Variable(identifier.clone()));
            }
            HirPattern::Bind { pattern, .. } | HirPattern::Mut(pattern) | HirPattern::Pin { pattern, .. } => {
                self.bind_unknown_pattern_bindings(pattern);
            }
            _ => {}
        }
    }

    /// 无法完成 lowering 的模式匹配回退路径。
    ///
    /// 模式类型或 extractor 未 resolved 时，scrutinee 的 tuple/object 形状无法验证，
    /// 这里记录 `MirDiagnostic::PatternLoweringFailed` 并返回 `Bool(false)`，
    /// 由 emit 阶段决定 trap 还是保留 `PatternMatch` 检查。
    pub(crate) fn lower_fallback_pattern_match(&mut self, pattern: HirPattern, _value: MirOperand) -> MirOperand {
        self.diagnostics
            .push(MirDiagnostic::PatternLoweringFailed {
                pattern, reason: "模式 lowering 缺少 MIR 可验证的 extractor 身份".to_string()
            });
        MirOperand::Constant(MirConstant::Bool(false))
    }

}

impl MirBuilder {
    pub(crate) fn lower_object_pattern_match_operand(
        &mut self,
        name: &Option<NamePath>,
        type_declaration: Option<nyar_types::ItemId>,
        fields: &[(Identifier, HirPattern)],
        field_ids: &[Option<nyar_types::FieldId>],
        value: MirOperand,
    ) -> MirOperand {
        let fallback =
            || HirPattern::Object { name: name.clone(), type_declaration, fields: fields.to_vec(), field_ids: field_ids.to_vec(), rest: None };
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
                    let extracted =
                        self.lower_extractor_payload_slot_operand(payload.clone(), slot, self.infer_extractor_slot_type(resolved, slot));
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

    fn lower_extractor_payload_slot_operand(&mut self, payload: MirOperand, slot: usize, expected_type: Option<ValkyrieType>) -> MirOperand {
        let payload_type = infer_builder_operand_type(&payload, &self.value_types);
        match payload_type.as_ref() {
            Some(ValkyrieType::Tuple(_)) => {
                let extracted = self.reject_unmodeled_static_operation("tuple projection", MirValueOrigin::Temporary);
                if let Some(expected_type) = expected_type {
                    self.value_types.insert(extracted, expected_type);
                }
                MirOperand::Value(extracted)
            }
            _ if slot == 0 => {
                // Single-field extractor payload: the value is the payload itself — no synthetic Call.
                // Do not clobber a concrete SSA type (e.g. VonDiagnostic from Result Apply) with the
                // generic unite parameter (`Named("E")`) from HIR extractor_payload_type.
                if let (Some(expected_type), MirOperand::Value(value)) = (expected_type, &payload) {
                    self.value_types.entry(*value).or_insert(expected_type);
                }
                payload
            }
            _ => {
                let extracted = self.reject_unmodeled_static_operation("tuple projection", MirValueOrigin::Temporary);
                if let Some(expected_type) = expected_type {
                    self.value_types.insert(extracted, expected_type);
                }
                MirOperand::Value(extracted)
            }
        }
    }

    fn infer_extractor_slot_type(&self, resolved: &HirResolvedCall, slot: usize) -> Option<ValkyrieType> {
        let payload_type = resolved.extractor_payload_type.clone().or_else(|| nullable_payload_type(&resolved.return_type))?;
        match payload_type {
            ValkyrieType::Tuple(items) => items.get(slot).cloned(),
            _ if slot == 0 => Some(payload_type.clone()),
            _ => None,
        }
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

    /// Short-circuit `!value`: `if value { false } else { true }`.
    pub(crate) fn lower_logical_not_operand(&mut self, value: MirOperand) -> MirOperand {
        let pre_bindings = self.bindings.clone();
        let cond_block = self.current_block;
        let then_block = self.new_block("not_then");
        let else_block = self.new_block("not_else");
        let merge_block = self.new_block("not_merge");
        let mut exit_value = None;

        self.current_block = cond_block;
        self.terminate(MirTerminator::Branch { condition: value, then_target: then_block, else_target: else_block });
        self.flush_block("not_cond");

        self.current_block = then_block;
        self.bindings = pre_bindings.clone();
        self.ensure_branch_exit_parameter(merge_block, &mut exit_value, Some(ValkyrieType::Boolean));
        self.terminate(MirTerminator::Jump { target: merge_block, arguments: vec![MirOperand::Constant(MirConstant::Bool(false))] });
        self.flush_block("not_then");

        self.current_block = else_block;
        self.bindings = pre_bindings.clone();
        self.ensure_branch_exit_parameter(merge_block, &mut exit_value, Some(ValkyrieType::Boolean));
        self.terminate(MirTerminator::Jump { target: merge_block, arguments: vec![MirOperand::Constant(MirConstant::Bool(true))] });
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
        self.terminate(MirTerminator::Branch { condition: lhs, then_target: then_block, else_target: else_block });
        self.flush_block("and_cond");

        self.current_block = then_block;
        self.bindings = pre_bindings.clone();
        self.ensure_branch_exit_parameter(merge_block, &mut exit_value, Some(ValkyrieType::Boolean));
        self.terminate(MirTerminator::Jump { target: merge_block, arguments: vec![rhs] });
        self.flush_block("and_then");

        self.current_block = else_block;
        self.bindings = pre_bindings.clone();
        self.ensure_branch_exit_parameter(merge_block, &mut exit_value, Some(ValkyrieType::Boolean));
        self.terminate(MirTerminator::Jump { target: merge_block, arguments: vec![MirOperand::Constant(MirConstant::Bool(false))] });
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
        self.terminate(MirTerminator::Branch { condition: lhs, then_target: then_block, else_target: else_block });
        self.flush_block("or_cond");

        self.current_block = then_block;
        self.bindings = pre_bindings.clone();
        self.ensure_branch_exit_parameter(merge_block, &mut exit_value, Some(ValkyrieType::Boolean));
        self.terminate(MirTerminator::Jump { target: merge_block, arguments: vec![MirOperand::Constant(MirConstant::Bool(true))] });
        self.flush_block("or_then");

        self.current_block = else_block;
        self.bindings = pre_bindings.clone();
        self.ensure_branch_exit_parameter(merge_block, &mut exit_value, Some(ValkyrieType::Boolean));
        self.terminate(MirTerminator::Jump { target: merge_block, arguments: vec![rhs] });
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
                    let mut slot_type = resolved.and_then(|resolved| self.infer_extractor_slot_type(resolved, slot));
                    let payload_ty = infer_builder_operand_type(&payload, &self.value_types);
                    if slot_type.as_ref().is_some_and(is_generic_parameter) {
                        slot_type = payload_ty.clone();
                    }
                    else if slot_type.is_none() {
                        slot_type = payload_ty;
                    }
                    let extracted = self.lower_extractor_payload_slot_operand(payload.clone(), slot, slot_type.clone());
                    // Preserve extractor payload/slot types on bindings (`Fine(plan)` → `plan`
                    // must stay `LegionSourceClosurePlan`) so later `plan.files.length()` can
                    // recover ArrayLen instead of emitting an untyped Call (SMIR002).
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

    pub(crate) fn extract_tuple_field_operand(&mut self, operand: &MirOperand, index: usize, tuple_ty: Option<&ValkyrieType>) -> MirValueRef {
        if let (MirOperand::Value(value), Some(tuple_ty)) = (operand, tuple_ty) {
            // Keep the producer's semantic tuple type attached to the
            // operand itself. `tuple_get_N` is a physical projection, but
            // its owner layout must still come from Semantic MIR metadata.
            self.value_types.insert(*value, tuple_ty.clone());
            let _ = ensure_layout_for_type(&mut self.aggregate_layouts, tuple_ty);
        }
        let extracted = self.reject_unmodeled_static_operation("tuple projection", MirValueOrigin::Temporary);
        if let Some(ValkyrieType::Tuple(types)) = tuple_ty {
            if let Some(elem_ty) = types.get(index) {
                self.value_types.insert(extracted, elem_ty.clone());
            }
        }
        extracted
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

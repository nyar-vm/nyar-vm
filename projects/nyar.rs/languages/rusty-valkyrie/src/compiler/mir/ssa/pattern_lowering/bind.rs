impl MirBuilder {
    pub(crate) fn record_static_binding(&mut self, pattern: &HirPattern, initializer: Option<&HirExpr>) {
        match pattern {
            HirPattern::Variable(identifier) => {
                if let Some(expr) = initializer {
                    self.static_bindings.insert(identifier.name.to_string(), expr.clone());
                }
                else {
                    self.static_bindings.remove(identifier.name.as_str());
                }
            }
            HirPattern::Tuple(items) => {
                for item in items {
                    self.record_static_binding(item, None);
                }
            }
            HirPattern::Extractor(extractor) => match extractor {
                HirExtractorPattern::Array { prefix, suffix, .. } => {
                    for item in prefix {
                        self.record_static_binding(item, None);
                    }
                    for item in suffix {
                        self.record_static_binding(item, None);
                    }
                }
                HirExtractorPattern::Constructor { fields, .. } => {
                    for field in fields {
                        self.record_static_binding(field, None);
                    }
                }
            },
            _ => {}
        }
    }

    pub(crate) fn bind_pattern_from_expr(&mut self, pattern: &HirPattern, expr: &HirExpr, ty: Option<ValkyrieType>) {
        let resolved_expr = self.resolve_static_expr(expr).unwrap_or_else(|| expr.clone());
        match pattern {
            HirPattern::Tuple(items) => {
                if let Some(tuple_items) = tuple_literal_exprs(&resolved_expr) {
                    for (item_pattern, item_expr) in items.iter().zip(tuple_items.iter()) {
                        self.bind_pattern_from_expr(item_pattern, item_expr, None);
                    }
                    return;
                }
            }
            HirPattern::Wildcard => {
                let _ = self.lower_expr_to_operand(&resolved_expr);
                return;
            }
            _ => {}
        }

        let operand = self.lower_expr_to_operand_with_hint(expr, ty.as_ref());
        self.bind_pattern_from_operand(pattern, operand, ty);
    }

    pub(crate) fn bind_pattern_from_operand(&mut self, pattern: &HirPattern, operand: MirOperand, ty: Option<ValkyrieType>) {
        self.bind_pattern_from_operand_with_payload(pattern, operand, ty, None);
    }

    pub(crate) fn bind_pattern_from_operand_with_payload(
        &mut self,
        pattern: &HirPattern,
        operand: MirOperand,
        ty: Option<ValkyrieType>,
        extractor_payload: Option<MirOperand>,
    ) {
        match pattern {
            HirPattern::Wildcard => {}
            HirPattern::Else | HirPattern::Literal(_) | HirPattern::Range { .. } | HirPattern::Name(_) | HirPattern::Type { .. } => {}
            HirPattern::Variable(identifier) => {
                let name = identifier.name.to_string();
                let value = self.next_value(MirValueOrigin::LetBinding { name: name.clone() });
                let operand_ty = infer_builder_operand_type(&operand, &self.value_types);
                // Prefer a concrete SSA type on the operand over a generic unite parameter
                // hint such as `Named("E")` from HIR extractor_payload_type.
                let inferred_type = match (ty.clone(), operand_ty.clone()) {
                    (Some(hint), Some(concrete)) if is_generic_parameter(&hint) => Some(concrete),
                    (hint, concrete) => hint.or(concrete),
                };
                if let Some(ref inferred_type) = inferred_type {
                    // 拷贝判据优先取 operand 的已冻结类型：`let b: Point = a` 把注解降成
                    // 表面名 `Named("Point")` 时没有 declaration 身份，不能据此把值类型
                    // 判成引用；操作数上的 `Nominal{Point}` 才是可判事实。操作数仍是开放
                    // binder 时才回退到注解。
                    let mut copy_type = None;
                    if let Some(operand_ty) = operand_ty.as_ref()
                        && value_copy_supported(&mut self.aggregate_layouts, operand_ty)
                    {
                        copy_type = Some(operand_ty.clone());
                    }
                    if copy_type.is_none() && value_copy_supported(&mut self.aggregate_layouts, inferred_type) {
                        copy_type = Some(inferred_type.clone());
                    }
                    if let Some(copy_type) = copy_type {
                        self.push_instruction(MirOperation::AggregateCopy { source: operand, dest: MirOperand::Value(value) }, vec![value]);
                        self.value_types.insert(value, copy_type);
                        self.bindings.insert(name, MirOperand::Value(value));
                        return;
                    }
                }
                // 绑定注解与源值类型比较前先按声明冻结的 primitive 身份归一表示
                // （别名解析到同一载体身份），使同一身份的两种表示自然相等；归一后
                // 仍不同且双方都是整数字面量形参口径，就是真正不同的数值类型，必须
                // 结构化诊断拒绝，绝不改写源值类型规避 `StoreVar` 校验（CAN015）。
                if let (Some(hint), MirOperand::Value(source)) = (&inferred_type, &operand) {
                    if let Some(concrete) = self.value_types.get(source).cloned() {
                        let hint = crate::compiler::mir::ssa::expr_helpers::normalize_primitive_nominal(hint, &self.field_declarations);
                        let concrete = crate::compiler::mir::ssa::expr_helpers::normalize_primitive_nominal(&concrete, &self.field_declarations);
                        if hint != concrete
                            && crate::compiler::mir::ssa::expr_helpers::is_integer_literal_formal_type(&hint, &self.field_declarations)
                            && crate::compiler::mir::ssa::expr_helpers::is_integer_literal_formal_type(&concrete, &self.field_declarations)
                        {
                            self.diagnostics.push(MirDiagnostic::BindingAnnotationTypeMismatch {
                                name: name.clone(),
                                annotation: format!("{hint:?}"),
                                source: format!("{concrete:?}"),
                            });
                        }
                    }
                }
                self.push_instruction(MirOperation::StoreVar { name: name.clone(), value: operand, ty: inferred_type.clone() }, vec![value]);
                if let Some(inferred_type) = inferred_type {
                    self.value_types.insert(value, inferred_type);
                }
                self.bindings.insert(name, MirOperand::Value(value));
            }
            HirPattern::TypedBind { identifier, ty: pattern_type } => {
                let typed_hint = ty.or_else(|| pattern_type.parts().last().cloned().map(ValkyrieType::Named));
                self.bind_pattern_from_operand_with_payload(&HirPattern::Variable(identifier.clone()), operand, typed_hint, None);
            }
            HirPattern::Tuple(items) => {
                // tuple projection 已删除：没有名义记录身份的位置投影不再合成。
                // 逐项保留结构化失败，不得把首个元素伪装成整个 payload。
                for item_pattern in items {
                    self.bind_unknown_pattern_bindings(item_pattern);
                }
            }
            HirPattern::Extractor(extractor) => {
                // Empty-field unite arms match by `tag` only — do not FieldGet `payload`
                // (nullary payload is null; CLR also rejects untagged unite payload gets).
                if matches!(extractor, HirExtractorPattern::Constructor { fields, .. } if fields.is_empty()) {
                    return;
                }
                let payload = match extractor_payload {
                    Some(payload) => payload,
                    None => match extractor {
                        HirExtractorPattern::Constructor { resolved: Some(resolved), .. }
                        | HirExtractorPattern::Array { resolved: Some(resolved), .. } => {
                            self.lower_extractor_call_operand(resolved, operand.clone())
                        }
                        _ => {
                            self.bind_unknown_pattern_bindings(&HirPattern::Extractor(extractor.clone()));
                            return;
                        }
                    },
                };
                self.bind_extractor_pattern_from_payload(extractor, payload);
            }
            HirPattern::Object { fields, field_ids, rest, .. } => {
                // Unite 花括号臂（`Include { value }`）经 probe 传来 payload 时：
                // 变体字段直接绑定到 payload（单字段变体 payload 已是 `value` 的类型），
                // 不可再按 scrutinee（WitDefinition）做 struct FieldGet。
                if let Some(payload) = extractor_payload {
                    let payload_ty = infer_builder_operand_type(&payload, &self.value_types);
                    if fields.len() <= 1 {
                        for (_, field_pattern) in fields {
                            self.bind_pattern_from_operand_with_payload(field_pattern, payload.clone(), payload_ty.clone(), None);
                        }
                    }
                    else {
                        for (field_name, field_pattern) in fields {
                            // 多字段变体 payload 是名义记录：按字段名命中已冻结字段身份，
                            // 不存在字段名就结构化失败，绝不回落到成员/自由函数查找。
                            let Some((field_id, field_type)) =
                                payload_ty.as_ref().and_then(|payload_ty| self.variant_record_field_by_name(payload_ty, field_name))
                            else {
                                self.diagnostics
                                    .push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedFieldIdentity { field: field_name.to_string() });
                                self.bind_unknown_pattern_bindings(field_pattern);
                                continue;
                            };
                            let output = self.next_value(MirValueOrigin::Temporary);
                            self.push_instruction(MirOperation::FieldGet { object: payload.clone(), field: field_id }, vec![output]);
                            self.value_types.insert(output, field_type);
                            self.bind_pattern_from_operand_with_payload(field_pattern, MirOperand::Value(output), None, None);
                        }
                    }
                    if let Some(rest) = rest {
                        self.bind_pattern_from_operand_with_payload(&HirPattern::Variable(rest.clone()), payload, payload_ty, None);
                    }
                }
                else {
                    self.bind_object_pattern_from_operand(fields, field_ids, rest.as_ref(), operand);
                }
            }
            HirPattern::Bind { identifier, pattern } => {
                self.bind_pattern_from_operand_with_payload(pattern, operand.clone(), ty.clone(), extractor_payload.clone());
                self.bind_pattern_from_operand_with_payload(&HirPattern::Variable(identifier.clone()), operand, ty, None);
            }
            HirPattern::Mut(pattern) | HirPattern::Pin { pattern, .. } => {
                self.bind_pattern_from_operand_with_payload(pattern, operand, ty, extractor_payload);
            }
            // OR patterns are handled by `lower_or_pattern_match_operand` for
            // the predicate. They introduce no single payload binding unless
            // represented by an explicit Bind pattern, so the binding phase
            // must not manufacture residual MIR for them.
            HirPattern::Or(_) => {}
            _ => {
                let value = self.next_value(MirValueOrigin::Temporary);
                // Keep the residual instruction well-typed so the shared
                // contract can report SMIR008 instead of masking it as SMIR001.
                self.value_types.insert(value, ValkyrieType::Boolean);
                self.instructions.push(MirInstruction::from_operation(
                    // Preserve an unsupported pattern as structured residual MIR.
                    // The shared Semantic MIR contract rejects it as SMIR008;
                    // do not disguise the residual as a fake symbol.
                    MirOperation::PatternMatch { value: operand.clone(), pattern: pattern.clone() },
                ));
            }
        }
    }

    pub(crate) fn lower_pattern_match_operand(&mut self, pattern: &HirPattern, value: MirOperand) -> MirOperand {
        match pattern {
            HirPattern::Wildcard | HirPattern::Variable(_) | HirPattern::Else => MirOperand::Constant(MirConstant::Bool(true)),
            HirPattern::Literal(literal) => self.lower_literal_pattern_match_operand(literal, value),
            HirPattern::Range { start, end, inclusive_end } => {
                self.lower_range_pattern_match_operand(start.as_ref(), end.as_ref(), *inclusive_end, value)
            }
            HirPattern::Tuple(items) => self.lower_tuple_pattern_match_operand(items, value),
            HirPattern::Extractor(extractor) => self.lower_extractor_pattern_match_operand(extractor, value),
            HirPattern::Name(name) => self.lower_fallback_pattern_match(HirPattern::Name(name.clone()), value),
            HirPattern::Type { path, declaration } => self.lower_type_pattern_match_operand(path, *declaration, value),
            HirPattern::TypedBind { identifier, ty } => {
                self.lower_fallback_pattern_match(HirPattern::TypedBind { identifier: identifier.clone(), ty: ty.clone() }, value)
            }
            HirPattern::Object { name, type_declaration, fields, field_ids, named_fields, .. } => {
                self.lower_object_pattern_match_operand(name, *type_declaration, fields, field_ids, *named_fields, value)
            }
            HirPattern::Or(patterns) => self.lower_or_pattern_match_operand(patterns, value),
            HirPattern::Bind { pattern, .. } => self.lower_pattern_match_operand(pattern, value),
            HirPattern::Mut(pattern) => self.lower_pattern_match_operand(pattern, value),
            HirPattern::Pin { pattern, .. } => self.lower_pattern_match_operand(pattern, value),
        }
    }

    pub(crate) fn lower_pattern_match_probe(&mut self, pattern: &HirPattern, value: MirOperand) -> (MirOperand, Option<ProbePayload>) {
        match pattern {
            HirPattern::Extractor(extractor) => {
                // Tagged unite/enum：按 tag 判别（空字段与带字段均适用）。
                if let Some((condition, payload)) = self.try_lower_sum_constructor_tag_match(extractor, value.clone()) {
                    return (condition, payload);
                }
                let Some(resolved) = extractor_resolved_call(extractor)
                else {
                    return (self.lower_fallback_pattern_match(HirPattern::Extractor(extractor.clone()), value), None);
                };
                let nullable = self.lower_extractor_call_operand(resolved, value.clone());
                let matched = self.lower_nullable_some_operand(nullable.clone());
                let condition = self.lower_extractor_nested_pattern_match_operand(extractor, nullable.clone(), matched, resolved);
                (condition, Some(ProbePayload::Materialized(nullable)))
            }
            _ => (self.lower_pattern_match_operand(pattern, value), None),
        }
    }

    /// 把 probe 返回的载荷在**当前块**物化为 `SumPayloadGet`。
    ///
    /// 调用点必须保证当前块只从「tag 判别已成功」的边可达：nullary 变体的载荷槽
    /// 按合同为 null（见 `HirPattern::Extractor` 空字段早退），在 check 块急切提取
    /// 会让 wasm 的 `ref.cast`/`struct.get` 在 `None` scrutinee 上 trap。
    pub(crate) fn materialize_probe_payload(&mut self, payload: ProbePayload) -> MirOperand {
        match payload {
            ProbePayload::Materialized(operand) => operand,
            ProbePayload::DeferredSum(spec) => {
                let output = self.next_value(MirValueOrigin::Temporary);
                self.push_instruction(
                    MirOperation::SumPayloadGet {
                        nominal: spec.nominal,
                        type_args: spec.type_args,
                        variant: spec.variant,
                        payload_type: spec.payload_type.clone(),
                        object: spec.object,
                    },
                    vec![output],
                );
                self.value_types.insert(output, spec.payload_type);
                MirOperand::Value(output)
            }
        }
    }

    pub(crate) fn lower_type_pattern_match_operand(
        &mut self,
        name: &NamePath,
        declaration: Option<nyar_types::ItemId>,
        value: MirOperand,
    ) -> MirOperand {
        let Some(declaration) = declaration
        else {
            return self.lower_fallback_pattern_match(HirPattern::Type { path: name.clone(), declaration: None }, value);
        };
        let Some(actual_type) = infer_builder_operand_type(&value, &self.value_types)
        else {
            return self.lower_fallback_pattern_match(HirPattern::Type { path: name.clone(), declaration: Some(declaration) }, value);
        };

        MirOperand::Constant(MirConstant::Bool(self.type_pattern_matches(&actual_type, declaration)))
    }

    /// Match a sum scrutinee by `SumVariantIs`. Requires HIR `sum_owner` + `sum_variant` on the
    /// extractor — MIR must not rediscover variant identity from spellings.
    fn try_lower_sum_variant_tag_match(
        &mut self,
        actual_type: &ValkyrieType,
        pattern_name: &NamePath,
        value: MirOperand,
        resolved: Option<&HirResolvedCall>,
    ) -> Option<MirOperand> {
        let pattern_variant = pattern_name.name();
        let variant_name = pattern_variant.as_str();
        let (nominal, variant_id) = self.resolve_sum_variant_identity(actual_type, variant_name, resolved)?;
        let type_args = self.sum_type_args_for_scrutinee(actual_type);
        let condition = self.next_value(MirValueOrigin::Temporary);
        self.push_instruction(MirOperation::SumVariantIs { nominal, type_args, variant: variant_id, object: value }, vec![condition]);
        self.value_types.insert(condition, ValkyrieType::Boolean);
        Some(MirOperand::Value(condition))
    }

    fn sum_type_args_for_scrutinee(&self, actual_type: &ValkyrieType) -> Vec<ValkyrieType> {
        let from_scrutinee = crate::compiler::mir::ssa::expr_lowering::type_args_from_sum_shaped(actual_type);
        if !from_scrutinee.is_empty() {
            return from_scrutinee;
        }
        let Some(scrutinee_owner) = named_type_name(actual_type)
        else {
            return from_scrutinee;
        };
        let Some(impl_owner) = self.impl_owner_type.as_ref()
        else {
            return from_scrutinee;
        };
        let Some(owner_name) = named_type_name(impl_owner)
        else {
            return from_scrutinee;
        };
        if sum_type_name_matches(scrutinee_owner, owner_name) {
            let from_owner = crate::compiler::mir::ssa::expr_lowering::type_args_from_sum_shaped(impl_owner);
            if !from_owner.is_empty() {
                return from_owner;
            }
        }
        if let Some(sum) = self.sum_types.iter().find(|sum| sum_type_name_matches(&sum.name, scrutinee_owner)) {
            if sum.generics.len() == self.impl_type_arguments.len() && !self.impl_type_arguments.is_empty() {
                return self.impl_type_arguments.clone();
            }
            if !sum.generics.is_empty() {
                return sum.generics.iter().map(|generic| ValkyrieType::Named(generic.name.clone())).collect();
            }
        }
        from_scrutinee
    }

    fn resolve_sum_variant_identity(
        &mut self,
        _actual_type: &ValkyrieType,
        _variant_name: &str,
        resolved: Option<&HirResolvedCall>,
    ) -> Option<(nyar_types::NominalInstanceId, nyar_types::VariantId)> {
        let resolved = resolved?;
        let (owner, variant) = resolved.sum_owner.zip(resolved.sum_variant)?;
        self.sum_identity_by_declarations(owner, variant)
    }

    fn resolve_sum_variant_payload_type(
        &self,
        actual_type: &ValkyrieType,
        _variant_name: &str,
        resolved: Option<&HirResolvedCall>,
    ) -> Option<ValkyrieType> {
        let resolved = resolved?;
        let owner = resolved.sum_owner?;
        let variant_declaration = resolved.sum_variant?;
        let sum = crate::compiler::mir::ssa::MirSumDeclaration::find_by_declaration(&self.sum_types, owner)?;
        let variant = sum.variants.iter().find(|item| item.declaration == Some(variant_declaration))?;
        // payload 类型必须与构造侧同口径：变体 declaration × 枚举实参。
        // 非泛型枚举同样走这里，多字段变体因此得到名义记录而不是 HIR tuple。
        let args = crate::compiler::mir::ssa::expr_lowering::type_args_from_sum_shaped(actual_type);
        if let Some(Some(payload_ty)) = sum.instantiate_payload(variant, &args) {
            if !looks_like_unbound_type_parameter(&payload_ty) {
                return Some(payload_ty);
            }
        }
        if let ValkyrieType::Nullable(inner) = actual_type {
            if !looks_like_unbound_type_parameter(inner) {
                return Some((**inner).clone());
            }
        }
        resolved.extractor_payload_type.clone().filter(|ty| !looks_like_unbound_type_parameter(ty))
    }

    pub(crate) fn lower_tuple_pattern_match_operand(&mut self, items: &[HirPattern], value: MirOperand) -> MirOperand {
        // tuple projection 已删除：元组位置匹配没有名义字段身份，一律结构化失败。
        let _ = value;
        self.lower_fallback_pattern_match(HirPattern::Tuple(items.to_vec()), MirOperand::Constant(MirConstant::Unit))
    }

    /// extractor 命中条件：先对空字段 unite 臂做 `tag` 比较；带 payload 的臂仍走 `T?`/`null`。
    pub(crate) fn lower_extractor_pattern_match_operand(&mut self, extractor: &HirExtractorPattern, value: MirOperand) -> MirOperand {
        if let Some((condition, _)) = self.try_lower_sum_constructor_tag_match(extractor, value.clone()) {
            return condition;
        }
        let Some(resolved) = extractor_resolved_call(extractor)
        else {
            return self.lower_fallback_pattern_match(HirPattern::Extractor(extractor.clone()), value);
        };
        let nullable = self.lower_extractor_call_operand(resolved, value.clone());
        let matched = self.lower_nullable_some_operand(nullable.clone());
        self.lower_extractor_nested_pattern_match_operand(extractor, nullable, matched, resolved)
    }

    /// Constructor extractor on a sum scrutinee → `SumVariantIs`；带字段时载荷只登记
    /// 为 `DeferredSumPayload` 计划，由成功后继块物化，不得在判别前读 null 载荷槽。
    fn try_lower_sum_constructor_tag_match(
        &mut self,
        extractor: &HirExtractorPattern,
        value: MirOperand,
    ) -> Option<(MirOperand, Option<ProbePayload>)> {
        let HirExtractorPattern::Constructor { name, fields, resolved, .. } = extractor
        else {
            return None;
        };
        let actual_type = infer_builder_operand_type(&value, &self.value_types)?;
        let resolved_ref = resolved.as_ref();
        let condition = self.try_lower_sum_variant_tag_match(&actual_type, name, value.clone(), resolved_ref)?;
        if fields.is_empty() {
            return Some((condition, None));
        }
        // Prefer sum-layout payload typing (substitutes Result Apply args). HIR-resolved
        // extractor metadata often still carries unbound `T`/`E` and must not win.
        // Never fall through to FieldGet `payload` against a payload-struct layout.
        let payload_spec = if let Some(spec) = self.plan_sum_payload(&actual_type, name, value.clone(), resolved_ref) {
            spec
        }
        else if let Some(resolved) = resolved_ref {
            self.plan_result_option_payload(resolved, value.clone())
                .or_else(|| self.plan_sum_payload(&actual_type, name, value.clone(), Some(resolved)))?
        }
        else {
            return None;
        };
        Some((condition, Some(ProbePayload::DeferredSum(payload_spec))))
    }

    /// 登记 `SumPayloadGet` 所需的 sum 载荷事实（不发射指令）。
    fn plan_sum_payload(
        &mut self,
        actual_type: &ValkyrieType,
        variant_name: &NamePath,
        value: MirOperand,
        resolved: Option<&HirResolvedCall>,
    ) -> Option<DeferredSumPayload> {
        let pattern_variant = variant_name.name();
        let variant = pattern_variant.as_str();
        let (nominal, variant_id) = self.resolve_sum_variant_identity(actual_type, variant, resolved)?;
        let payload_ty = self.resolve_sum_variant_payload_type(actual_type, variant, resolved)?;
        Some(DeferredSumPayload {
            object: value,
            nominal,
            type_args: self.sum_type_args_for_scrutinee(actual_type),
            variant: variant_id,
            payload_type: payload_ty,
        })
    }

    /// `case Fine(x)` / `Fail(e)` / `Some(v)` must use SumPayloadGet — never FieldGet
    /// against a payload structure layout (that caused SMIR010 on LegionSourceClosurePlan).
    fn plan_result_option_payload(&mut self, resolved: &HirResolvedCall, value: MirOperand) -> Option<DeferredSumPayload> {
        let (owner, variant_declaration) = resolved.sum_owner.zip(resolved.sum_variant)?;
        let (nominal, variant_id) = self.sum_identity_by_declarations(owner, variant_declaration)?;
        let object_ty = infer_builder_operand_type(&value, &self.value_types)?;
        // 具体实参代入优先于 HIR 上残留的 Named("T"|"E")；禁止 Fine/Fail/Some 拼写选槽。
        let payload_ty = self.resolve_sum_variant_payload_type(&object_ty, "", Some(resolved))?;
        Some(DeferredSumPayload {
            object: value,
            nominal,
            type_args: crate::compiler::mir::ssa::expr_lowering::type_args_from_sum_shaped(&object_ty),
            variant: variant_id,
            payload_type: payload_ty,
        })
    }

    /// `case Fine(x)` / `Fail(e)` / `Some(v)` must use SumPayloadGet — never FieldGet
    /// against a payload structure layout (that caused SMIR010 on LegionSourceClosurePlan).
    fn try_lower_result_option_extractor(&mut self, resolved: &HirResolvedCall, value: MirOperand) -> Option<MirOperand> {
        let spec = self.plan_result_option_payload(resolved, value)?;
        Some(self.materialize_probe_payload(ProbePayload::DeferredSum(spec)))
    }

    pub(crate) fn lower_literal_pattern_match_operand(&mut self, literal: &HirLiteral, value: MirOperand) -> MirOperand {
        let actual_type_hint = infer_builder_operand_type(&value, &self.value_types);
        let (constant, expected_type) = match lower_literal(literal, actual_type_hint.as_ref()) {
            Ok(pair) => pair,
            Err(_) => return self.lower_fallback_pattern_match(HirPattern::Literal(literal.clone()), value),
        };
        match literal {
            HirLiteral::Bool(expected) => {
                let Some(actual_type) = infer_builder_operand_type(&value, &self.value_types)
                else {
                    return self.lower_fallback_pattern_match(HirPattern::Literal(literal.clone()), value);
                };
                if actual_type != ValkyrieType::Boolean {
                    return MirOperand::Constant(MirConstant::Bool(false));
                }
                if *expected { value } else { self.lower_logical_not_operand(value) }
            }
            HirLiteral::Integer(_) | HirLiteral::Float64(_) => {
                let Some(actual_type) = infer_builder_operand_type(&value, &self.value_types)
                else {
                    return self.lower_fallback_pattern_match(HirPattern::Literal(literal.clone()), value);
                };
                if !crate::compiler::mir::ssa::expr_helpers::is_integer_literal_formal_type(&actual_type, &self.field_declarations) {
                    return self.lower_fallback_pattern_match(HirPattern::Literal(literal.clone()), value);
                }
                if let Some(expected_type) = &expected_type {
                    if actual_type != *expected_type
                        && !crate::compiler::mir::ssa::expr_helpers::is_integer_literal_formal_type(expected_type, &self.field_declarations)
                    {
                        return MirOperand::Constant(MirConstant::Bool(false));
                    }
                }
                self.lower_eq_constant_operand(value, constant, &actual_type)
            }
            // 字符/字节字面量模式与整数字面量同一等值比较路径：值是码位/字节常量，
            // 形参口径由对应 formal 判定承担。
            HirLiteral::Character(_) => {
                let Some(actual_type) = infer_builder_operand_type(&value, &self.value_types)
                else {
                    return self.lower_fallback_pattern_match(HirPattern::Literal(literal.clone()), value);
                };
                if !crate::compiler::mir::ssa::expr_helpers::is_character_literal_formal_type(&actual_type, &self.field_declarations) {
                    return self.lower_fallback_pattern_match(HirPattern::Literal(literal.clone()), value);
                }
                self.lower_eq_constant_operand(value, constant, &actual_type)
            }
            HirLiteral::Byte(_) => {
                let Some(actual_type) = infer_builder_operand_type(&value, &self.value_types)
                else {
                    return self.lower_fallback_pattern_match(HirPattern::Literal(literal.clone()), value);
                };
                if !crate::compiler::mir::ssa::expr_helpers::is_integer_literal_formal_type(&actual_type, &self.field_declarations) {
                    return self.lower_fallback_pattern_match(HirPattern::Literal(literal.clone()), value);
                }
                self.lower_eq_constant_operand(value, constant, &actual_type)
            }
            HirLiteral::Unit => {
                if matches!(infer_builder_operand_type(&value, &self.value_types), Some(ValkyrieType::Unit)) {
                    MirOperand::Constant(MirConstant::Bool(true))
                }
                else {
                    MirOperand::Constant(MirConstant::Bool(false))
                }
            }
            HirLiteral::Null => {
                let Some(actual_type) = infer_builder_operand_type(&value, &self.value_types)
                else {
                    return self.lower_fallback_pattern_match(HirPattern::Literal(literal.clone()), value);
                };
                if !crate::compiler::hir::is_nullable_type(&actual_type) {
                    return MirOperand::Constant(MirConstant::Bool(false));
                }
                // 匹配 `null` 即匹配空引用，语义由 NullIsNull 承载，不比较名义 Option 变体。
                self.emit_null_is_null_operand(value)
            }
            HirLiteral::String(_) => {
                let Some(expected_type) = expected_type
                else {
                    return self.lower_fallback_pattern_match(HirPattern::Literal(literal.clone()), value);
                };
                let Some(actual_type) = infer_builder_operand_type(&value, &self.value_types)
                else {
                    return self.lower_fallback_pattern_match(HirPattern::Literal(literal.clone()), value);
                };
                if actual_type != expected_type {
                    return MirOperand::Constant(MirConstant::Bool(false));
                }

                match (value, constant) {
                    (MirOperand::Constant(MirConstant::Utf8(actual)), MirConstant::Utf8(expected)) => {
                        MirOperand::Constant(MirConstant::Bool(actual == expected))
                    }
                    (value, _) => self.lower_fallback_pattern_match(HirPattern::Literal(literal.clone()), value),
                }
            }
        }
    }

    pub(crate) fn lower_range_pattern_match_operand(
        &mut self,
        start: Option<&HirLiteral>,
        end: Option<&HirLiteral>,
        inclusive_end: bool,
        value: MirOperand,
    ) -> MirOperand {
        let Some(actual_type) = infer_builder_operand_type(&value, &self.value_types)
        else {
            return self.lower_fallback_pattern_match(HirPattern::Range { start: start.cloned(), end: end.cloned(), inclusive_end }, value);
        };

        if !crate::compiler::mir::ssa::expr_helpers::is_integer_range_scrutinee_type(&actual_type, &self.field_declarations) {
            self.diagnostics.push(MirDiagnostic::PatternLoweringFailed {
                pattern: HirPattern::Range { start: start.cloned(), end: end.cloned(), inclusive_end },
                reason: "区间模式只能用于整型 scrutinee；浮点/文本/布尔 scrutinee 需改用字面量模式或显式比较".to_string(),
            });
            return MirOperand::Constant(MirConstant::Bool(false));
        }

        let mut accumulated: Option<MirOperand> = None;
        if let Some(start_literal) = start {
            let Ok((start_constant, _)) = lower_literal(start_literal, Some(&actual_type))
            else {
                // 端点无法编码进 MIR 时不得静默变成「不匹配」：非法程序必须结构化失败。
                self.diagnostics.push(MirDiagnostic::PatternLoweringFailed {
                    pattern: HirPattern::Range { start: start.cloned(), end: end.cloned(), inclusive_end },
                    reason: "区间模式起点无法按 scrutinee 类型编码为常量".to_string(),
                });
                return MirOperand::Constant(MirConstant::Bool(false));
            };
            // value >= start ? !(value < start)
            let lt_start = self.lower_lt_constant_operand(value.clone(), start_constant, &actual_type);
            let lower_bound = self.lower_logical_not_operand(lt_start);
            accumulated = Some(self.merge_pattern_match_operands(accumulated, lower_bound));
        }
        if let Some(end_literal) = end {
            let Ok((end_constant, _)) = lower_literal(end_literal, Some(&actual_type))
            else {
                // 与起点同一门：端点编码失败必须留下诊断，不能被当作「不匹配」。
                self.diagnostics.push(MirDiagnostic::PatternLoweringFailed {
                    pattern: HirPattern::Range { start: start.cloned(), end: end.cloned(), inclusive_end },
                    reason: "区间模式终点无法按 scrutinee 类型编码为常量".to_string(),
                });
                return MirOperand::Constant(MirConstant::Bool(false));
            };
            let upper_bound = if inclusive_end {
                // value <= end ? !(end < value)
                let end_operand = self.materialize_pattern_constant_operand(end_constant, &actual_type);
                let lt_end = self.lower_lt_operands(end_operand, value, &actual_type);
                self.lower_logical_not_operand(lt_end)
            }
            else {
                self.lower_lt_constant_operand(value, end_constant, &actual_type)
            };
            accumulated = Some(self.merge_pattern_match_operands(accumulated, upper_bound));
        }
        accumulated.unwrap_or(MirOperand::Constant(MirConstant::Bool(true)))
    }
}

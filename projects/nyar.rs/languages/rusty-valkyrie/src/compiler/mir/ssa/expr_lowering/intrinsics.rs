use crate::compiler::{
    hir::is_nullable_type,
    types::{
        Identifier, NamePath,
        hir::{FunctionType, HirCallableDomain, HirExpr, HirExprKind, HirLiteral, HirResolvedCall, ValkyrieType},
    },
};
use nyar_types::IntrinsicId;

use crate::compiler::mir::ssa::{
    MirBuilder, MirConstant, MirInstruction, MirOperand, MirOperation, MirStorageKind, MirValueOrigin, callee_name_matches,
    expr_helpers::{is_array_shaped_valkyrie_type, lower_resolved_callee, named_type_name, peel_generic_apply},
    infer_builder_operand_type, lower_callee_operand,
    value_semantics::{ensure_layout_for_type, layout_id_for_type, storage_kind_for_type},
};

use super::{
    apply_payload_to_sum_type_args, concretize_variant_constructor_types, instantiated_payload_for_call, refine_auto_apply_type,
    refine_auto_sum_instantiation_args, type_args_from_sum_shaped, unresolved_variant_labels,
};

impl MirBuilder {
    /// 查询调用点的结构化 intrinsic 身份。
    ///
    /// 只信任 `HirResolvedCall.intrinsic`（由 overload 阶段给语言内建绑定）；
    /// **禁止**按符号路径段猜测 intrinsic 身份——那条旁路会绕过 adaptor/import binding，
    /// 让没有后端实现的 intrinsic 悄悄生成 opcode。
    ///
    /// 即使查到 intrinsic 身份，lowering 也不会走专用 opcode 路径：
    /// intrinsic 调用必须降级为普通 `Call { callee: Callable(instance) }`，
    /// 让 linker adaptor / backend TargetExecutionPlan 链路消费。
    pub(crate) fn callee_intrinsic_id(resolved: Option<&HirResolvedCall>, _callee: &MirOperand) -> Option<IntrinsicId> {
        resolved.and_then(|call| call.intrinsic)
    }

    /// 检查 intrinsic 调用是否已绑定 adaptor（有 ItemId 或 ItemInstanceId）。
    ///
    /// 语言 builtin intrinsic 候选（`language_builtin_candidates`）只挂 intrinsic 身份，
    /// 没有 declaration/instance；源码 `[intrinsic(...)]` 种子会同时挂 declaration。
    /// **未绑定 adaptor 的 intrinsic 不能走专用 opcode 路径**，必须降级到普通 Call。
    pub(crate) fn has_intrinsic_adaptor_binding(resolved: Option<&HirResolvedCall>) -> bool {
        resolved.is_some_and(|call| call.intrinsic.is_some() && (call.instance.is_some() || call.declaration.is_some()))
    }

    pub(crate) fn try_lower_ref_deref_intrinsic(
        &mut self,
        resolved: Option<&HirResolvedCall>,
        callee: &MirOperand,
        arguments: &[MirOperand],
    ) -> Option<MirOperand> {
        if Self::callee_intrinsic_id(resolved, callee) != Some(IntrinsicId::RefDeref) {
            return None;
        }
        // intrinsic 必须绑定 adaptor（有 ItemId 或 ItemInstanceId）才能走专用 opcode；
        // 否则降级到普通 Call 路径，让下游 adaptor 消费。
        if !Self::has_intrinsic_adaptor_binding(resolved) {
            return None;
        }
        arguments.first().cloned()
    }

    /// `T?` ???????? sum ??? `Nominal{ItemId}`?
    /// ??? `Named("Option")` ????? variant ???? `call.sum_owner`?
    pub(crate) fn is_option_type(&self, actual_type: &ValkyrieType) -> bool {
        match actual_type {
            ValkyrieType::Nullable(_) => true,
            ValkyrieType::Nominal { declaration, .. } => {
                crate::compiler::mir::ssa::MirSumDeclaration::find_by_declaration(&self.sum_types, *declaration).is_some()
            }
            _ => false,
        }
    }

    /// Some/Fine/Fail 短名命中但缺少 sum 身份时失败关闭，禁止回落 Invoke。
    pub(crate) fn fail_unresolved_sum_variant_call(&mut self, resolved: Option<&HirResolvedCall>) -> MirOperand {
        let (sum_type, variant) = unresolved_variant_labels(resolved);
        self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedVariantIdentity { sum_type, variant });
        MirOperand::Constant(MirConstant::Unit)
    }

    pub(crate) fn option_payload_type(actual_type: &ValkyrieType) -> Option<ValkyrieType> {
        match actual_type {
            ValkyrieType::Nullable(inner) => Some(*inner.clone()),
            ValkyrieType::Nominal { arguments, .. } => arguments.first().cloned(),
            ValkyrieType::Apply(_, args) => args.first().cloned(),
            _ => None,
        }
    }

    pub(crate) fn option_uses_generic_payload(actual_type: &ValkyrieType) -> bool {
        Self::option_payload_type(actual_type).is_some_and(|payload| matches!(payload, ValkyrieType::Named(name) if name.as_str() == "T"))
    }

    /// ??????sum declaration ItemId ??????????? `Named`??
    pub(crate) fn sum_apply_type(&self, declaration: nyar_types::ItemId, type_args: &[ValkyrieType]) -> Option<ValkyrieType> {
        let _ = crate::compiler::mir::ssa::MirSumDeclaration::find_by_declaration(&self.sum_types, declaration)?;
        Some(ValkyrieType::Nominal { declaration, arguments: type_args.to_vec() })
    }

    /// 仅从 impl 实参或 sum declaration 的 Generic binder 补全 Result 类型参数；禁止发明 `Named(T/E)`。
    pub(crate) fn pad_result_type_args(&self, type_args: Vec<ValkyrieType>) -> Vec<ValkyrieType> {
        if type_args.len() >= 2 {
            return type_args;
        }
        let error_arg = self.impl_type_arguments.get(1).cloned().or_else(|| {
            self.result_declaration.and_then(|declaration| {
                crate::compiler::mir::ssa::MirSumDeclaration::find_by_declaration(&self.sum_types, declaration)
                    .and_then(|sum| sum.generics.get(1).map(|generic| ValkyrieType::Generic(generic.clone())))
            })
        });
        match type_args.len() {
            0 => {
                if self.impl_type_arguments.len() >= 2 {
                    self.impl_type_arguments.clone()
                }
                else if let Some(declaration) = self.result_declaration {
                    crate::compiler::mir::ssa::MirSumDeclaration::find_by_declaration(&self.sum_types, declaration)
                        .map(|sum| sum.generics.iter().map(|generic| ValkyrieType::Generic(generic.clone())).collect())
                        .unwrap_or_default()
                }
                else {
                    Vec::new()
                }
            }
            1 => {
                if let Some(error) = error_arg {
                    vec![type_args[0].clone(), error]
                }
                else {
                    type_args
                }
            }
            _ => type_args,
        }
    }

    pub(crate) fn sum_instantiation_args(
        &self,
        declaration: nyar_types::ItemId,
        contextual: &ValkyrieType,
        explicit_type_args: &[ValkyrieType],
    ) -> Vec<ValkyrieType> {
        if !explicit_type_args.is_empty() {
            return explicit_type_args.to_vec();
        }
        let from_context = type_args_from_sum_shaped(contextual);
        if !from_context.is_empty() {
            return from_context;
        }
        crate::compiler::mir::ssa::MirSumDeclaration::find_by_declaration(&self.sum_types, declaration)
            .map(|sum| sum.generics.iter().map(|generic| ValkyrieType::Generic(generic.clone())).collect())
            .unwrap_or_default()
    }

    pub(crate) fn infer_array_element_type(&self, array: &MirOperand) -> Option<ValkyrieType> {
        infer_builder_operand_type(array, &self.value_types).and_then(|ty| match ty {
            ValkyrieType::Array(inner) => Some(*inner),
            ValkyrieType::Apply(base, args) if named_type_name(base.as_ref()) == Some("Array") => args.first().cloned(),
            ValkyrieType::FixedArray { element, .. } => Some(*element),
            _ => None,
        })
    }

    pub(crate) fn try_lower_option_is_some(&mut self, receiver: MirOperand, resolved: Option<&HirResolvedCall>) -> Option<MirOperand> {
        let call = resolved?;
        let (owner, variant_declaration) = call.sum_owner.zip(call.sum_variant)?;
        let actual_type = infer_builder_operand_type(&receiver, &self.value_types).filter(|ty| self.is_option_type(ty))?;
        let (nominal, variant) = self.sum_identity_by_declarations(owner, variant_declaration)?;
        let value = self.next_value(MirValueOrigin::CallResult);
        self.push_instruction(
            MirOperation::SumVariantIs { nominal, type_args: type_args_from_sum_shaped(&actual_type), variant, object: receiver },
            vec![value],
        );
        self.value_types.insert(value, ValkyrieType::Boolean);
        Some(MirOperand::Value(value))
    }

    pub(crate) fn option_shaped_type(
        &self,
        receiver: &MirOperand,
        hint: Option<&ValkyrieType>,
        payload_hint: Option<&ValkyrieType>,
    ) -> Option<ValkyrieType> {
        infer_builder_operand_type(receiver, &self.value_types)
            .filter(|ty| self.is_option_type(ty))
            .or_else(|| hint.cloned().filter(|ty| self.is_option_type(ty)))
            .or_else(|| {
                payload_hint
                    .and_then(|payload| self.option_declaration.and_then(|declaration| self.sum_apply_type(declaration, &[payload.clone()])))
            })
    }

    /// Option ????????HIR ???? extractor ????
    pub(crate) fn is_option_unwrap_call(resolved: Option<&HirResolvedCall>) -> bool {
        resolved.is_some_and(|call| matches!(call.domain, HirCallableDomain::Extractor) || call.extractor_payload_type.is_some())
    }

    pub(crate) fn is_option_is_some_call(&self, resolved: Option<&HirResolvedCall>) -> bool {
        resolved.is_some_and(|call| {
            matches!(call.return_type, ValkyrieType::Boolean) && call.parameter_types.first().is_some_and(|ty| self.is_option_type(ty))
        })
    }

    pub(crate) fn try_lower_option_unwrap(
        &mut self,
        receiver: MirOperand,
        resolved: Option<&HirResolvedCall>,
        hint: Option<&ValkyrieType>,
        payload_hint: Option<&ValkyrieType>,
    ) -> Option<MirOperand> {
        let call = resolved?;
        let (owner, variant_declaration) = call.sum_owner.zip(call.sum_variant)?;
        let actual_type = self.option_shaped_type(&receiver, hint, payload_hint)?;
        let payload_type =
            call.extractor_payload_type.clone().or_else(|| Self::option_payload_type(&actual_type)).or_else(|| payload_hint.cloned())?;
        let (nominal, variant) = self.sum_identity_by_declarations(owner, variant_declaration)?;
        if let MirOperand::Value(receiver_ref) = &receiver {
            if self.value_types.get(receiver_ref).is_none_or(|ty| !self.is_option_type(ty)) {
                if let Some(option_ty) = self.sum_apply_type(owner, &[payload_type.clone()]) {
                    self.value_types.insert(*receiver_ref, option_ty);
                }
            }
        }
        let value = self.next_value(MirValueOrigin::CallResult);
        self.push_instruction(
            MirOperation::SumPayloadGet {
                nominal,
                type_args: type_args_from_sum_shaped(&actual_type),
                variant,
                payload_type: payload_type.clone(),
                object: receiver,
            },
            vec![value],
        );
        self.value_types.insert(value, payload_type);
        Some(MirOperand::Value(value))
    }

    pub(crate) fn try_lower_option_none_constructor(
        &mut self,
        resolved: Option<&HirResolvedCall>,
        expected_type: Option<&ValkyrieType>,
        explicit_generic_arguments: &[ValkyrieType],
    ) -> Option<MirOperand> {
        let call = resolved?;
        if call.domain != HirCallableDomain::Constructor {
            return None;
        }
        let (owner, variant_declaration) = call.sum_owner.zip(call.sum_variant)?;
        let contextual = expected_type
            .cloned()
            .or_else(|| (!explicit_generic_arguments.is_empty()).then(|| self.sum_apply_type(owner, explicit_generic_arguments)).flatten())
            .unwrap_or_else(|| call.return_type.clone());
        if !self.is_option_type(&contextual) && contextual.nominal_declaration() != Some(owner) && explicit_generic_arguments.is_empty() {
            return None;
        }
        let type_args = self.sum_instantiation_args(owner, &contextual, explicit_generic_arguments);
        let return_type = self.sum_apply_type(owner, &type_args).unwrap_or_else(|| call.return_type.clone());
        let (nominal, variant) = self.sum_identity_by_declarations(owner, variant_declaration)?;
        let value = self.next_value(MirValueOrigin::CallResult);
        self.push_instruction(MirOperation::SumNew { nominal, type_args, variant, payload_type: None, payload: None }, vec![value]);
        self.value_types.insert(value, return_type);
        Some(MirOperand::Value(value))
    }

    pub(crate) fn try_lower_result_variant_constructor(
        &mut self,
        _variant: &str,
        payload_expr: &HirExpr,
        resolved: Option<&HirResolvedCall>,
        expected_type: Option<&ValkyrieType>,
    ) -> Option<MirOperand> {
        let call = resolved?;
        if call.domain != HirCallableDomain::Constructor {
            return None;
        }
        let (owner, variant_declaration) = call.sum_owner.zip(call.sum_variant)?;
        let (nominal, variant_id) = self.sum_identity_by_declarations(owner, variant_declaration)?;
        // 调用方短名仅供诊断；payload/实参位置只认 sum_owner + binder。
        let contextual = expected_type.cloned().unwrap_or_else(|| call.return_type.clone());
        let payload_hint = call.parameter_types.first().cloned().or_else(|| instantiated_payload_for_call(&self.sum_types, call, &contextual));
        let payload_operand = self.lower_expr_to_operand_with_hint(payload_expr, payload_hint.as_ref());
        let payload_type = infer_builder_operand_type(&payload_operand, &self.value_types).or(payload_hint);
        let mut emitted_type_args = self.pad_result_type_args(self.sum_instantiation_args(owner, &contextual, &[]));
        if let Some(payload) = payload_type.as_ref() {
            if let Some(sum) = crate::compiler::mir::ssa::MirSumDeclaration::find_by_declaration(&self.sum_types, owner) {
                if let Some(variant_decl) = sum.variants.iter().find(|item| item.declaration == Some(variant_declaration)) {
                    apply_payload_to_sum_type_args(sum, variant_decl, &mut emitted_type_args, payload);
                }
            }
        }
        let return_type = if type_args_from_sum_shaped(&contextual).is_empty() && !emitted_type_args.is_empty() {
            self.sum_apply_type(owner, &emitted_type_args).unwrap_or_else(|| contextual.clone())
        }
        else {
            contextual.clone()
        };
        let (return_type, payload_type) =
            concretize_variant_constructor_types(&return_type, payload_type, expected_type, &[payload_operand.clone()], &self.value_types);
        let emitted_type_args = self.pad_result_type_args(if type_args_from_sum_shaped(&return_type).is_empty() {
            emitted_type_args
        }
        else {
            type_args_from_sum_shaped(&return_type)
        });
        let value = self.next_value(MirValueOrigin::CallResult);
        self.push_instruction(
            MirOperation::SumNew { nominal, type_args: emitted_type_args, variant: variant_id, payload_type, payload: Some(payload_operand) },
            vec![value],
        );
        self.value_types.insert(value, return_type);
        Some(MirOperand::Value(value))
    }

    pub(crate) fn try_lower_option_some_constructor(
        &mut self,
        payload_expr: &HirExpr,
        resolved: Option<&HirResolvedCall>,
        expected_type: Option<&ValkyrieType>,
    ) -> Option<MirOperand> {
        let call = resolved?;
        if call.domain != HirCallableDomain::Constructor {
            return None;
        }
        let (owner, variant_declaration) = call.sum_owner.zip(call.sum_variant)?;
        let (nominal, variant) = self.sum_identity_by_declarations(owner, variant_declaration)?;
        let payload_hint = call.parameter_types.first().cloned().or_else(|| expected_type.and_then(|ty| Self::option_payload_type(ty)));
        let payload_operand = self.lower_expr_to_operand_with_hint(payload_expr, payload_hint.as_ref());
        let payload_type = infer_builder_operand_type(&payload_operand, &self.value_types).or(payload_hint);
        let contextual = expected_type
            .cloned()
            .or_else(|| payload_type.as_ref().and_then(|payload| self.sum_apply_type(owner, &[payload.clone()])))
            .unwrap_or_else(|| call.return_type.clone());
        let contextual = refine_auto_apply_type(&contextual, payload_type.as_ref());
        let type_args = refine_auto_sum_instantiation_args(&self.sum_instantiation_args(owner, &contextual, &[]), payload_type.as_ref());
        let return_type = if type_args_from_sum_shaped(&contextual).is_empty() && !type_args.is_empty() {
            self.sum_apply_type(owner, &type_args).unwrap_or_else(|| refine_auto_apply_type(&contextual, payload_type.as_ref()))
        }
        else {
            refine_auto_apply_type(&contextual, payload_type.as_ref())
        };
        let emitted_type_args = if !type_args.is_empty() {
            type_args
        }
        else {
            refine_auto_sum_instantiation_args(&type_args_from_sum_shaped(&return_type), payload_type.as_ref())
        };
        let value = self.next_value(MirValueOrigin::CallResult);
        self.push_instruction(
            MirOperation::SumNew {
                nominal,
                type_args: emitted_type_args,
                variant,
                payload_type: payload_type.clone(),
                payload: Some(payload_operand),
            },
            vec![value],
        );
        self.value_types.insert(value, return_type);
        Some(MirOperand::Value(value))
    }

    pub(crate) fn emit_array_length_operand(&mut self, array: MirOperand, expected_type: Option<&ValkyrieType>) -> MirOperand {
        // 结果类型只消费调用点/声明注解提供的整数事实（`resolved.return_type` 已由
        // `try_lower_array_len_intrinsic` 合入）。事实缺失时结构化失败；禁止按名字
        // 造 `Named("usize")` 再与 `Nominal{ItemId}` 形成双 TypeId（CAN015 StoreVar）。
        let Some(length_ty) = expected_type
            .filter(|ty| crate::compiler::mir::ssa::expr_helpers::is_integer_literal_formal_type(ty, &self.field_declarations))
            .cloned()
        else {
            self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedValueType {
                context: format!("array length result missing integer type fact: {array:?}"),
            });
            return MirOperand::Constant(MirConstant::Unit);
        };
        let value = self.next_value(MirValueOrigin::CallResult);
        // results 必须挂在指令上，否则 wasm `array.len` 结果槽为空。
        self.push_instruction(MirOperation::ArrayLength { array }, vec![value]);
        self.value_types.insert(value, length_ty);
        MirOperand::Value(value)
    }

    pub(crate) fn try_lower_array_len_intrinsic(
        &mut self,
        resolved: Option<&HirResolvedCall>,
        callee: &MirOperand,
        arguments: &[MirOperand],
        expected_type: Option<&ValkyrieType>,
    ) -> Option<MirOperand> {
        if Self::callee_intrinsic_id(resolved, callee) != Some(IntrinsicId::ArrayLen) {
            return None;
        }
        if !Self::has_intrinsic_adaptor_binding(resolved) {
            return None;
        }
        let array = arguments.first().cloned()?;
        let hint = resolved
            .map(|call| &call.return_type)
            .filter(|ty| crate::compiler::mir::ssa::expr_helpers::is_integer_literal_formal_type(ty, &self.field_declarations))
            .or(expected_type);
        Some(self.emit_array_length_operand(array, hint))
    }

    pub(crate) fn try_lower_array_get_on_array(
        &mut self,
        array: MirOperand,
        ordinal: MirOperand,
        resolved: Option<&HirResolvedCall>,
        expected_type: Option<&ValkyrieType>,
    ) -> Option<MirOperand> {
        let array_ty = infer_builder_operand_type(&array, &self.value_types)
            .or_else(|| resolved.and_then(|call| call.parameter_types.first()).filter(|ty| is_array_shaped_valkyrie_type(ty)).cloned())?;
        if !is_array_shaped_valkyrie_type(&array_ty) {
            return None;
        }
        let element_type = self.infer_array_element_type(&array)?;
        let value = self.next_value(MirValueOrigin::CallResult);
        self.push_instruction(MirOperation::ArrayGet { array, index: ordinal }, vec![value]);
        let return_type = resolved
            .map(|call| call.return_type.clone())
            .or_else(|| expected_type.cloned())
            .filter(|ty| !self.is_option_type(ty))
            .unwrap_or(element_type);
        self.value_types.insert(value, return_type);
        Some(MirOperand::Value(value))
    }

    pub(crate) fn try_lower_array_get_intrinsic(
        &mut self,
        resolved: Option<&HirResolvedCall>,
        callee: &MirOperand,
        arguments: &[MirOperand],
        expected_type: Option<&ValkyrieType>,
    ) -> Option<MirOperand> {
        if Self::callee_intrinsic_id(resolved, callee) != Some(IntrinsicId::ArrayGet) {
            return None;
        }
        if !Self::has_intrinsic_adaptor_binding(resolved) {
            return None;
        }
        // Option 包装不生成 ArrayGet intrinsic；保留为普通 Call 让 adaptor 处理
        let wraps_option = resolved
            .map(|call| self.is_option_type(&call.return_type))
            .unwrap_or_else(|| expected_type.is_some_and(|ty| self.is_option_type(ty)));
        if wraps_option {
            return None;
        }
        let array = arguments.first().cloned()?;
        let ordinal = arguments.get(1).cloned()?;
        self.try_lower_array_get_on_array(array, ordinal, resolved, expected_type)
    }

    /// intrinsic 必须绑定 adaptor（有 ItemId/ItemInstanceId）才能走专用 opcode
    pub(crate) fn try_lower_array_set_intrinsic(
        &mut self,
        resolved: Option<&HirResolvedCall>,
        callee: &MirOperand,
        arguments: &[MirOperand],
        _expected_type: Option<&ValkyrieType>,
    ) -> Option<MirOperand> {
        if Self::callee_intrinsic_id(resolved, callee) != Some(IntrinsicId::ArraySet) || arguments.len() != 3 {
            return None;
        }
        if !Self::has_intrinsic_adaptor_binding(resolved) {
            return None;
        }
        let array = arguments[0].clone();
        let array_ty = infer_builder_operand_type(&array, &self.value_types)
            .or_else(|| resolved.and_then(|call| call.parameter_types.first()).filter(|ty| is_array_shaped_valkyrie_type(ty)).cloned())?;
        if !is_array_shaped_valkyrie_type(&array_ty) {
            return None;
        }
        self.push_instruction(
            MirOperation::ArraySet { array: arguments[0].clone(), index: arguments[1].clone(), value: arguments[2].clone() },
            Vec::new(),
        );
        Some(MirOperand::Constant(MirConstant::Unit))
    }

    /// intrinsic 必须绑定 adaptor（有 ItemId/ItemInstanceId）才能走专用 opcode
    pub(crate) fn try_lower_array_push_intrinsic(
        &mut self,
        resolved: Option<&HirResolvedCall>,
        callee: &MirOperand,
        arguments: &[MirOperand],
        expected_type: Option<&ValkyrieType>,
    ) -> Option<MirOperand> {
        if Self::callee_intrinsic_id(resolved, callee) != Some(IntrinsicId::ArrayPush) || arguments.len() != 2 {
            return None;
        }
        if !Self::has_intrinsic_adaptor_binding(resolved) {
            return None;
        }
        let array = arguments[0].clone();
        let array_ty = infer_builder_operand_type(&array, &self.value_types)
            .or_else(|| expected_type.filter(|ty| is_array_shaped_valkyrie_type(ty)).cloned())?;
        if !is_array_shaped_valkyrie_type(&array_ty) {
            return None;
        }
        let value = self.next_value(MirValueOrigin::CallResult);
        self.push_instruction(MirOperation::ArrayPush { array: arguments[0].clone(), value: arguments[1].clone() }, vec![value]);
        let return_type = resolved
            .map(|call| call.return_type.clone())
            .filter(|ty| is_array_shaped_valkyrie_type(ty))
            .or_else(|| expected_type.filter(|ty| is_array_shaped_valkyrie_type(ty)).cloned())
            .unwrap_or(array_ty);
        self.value_types.insert(value, return_type);
        Some(MirOperand::Value(value))
    }

    pub(crate) fn try_lower_array_intrinsics(
        &mut self,
        resolved: Option<&HirResolvedCall>,
        callee: &MirOperand,
        arguments: &[MirOperand],
        expected_type: Option<&ValkyrieType>,
    ) -> Option<MirOperand> {
        self.try_lower_array_len_intrinsic(resolved, callee, arguments, expected_type)
            .or_else(|| self.try_lower_array_get_intrinsic(resolved, callee, arguments, expected_type))
            .or_else(|| self.try_lower_array_set_intrinsic(resolved, callee, arguments, expected_type))
            .or_else(|| self.try_lower_array_push_intrinsic(resolved, callee, arguments, expected_type))
    }

    /// Positional `Range(a, b)` ??????????`StructNew`???????????`Call`??
    pub(crate) fn try_lower_struct_constructor_call(&mut self, call: &HirResolvedCall, arguments: &[MirOperand]) -> Option<MirOperand> {
        if call.sum_owner.is_some() || call.sum_variant.is_some() {
            return None;
        }
        let declaration_id = call.return_type.nominal_declaration().or(call.declaration)?;
        let declaration = crate::compiler::mir::ssa::MirStruct::find_by_declaration(&self.field_declarations, declaration_id)?;
        if declaration.fields.len() != arguments.len() {
            return None;
        }
        let nominal = declaration.nominal;
        let fields = declaration.fields.iter().map(|field| field.id).zip(arguments.iter().cloned()).collect::<Vec<_>>();
        let value = self.next_value(MirValueOrigin::Temporary);
        self.push_instruction(MirOperation::StructNew { nominal, fields }, vec![value]);
        self.value_types.insert(value, call.return_type.clone());
        Some(MirOperand::Value(value))
    }

    pub(crate) fn field_type_for_semantic_type(&self, ty: &ValkyrieType, field: &str) -> Option<ValkyrieType> {
        let declaration_id = ty.nominal_declaration()?;
        let structure = crate::compiler::mir::ssa::MirStruct::find_by_declaration(&self.field_declarations, declaration_id)?;
        structure.instantiate_field(ty, field)
    }

    /// Resolve `Nyar`/`Valkyrie` function type for a call callee (Value or named Symbol).
    pub(crate) fn function_type_of_callee(&self, callee: &MirOperand) -> Option<FunctionType> {
        match callee {
            MirOperand::Value(value) => match self.value_types.get(value) {
                Some(ValkyrieType::Function(func)) => Some(func.as_ref().clone()),
                _ => None,
            },
            _ => None,
        }
    }

    pub(crate) fn storage_for_type(&self, ty: &ValkyrieType) -> MirStorageKind {
        storage_kind_for_type(ty, &self.aggregate_layouts.value_type_declarations)
    }

    pub(crate) fn storage_for_object_operand(&self, operand: &MirOperand) -> MirStorageKind {
        infer_builder_operand_type(operand, &self.value_types).map(|ty| self.storage_for_type(&ty)).unwrap_or(MirStorageKind::Reference)
    }

    pub(crate) fn layout_id_for_type(&mut self, ty: &ValkyrieType) -> Option<crate::compiler::mir::ssa::LayoutId> {
        if let Some(id) = layout_id_for_type(ty, &self.aggregate_layouts) {
            return Some(id);
        }
        match ty {
            ValkyrieType::Named(name) => None,
            _ if self.storage_for_type(ty) == MirStorageKind::Value => ensure_layout_for_type(&mut self.aggregate_layouts, ty),
            _ => None,
        }
    }

    pub(crate) fn layout_id_for_object_operand(&mut self, operand: &MirOperand) -> Option<crate::compiler::mir::ssa::LayoutId> {
        infer_builder_operand_type(operand, &self.value_types).and_then(|ty| self.layout_id_for_type(&ty))
    }

    pub(crate) fn field_type_for_object_operand(&self, object_operand: &MirOperand, field: &Identifier) -> Option<ValkyrieType> {
        let object_ty = infer_builder_operand_type(object_operand, &self.value_types)?;
        self.field_type_for_semantic_type(&object_ty, field.as_str())
    }
}

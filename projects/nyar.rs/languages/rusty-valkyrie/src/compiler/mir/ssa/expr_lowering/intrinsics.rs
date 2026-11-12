use crate::compiler::types::{
    Identifier,
    hir::{FunctionType, HirCallableDomain, HirExpr, HirResolvedCall, ValkyrieType},
};

use crate::compiler::mir::ssa::{
    MirBuilder, MirConstant, MirOperand, MirOperation, MirStorageKind, MirValueOrigin, infer_builder_operand_type,
    value_semantics::{ensure_layout_for_type, layout_id_for_type, storage_kind_for_type},
};

use super::{type_args_from_sum_shaped, unresolved_variant_labels};

impl MirBuilder {
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

    /// ??????sum declaration ItemId ??????????? `Named`??
    pub(crate) fn sum_apply_type(&self, declaration: nyar_types::ItemId, type_args: &[ValkyrieType]) -> Option<ValkyrieType> {
        let _ = crate::compiler::mir::ssa::MirSumDeclaration::find_by_declaration(&self.sum_types, declaration)?;
        // sum 声明（枚举）不是 `[primitive]` 声明，没有自举身份。
        Some(ValkyrieType::Nominal { declaration, arguments: type_args.to_vec(), primitive_identity: None })
    }

    /// sum 实例化实参只来自显式类型实参与 HIR 已结算的上下文形状；缺额由调用方按声明
    /// binder 数校验并给出结构化诊断，绝不从声明 binder 表合成实参。
    pub(crate) fn sum_instantiation_args(&self, contextual: &ValkyrieType, explicit_type_args: &[ValkyrieType]) -> Vec<ValkyrieType> {
        if !explicit_type_args.is_empty() {
            return explicit_type_args.to_vec();
        }
        type_args_from_sum_shaped(contextual)
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
        span: &crate::compiler::types::SourceSpan,
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
        let type_args = self.sum_instantiation_args(&contextual, explicit_generic_arguments);
        // 实例化实参数量与声明 binder 表不一致：HIR 未结算完整事实，带 span 结构化拒绝，
        // 不从 binder 表合成缺额实参。
        if let Some(sum) = crate::compiler::mir::ssa::MirSumDeclaration::find_by_declaration(&self.sum_types, owner)
            && sum.generics.len() != type_args.len()
        {
            self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedSumInstanceFacts {
                span: span.clone(),
                context: format!(
                    "构造 `{}` 缺少完整实例化实参：声明 {} 个 binder，实收 {} 个",
                    call.symbol,
                    sum.generics.len(),
                    type_args.len()
                ),
            });
            return Some(MirOperand::Constant(MirConstant::Unit));
        }
        let return_type = self.sum_apply_type(owner, &type_args).unwrap_or_else(|| call.return_type.clone());
        let (nominal, variant) = self.sum_identity_by_declarations(owner, variant_declaration)?;
        let value = self.next_value(MirValueOrigin::CallResult);
        self.push_instruction(MirOperation::SumNew { nominal, type_args, variant, payload_type: None, payloads: Vec::new() }, vec![value]);
        self.value_types.insert(value, return_type);
        Some(MirOperand::Value(value))
    }

    /// 一元 payload 变体构造（`Some(v)`/`Fine {..}`/`Fail(e)` 等）：只投影 HIR 已结算的
    /// 完整事实，不按声明 binder 元数分流 Result/Option——两者走同一构造合同。
    ///
    /// 类型实参来自已结算返回类型或调用方期望形状；payload 类型来自 HIR 形参类型，
    /// 或按已结算实参代入变体声明取得。缺事实（实参数与声明不符、仍含 `AutoType`、
    /// 有字段变体拿不到 payload 类型）立即产生带 source/span 的结构化诊断；
    /// MIR 不从 payload 类型回填类型实参、不从 binder 表合成实参。
    pub(crate) fn try_lower_payload_variant_constructor(
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
        let (nominal, variant_id) = self.sum_identity_by_declarations(owner, variant_declaration)?;
        let contextual = expected_type.cloned().unwrap_or_else(|| call.return_type.clone());
        let type_args = self.sum_instantiation_args(&contextual, &[]);
        if let Some(sum) = crate::compiler::mir::ssa::MirSumDeclaration::find_by_declaration(&self.sum_types, owner) {
            if sum.generics.len() != type_args.len() {
                self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedSumInstanceFacts {
                    span: payload_expr.span.clone(),
                    context: format!(
                        "变体构造 `{}` 的实例化实参未由 HIR 结算完整：声明 {} 个 binder，实收 {} 个",
                        call.symbol,
                        sum.generics.len(),
                        type_args.len()
                    ),
                });
                return Some(MirOperand::Constant(MirConstant::Unit));
            }
        }
        if type_args.iter().any(|arg| matches!(arg, ValkyrieType::AutoType)) {
            self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedSumInstanceFacts {
                span: payload_expr.span.clone(),
                context: format!("变体构造 `{}` 的类型实参仍含 `AutoType`：HIR 未结算单态化实例", call.symbol),
            });
            return Some(MirOperand::Constant(MirConstant::Unit));
        }
        let sum = crate::compiler::mir::ssa::MirSumDeclaration::find_by_declaration(&self.sum_types, owner);
        let variant = sum.and_then(|sum| sum.variants.iter().find(|item| item.declaration == Some(variant_declaration)));
        let payload_type = call.parameter_types.first().cloned().or_else(|| match (sum, variant) {
            (Some(sum), Some(variant)) => sum.instantiate_payload(variant, &type_args).flatten(),
            _ => None,
        });
        if payload_type.is_none() && variant.is_some_and(|variant| !variant.fields.is_empty()) {
            self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedSumInstanceFacts {
                span: payload_expr.span.clone(),
                context: format!("变体构造 `{}` 缺少 payload 类型事实：形参类型未结算且声明代入无结果", call.symbol),
            });
            return Some(MirOperand::Constant(MirConstant::Unit));
        }
        let payload_operand = self.lower_expr_to_operand_with_hint(payload_expr, payload_type.as_ref());
        let return_type = self.sum_apply_type(owner, &type_args).unwrap_or_else(|| contextual.clone());
        let value = self.next_value(MirValueOrigin::CallResult);
        self.push_instruction(
            MirOperation::SumNew { nominal, type_args, variant: variant_id, payload_type, payloads: vec![payload_operand] },
            vec![value],
        );
        self.value_types.insert(value, return_type);
        Some(MirOperand::Value(value))
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

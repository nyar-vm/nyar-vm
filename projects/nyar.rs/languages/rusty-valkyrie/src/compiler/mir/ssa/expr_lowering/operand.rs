use crate::compiler::{
    hir::is_nullable_type,
    types::{
        Identifier, NamePath,
        hir::{FunctionType, HirCallableDomain, HirExpr, HirExprKind, HirLiteral, HirResolvedCall, ValkyrieType},
    },
};

use crate::compiler::mir::ssa::{
    MirBuilder, MirConstant, MirInstruction, MirOperand, MirOperation, MirStorageKind, MirValueOrigin, callee_name_matches,
    expr_helpers::{is_array_shaped_valkyrie_type, lower_resolved_callee, named_type_name, peel_generic_apply},
    infer_builder_operand_type, lower_callee_operand,
    value_semantics::{ensure_layout_for_type, layout_id_for_type, storage_kind_for_type},
};

use super::{
    concretize_variant_constructor_types, lower_literal, sum_new_parts_from_constructor, type_args_from_sum_shaped, unresolved_variant_labels,
};

impl MirBuilder {
    pub(crate) fn lower_expr_to_operand(&mut self, expr: &HirExpr) -> MirOperand {
        self.lower_expr_to_operand_with_hint(expr, None)
    }

    pub(crate) fn lower_expr_to_operand_with_hint(&mut self, expr: &HirExpr, expected_type: Option<&ValkyrieType>) -> MirOperand {
        match &expr.kind {
            HirExprKind::Literal(literal) => {
                let (constant, ty) = lower_literal(literal, expected_type);
                // 调用点形参为 `Nominal{usize}` 时，overload 已特化字面量；MIR 必须写出同一类型，
                // 否则 SMIR007 仍见 Integer32 vs Nominal。
                let ty = match (literal, expected_type) {
                    (HirLiteral::Integer64(_), Some(expected))
                        if crate::compiler::mir::ssa::expr_helpers::is_integer_literal_formal_type(expected, &self.field_declarations) =>
                    {
                        Some(expected.clone())
                    }
                    _ => ty,
                };
                let value = self.next_value(MirValueOrigin::Literal);
                self.push_instruction(MirOperation::LoadConstant { constant, ty: ty.clone() }, vec![value]);
                if let Some(ty) = ty {
                    self.value_types.insert(value, ty);
                }
                MirOperand::Value(value)
            }
            HirExprKind::Variable(identifier) => {
                if let Some(bound) = self.bindings.get(identifier.name.as_str()).cloned() {
                    return bound;
                }
                self.diagnostics
                    .push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedCallableIdentity { symbol: identifier.name.to_string() });
                MirOperand::Constant(MirConstant::Unit)
            }
            HirExprKind::Path(path) => {
                let value = self.next_value(MirValueOrigin::Path);
                let _ = path;
                MirOperand::Value(value)
            }
            HirExprKind::Call { callee, args, resolved, .. } => {
                let (explicit_generic_arguments, callee) = peel_generic_apply(callee.as_ref());
                // HIR may lower `expr.unwrap()` to `unwrap(expr)` (functional call).
                // ????Option ??????? Extractor ??????????????????
                if args.len() == 1 && Self::is_option_unwrap_call(resolved.as_ref()) {
                    let receiver_operand = self.lower_expr_to_operand(&args[0].value);
                    let payload_hint = resolved.as_ref().map(|call| call.return_type.clone()).or_else(|| expected_type.cloned());
                    let hint =
                        infer_builder_operand_type(&receiver_operand, &self.value_types).filter(|ty| self.is_option_type(ty)).or_else(|| {
                            let declaration = resolved.as_ref().and_then(|call| call.sum_owner).or(self.option_declaration)?;
                            payload_hint.as_ref().and_then(|payload| self.sum_apply_type(declaration, &[payload.clone()]))
                        });
                    if let Some(operand) =
                        self.try_lower_option_unwrap(receiver_operand.clone(), resolved.as_ref(), hint.as_ref(), payload_hint.as_ref())
                    {
                        return operand;
                    }
                }
                // 一元 payload 变体构造：只认已冻结 sum_owner/sum_variant；按 owner binder 元数分流，禁止 Some/Fine/Fail 短名入口。
                if args.len() == 1 {
                    if let Some(call) = resolved.as_ref() {
                        if call.domain == crate::compiler::types::hir::HirCallableDomain::Constructor {
                            if let Some(owner) = call.sum_owner.filter(|_| call.sum_variant.is_some()) {
                                let binary = crate::compiler::mir::ssa::MirSumDeclaration::find_by_declaration(&self.sum_types, owner)
                                    .is_some_and(|sum| sum.generics.len() == 2);
                                return if binary {
                                    self.try_lower_result_variant_constructor("", &args[0].value, resolved.as_ref(), expected_type)
                                        .unwrap_or_else(|| self.fail_unresolved_sum_variant_call(resolved.as_ref()))
                                }
                                else {
                                    self.try_lower_option_some_constructor(&args[0].value, resolved.as_ref(), expected_type)
                                        .unwrap_or_else(|| self.fail_unresolved_sum_variant_call(resolved.as_ref()))
                                };
                            }
                        }
                    }
                }
                // 禁止按 callee 名 `tuple` 补结构语义；须等真实 Tuple AST/HIR/类型身份落地。
                if callee_name_matches(&callee.kind, "tuple") {
                    self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedCallableIdentity { symbol: "tuple".to_string() });
                    return MirOperand::Constant(MirConstant::Unit);
                }
                if let Some((receiver_operand, _method_name)) =
                    self.extract_method_call(callee).filter(|_| resolved.as_ref().is_some_and(|call| call.has_receiver))
                {
                    let param_types = resolved.as_ref().map(|call| call.parameter_types.as_slice());
                    // Skip receiver slot (index 0) when binding hints for explicit args.
                    let mut arguments = args
                        .iter()
                        .enumerate()
                        .map(|(index, arg)| {
                            let hint = param_types.and_then(|params| params.get(index + 1));
                            self.lower_expr_to_operand_with_hint(&arg.value, hint)
                        })
                        .collect::<Vec<_>>();
                    arguments.insert(0, receiver_operand.clone());
                    if args.is_empty() && self.is_option_is_some_call(resolved.as_ref()) {
                        if self.option_shaped_type(&receiver_operand, None, None).is_some() {
                            if let Some(operand) = self.try_lower_option_is_some(receiver_operand.clone(), resolved.as_ref()) {
                                return operand;
                            }
                        }
                    }
                    if args.is_empty() && Self::is_option_unwrap_call(resolved.as_ref()) {
                        let payload_hint = resolved.as_ref().map(|call| call.return_type.clone()).or_else(|| expected_type.cloned());
                        // ??????Option ??????????Option ?????? payload ????
                        let hint = infer_builder_operand_type(&receiver_operand, &self.value_types)
                            .filter(|ty| self.is_option_type(ty))
                            .or_else(|| {
                                let declaration = resolved.as_ref().and_then(|call| call.sum_owner).or(self.option_declaration)?;
                                payload_hint.as_ref().and_then(|payload| self.sum_apply_type(declaration, &[payload.clone()]))
                            });
                        if let Some(operand) =
                            self.try_lower_option_unwrap(receiver_operand.clone(), resolved.as_ref(), hint.as_ref(), payload_hint.as_ref())
                        {
                            return operand;
                        }
                    }
                    if let Some(operand) = self.try_lower_array_intrinsics(resolved.as_ref(), &arguments, expected_type) {
                        return operand;
                    }
                    let resolved_call = resolved.as_ref().expect("Semantic MIR requires a resolved instance call contract");
                    let Some(callee) = lower_resolved_callee(resolved_call)
                    else {
                        self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedCallableIdentity {
                            symbol: resolved_call.symbol.to_string(),
                        });
                        return MirOperand::Constant(MirConstant::Unit);
                    };
                    let return_type = self.return_type_for_resolved_call(&callee, resolved_call);
                    return self.push_call_returning(callee, arguments, return_type);
                }
                // `obj.field(args)` where `obj.field` is a function-typed field (e.g.
                // `FilterIterator._predicate`). Lower as indirect call: load the field
                // value via FieldGet, then `Call { dispatch: Indirect, callee: <value> }`.
                // This lets backends emit `Delegate::DynamicInvoke` / `call_ref` etc.
                // instead of treating the field name as a static method symbol.
                if let HirExprKind::FieldAccess { object, field, field_id } = &callee.kind {
                    let receiver_operand = self.lower_expr_to_operand(object);
                    let field_ty = self.field_type_for_object_operand(&receiver_operand, field);
                    if matches!(field_ty, Some(ValkyrieType::Function(_))) {
                        let Some(field_id) = *field_id
                        else {
                            self.diagnostics
                                .push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedFieldIdentity { field: field.to_string() });
                            return MirOperand::Constant(MirConstant::Unit);
                        };
                        let callee_value = self.next_value(MirValueOrigin::Temporary);
                        self.push_instruction(MirOperation::FieldGet { object: receiver_operand, field: field_id }, vec![callee_value]);
                        if let Some(field_ty) = field_ty {
                            self.value_types.insert(callee_value, field_ty);
                        }
                        let param_types = resolved.as_ref().map(|call| call.parameter_types.as_slice());
                        let arguments = args
                            .iter()
                            .enumerate()
                            .map(|(index, arg)| {
                                let hint = param_types.and_then(|params| params.get(index));
                                self.lower_expr_to_operand_with_hint(&arg.value, hint)
                            })
                            .collect::<Vec<_>>();
                        let return_type =
                            resolved.as_ref().expect("function-typed field call requires resolved call contract").return_type.clone();
                        // `unit` ?????? value ??BPHYS001???
                        return self.push_call_returning(MirOperand::Value(callee_value), arguments, return_type);
                    }

                    // `obj.field.method()` where `obj` is not yet in bindings (e.g. nested
                    // temporaries) still must lower as a receiver Call, not a dotted Symbol.
                    let has_receiver = resolved.as_ref().is_some_and(|call| call.has_receiver);
                    let param_types = resolved.as_ref().map(|call| call.parameter_types.as_slice());
                    let mut arguments = args
                        .iter()
                        .enumerate()
                        .map(|(index, arg)| {
                            let hint = param_types.and_then(|params| params.get(index + usize::from(has_receiver)));
                            self.lower_expr_to_operand_with_hint(&arg.value, hint)
                        })
                        .collect::<Vec<_>>();
                    if has_receiver {
                        arguments.insert(0, receiver_operand.clone());
                    }
                    if let Some(operand) = self.try_lower_array_intrinsics(resolved.as_ref(), &arguments, expected_type) {
                        return operand;
                    }
                    let resolved_call = resolved.as_ref().expect("Semantic MIR requires a resolved field receiver call contract");
                    let Some(callee) = lower_resolved_callee(resolved_call)
                    else {
                        self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedCallableIdentity {
                            symbol: resolved_call.symbol.to_string(),
                        });
                        return MirOperand::Constant(MirConstant::Unit);
                    };
                    let return_type = self.return_type_for_resolved_call(&callee, resolved_call);
                    // `unit` ?????? value ??BPHYS001???
                    return self.push_call_returning(callee, arguments, return_type);
                }
                let is_prefix_not = callee_name_matches(&callee.kind, "prefix !");
                let param_types = resolved.as_ref().map(|call| call.parameter_types.as_slice());
                // Lower args before unite constructor paths so `Some(v)` keeps payload hints.
                let mut arguments = Vec::with_capacity(args.len());
                for (index, arg) in args.iter().enumerate() {
                    let hint_owned: Option<ValkyrieType> = param_types
                        .and_then(|params| params.get(index).cloned())
                        .or_else(|| {
                            (param_types.is_none() && expected_type.is_some() && matches!(arg.value.kind, HirExprKind::Call { .. }))
                                .then(|| expected_type.cloned())
                                .flatten()
                        })
                        .or_else(|| is_prefix_not.then_some(ValkyrieType::Boolean))
                        .or_else(|| {
                            if index == 0 || arguments.is_empty() {
                                return None;
                            }
                            let first_ty = infer_builder_operand_type(&arguments[0], &self.value_types)?;
                            match first_ty {
                                ValkyrieType::Array(inner) => Some(*inner),
                                _ => None,
                            }
                        });
                    arguments.push(self.lower_expr_to_operand_with_hint(&arg.value, hint_owned.as_ref()));
                }
                // Unite constructors must lower before static callee projection.
                if let Some(call) = resolved.as_ref() {
                    if call.domain == HirCallableDomain::Constructor {
                        if args.is_empty() {
                            if let Some(operand) =
                                self.try_lower_option_none_constructor(Some(call), expected_type, &explicit_generic_arguments)
                            {
                                return operand;
                            }
                        }
                        if let Some(operand) = self.try_lower_struct_constructor_call(call, &arguments) {
                            return operand;
                        }
                        if let Some((sum_type, type_args, variant, payload_type, payload)) =
                            sum_new_parts_from_constructor(call, &arguments, &self.sum_types)
                        {
                            let value = self.next_value(MirValueOrigin::CallResult);
                            let Some((owner, variant_declaration)) = call.sum_owner.zip(call.sum_variant)
                            else {
                                self.diagnostics
                                    .push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedVariantIdentity { sum_type, variant });
                                return MirOperand::Constant(MirConstant::Unit);
                            };
                            let Some((nominal, variant_id)) = self.sum_identity_by_declarations(owner, variant_declaration)
                            else {
                                return MirOperand::Constant(MirConstant::Unit);
                            };
                            let (return_type, payload_type) = concretize_variant_constructor_types(
                                &call.return_type,
                                payload_type,
                                expected_type,
                                &arguments,
                                &self.value_types,
                            );
                            self.push_instruction(
                                MirOperation::SumNew { nominal, type_args, variant: variant_id, payload_type, payload },
                                vec![value],
                            );
                            self.value_types.insert(value, return_type);
                            return MirOperand::Value(value);
                        }
                        let (sum_type, variant) = unresolved_variant_labels(Some(call));
                        self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedVariantIdentity { sum_type, variant });
                        return MirOperand::Constant(MirConstant::Unit);
                    }
                }
                if let Some(operand) = self.try_lower_array_intrinsics(resolved.as_ref(), &arguments, expected_type) {
                    return operand;
                }
                let callee = match lower_callee_operand(callee, resolved.as_ref(), self) {
                    Some(callee) => callee,
                    None if resolved.as_ref().is_some_and(|call| call.domain == HirCallableDomain::Constructor) => {
                        let call = resolved.as_ref().expect("constructor call");
                        match lower_resolved_callee(call) {
                            Some(callee) => callee,
                            None => {
                                self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedCallableIdentity {
                                    symbol: call.symbol.to_string(),
                                });
                                return MirOperand::Constant(MirConstant::Unit);
                            }
                        }
                    }
                    // HIR 把运算符成员调用（`a op b` → `infix ==`）展开为单段 Path callee，
                    // 接收者就是首位实参；`has_receiver` 是 HIR 冻结的接收者合同。callee 不是
                    // FieldAccess 不代表没有接收者，这里按同一合同投影 callee（含 trait
                    // witness），不得把调用退化成 Unit。
                    None if resolved.as_ref().is_some_and(|call| call.has_receiver)
                        && matches!(&callee.kind, HirExprKind::Path(path) if path.parts().len() == 1) =>
                    {
                        let call = resolved.as_ref().expect("receiver-implicit call contract");
                        let Some(callee) = lower_resolved_callee(call)
                        else {
                            self.diagnostics
                                .push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedCallableIdentity { symbol: call.symbol.to_string() });
                            return MirOperand::Constant(MirConstant::Unit);
                        };
                        let return_type = self.return_type_for_resolved_call(&callee, call);
                        return self.push_call_returning(callee, arguments, return_type);
                    }
                    None => {
                        self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedCallableIdentity {
                            symbol: "construct missing HIR callable contract".to_string(),
                        });
                        return MirOperand::Constant(MirConstant::Unit);
                    }
                };
                if let Some(operand) = self.try_lower_ref_deref_intrinsic(resolved.as_ref(), &callee, &arguments) {
                    return operand;
                }
                if let Some(operand) = self.try_lower_null_intrinsics(resolved.as_ref(), &callee, &arguments) {
                    return operand;
                }
                let function_ty = self.function_type_of_callee(&callee);
                if arguments.len() == 1 {
                    if self.is_option_is_some_call(resolved.as_ref()) && self.option_shaped_type(&arguments[0], None, None).is_some() {
                        if let Some(operand) = self.try_lower_option_is_some(arguments[0].clone(), resolved.as_ref()) {
                            return operand;
                        }
                    }
                }
                let Some(return_type) = resolved
                    .as_ref()
                    .map(|call| call.return_type.clone())
                    .or_else(|| self.declared_return_type_for_call(&callee, resolved.as_ref()))
                    .or_else(|| function_ty.map(|func| func.return_type))
                else {
                    self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedValueType {
                        context: format!("call missing return type fact: {callee:?}"),
                    });
                    return MirOperand::Constant(MirConstant::Unit);
                };
                let return_type = crate::compiler::mir::ssa::expr_helpers::refine_call_return_type(return_type, expected_type);
                self.push_call_returning(callee, arguments, return_type)
            }
            HirExprKind::ArrayNew { element_type, length } => {
                let length_operand = self.lower_expr_to_operand(length);
                let array_type = ValkyrieType::Array(Box::new(element_type.clone()));
                let value = self.next_value(MirValueOrigin::Temporary);
                self.push_instruction(
                    MirOperation::ArrayNew {
                        array_type: array_type.clone(),
                        length: length_operand,
                        initialization: crate::compiler::mir::ssa::ArrayInitialization::Default,
                    },
                    vec![value],
                );
                self.value_types.insert(value, array_type);
                MirOperand::Value(value)
            }
            HirExprKind::ArrayLiteral { items } => {
                // ????????ArrayFromElements???? Array ?? array_type ????
                let array_type = match expected_type {
                    Some(ValkyrieType::FixedArray { element, length }) => {
                        ValkyrieType::FixedArray { element: element.clone(), length: *length }
                    }
                    Some(ValkyrieType::Array(item)) => ValkyrieType::Array(item.clone()),
                    _ => {
                        let element_type = infer_array_literal_element_type(items, None);
                        ValkyrieType::Array(Box::new(element_type))
                    }
                };
                let element_hint = match &array_type {
                    ValkyrieType::FixedArray { element, .. } | ValkyrieType::Array(element) => Some(element.as_ref()),
                    _ => None,
                };
                let elements = items.iter().map(|item| self.lower_expr_to_operand_with_hint(item, element_hint)).collect::<Vec<_>>();
                let array_value = self.next_value(MirValueOrigin::Temporary);
                self.push_instruction(MirOperation::ArrayFromElements { array_type: array_type.clone(), elements }, vec![array_value]);
                self.value_types.insert(array_value, array_type);
                MirOperand::Value(array_value)
            }
            HirExprKind::TupleLiteral { items } => {
                let fields = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let hint = expected_type.and_then(|ty| match ty {
                            ValkyrieType::Tuple(element_types) => element_types.get(index),
                            _ => None,
                        });
                        self.lower_expr_to_operand_with_hint(item, hint)
                    })
                    .collect::<Vec<_>>();
                let tuple_type = match expected_type {
                    Some(ValkyrieType::Tuple(element_types)) if element_types.len() == items.len() => {
                        ValkyrieType::Tuple(element_types.clone())
                    }
                    _ => ValkyrieType::Tuple(
                        fields
                            .iter()
                            .map(|operand| infer_builder_operand_type(operand, &self.value_types).unwrap_or(ValkyrieType::AutoType))
                            .collect(),
                    ),
                };
                let tuple_value = self.next_value(MirValueOrigin::Temporary);
                self.push_instruction(MirOperation::TupleNew { fields }, vec![tuple_value]);
                self.value_types.insert(tuple_value, tuple_type);
                MirOperand::Value(tuple_value)
            }
            HirExprKind::Construct { path, args, resolved } => {
                let name = path.parts().last().cloned().unwrap_or_else(|| Identifier::new("_"));
                let struct_type_name = self.struct_new_owner_name(&name);
                let mut fields: Vec<(nyar_types::FieldId, MirOperand)> = Vec::with_capacity(args.len());
                let mut field_values = Vec::with_capacity(args.len());
                for (arg_index, arg) in args.iter().enumerate() {
                    if let HirExprKind::FieldInit { name: field_name, value, field_id } = &arg.kind {
                        // Empty `[]` without a hint defaults to `i32[]` (see
                        // `infer_array_literal_element_type`). Pass the struct field type so
                        // `visited: [utf8] = []` lowers as `string[]`, not `int32[]`.
                        // 字段身份只消费 HIR 冻结结果；缺失即结构化失败。
                        let owner = self.struct_construct_result_type(&name, resolved.as_ref());
                        let field_ty = self
                            .field_type_for_semantic_type(&owner, field_name.as_str())
                            .or_else(|| resolved.as_ref().and_then(|call| call.parameter_types.get(arg_index).cloned()))
                            .map(|ty| crate::compiler::mir::ssa::resolve_self_type_with_owner(&ty, self.impl_owner_type.as_ref()));
                        let value_operand = self.lower_expr_to_operand_with_hint(value, field_ty.as_ref());
                        // StructNew SMIR010 compares operand `value_types` (after
                        // concretize) to aggregate layout field types.  Hints steer
                        // `[]` element typing but do not update `value_types`; without
                        // this, `ArrayList { _items: [] }` stays `i32[]` while layout
                        // keeps `Array(Generic(T))`.
                        if let (Some(declared_ty), MirOperand::Value(value_ref)) = (&field_ty, &value_operand) {
                            self.value_types.insert(*value_ref, declared_ty.clone());
                        }
                        // 字段身份：优先用 HIR 已分配的 field_id；缺失时 fallback
                        // 到 field_declarations 按字段名查找（generic class 内部
                        // Construct 的 FieldInit 在 HIR 阶段不会分配 field_id），
                        // 同时也检查 sum variant fields（unite/enum 构造器）。
                        let field_id = field_id.or_else(|| {
                            let decl_id = owner.nominal_declaration()?;
                            // 先查 struct declarations
                            if let Some(struct_decl) = self.field_declarations.iter().find(|s| s.declaration == Some(decl_id)) {
                                if let Some(f) = struct_decl.fields.iter().find(|f| f.name == field_name.as_str()) {
                                    return Some(f.id);
                                }
                            }
                            // 再查 sum variant fields —— unite/enum 构造器的字段身份在 sum_types 里
                            if let Some(call) = resolved.as_ref() {
                                if let (Some(sum_decl), Some(variant_decl)) = (call.sum_owner, call.sum_variant) {
                                    if let Some(sum) = self.sum_types.iter().find(|s| s.declaration == Some(sum_decl)) {
                                        if let Some(variant) = sum.variants.iter().find(|v| v.declaration == Some(variant_decl)) {
                                            if let Some(f) = variant.fields.iter().find(|f| f.name == field_name.as_str()) {
                                                return Some(f.id);
                                            }
                                        }
                                    }
                                }
                            }
                            None
                        });
                        let Some(field_id) = field_id
                        else {
                            self.diagnostics
                                .push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedFieldIdentity { field: field_name.to_string() });
                            return MirOperand::Constant(MirConstant::Unit);
                        };
                        field_values.push(value_operand.clone());
                        fields.push((field_id, value_operand));
                    }
                }
                // Unite/enum constructors lower as `SumNew` only when HIR already closed
                // `sum_owner` / `sum_variant` facts. Do not rediscover variant identity
                // from spellings in MIR.
                if let Some(call) = resolved.as_ref() {
                    if call.domain == HirCallableDomain::Constructor {
                        if let Some((owner, variant_declaration)) = call.sum_owner.zip(call.sum_variant) {
                            let value = self.next_value(MirValueOrigin::CallResult);
                            let ctor_return = call.return_type.clone();
                            let type_args = type_args_from_sum_shaped(&ctor_return);
                            let payload_type = call.parameter_types.first().cloned();
                            let payload = match field_values.len() {
                                0 => None,
                                1 => Some(field_values[0].clone()),
                                _ => field_values.first().cloned(),
                            };
                            let Some((nominal, variant_id)) = self.sum_identity_by_declarations(owner, variant_declaration)
                            else {
                                let (sum_type, variant) = unresolved_variant_labels(Some(call));
                                self.diagnostics
                                    .push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedVariantIdentity { sum_type, variant });
                                return MirOperand::Constant(MirConstant::Unit);
                            };
                            let (return_type, payload_type) = concretize_variant_constructor_types(
                                &ctor_return,
                                payload_type,
                                expected_type,
                                &field_values,
                                &self.value_types,
                            );
                            self.push_instruction(
                                MirOperation::SumNew { nominal, type_args, variant: variant_id, payload_type, payload },
                                vec![value],
                            );
                            self.value_types.insert(value, return_type);
                            return MirOperand::Value(value);
                        }
                        if call.sum_owner.is_some() || call.sum_variant.is_some() {
                            let (sum_type, variant) = unresolved_variant_labels(Some(call));
                            self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedVariantIdentity { sum_type, variant });
                            return MirOperand::Constant(MirConstant::Unit);
                        }
                    }
                }
                let value = self.next_value(MirValueOrigin::Temporary);
                let result_type = self.struct_construct_result_type(&name, resolved.as_ref());
                let Some(declaration_id) = result_type.nominal_declaration().or_else(|| resolved.as_ref().and_then(|call| call.declaration))
                else {
                    self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedNominalIdentity { type_name: struct_type_name });
                    return MirOperand::Constant(MirConstant::Unit);
                };
                let Some(nominal) = self.nominal_identity_for_declaration(declaration_id)
                else {
                    self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedNominalIdentity { type_name: struct_type_name });
                    return MirOperand::Constant(MirConstant::Unit);
                };
                self.push_instruction(MirOperation::StructNew { nominal, fields }, vec![value]);
                self.value_types.insert(value, result_type);
                MirOperand::Value(value)
            }
            HirExprKind::FieldAccess { object, field, field_id } => {
                let object_operand = self.lower_singleton_field_object(object).unwrap_or_else(|| self.lower_expr_to_operand(object));
                // 禁止按字段名 `length` 改写成 ArrayLength；数组长度只能来自
                // 已在 HIR 解析的 BuiltinStructureOp::ArrayLen Call / 明确 ArrayLength 节点。
                let Some(field_id) = *field_id
                else {
                    self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedFieldIdentity { field: field.to_string() });
                    return MirOperand::Constant(MirConstant::Unit);
                };
                self.lower_object_field_get_operand(object_operand, field_id, field)
            }
            HirExprKind::StoreField { object, field, value, field_id } => {
                let object_operand = self.lower_singleton_field_object(object).unwrap_or_else(|| self.lower_expr_to_operand(object));
                let storage = self.storage_for_object_operand(&object_operand);
                let layout_id = self.layout_id_for_object_operand(&object_operand);
                let field_type = self.field_type_for_object_operand(&object_operand, field);
                let value_operand = self.lower_expr_to_operand_with_hint(value, field_type.as_ref());
                let Some(field_id) = *field_id
                else {
                    self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedFieldIdentity { field: field.to_string() });
                    return MirOperand::Constant(MirConstant::Unit);
                };
                self.instructions.push(MirInstruction::from_operation(MirOperation::FieldSet {
                    object: object_operand,
                    field: field_id,
                    value: value_operand,
                }));
                MirOperand::Constant(MirConstant::Unit)
            }
            HirExprKind::Return(value) => {
                let return_ty = self.current_return_type.clone();
                let terminand = value.as_deref().map(|e| self.lower_expr_to_operand_with_hint(e, Some(&return_ty)));
                self.lower_explicit_return(terminand);
                MirOperand::Constant(MirConstant::Unit)
            }
            HirExprKind::Assign { target, value } => {
                let name = target.as_str().to_string();
                let existing = self.bindings.get(&name).cloned();
                let expected = existing.as_ref().and_then(|operand| infer_builder_operand_type(operand, &self.value_types));
                let operand = self.lower_expr_to_operand_with_hint(value, expected.as_ref());
                if let Some(ty) = infer_builder_operand_type(&operand, &self.value_types) {
                    if self.storage_for_type(&ty) == MirStorageKind::Value {
                        let layout_id = ensure_layout_for_type(&mut self.aggregate_layouts, &ty);
                        if let Some(layout_id) = layout_id {
                            let dest = self.next_value(MirValueOrigin::LetBinding { name: name.clone() });
                            self.push_instruction(
                                MirOperation::AggregateCopy { source: operand.clone(), dest: MirOperand::Value(dest) },
                                vec![dest],
                            );
                            self.value_types.insert(dest, ty);
                            self.bindings.insert(name, MirOperand::Value(dest));
                            return MirOperand::Constant(MirConstant::Unit);
                        }
                    }
                }
                let new_value = self.next_value(MirValueOrigin::LetBinding { name: name.clone() });
                self.push_instruction(MirOperation::StoreVar { name: name.clone(), value: operand.clone(), ty: None }, vec![new_value]);
                if let Some(ty) = infer_builder_operand_type(&operand, &self.value_types) {
                    self.value_types.insert(new_value, ty);
                }
                self.bindings.insert(name, MirOperand::Value(new_value));
                MirOperand::Constant(MirConstant::Unit)
            }
            HirExprKind::If { condition, then_branch, else_branch } => self.lower_if_expr(condition, then_branch, else_branch, expected_type),
            HirExprKind::IfLet { pattern, scrutinee, then_branch, else_branch } => {
                self.lower_if_let_expr(pattern, scrutinee, then_branch, else_branch)
            }
            HirExprKind::Block(body) => self.lower_block_expr(body),
            HirExprKind::Loop { label, pattern, iterator, condition, body, .. } => {
                self.lower_loop_expr(label, pattern, iterator, condition, body)
            }
            HirExprKind::Break { label, expr } => self.lower_break_expr(label, expr),
            HirExprKind::Continue { label } => self.lower_continue_expr(label),
            HirExprKind::Match { scrutinee, arms } => self.lower_match_expr(scrutinee, arms, expected_type),
            HirExprKind::Case { scrutinee, arms } => self.lower_case_expr(scrutinee, arms, expected_type),
            HirExprKind::Yield(value) => self.lower_yield_expr(value.as_deref()),
            HirExprKind::YieldFrom(value) => self.lower_yield_from_expr(value),
            HirExprKind::Await(value) => self.lower_await_expr(value),
            HirExprKind::Awake(value) => self.lower_awake_expr(value),
            HirExprKind::BlockOn(value) => self.lower_block_on_expr(value),
            HirExprKind::Raise(value) => self.lower_raise_expr(value),
            HirExprKind::Resume(value) => self.lower_resume_expr(value),
            HirExprKind::Catch { expr, arms } => self.lower_catch_expr(expr, arms),
            HirExprKind::TryPropagate { inner, resolved_ok_variant, .. } => self.lower_try_propagate_expr(inner, resolved_ok_variant.as_ref()),
            HirExprKind::TryScope { is_optional, is_forced, result_type, body } => {
                self.lower_try_scope_expr(*is_optional, *is_forced, result_type, body)
            }
            HirExprKind::AnonymousClass { class_name: Some(class_name), fields, captures, .. } => {
                self.lower_anonymous_class_expr(class_name, fields, captures)
            }
            HirExprKind::Fallthrough => self.lower_fallthrough_expr(),
            HirExprKind::Cast { expr, ty } => {
                let operand = self.lower_expr_to_operand(expr);
                if let MirOperand::Value(value_ref) = operand {
                    if self.value_types.get(&value_ref) == Some(ty) {
                        return MirOperand::Value(value_ref);
                    }
                    // `expr as T` 必须保留为 Cast：Copy 要求源/结果同 TypeId，不能用来偷换类型。
                    let casted = self.next_value(MirValueOrigin::Temporary);
                    self.push_instruction(MirOperation::Cast { source: MirOperand::Value(value_ref), ty: ty.clone() }, vec![casted]);
                    self.value_types.insert(casted, ty.clone());
                    MirOperand::Value(casted)
                }
                else {
                    operand
                }
            }
            _ => {
                self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnsupportedExpression {
                    span: expr.span.clone(),
                    kind: format!("{:?}", expr.kind),
                });
                let value = self.next_value(MirValueOrigin::Temporary);
                self.instructions.push(MirInstruction::from_operation(MirOperation::LoadConstant {
                    constant: MirConstant::Unit,
                    ty: Some(ValkyrieType::Unit),
                }));
                self.value_types.insert(value, ValkyrieType::Unit);
                MirOperand::Value(value)
            }
        }
    }
}

pub(crate) fn infer_array_literal_element_type(items: &[HirExpr], hint: Option<&ValkyrieType>) -> ValkyrieType {
    if let Some(hint) = hint {
        return hint.clone();
    }
    match items.first().map(|item| &item.kind) {
        Some(HirExprKind::Literal(HirLiteral::Bool(_))) => ValkyrieType::Boolean,
        Some(HirExprKind::Literal(HirLiteral::String(_))) => ValkyrieType::Utf8,
        Some(HirExprKind::Literal(HirLiteral::Float64(_))) => ValkyrieType::Float64,
        Some(HirExprKind::Literal(HirLiteral::Integer64(_))) => ValkyrieType::Integer32 { signed: true },
        _ => ValkyrieType::Integer32 { signed: true },
    }
}

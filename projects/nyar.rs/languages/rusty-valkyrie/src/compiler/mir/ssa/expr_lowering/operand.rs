use crate::compiler::{
    hir::is_nullable_type,
    types::{
        Identifier, NamePath,
        hir::{FunctionType, HirCallableDomain, HirExpr, HirExprKind, HirLiteral, HirResolvedCall, HirTupleField, ValkyrieType},
    },
};

use crate::compiler::mir::ssa::{
    MirBuilder, MirConstant, MirInstruction, MirOperand, MirOperation, MirValueOrigin, callee_name_matches,
    expr_helpers::{is_array_shaped_valkyrie_type, lower_resolved_callee, named_type_name, peel_generic_apply},
    infer_builder_operand_type, lower_callee_operand,
    value_semantics::{layout_id_for_type, value_copy_supported},
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
                let (constant, ty) = match lower_literal(literal, expected_type, &self.field_declarations) {
                    Ok(pair) => pair,
                    Err(reason) => {
                        self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnsupportedExpression {
                            span: expr.span.clone(),
                            kind: reason,
                        });
                        return MirOperand::Constant(MirConstant::Unit);
                    }
                };
                // 调用点形参为 `Nominal{usize}` 时，overload 已特化字面量；MIR 必须写出同一类型，
                // 否则 SMIR007 仍见 Integer32 vs Nominal。
                let ty = match (literal, expected_type) {
                    (HirLiteral::Integer64(_), Some(expected))
                        if crate::compiler::mir::ssa::expr_helpers::is_integer_literal_formal_type(expected, &self.field_declarations) =>
                    {
                        Some(expected.clone())
                    }
                    (HirLiteral::Byte(_), Some(expected))
                        if crate::compiler::mir::ssa::expr_helpers::is_integer_literal_formal_type(expected, &self.field_declarations) =>
                    {
                        Some(expected.clone())
                    }
                    (HirLiteral::Character(_), Some(expected))
                        if crate::compiler::mir::ssa::expr_helpers::is_character_literal_formal_type(expected, &self.field_declarations) =>
                    {
                        Some(expected.clone())
                    }
                    // 字符串常量只能写编码类型：SMIR007 的文本常量条款要求 `LoadConstant`
                    // 的 SSA 类型等于字面量编码（`Utf8`/`Utf16`），名义形参的兼容性
                    // 由调用边界的 `call_site_type_compatible` 核对，不在此改写。
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
                if let Some(operand) = self.lower_unit_variant_path(identifier.name.as_str(), expected_type) {
                    return operand;
                }
                // 防御性断言：HIR 提升已把捕获名改写为环境形参的 `FieldAccess`，
                // 正常路径不应到达这里；保留该检查避免任何影子局部静默兜底。
                if self.captured_bindings.contains(identifier.name.as_str()) {
                    self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnsupportedExpression {
                        span: expr.span.clone(),
                        kind: format!(
                            "lambda captures outer binding `{}`: capture environments are not represented in Semantic MIR",
                            identifier.name
                        ),
                    });
                    return MirOperand::Constant(MirConstant::Unit);
                }
                self.diagnostics
                    .push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedCallableIdentity { symbol: identifier.name.to_string() });
                MirOperand::Constant(MirConstant::Unit)
            }
            HirExprKind::Path(path) => {
                if let [identifier] = path.parts() {
                    if let Some(operand) = self.lower_unit_variant_path(identifier.as_str(), expected_type) {
                        return operand;
                    }
                }
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
                if let HirExprKind::FieldAccess { object, field, field_id, .. } = &callee.kind {
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
                        if let Some((sum_type, type_args, variant, payload_type, payloads, multi_field)) =
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
                            let concretized = concretize_variant_constructor_types(
                                &call.return_type,
                                payload_type.clone(),
                                expected_type,
                                &arguments,
                                &self.value_types,
                            );
                            let return_type = concretized.0;
                            // 多字段变体：payload 类型必须保持变体记录身份，不能被首字段类型覆盖。
                            let payload_type = if multi_field { payload_type } else { concretized.1 };
                            self.push_instruction(
                                MirOperation::SumNew { nominal, type_args, variant: variant_id, payload_type, payloads },
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
            HirExprKind::CollectionInit { with_length, append, items, .. } => {
                // 集合字面量展开：消费 HIR post-binding pass 冻结的两槽调用合同，
                // 先 `Invoke(with_length, [n])` 得容器临时值，再逐项 `Invoke(append, [tmp, e])`。
                // 两 callee 身份直接取 HIR 冻结的 `ItemInstanceId`，禁止按名回查、禁止新 opcode；
                // 指令形态与普通方法调用一致（`mut self` 接收者即首位实参，ByAddress 由后端按
                // 同一接收者合同投影）。空字面量只发 `with_length(0)`。
                let Some(with_length_callee) = lower_resolved_callee(with_length)
                else {
                    self.diagnostics
                        .push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedCallableIdentity { symbol: with_length.symbol.to_string() });
                    return MirOperand::Constant(MirConstant::Unit);
                };
                let Some(append_callee) = lower_resolved_callee(append)
                else {
                    self.diagnostics
                        .push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedCallableIdentity { symbol: append.symbol.to_string() });
                    return MirOperand::Constant(MirConstant::Unit);
                };
                // 长度实参类型取冻结合同首形参（`usize`），与普通调用形参 hint 同一口径。
                let length_hint = with_length.parameter_types.first().cloned();
                let length_operand = self.lower_expr_to_operand_with_hint(
                    &HirExpr { kind: HirExprKind::Literal(HirLiteral::Integer64(items.len() as i64)), span: expr.span.clone() },
                    length_hint.as_ref(),
                );
                let container = self.push_call_returning(with_length_callee, vec![length_operand], with_length.return_type.clone());
                // 源序单次求值：逐项 lower 元素并追加；元素形参类型取冻结合同次形参。
                let element_hint = append.parameter_types.get(1);
                for item in items {
                    let item_operand = self.lower_expr_to_operand_with_hint(item, element_hint);
                    self.push_call_returning(append_callee.clone(), vec![container.clone(), item_operand], append.return_type.clone());
                }
                container
            }
            HirExprKind::TupleLiteral { items } => {
                let fields = items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let hint = expected_type.and_then(|ty| match ty {
                            ValkyrieType::Tuple(element_types) => element_types.get(index),
                            ValkyrieType::LabeledTuple(tuple_fields) => tuple_fields.get(index).map(|field| &field.ty),
                            _ => None,
                        });
                        self.lower_expr_to_operand_with_hint(item, hint)
                    })
                    .collect::<Vec<_>>();
                let tuple_type = match expected_type {
                    Some(ValkyrieType::Tuple(element_types)) if element_types.len() == items.len() => {
                        ValkyrieType::Tuple(element_types.clone())
                    }
                    // 字面量没有标签语法：标签只从期望类型取得，元素类型仍按实际字面量推断，
                    // 让真正的类型不匹配留到后续校验，而不是被期望类型掩盖。
                    Some(ValkyrieType::LabeledTuple(expected_fields)) if expected_fields.len() == items.len() => {
                        ValkyrieType::LabeledTuple(
                            expected_fields
                                .iter()
                                .zip(fields.iter())
                                .map(|(field, operand)| HirTupleField {
                                    name: field.name.clone(),
                                    ty: infer_builder_operand_type(operand, &self.value_types)
                                        .unwrap_or_else(|| field.ty.clone()),
                                })
                                .collect(),
                        )
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
            HirExprKind::Construct { path, args, resolved, .. } => {
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
                        // 字段身份只消费 HIR 解析期已写回的 `field_id`；MIR 不再按
                        // owner + 字段名回查声明（原名拼写兜底已清退），缺失即
                        // 结构化诊断拒绝进入 canonical。
                        let Some(field_id) = *field_id
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
                            // 多字段变体：按声明顺序收集全部字段操作数，payload 类型取
                            // 「变体 declaration × 枚举实参」的记录身份，不取首字段类型。
                            let multi_payload = {
                                let sum = self.sum_types.iter().find(|item| item.declaration == Some(owner));
                                let variant = sum.and_then(|sum| {
                                    sum.variants.iter().find(|item| item.declaration == Some(variant_declaration))
                                });
                                variant.filter(|variant| variant.fields.len() > 1).and_then(|variant| {
                                    let record_type = sum.and_then(|sum| sum.instantiate_payload(variant, &type_args)).flatten();
                                    let ordered = variant
                                        .fields
                                        .iter()
                                        .map(|declared| {
                                            fields.iter().find(|(id, _)| *id == declared.id).map(|(_, operand)| operand.clone())
                                        })
                                        .collect::<Option<Vec<_>>>()?;
                                    Some((record_type, ordered))
                                })
                            };
                            let (payload_type, payloads, has_record_payload) = match multi_payload {
                                Some((record_type, payloads)) => (record_type, payloads, true),
                                None => {
                                    let payload_type = call.parameter_types.first().cloned();
                                    let payloads = match field_values.len() {
                                        0 => Vec::new(),
                                        1 => vec![field_values[0].clone()],
                                        _ => field_values.clone(),
                                    };
                                    (payload_type, payloads, false)
                                }
                            };
                            let Some((nominal, variant_id)) = self.sum_identity_by_declarations(owner, variant_declaration)
                            else {
                                let (sum_type, variant) = unresolved_variant_labels(Some(call));
                                self.diagnostics
                                    .push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedVariantIdentity { sum_type, variant });
                                return MirOperand::Constant(MirConstant::Unit);
                            };
                            let concretized = concretize_variant_constructor_types(
                                &ctor_return,
                                payload_type.clone(),
                                expected_type,
                                &field_values,
                                &self.value_types,
                            );
                            let return_type = concretized.0;
                            let payload_type = if has_record_payload { payload_type } else { concretized.1 };
                            self.push_instruction(
                                MirOperation::SumNew { nominal, type_args, variant: variant_id, payload_type, payloads },
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
            HirExprKind::FieldAccess { object, field, field_id, tuple_index } => {
                let object_operand = self.lower_singleton_field_object(object).unwrap_or_else(|| self.lower_expr_to_operand(object));
                if let Some(index) = *tuple_index {
                    // 元组位置由 `AggregateFieldIndex::resolve_field` 在 HIR 冻结
                    // （带标签按标签、无标签按 `.0` 位置），这里只消费位置事实，不重新按名字猜。
                    let object_type = infer_builder_operand_type(&object_operand, &self.value_types);
                    let field_type = match object_type.as_ref() {
                        Some(ValkyrieType::LabeledTuple(fields)) => fields.get(index as usize).map(|field| field.ty.clone()),
                        Some(ValkyrieType::Tuple(fields)) => fields.get(index as usize).cloned(),
                        _ => None,
                    };
                    let Some(field_type) = field_type
                    else {
                        self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedFieldIdentity { field: field.to_string() });
                        return MirOperand::Constant(MirConstant::Unit);
                    };
                    let value = self.next_value(MirValueOrigin::Temporary);
                    self.push_instruction(MirOperation::TupleGet { object: object_operand, index }, vec![value]);
                    self.value_types.insert(value, field_type);
                    return MirOperand::Value(value);
                }
                if let Some(object_type) = infer_builder_operand_type(&object_operand, &self.value_types) {
                    if matches!(object_type, ValkyrieType::Tuple(_)) && field_id.is_none() {
                        // HIR 未冻结位置的无名元组读取：纯数字成员名是越界（结构化错误），
                        // 其余按名访问保持结构化失败，禁止退回按位置解析。
                        match crate::compiler::mir::AggregateFieldIndex::resolve_tuple_field(&object_type, field) {
                            crate::compiler::mir::TupleFieldResolution::IndexOutOfRange { index, arity } => {
                                self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::TupleIndexOutOfRange {
                                    field: field.to_string(),
                                    index,
                                    arity,
                                });
                            }
                            crate::compiler::mir::TupleFieldResolution::Positional { .. } => {
                                self.diagnostics
                                    .push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedFieldIdentity { field: field.to_string() });
                            }
                            _ => {
                                self.diagnostics
                                    .push(crate::compiler::mir::ssa::MirDiagnostic::UnlabeledTupleFieldAccess { field: field.to_string() });
                            }
                        }
                        return MirOperand::Constant(MirConstant::Unit);
                    }
                }
                // 禁止按字段名 `length` 改写成 ArrayLength；数组长度只能来自
                // 已解析的库调用合同 / 明确 ArrayLength 节点。
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
                // 被返回的表达式本身已发散（例如所有臂都 `return` 的 `match`）时，
                // 控制流不会走到这里；再发 `Return { Unit }` 会把发散改写成
                // `actual=Unit` 的值返回（SMIR007）。
                if self.terminator.is_none() {
                    self.lower_explicit_return(terminand);
                }
                MirOperand::Constant(MirConstant::Unit)
            }
            HirExprKind::Assign { target, value } => {
                let name = target.as_str().to_string();
                // 防御性断言：可变捕获在 HIR 提升期已改写为环境上的 `StoreField`，
                // 正常路径不应到达这里；保留该检查，禁止新建影子局部兜底。
                if !self.bindings.contains_key(&name) && self.captured_bindings.contains(&name) {
                    self.diagnostics.push(crate::compiler::mir::ssa::MirDiagnostic::UnsupportedExpression {
                        span: expr.span.clone(),
                        kind: format!(
                            "lambda writes outer binding `{name}`: capture environments are not represented in Semantic MIR"
                        ),
                    });
                    return MirOperand::Constant(MirConstant::Unit);
                }
                let existing = self.bindings.get(&name).cloned();
                let expected = existing.as_ref().and_then(|operand| infer_builder_operand_type(operand, &self.value_types));
                let operand = self.lower_expr_to_operand_with_hint(value, expected.as_ref());
                if let Some(ty) = infer_builder_operand_type(&operand, &self.value_types) {
                    if value_copy_supported(&mut self.aggregate_layouts, &ty) {
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
            HirExprKind::Block(body) => self.lower_block_expr(body, expected_type),
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
            HirExprKind::Lambda { params, return_type, captures, hoisted, .. } => {
                // 只消费 HIR 提升期冻结的合成函数实例；捕获环境本轮不物化。
                self.lower_lambda_expr(params, return_type, expected_type, &expr.span, captures, hoisted.as_ref())
            }
            HirExprKind::FunctionReference(reference) => {
                // 具名函数作值：只消费 HIR 冻结的实例身份，缺失即结构化失败，
                // 禁止按 symbol 短名回查或现场铸造。
                let Some(instance) = reference.instance
                else {
                    self.diagnostics
                        .push(crate::compiler::mir::ssa::MirDiagnostic::UnresolvedCallableIdentity { symbol: reference.symbol.to_string() });
                    return MirOperand::Constant(MirConstant::Unit);
                };
                let value = self.next_value(MirValueOrigin::Temporary);
                self.push_instruction(
                    MirOperation::LoadFunctionRef { instance, environment: None, captured: Vec::new() },
                    vec![value],
                );
                self.value_types.insert(value, ValkyrieType::Function(reference.signature.clone()));
                MirOperand::Value(value)
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

    /// 裸零字段变体名（`None` / `Word`）按调用点冻结的期望类型降成 `SumNew`。
    ///
    /// HIR 已把该形参槽位定型为某个 sum 声明；MIR 只按 `sum_owner` 声明身份与
    /// `variant.declaration` 取变体，不按拼写猜。期望类型不是已冻结的 sum、声明里
    /// 没有同名零字段变体时一律返回 `None`，保留原路径降级语义。
    fn lower_unit_variant_path(&mut self, name: &str, expected: Option<&ValkyrieType>) -> Option<MirOperand> {
        let expected = expected?;
        let declaration = expected.nominal_declaration()?;
        let sum = crate::compiler::mir::ssa::MirSumDeclaration::find_by_declaration(&self.sum_types, declaration)?;
        let variant = sum.variants.iter().find(|variant| variant.name == name && variant.fields.is_empty())?;
        let variant_declaration = variant.declaration?;
        let (nominal, variant_id) = self.sum_identity_by_declarations(declaration, variant_declaration)?;
        let type_args = type_args_from_sum_shaped(expected);
        let value = self.next_value(MirValueOrigin::Path);
        self.push_instruction(
            MirOperation::SumNew { nominal, type_args, variant: variant_id, payload_type: None, payloads: Vec::new() },
            vec![value],
        );
        self.value_types.insert(value, expected.clone());
        Some(MirOperand::Value(value))
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

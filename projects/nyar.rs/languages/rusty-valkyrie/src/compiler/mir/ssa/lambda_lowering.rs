//! 无捕获 lambda 的 MIR lowering。
//!
//! lambda 不是 `UnsupportedExpression`：它落成同一 `MirModule` 内一个合成的
//! `MirFunction`，表达式位点发射 `LoadFunctionRef { instance }`，结果是
//! `ValkyrieType::Function` 值载体。捕获环境本轮不表示——lambda 体内引用外层
//! 局部变量会经 `captured_bindings` 结构化失败，而不是静默落 `Unit`。

use std::cell::Cell;

use nyar_types::{ItemId, ItemInstanceId};

use crate::compiler::types::{
    Identifier, NamePath, SourceSpan,
    hir::{FunctionType, HirBlock, HirParam, ValkyrieType},
};

use super::{MirConstant, MirDiagnostic, MirFunction, MirOperand, MirOperation, MirTerminator, MirValueOrigin, builder::MirBuilder};

/// 程序级 lambda 身份分配器。
///
/// lambda 的 `ItemId` / `ItemInstanceId` 必须落在所有 HIR 声明、HIR 实例与宿主合同
/// 之后；同一程序内所有 builder 共享同一个分配器，避免跨模块身份撞车。
pub struct LambdaInstanceAllocator {
    next_declaration: Cell<u32>,
    next_instance: Cell<u32>,
}

impl LambdaInstanceAllocator {
    /// 以两个互不相交的起点构造分配器（均从 0 起稠密编号）。
    pub fn new(declaration_base: u32, instance_base: u32) -> Self {
        Self { next_declaration: Cell::new(declaration_base), next_instance: Cell::new(instance_base) }
    }

    fn allocate(&self) -> Option<(ItemId, ItemInstanceId)> {
        let declaration = ItemId::from_index(self.next_declaration.get())?;
        let instance = ItemInstanceId::from_index(self.next_instance.get())?;
        self.next_declaration.set(self.next_declaration.get().saturating_add(1));
        self.next_instance.set(self.next_instance.get().saturating_add(1));
        Some((declaration, instance))
    }
}

/// 注解未给出可比较类型（`auto` / `Self`）时采用上下文函数类型。
fn annotation_is_unknown(ty: &ValkyrieType) -> bool {
    matches!(ty, ValkyrieType::AutoType | ValkyrieType::SelfType)
}

impl MirBuilder {
    pub(super) fn lower_lambda_expr(
        &mut self,
        params: &[HirParam],
        return_type: &ValkyrieType,
        body: &HirBlock,
        expected_type: Option<&ValkyrieType>,
        span: &SourceSpan,
    ) -> MirOperand {
        let expected = match expected_type {
            Some(ValkyrieType::Function(function)) => Some(function.as_ref()),
            _ => None,
        };
        let Some(signature) = self.resolve_lambda_signature(params, return_type, expected, span) else {
            return MirOperand::Constant(MirConstant::Unit);
        };
        let Some(allocator) = self.lambda_allocator.clone() else {
            self.diagnostics.push(MirDiagnostic::UnsupportedExpression {
                span: span.clone(),
                kind: "lambda lowering requires a program-level instance allocator".to_string(),
            });
            return MirOperand::Constant(MirConstant::Unit);
        };
        let Some((declaration, instance)) = allocator.allocate() else {
            self.diagnostics.push(MirDiagnostic::UnsupportedExpression {
                span: span.clone(),
                kind: "lambda instance identity pool exhausted".to_string(),
            });
            return MirOperand::Constant(MirConstant::Unit);
        };
        let mir_function = self.lower_lambda_function(params, &signature, body, declaration, instance);
        self.pending_functions.push(mir_function);
        let value = self.next_value(MirValueOrigin::Temporary);
        self.push_instruction(MirOperation::LoadFunctionRef { instance }, vec![value]);
        self.value_types.insert(value, ValkyrieType::Function(Box::new(signature)));
        MirOperand::Value(value)
    }

    /// 形参数量/类型必须与上下文函数类型一致；缺失上下文时要求源码注解完整。
    fn resolve_lambda_signature(
        &mut self,
        params: &[HirParam],
        return_type: &ValkyrieType,
        expected: Option<&FunctionType>,
        span: &SourceSpan,
    ) -> Option<FunctionType> {
        let declared_params: Vec<ValkyrieType> = params.iter().map(|param| param.ty.clone()).collect();
        match expected {
            Some(expected) => {
                if expected.params.len() != declared_params.len() {
                    self.diagnostics.push(MirDiagnostic::UnsupportedExpression {
                        span: span.clone(),
                        kind: format!(
                            "lambda arity mismatch: declared {} parameter(s) but expected function type takes {}",
                            declared_params.len(),
                            expected.params.len()
                        ),
                    });
                    return None;
                }
                let mut resolved_params = Vec::with_capacity(declared_params.len());
                for (index, (declared, expected_param)) in declared_params.iter().zip(expected.params.iter()).enumerate() {
                    if annotation_is_unknown(declared) {
                        resolved_params.push(expected_param.clone());
                    }
                    else if declared == expected_param {
                        resolved_params.push(declared.clone());
                    }
                    else {
                        self.diagnostics.push(MirDiagnostic::UnsupportedExpression {
                            span: span.clone(),
                            kind: format!(
                                "lambda parameter[{index}] annotation `{declared:?}` differs from expected function type parameter `{expected_param:?}`"
                            ),
                        });
                        return None;
                    }
                }
                let resolved_return = if annotation_is_unknown(return_type) {
                    expected.return_type.clone()
                }
                else if return_type == &expected.return_type {
                    return_type.clone()
                }
                else {
                    self.diagnostics.push(MirDiagnostic::UnsupportedExpression {
                        span: span.clone(),
                        kind: format!(
                            "lambda return annotation `{return_type:?}` differs from expected function type return `{:?}`",
                            expected.return_type
                        ),
                    });
                    return None;
                };
                Some(FunctionType { params: resolved_params, return_type: resolved_return })
            }
            None => {
                if declared_params.iter().any(annotation_is_unknown) || annotation_is_unknown(return_type) {
                    self.diagnostics.push(MirDiagnostic::UnsupportedExpression {
                        span: span.clone(),
                        kind: "lambda signature needs a contextual function type or fully annotated parameters and return type"
                            .to_string(),
                    });
                    return None;
                }
                Some(FunctionType { params: declared_params, return_type: return_type.clone() })
            }
        }
    }

    fn lower_lambda_function(
        &mut self,
        params: &[HirParam],
        signature: &FunctionType,
        body: &HirBlock,
        declaration: ItemId,
        instance: ItemInstanceId,
    ) -> MirFunction {
        let mut child = MirBuilder::new(
            self.aggregate_layouts.clone(),
            self.singleton_accessor_instances.clone(),
            self.effectful_inline_targets.clone(),
            self.callable_returns.clone(),
            self.member_dispatch.clone(),
            None,
            self.host_runtime_callables.clone(),
        );
        child.sum_types = self.sum_types.clone();
        child.option_declaration = self.option_declaration;
        child.result_declaration = self.result_declaration;
        child.field_declarations = self.field_declarations.clone();
        child.static_bindings = self.static_bindings.clone();
        child.impl_type_arguments = self.impl_type_arguments.clone();
        child.current_return_type = signature.return_type.clone();
        child.lambda_allocator = self.lambda_allocator.clone();
        // lambda 只拥有自己的形参；外层局部名一旦被引用就是捕获，本轮不表示。
        child.captured_bindings = self.bindings.keys().cloned().collect();

        let mut param_values = Vec::new();
        for (index, (param, ty)) in params.iter().zip(signature.params.iter()).enumerate() {
            let value = child.next_value(MirValueOrigin::Parameter { index, name: param.name.name.to_string() });
            child.bindings.insert(param.name.name.to_string(), MirOperand::Value(value));
            child.value_types.insert(value, ty.clone());
            param_values.push(value);
        }
        child.blocks[0].parameters = param_values.clone();

        if child.terminator.is_none() {
            for statement in &body.statements {
                child.lower_statement(statement);
                if child.terminator.is_some() {
                    break;
                }
            }
        }
        if child.terminator.is_none() {
            if let Some(expr) = &body.expr {
                let return_ty = child.current_return_type.clone();
                if matches!(return_ty, ValkyrieType::Unit | ValkyrieType::Void) {
                    let _ = child.lower_expr_to_operand(expr);
                    if child.terminator.is_none() {
                        child.terminate(MirTerminator::Return { value: None });
                    }
                }
                else {
                    let operand = child.lower_expr_to_operand_with_hint(expr, Some(&return_ty));
                    if child.terminator.is_none() {
                        child.terminate(MirTerminator::Return { value: Some(operand) });
                    }
                }
            }
            else {
                child.terminate(MirTerminator::Return { value: None });
            }
        }

        let current_label = child.current_label.clone();
        child.flush_block(&current_label);
        // 嵌套 lambda 先并回父级，父级最终统一交给模块入口。
        self.pending_functions.append(&mut child.pending_functions);
        self.diagnostics.append(&mut child.diagnostics);

        let symbol = NamePath::new(vec![Identifier::new(&format!("__lambda_{}", instance.index()))]);
        MirFunction {
            display_symbol: crate::compiler::hir::symbols::stable_symbol_key(&symbol),
            symbol,
            declaration: Some(declaration),
            instance: Some(instance),
            return_type: signature.return_type.clone(),
            param_types: signature.params.clone(),
            value_types: child.value_types,
            entry: child.entry,
            values: child.values,
            blocks: child.blocks,
        }
    }
}

//! lambda 的 MIR lowering。
//!
//! lambda 值只消费 HIR 提升期冻结的合成函数实例：表达式位点发射
//! `LoadFunctionRef { instance }`，结果是 `ValkyrieType::Function` 值载体。
//! 合成函数本身是普通 HIR 项，由模块入口正常降低；MIR 不再现场合成函数或铸造实例。
//!
//! 捕获环境的目标侧物化不在本阶段：非空捕获一律结构化失败，并点名 lambda 实例
//! 与捕获名，不允许静默落 `Unit` 或影子局部。

use crate::compiler::types::{
    SourceSpan,
    hir::{FunctionType, HirCapture, HirLambdaIdentity, HirParam, ValkyrieType},
};

use super::{MirConstant, MirDiagnostic, MirOperand, MirOperation, MirValueOrigin, builder::MirBuilder};

/// 注解未给出可比较类型（`auto` / `Self`）时采用上下文函数类型。
fn annotation_is_unknown(ty: &ValkyrieType) -> bool {
    matches!(ty, ValkyrieType::AutoType | ValkyrieType::SelfType)
}

impl MirBuilder {
    pub(super) fn lower_lambda_expr(
        &mut self,
        params: &[HirParam],
        return_type: &ValkyrieType,
        expected_type: Option<&ValkyrieType>,
        span: &SourceSpan,
        captures: &[HirCapture],
        hoisted: Option<&HirLambdaIdentity>,
    ) -> MirOperand {
        let expected = match expected_type {
            Some(ValkyrieType::Function(function)) => Some(function.as_ref()),
            _ => None,
        };
        let Some(signature) = self.resolve_lambda_signature(params, return_type, expected, span) else {
            return MirOperand::Constant(MirConstant::Unit);
        };

        if !captures.is_empty() {
            let instance = hoisted.and_then(|identity| identity.instance).map(|instance| instance.index().to_string());
            let instance = instance.unwrap_or_else(|| "<unregistered>".to_string());
            let names = captures.iter().map(|capture| capture.identifier.name.to_string()).collect::<Vec<_>>().join(", ");
            self.diagnostics.push(MirDiagnostic::UnsupportedExpression {
                span: span.clone(),
                kind: format!(
                    "lambda instance `{instance}` captures [{names}]; capture environments are not represented in MIR for this target"
                ),
            });
            return MirOperand::Constant(MirConstant::Unit);
        }

        let Some(instance) = hoisted.and_then(|identity| identity.instance) else {
            self.diagnostics.push(MirDiagnostic::UnsupportedExpression {
                span: span.clone(),
                kind: "lambda synthetic function identity is missing from HIR; MIR must not mint it".to_string(),
            });
            return MirOperand::Constant(MirConstant::Unit);
        };

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
}

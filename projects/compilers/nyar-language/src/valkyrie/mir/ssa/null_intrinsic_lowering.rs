use crate::types::hir::ValkyrieType;
use nyar_types::IntrinsicId;

use super::{MirBuilder, MirOperand, MirOperation, MirValueOrigin};

impl MirBuilder {
    pub(super) fn emit_null_is_null_operand(&mut self, value: MirOperand) -> MirOperand {
        let result = self.next_value(MirValueOrigin::CallResult);
        self.push_instruction(MirOperation::NullIsNull { value }, vec![result]);
        self.value_types.insert(result, ValkyrieType::Boolean);
        MirOperand::Value(result)
    }

    pub(super) fn emit_null_unwrap_operand(&mut self, value: MirOperand, payload_type: ValkyrieType) -> MirOperand {
        let result = self.next_value(MirValueOrigin::CallResult);
        self.push_instruction(MirOperation::NullUnwrap { value }, vec![result]);
        self.value_types.insert(result, payload_type);
        MirOperand::Value(result)
    }

    pub(super) fn try_lower_null_intrinsics(
        &mut self,
        resolved: Option<&crate::types::hir::HirResolvedCall>,
        callee: &MirOperand,
        arguments: &[MirOperand],
    ) -> Option<MirOperand> {
        match Self::callee_intrinsic_id(resolved, callee)? {
            IntrinsicId::IsNull if arguments.len() == 1 => Some(self.emit_null_is_null_operand(arguments[0].clone())),
            IntrinsicId::UnwrapNull if arguments.len() == 1 => {
                let payload_type = resolved.map(|call| call.return_type.clone()).or_else(|| {
                    super::infer_builder_operand_type(&arguments[0], &self.value_types).and_then(|ty| crate::hir::nullable_payload_type(&ty))
                })?;
                Some(self.emit_null_unwrap_operand(arguments[0].clone(), payload_type))
            }
            _ => None,
        }
    }
}

use crate::compiler::types::hir::ValkyrieType;

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

    /// 所有 null intrinsic lowering 分支统一降级为普通 Call。
    ///
    /// 核心合同同 [`crate::compiler::mir::ssa::MirBuilder::try_lower_ref_deref_intrinsic`] 顶部：
    /// intrinsic 专用 opcode 只允许由 adaptor 链接进来的后端专用绑定产生；
    /// 源码 intrinsic 自由函数、语言 builtin intrinsic 候选一律走普通 Call 路径。
    pub(super) fn try_lower_null_intrinsics(
        &mut self,
        _resolved: Option<&crate::compiler::types::hir::HirResolvedCall>,
        _callee: &MirOperand,
        _arguments: &[MirOperand],
    ) -> Option<MirOperand> {
        None
    }
}

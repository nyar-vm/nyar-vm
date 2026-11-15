//! 后端模式匹配最小契约：MIR 已显式化 probe/bind/branch，后端只需处理
//! `MirTerminator::Branch` / `MirTerminator::Jump` 与 `MirInstructionKind::{Call,
//! FieldGet, AggregateCopy}` 等低层指令，不得重新解释高层 pattern。
//!
//! 本模块提供 [`validate_pattern_matching_invariants`]，由各后端 lowering 入口
//! 调用，确保后端不会因残留的高层 pattern 语义而生成错误代码。
//!
//! case / match 链的身份与合法性由语言前端拥有（Valkyrie 在自己的 `MirFunction`
//! 上保留 `MirCaseChain` 并验证）；目标侧只消费已展开的 probe/bind/branch。

use crate::backend_plan_views::{
    ExecutableBlockRef as MirBlockRef, ExecutableFunction as MirFunction, ExecutableInstructionKind as MirInstructionKind,
};

/// 后端模式匹配不变量校验失败时返回的错误。
///
/// 后端在 lowering 入口处调用 [`validate_pattern_matching_invariants`]；若 MIR 残留
/// 了高层 `PatternMatch` 指令，说明 MIR lowering 未完成，后端不得自行解释 pattern。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternMatchingContractError {
    /// MIR 中残留了 `MirInstructionKind::PatternMatch` 指令。
    ResidualPatternMatchInstruction {
        /// 残留指令所在 block 的 id。
        block: MirBlockRef,
    },
}

/// 校验后端模式匹配最小契约的不变量。
///
/// 该函数只做“语言语义已展开”的检查，不重新解释 pattern，也不复制语言侧的
/// case 链合法性判定。
pub fn validate_pattern_matching_invariants(function: &MirFunction) -> Result<(), PatternMatchingContractError> {
    for block in &function.blocks {
        for instruction in &block.instructions {
            if matches!(instruction.kind, MirInstructionKind::PatternMatch { .. }) {
                return Err(PatternMatchingContractError::ResidualPatternMatchInstruction { block: block.id });
            }
        }
    }
    Ok(())
}

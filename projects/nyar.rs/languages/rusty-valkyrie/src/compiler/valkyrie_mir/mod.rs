//! Valkyrie 自有的已验证 MIR 成功载荷。
//!
//! 本模块拥有 Valkyrie 的语义 IR：块、指令、操作、终结符、effect、M2 验证器
//! 与表示计划成功载荷。它由 Valkyrie 前端生产、验证并最终经语言 × 目标适配器
//! 映射到平台；Nyar 平台核心与该 MIR 无类型依赖。
//!
//! - `operation`：MIR 结构与合同错误。
//! - `validation`：M2 验证器与链接事实验证。
//! - `program`：`ValkyrieMirProgram` 与 `CompiledValkyrieProgram` 成功载荷。
//! - `text`：程序级文本载体表示判定。
//! - `pipeline`：单向编译流的阶段 trait 合同。

mod operation;
mod program;
mod text;
mod validation;

pub mod pipeline;

pub use operation::{
    ValkyrieMir, ValkyrieMirArrayInitialization, ValkyrieMirBlock, ValkyrieMirBlockId, ValkyrieMirCallee, ValkyrieMirConstant,
    ValkyrieMirEffectKind, ValkyrieMirError, ValkyrieMirFunction, ValkyrieMirInstruction, ValkyrieMirOperand, ValkyrieMirOperation,
    ValkyrieMirTerminator, ValkyrieMirWitness,
};
pub use program::{CompiledValkyrieProgram, CompiledValkyrieProgramError, ValkyrieMirProgram};
pub use text::declared_text_representation;
pub use validation::validate_named_residues;

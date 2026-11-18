//! Valkyrie 语言实现的消费门面。
//!
//! 类型模型、HIR/MIR、语义分析、诊断、驱动、目标适配与语言服务全部由 Boost
//! **`compiler-valkyrie`** 拥有；本模块只做原样再导出，保持 `crate::compiler::*` 路径不变。
//! 交付侧（Asgard / Legion）留在产品仓。

pub use compiler_valkyrie::compiler::*;

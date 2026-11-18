#![deny(missing_docs)]

//! 发射门面：AOT 降低、可执行模型、ABI 结构与各目标发射器全部由 Boost `compiler-emitter` 所有。
//!
//! 本 crate 只做原样再导出，不持有目标 lowering、字节编码或产物装配实现。

pub use compiler_emitter::*;

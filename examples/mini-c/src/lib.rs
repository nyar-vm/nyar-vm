//! Mini C 解释器
//!
//! 基于 Oaks (前端), Chomsky (优化), Gaia (后端) 和 Nyar VM (运行时) 架构实现。

pub mod frontend;
pub mod optimizer;
pub mod runtime;

pub use oak_c::{CLexer, CParser, CRoot};
pub use chomsky_uast::UastNode;
pub use gaia_jit::GaiaJit;

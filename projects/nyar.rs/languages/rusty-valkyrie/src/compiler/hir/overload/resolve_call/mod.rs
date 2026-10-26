//! 表达式/语句/模式级调用解析（不含 try_resolve_* 核心匹配逻辑）。

mod pattern_policy;
mod resolve;
mod validate;

pub(crate) use pattern_policy::pattern_extractor_receiver_type;
pub(crate) use resolve::bind_pattern_type;
pub use resolve::resolve_hir_calls;
pub use validate::validate_extractor_patterns;

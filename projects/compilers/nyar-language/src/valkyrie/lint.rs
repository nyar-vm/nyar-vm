//! 编辑器 / CLI 诊断入口：只做 HIR 级验证，不进入正式产物链。

use crate::valkyrie::{frontend::ParseError, hir::ValkyrieCompiler};

/// 对孤立 `.v` 源码做 HIR 级 lint。不进入正式产物链，也不是 Legion build 的替代入口。
pub fn lint_valkyrie_source(compiler: &ValkyrieCompiler, source: &str) -> Result<(), ParseError> {
    compiler.compile_source(source).map(|_| ())
}

/// 对孤立 `.vx` 源码做 HIR 级 lint。
pub fn lint_valkyrie_vx_source(compiler: &ValkyrieCompiler, source: &str) -> Result<(), ParseError> {
    compiler.compile_vx_source(source).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lint_accepts_minimal_function_without_entering_artifact_pipeline() {
        lint_valkyrie_source(&ValkyrieCompiler::default(), "micro identity(value: i32) -> i32 { return value }")
            .expect("HIR lint must not require resolver source groups");
    }
}

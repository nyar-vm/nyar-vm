use std::collections::BTreeSet;

use crate::compiler::semantic::closure::{DeclarationKey, SemanticClosureError};
use nyar::{CanonicalSpecification, CanonicalTarget};

use super::types::{PlannedSemanticSourceGroup, PlannerError, PlannerResult};
use crate::legion::workspace::source_snapshot::compile_source_snapshot;

/// 与 `legion build` 一致的模板展开架构键。
pub(crate) fn compile_arch_key(target: &CanonicalTarget) -> &'static str {
    if target.specification == CanonicalSpecification::Wasi { "wasi" } else { target.arch.as_str() }
}

/// 入口语义闭包内被引用的声明身份（不含 host provider 物化）。
///
/// 无 `[main]` / `[export]` 入口时返回 `None`：library / 仅 inventory 规划仍按旧行为为全部 contract 选 provider。
pub(crate) fn demanded_declaration_keys(groups: &[PlannedSemanticSourceGroup], arch: &str) -> PlannerResult<Option<BTreeSet<DeclarationKey>>> {
    let compiler_groups = compile_source_snapshot(groups).map_err(|error| PlannerError::CompileFailed { message: error.to_string() })?;
    match crate::compiler::semantic::closure::source_group_closure_with_host_bindings(&compiler_groups, &[], arch) {
        Ok(closure) => Ok(Some(closure.demanded_declarations)),
        Err(SemanticClosureError::MissingEntryDeclaration { .. }) => Ok(None),
        Err(error) => Err(PlannerError::CompileFailed { message: semantic_closure_message(&error) }),
    }
}

fn semantic_closure_message(error: &SemanticClosureError) -> String {
    error.to_string()
}

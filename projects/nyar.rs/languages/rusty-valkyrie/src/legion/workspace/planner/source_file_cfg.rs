//! 文件级 `[cfg(...)]` / `[when(...)]`：在 namespace 合并与 host inventory 之前剔除整文件。
//!
//! Oak 将文件前导条件挂在紧随其后的 `namespace` 声明的 `annotations` 上；
//! 仅当**首个**根项为分号式 namespace 且其 annotations **全部为** `cfg` / `when` 时视为文件级门控。

use std::{fs, path::PathBuf};

use nyar::CanonicalTarget;

use crate::compiler::syntax::oak::{ast::StatementNode, namespace_is_declaration_only, parse_source, ValkyrieRoot};

use super::target_cfg::cfg_predicates_match_target;
use super::types::{CfgPredicate, PlannedSemanticSourceGroup, PlannerError, PlannerResult};

/// 读取源码并判断该文件是否参与当前 target 的语义图。
pub(crate) fn source_file_active_for_target(source: &str, target: CanonicalTarget) -> bool {
    match parse_source(source) {
        Ok(root) => match file_level_cfg_predicates_from_root(&root) {
            None => true,
            Some(predicates) => cfg_predicates_match_target(&predicates, target),
        },
        Err(_) => false,
    }
}

/// 前导仅含 `cfg(...)` 时返回谓词列表；否则 `None`（文件无文件级门控）。
pub(crate) fn file_level_cfg_predicates(source: &str) -> Option<Vec<CfgPredicate>> {
    parse_source(source).ok().and_then(|root| file_level_cfg_predicates_from_root(&root))
}

pub(crate) fn file_level_cfg_predicates_from_root(root: &ValkyrieRoot) -> Option<Vec<CfgPredicate>> {
    let first = root.items.first()?;
    let namespace = match first {
        StatementNode::Namespace(namespace) => namespace,
        _ => return None,
    };
    if !namespace_is_declaration_only(namespace) {
        return None;
    }
    if namespace.annotations.is_empty() {
        return None;
    }
    let mut predicates = Vec::new();
    for attribute in &namespace.annotations {
        match attribute.name.name.as_str() {
            "cfg" | "when" => {
                if let Some(predicate) = CfgPredicate::from_compile_condition_attribute(attribute) {
                    predicates.push(predicate);
                }
            }
            _ => return None,
        }
    }
    if predicates.is_empty() { None } else { Some(predicates) }
}

pub(crate) fn filter_semantic_source_groups_for_target(
    groups: Vec<PlannedSemanticSourceGroup>,
    target: CanonicalTarget,
) -> PlannerResult<Vec<PlannedSemanticSourceGroup>> {
    groups
        .into_iter()
        .map(|mut group| {
            group.source_files = filter_paths_for_target(group.source_files, target)?;
            Ok(group)
        })
        .collect()
}

pub(crate) fn filter_source_files_for_target(paths: Vec<PathBuf>, target: CanonicalTarget) -> PlannerResult<Vec<PathBuf>> {
    filter_paths_for_target(paths, target)
}

fn filter_paths_for_target(paths: Vec<PathBuf>, target: CanonicalTarget) -> PlannerResult<Vec<PathBuf>> {
    let mut filtered = Vec::with_capacity(paths.len());
    for path in paths {
        let content = fs::read_to_string(&path).map_err(PlannerError::Io)?;
        if source_file_active_for_target(&content, target) {
            filtered.push(path);
        }
    }
    Ok(filtered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyar::CanonicalTarget;

    #[test]
    fn file_cfg_browser_file_inactive_on_wasi() {
        let source = r#"[cfg(target_vendor = "browser")]
namespace std.adaptor.wasm;

[host_provider(std::math::random_next_i32)]
[js("random")]
micro js_random_next_i32(seed: i32): i32
"#;
        assert!(source_file_active_for_target(source, CanonicalTarget::wasm()));
        assert!(!source_file_active_for_target(source, CanonicalTarget::wasi()));
    }

    #[test]
    fn file_without_leading_cfg_stays_active() {
        let source = r#"namespace std.adaptor.wasm;

[cfg(target_vendor = "browser")]
[host_provider(std::math::random_next_i32)]
[js("random")]
micro js_random_next_i32(seed: i32): i32
"#;
        assert!(file_level_cfg_predicates(source).is_none());
        assert!(source_file_active_for_target(source, CanonicalTarget::wasi()));
    }

    #[test]
    fn file_when_wasip3_inactive_on_browser_wasm() {
        let source = r#"[when(target.specification == "wasip3")]
namespace std.adaptor.wasi;
"#;
        let root = crate::compiler::syntax::oak::parse_source(source).expect("when file gate must parse");
        let predicates = file_level_cfg_predicates_from_root(&root).expect("file-level when");
        assert!(!cfg_predicates_match_target(&predicates, CanonicalTarget::wasm()));
        assert!(cfg_predicates_match_target(&predicates, CanonicalTarget::wasip3()));
        assert!(!source_file_active_for_target(source, CanonicalTarget::wasm()));
        assert!(source_file_active_for_target(source, CanonicalTarget::wasip3()));
    }

    #[test]
    fn unknown_cfg_predicate_makes_file_inactive() {
        let source = r#"[cfg(target_vendor = "browser")]
namespace x;
"#;
        assert!(!source_file_active_for_target(source, CanonicalTarget::wasi()));
        let bad = r#"[cfg(no_such_key = "x")]
namespace x;
"#;
        assert!(!source_file_active_for_target(bad, CanonicalTarget::wasm()));
    }
}

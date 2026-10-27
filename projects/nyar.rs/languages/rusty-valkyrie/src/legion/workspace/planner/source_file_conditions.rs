//! 文件级 `[gate(...)]` / `[when(...)]`：各自独立解析；合并规则不在本模块定义。
//!
//! Oak 将文件前导 attribute 挂在紧随其后的 `namespace …;` 的 `annotations` 上。
//! 仅当首个根项为分号式 namespace，且 annotations **仅含** `gate` / `when` 时视为文件级条件附着点。

use std::{collections::BTreeSet, fs, path::PathBuf};

use nyar::CanonicalTarget;

use crate::compiler::syntax::oak::{ast::StatementNode, namespace_is_declaration_only, parse_source, ValkyrieRoot};

use super::ast_attributes::{gate_attributes_from_items, when_attributes_from_items};
use super::condition_eval::{gate_axis_passes, target_axis_passes};
use super::types::{FileLevelConditionItems, PlannedSemanticSourceGroup, PlannerError, PlannerResult};

/// 读取源码并判断该文件是否参与当前 target + feature set 的语义图。
///
/// gate 轴：有 gate 声明时必须全部 Enabled（空 gate 恒通过）。
/// when 轴：0 条恒通过；恰好 1 条按 target 求值；2+ 条视为未规定语义 → 不通过。
pub(crate) fn source_file_active_for_target_and_features(
    source: &str,
    target: CanonicalTarget,
    enabled_features: Option<&BTreeSet<String>>,
) -> bool {
    match parse_source(source) {
        Ok(root) => match file_level_condition_items_from_root(&root) {
            None => true,
            Some(items) => {
                target_axis_passes(&items.whens, target) && gate_axis_passes(&items.gates, enabled_features)
            }
        },
        Err(_) => false,
    }
}

/// 兼容旧签名：不传 feature 时 gate 轴视为空 gate 恒通过。
#[allow(dead_code)]
pub(crate) fn source_file_active_for_target(source: &str, target: CanonicalTarget) -> bool {
    source_file_active_for_target_and_features(source, target, None)
}

pub(crate) fn file_level_condition_items(source: &str) -> Option<FileLevelConditionItems> {
    parse_source(source).ok().and_then(|root| file_level_condition_items_from_root(&root))
}

pub(crate) fn file_level_condition_items_from_root(root: &ValkyrieRoot) -> Option<FileLevelConditionItems> {
    let first = root.items.first()?;
    let namespace = match first {
        StatementNode::Namespace(namespace) => namespace,
        _ => return None,
    };
    if !namespace_is_declaration_only(namespace) || namespace.annotations.is_empty() {
        return None;
    }
    for attribute in &namespace.annotations {
        match attribute.name.name.as_str() {
            "gate" | "when" => {}
            _ => return None,
        }
    }
    let gates = gate_attributes_from_items(&namespace.annotations);
    let whens = when_attributes_from_items(&namespace.annotations);
    if gates.is_empty() && whens.is_empty() {
        return None;
    }
    Some(FileLevelConditionItems { gates, whens })
}

pub(crate) fn filter_semantic_source_groups_for_target(
    groups: Vec<PlannedSemanticSourceGroup>,
    target: CanonicalTarget,
    enabled_features: Option<&BTreeSet<String>>,
) -> PlannerResult<Vec<PlannedSemanticSourceGroup>> {
    groups
        .into_iter()
        .map(|mut group| {
            group.source_files = filter_paths_for_target(group.source_files, target, enabled_features)?;
            Ok(group)
        })
        .collect()
}

pub(crate) fn filter_source_files_for_target(
    paths: Vec<PathBuf>,
    target: CanonicalTarget,
    enabled_features: Option<&BTreeSet<String>>,
) -> PlannerResult<Vec<PathBuf>> {
    filter_paths_for_target(paths, target, enabled_features)
}

fn filter_paths_for_target(
    paths: Vec<PathBuf>,
    target: CanonicalTarget,
    enabled_features: Option<&BTreeSet<String>>,
) -> PlannerResult<Vec<PathBuf>> {
    let mut filtered = Vec::with_capacity(paths.len());
    for path in paths {
        let content = fs::read_to_string(&path).map_err(PlannerError::Io)?;
        if source_file_active_for_target_and_features(&content, target, enabled_features) {
            filtered.push(path);
        }
    }
    Ok(filtered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyar::CanonicalTarget;

    use super::super::target_when::when_item_matches_target;

    #[test]
    fn file_when_browser_file_inactive_on_wasi() {
        let source = r#"[when(target.vendor == "browser")]
namespace std.adaptor.wasm;

[host_provider(std::math::random_next_i32)]
[js("random")]
micro js_random_next_i32(seed: i32): i32
"#;
        assert!(source_file_active_for_target(source, CanonicalTarget::wasm()));
        assert!(!source_file_active_for_target(source, CanonicalTarget::wasi()));
    }

    #[test]
    fn file_without_leading_when_stays_active() {
        let source = r#"namespace std.adaptor.wasm;

[when(target.vendor == "browser")]
[host_provider(std::math::random_next_i32)]
[js("random")]
micro js_random_next_i32(seed: i32): i32
"#;
        assert!(file_level_condition_items(source).is_none());
        assert!(source_file_active_for_target(source, CanonicalTarget::wasi()));
    }

    #[test]
    fn file_when_wasip3_inactive_on_browser_wasm() {
        let source = r#"[when(target.specification == "wasip3")]
namespace std.adaptor.wasi;
"#;
        let root = crate::compiler::syntax::oak::parse_source(source).expect("when file gate must parse");
        let items = file_level_condition_items_from_root(&root).expect("file-level when");
        assert_eq!(items.whens.len(), 1);
        assert!(!when_item_matches_target(&items.whens[0], CanonicalTarget::wasm()));
        assert!(when_item_matches_target(&items.whens[0], CanonicalTarget::wasip3()));
        assert!(!source_file_active_for_target(source, CanonicalTarget::wasm()));
        assert!(source_file_active_for_target(source, CanonicalTarget::wasip3()));
    }

    #[test]
    fn multiple_file_when_items_do_not_implicit_and() {
        let source = r#"[when(target.vendor == "browser")]
[when(target.vendor == "wechat")]
namespace std.adaptor.wasm;
"#;
        assert!(!source_file_active_for_target(source, CanonicalTarget::wasm()));
    }

    #[test]
    fn gate_and_when_coexist_without_combining_at_parse_layer() {
        let source = r#"[gate(feature = "fetch")]
[when(target.vendor == "browser")]
namespace std.adaptor.wasm.fetch;
"#;
        let items = file_level_condition_items(source).expect("gate+when file");
        assert_eq!(items.gates.len(), 1);
        assert_eq!(items.gates[0].key, "feature");
        assert_eq!(items.gates[0].value, "fetch");
        assert_eq!(items.whens.len(), 1);
        assert!(source_file_active_for_target(source, CanonicalTarget::wasm()));
        assert!(!source_file_active_for_target(source, CanonicalTarget::wasi()));
    }

    #[test]
    fn unknown_when_predicate_makes_file_inactive() {
        let source = r#"[when(target.vendor == "browser")]
namespace x;
"#;
        assert!(!source_file_active_for_target(source, CanonicalTarget::wasi()));
        let bad = r#"[when(no_such_key = "x")]
namespace x;
"#;
        assert!(!source_file_active_for_target(bad, CanonicalTarget::wasm()));
    }

    #[test]
    fn valkyrie_corpus_fetch_when_files_track_target() {
        use std::path::PathBuf;

        let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../../valkyrie.v/projects/std.adaptors._/std.adaptor.wasm/source");
        if !corpus.is_dir() {
            eprintln!("skip: valkyrie.v adaptor source not beside nyar-vm.rs");
            return;
        }
        let shared = std::fs::read_to_string(corpus.join("fetch.v")).expect("fetch.v");
        assert!(source_file_active_for_target(&shared, CanonicalTarget::wasm()));
        let mp: CanonicalTarget = "wasm32-unknown-miniprogram-wasm".parse().unwrap();
        assert!(source_file_active_for_target(&shared, mp));

        let browser = std::fs::read_to_string(corpus.join("fetch.browser.v")).expect("fetch.browser.v");
        assert!(source_file_active_for_target(&browser, CanonicalTarget::wasm()));
        assert!(!source_file_active_for_target(&browser, mp));

        let wechat = std::fs::read_to_string(corpus.join("fetch.wechat.v")).expect("fetch.wechat.v");
        assert!(!source_file_active_for_target(&wechat, CanonicalTarget::wasm()));
        assert!(source_file_active_for_target(&wechat, mp));
    }
}

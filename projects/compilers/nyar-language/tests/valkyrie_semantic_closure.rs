use nyar_language::*;
use std::collections::BTreeSet;

fn plan_from_groups(groups: &[(&str, bool, Vec<(String, String)>)]) -> SemanticClosure {
    let refs = groups.iter().map(|(key, is_entry, files)| (*key, *is_entry, files.as_slice())).collect::<Vec<_>>();
    plan_semantic_closure(&refs)
}

fn file(path: &str, source: &str) -> (String, String) {
    (path.to_string(), source.to_string())
}

#[test]
fn unreachable_inventory_syntax_error_does_not_expand_into_body_paths() {
    let groups = [
        (
            "library",
            false,
            vec![file("source/good.v", "micro answer() -> i32 { return 23 }"), file("source/broken.v", "micro broken() { value = }")],
        ),
        ("app", true, vec![file("solution.v", "micro run() -> i32 { let value: i32 = answer(); return value }")]),
    ];
    let closure = plan_from_groups(&groups);
    assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["source/good.v".into()]));
    assert_eq!(closure.groups[1].body_lowering_source_paths, BTreeSet::from(["solution.v".into()]));
    assert!(!closure.digest.is_empty());
    assert!(closure.demanded_declarations.iter().any(|key| key.name == "answer"));
    // 无法建立索引的文件必须显式记录，供下游在声明缺失时 fail-closed。
    assert_eq!(closure.unindexed_source_files.len(), 1);
    assert!(closure.unindexed_source_files.contains_key("library::source/broken.v"));
}

#[test]
fn entry_group_keeps_all_files_in_body_paths() {
    let groups = [("app", true, vec![file("solution.v", "micro broken() { value = }")])];
    let closure = plan_from_groups(&groups);
    assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["solution.v".into()]));
}

#[test]
fn transitive_cross_package_symbol_pulls_base_dependency_file() {
    let groups = [
        ("base", false, vec![file("source/answer.v", "micro answer() -> i32 { return 23 }")]),
        ("library", false, vec![file("source/relay.v", "micro relay() -> i32 { return answer() }")]),
        ("app", true, vec![file("solution.v", "micro run() -> i32 { return relay() }")]),
    ];
    let closure = plan_from_groups(&groups);
    assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["source/answer.v".into()]));
    assert_eq!(closure.groups[1].body_lowering_source_paths, BTreeSet::from(["source/relay.v".into()]));
}

#[test]
fn cross_package_reachable_symbol_pulls_dependency_file_into_closure() {
    let groups = [
        (
            "library",
            false,
            vec![file("source/answer.v", "micro answer() -> i32 { return 23 }"), file("source/unused.v", "micro unused() -> i32 { return 0 }")],
        ),
        ("app", true, vec![file("solution.v", "micro run() -> i32 { return answer() }")]),
    ];
    let closure = plan_from_groups(&groups);
    assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["source/answer.v".into()]));
    assert!(!closure.groups[0].body_lowering_source_paths.contains("source/unused.v"));
}

#[test]
fn namespace_chain_does_not_pull_same_name_from_other_namespace() {
    let groups = [
        (
            "library",
            false,
            vec![
                file("source/a.v", "namespace a;\nmicro helper() -> i32 { return 1 }"),
                file("source/b.v", "namespace b;\nmicro helper() -> i32 { return 2 }"),
            ],
        ),
        ("app", true, vec![file("solution.v", "namespace a;\nmicro run() -> i32 { return helper() }")]),
    ];
    let closure = plan_from_groups(&groups);
    assert_eq!(closure.groups[0].body_lowering_source_paths, BTreeSet::from(["source/a.v".into()]));
}

#[test]
fn member_reference_resolves_within_namespace_before_global_fallback() {
    let groups = [
        (
            "library",
            false,
            vec![
                file("source/a.v", "namespace a;\nstructure S { _x: i32 }\nimply S { micro is_empty(self): bool { return true } }"),
                file("source/b.v", "namespace b;\nstructure T { _x: i32 }\nimply T { micro is_empty(self): bool { return true } }"),
            ],
        ),
        ("app", true, vec![file("solution.v", "namespace a;\nmicro run(s: S) -> bool { return s.is_empty() }")]),
    ];
    let closure = plan_from_groups(&groups);
    let lowered = &closure.groups[0].body_lowering_source_paths;
    assert!(lowered.contains("source/a.v"));
    assert!(!lowered.contains("source/b.v"));
}

#[test]
fn declared_member_pulls_owning_aggregate_shell() {
    let groups = [
        (
            "library",
            false,
            vec![file("source/hash.v", "namespace a;\nstructure S { _x: i32 }\nimply S { micro hash(self): usize { return 0 } }")],
        ),
        ("app", true, vec![file("solution.v", "namespace a;\nmicro run(s: S) -> usize { return s.hash() }")]),
    ];
    let closure = plan_from_groups(&groups);
    let demanded = &closure.groups[0].body_lowering_declarations;
    assert!(demanded.iter().any(|key| key.kind == DeclarationKind::Structure && key.name == "S"));
    assert!(demanded.iter().any(|key| key.name == "hash"));
}

#[test]
fn impl_block_shell_references_enter_closure_without_demanding_its_methods() {
    let groups = [
        (
            "library",
            false,
            vec![
                file("source/iterator/_.v", "namespace std.iterator;\ntrait Iterator { }"),
                file(
                    "source/impl.v",
                    "namespace std.collection;\nstructure S { _x: i32 }\nimply S: std::iterator::Iterator { micro not_used(self): bool { return true } }",
                ),
            ],
        ),
        ("app", true, vec![file("solution.v", "namespace std.collection;\nmicro run(s: S) -> bool { return true }")]),
    ];
    let closure = plan_from_groups(&groups);
    let lowered = &closure.groups[0].body_lowering_source_paths;
    assert!(lowered.contains("source/impl.v"));
    // impl 块外壳始终物化，其 trait 路径必须在闭包内可解析。
    assert!(lowered.contains("source/iterator/_.v"));
    // 未被需求的方法体不进入 body lowering。
    assert!(!closure.groups[0].body_lowering_declarations.iter().any(|key| key.name == "not_used"));
}

#[test]
fn closure_digest_is_stable_for_identical_input() {
    let groups = [("app", true, vec![file("solution.v", "micro run() -> i32 { return 23 }")])];
    let first = plan_from_groups(&groups);
    let second = plan_from_groups(&groups);
    assert_eq!(first.digest, second.digest);
    assert_eq!(first.declaration_index.len(), 1);
}

#[test]
fn closure_digest_binds_demanded_source_content() {
    let baseline = [("app", true, vec![file("solution.v", "micro run() -> i32 { return 23 }")])];
    let changed = [("app", true, vec![file("solution.v", "micro run() -> i32 { return 24 }")])];
    assert_ne!(plan_from_groups(&baseline).digest, plan_from_groups(&changed).digest);
}

#[test]
fn closure_digest_ignores_unreachable_dependency_body_change() {
    let groups = |body: &str| {
        [
            ("library", false, vec![file("source/answer.v", "micro answer() -> i32 { return 23 }"), file("source/unused.v", body)]),
            ("app", true, vec![file("solution.v", "micro run() -> i32 { return answer() }")]),
        ]
    };
    let first = plan_from_groups(&groups("micro unused() -> i32 { return 1 }"));
    let second = plan_from_groups(&groups("micro unused() -> i32 { return 2 }"));
    assert_eq!(first.digest, second.digest);
}

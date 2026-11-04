//! `ImplyChecker` 对 super trait 链成环的声明级检查。

use rusty_valkyrie::{
    compiler::semantic::type_checker::{ImplyChecker, ImplyErrorKind},
    types::hir::{HirModule, HirTrait, ValkyrieType as HirType},
    *,
};

fn name_path(parts: &[&str]) -> NamePath {
    NamePath::new(parts.iter().map(|part| Identifier::new(part)).collect())
}

fn unqualified(name: &str) -> HirType {
    HirType::Named(Identifier::new(name))
}

fn qualified(namespace: &[&str], name: &str) -> HirType {
    HirType::QualifiedPath { namespace: namespace.iter().map(|part| part.to_string()).collect(), name: Identifier::new(name) }
}

fn trait_with_super_traits(name: &str, namespace: &[&str], super_traits: Vec<HirType>) -> HirTrait {
    HirTrait {
        declaration: None,
        declaring_namespace: name_path(namespace),
        name: Identifier::new(name),
        doc: Default::default(),
        generics: Vec::new(),
        methods: Vec::new(),
        associated_types: Vec::new(),
        associated_constants: Vec::new(),
        super_traits,
        default_methods: Vec::new(),
        visibility: Default::default(),
    }
}

fn check(module: &HirModule) -> Vec<ImplyErrorKind> {
    let mut checker = ImplyChecker::new();
    checker.check_module(module).into_iter().map(|error| error.kind).collect()
}

#[test]
fn mutual_super_traits_report_single_closed_cycle() {
    let module = HirModule {
        traits: vec![
            trait_with_super_traits("A", &["test"], vec![unqualified("B")]),
            trait_with_super_traits("B", &["test"], vec![unqualified("A")]),
        ],
        ..HirModule::default()
    };
    let kinds = check(&module);

    assert_eq!(kinds.len(), 1, "成环声明应恰好报一条违规: {kinds:?}");
    match &kinds[0] {
        ImplyErrorKind::SuperTraitCycle { trait_name, cycle } => {
            assert_eq!(trait_name, &name_path(&["test", "A"]));
            assert_eq!(cycle, &vec![name_path(&["test", "A"]), name_path(&["test", "B"]), name_path(&["test", "A"])]);
        }
        other => panic!("期望 SuperTraitCycle，实得 {other:?}"),
    }
}

#[test]
fn acyclic_super_trait_chain_reports_no_cycle() {
    let module = HirModule {
        traits: vec![
            trait_with_super_traits("A", &["test"], vec![unqualified("B")]),
            trait_with_super_traits("B", &["test"], vec![unqualified("C")]),
        ],
        ..HirModule::default()
    };
    let kinds = check(&module);

    assert!(!kinds.iter().any(|kind| matches!(kind, ImplyErrorKind::SuperTraitCycle { .. })), "无环 DAG 不应报成环: {kinds:?}");
}

#[test]
fn qualified_cross_namespace_cycle_is_reported() {
    let module = HirModule {
        traits: vec![
            trait_with_super_traits("A", &["shared"], vec![qualified(&["shared"], "B")]),
            trait_with_super_traits("B", &["shared"], vec![qualified(&["shared"], "A")]),
        ],
        ..HirModule::default()
    };
    let kinds = check(&module);

    assert_eq!(kinds.len(), 1, "限定路径成环应恰好报一条违规: {kinds:?}");
    match &kinds[0] {
        ImplyErrorKind::SuperTraitCycle { trait_name, cycle } => {
            assert_eq!(trait_name, &name_path(&["shared", "A"]));
            assert_eq!(cycle, &vec![name_path(&["shared", "A"]), name_path(&["shared", "B"]), name_path(&["shared", "A"])]);
        }
        other => panic!("期望 SuperTraitCycle，实得 {other:?}"),
    }
}

#[test]
fn same_short_name_in_different_namespaces_is_not_merged() {
    // n1::B 被 n2::B 覆盖时（按短名建索引）会把 n2::B 的 `A` 误连到 n1::A，凭空造出环。
    let module = HirModule {
        traits: vec![
            trait_with_super_traits("A", &["n1"], vec![unqualified("B")]),
            trait_with_super_traits("B", &["n1"], Vec::new()),
            trait_with_super_traits("B", &["n2"], vec![unqualified("A")]),
        ],
        ..HirModule::default()
    };
    let kinds = check(&module);

    assert!(!kinds.iter().any(|kind| matches!(kind, ImplyErrorKind::SuperTraitCycle { .. })), "跨命名空间同名 trait 不应合并判等: {kinds:?}");
}

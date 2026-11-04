//! `ImplyChecker` 对 super trait 链成环的声明级检查。

use rusty_valkyrie::{
    compiler::semantic::type_checker::{ImplyChecker, ImplyErrorKind},
    types::hir::{HirModule, HirTrait, ValkyrieType as HirType},
    *,
};

fn trait_with_super_traits(name: &str, super_traits: &[&str]) -> HirTrait {
    HirTrait {
        declaration: None,
        declaring_namespace: NamePath::new(vec![Identifier::new("test")]),
        name: Identifier::new(name),
        doc: Default::default(),
        generics: Vec::new(),
        methods: Vec::new(),
        associated_types: Vec::new(),
        associated_constants: Vec::new(),
        super_traits: super_traits.iter().map(|super_name| HirType::Named(Identifier::new(super_name))).collect(),
        default_methods: Vec::new(),
        visibility: Default::default(),
    }
}

fn path(name: &str) -> NamePath {
    NamePath::new(vec![Identifier::new(name)])
}

#[test]
fn mutual_super_traits_report_single_closed_cycle() {
    let module = HirModule { traits: vec![trait_with_super_traits("A", &["B"]), trait_with_super_traits("B", &["A"])], ..HirModule::default() };
    let mut checker = ImplyChecker::new();
    let errors = checker.check_module(&module);

    assert_eq!(errors.len(), 1, "成环声明应恰好报一条违规: {errors:?}");
    match &errors[0].kind {
        ImplyErrorKind::SuperTraitCycle { trait_name, cycle } => {
            assert_eq!(trait_name, &path("A"));
            assert_eq!(cycle, &vec![path("A"), path("B"), path("A")]);
        }
        other => panic!("期望 SuperTraitCycle，实得 {other:?}"),
    }
}

#[test]
fn acyclic_super_trait_chain_reports_no_cycle() {
    let module = HirModule {
        traits: vec![trait_with_super_traits("A", &["B"]), trait_with_super_traits("B", &["C"])],
        ..HirModule::default()
    };
    let mut checker = ImplyChecker::new();
    let errors = checker.check_module(&module);

    assert!(
        errors.iter().all(|error| !matches!(error.kind, ImplyErrorKind::SuperTraitCycle { .. })),
        "无环 DAG 不应报成环: {errors:?}"
    );
}

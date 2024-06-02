use nyar_optimizer::*;
use nyar_types::{CapabilityTag, Identifier, QualifiedName};

fn qualified(parts: &[&str]) -> QualifiedName {
    QualifiedName::new(parts.iter().map(|part| Identifier::new(part)).collect())
}

#[test]
fn constant_fold_addition() {
    let session = OptimizationSession;
    let result = session.optimize(OptimizationRequest {
        program: ObjectAlgebraicProgram {
            module_name: qualified(&["demo"]),
            structured_terms: vec![parse_term("core.add(2, 3)").expect("term")],
            ..Default::default()
        },
        capabilities: Vec::new(),
        rewrite_theory: RewriteTheory::default(),
        projection_policy: ProjectionPolicy {
            family: FutamuraProjectionFamily::Clr,
            host_boundary: HostProjectionBoundary::Clr,
            reference_management: ReferenceManagement::HostGc,
            prefer_small_artifacts: false,
            preserve_effect_boundaries: true,
        },
    });

    assert!(result.egraph.constant_fold_count >= 1);
    assert!(result.egraph.saturated);
    assert_eq!(result.program.structured_terms, vec![AlgebraicTerm::Literal(5)]);
}

#[test]
fn identity_rewrite_eliminates_add_zero() {
    let session = OptimizationSession;
    let x = qualified(&["demo", "x"]);
    let result = session.optimize(OptimizationRequest {
        program: ObjectAlgebraicProgram {
            module_name: qualified(&["demo"]),
            structured_terms: vec![AlgebraicTerm::apply(
                qualified(&["core", "add"]),
                vec![AlgebraicTerm::symbol(x.clone()), AlgebraicTerm::literal(0)],
            )],
            ..Default::default()
        },
        capabilities: Vec::new(),
        rewrite_theory: RewriteTheory::default(),
        projection_policy: ProjectionPolicy {
            family: FutamuraProjectionFamily::Clr,
            host_boundary: HostProjectionBoundary::Clr,
            reference_management: ReferenceManagement::HostGc,
            prefer_small_artifacts: false,
            preserve_effect_boundaries: true,
        },
    });

    assert!(result.egraph.applied_rewrite_count >= 1 || result.egraph.constant_fold_count >= 1);
    assert_eq!(result.program.structured_terms, vec![AlgebraicTerm::Symbol(x)]);
}

#[test]
fn nested_constant_fold() {
    let session = OptimizationSession;
    let result = session.optimize(OptimizationRequest {
        program: ObjectAlgebraicProgram {
            module_name: qualified(&["demo"]),
            structured_terms: vec![parse_term("core.add(core.mul(2, 3), 4)").expect("term")],
            ..Default::default()
        },
        capabilities: Vec::new(),
        rewrite_theory: RewriteTheory::default(),
        projection_policy: ProjectionPolicy {
            family: FutamuraProjectionFamily::Clr,
            host_boundary: HostProjectionBoundary::Clr,
            reference_management: ReferenceManagement::HostGc,
            prefer_small_artifacts: false,
            preserve_effect_boundaries: true,
        },
    });

    assert_eq!(result.program.structured_terms, vec![AlgebraicTerm::Literal(10)]);
    assert!(result.egraph.saturated);
}

#[test]
fn flat_equation_union_still_works() {
    let mut rewrite_theory = RewriteTheory::default();
    rewrite_theory.equate(RewriteEquation {
        left: qualified(&["graphic", "dot"]),
        right: qualified(&["graphic", "dot_commutative"]),
        phase: RewritePhase::Saturate,
        required_capabilities: vec![CapabilityTag::new("gpu-shader")],
    });

    let session = OptimizationSession;
    let result = session.optimize(OptimizationRequest {
        program: ObjectAlgebraicProgram {
            module_name: qualified(&["demo"]),
            exports: vec![qualified(&["graphic", "dot"])],
            dimensions: vec![ObjectAlgebraicDimension {
                name: Identifier::new("graphic"),
                exported_operations: vec![qualified(&["graphic", "dot"])],
                required_capabilities: vec![CapabilityTag::new("gpu-shader")],
                reference_management_hint: None,
            }],
            ..Default::default()
        },
        capabilities: vec![CapabilityTag::new("gpu-shader")],
        rewrite_theory,
        projection_policy: ProjectionPolicy {
            family: FutamuraProjectionFamily::Gpu,
            host_boundary: HostProjectionBoundary::Native,
            reference_management: ReferenceManagement::PerceusRc,
            prefer_small_artifacts: false,
            preserve_effect_boundaries: true,
        },
    });

    assert!(result.egraph.applied_equation_count >= 1);
}

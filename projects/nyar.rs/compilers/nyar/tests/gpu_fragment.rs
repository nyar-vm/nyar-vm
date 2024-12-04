use nyar::{
    abstractions::BackendInputKind,
    backends::gpu::{GpuDxilBackend, GpuSpirvBackend},
    builtin_graphic_manifest, builtin_neural_manifest,
    packaging::TargetLane,
};

#[test]
fn rewrite_theory_manifest_graphic_matches_expected_rule_count() {
    let manifest = builtin_graphic_manifest();
    let theory = manifest.to_rewrite_theory();
    assert_eq!(manifest.rules.len(), 8);
    assert_eq!(manifest.fragment, "graphic");
    assert!(!theory.rules.is_empty());
    assert!(!theory.equations.is_empty());
}

#[test]
fn rewrite_theory_manifest_neural_matches_expected_rule_count() {
    let manifest = builtin_neural_manifest();
    let theory = manifest.to_rewrite_theory();
    assert_eq!(manifest.rules.len(), 5);
    assert_eq!(manifest.fragment, "neural");
    assert!(!theory.rules.is_empty());
}

#[test]
fn gpu_backend_placeholders_accept_spirv_and_dxil() {
    assert!(GpuSpirvBackend::accept(BackendInputKind::SpirvModule, TargetLane::Gpu));
    assert!(GpuDxilBackend::accept(BackendInputKind::DxilContainer, TargetLane::Gpu));
    assert!(!GpuSpirvBackend::accept(BackendInputKind::DxilContainer, TargetLane::Gpu));
}

use nyar::{
    abstractions::{BackendInputKind, BinaryTarget},
    backends::{CompilationOptions, gpu::{GpuDxilBackend, GpuSpirvBackend}},
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

/// 占位后端不得把「没有实现」伪装成「编译成功且无产物」。
#[test]
fn gpu_placeholders_do_not_claim_compile_success() {
    let options = CompilationOptions {
        target: BinaryTarget::clr(),
        artifact_name: "gpu_probe".to_string(),
        emit_debug_symbols: false,
        optimize: false,
    };
    let spirv = GpuSpirvBackend.compile_placeholder(&options).expect_err("SPIR-V placeholder 必须失败闭合");
    assert!(spirv.contains("不得宣称编译成功"), "{spirv}");
    let dxil = GpuDxilBackend.compile_placeholder(&options).expect_err("DXIL placeholder 必须失败闭合");
    assert!(dxil.contains("不得宣称编译成功"), "{dxil}");
}

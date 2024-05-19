use nyar::{HostProjectionBoundary, TargetBackendFamily};
use nyar_emitter::*;

#[test]
fn frozen_families_have_neither_capability_nor_lowering() {
    for (family, boundary) in [
        (TargetBackendFamily::Clr, HostProjectionBoundary::Clr),
        (TargetBackendFamily::Jvm, HostProjectionBoundary::Jvm),
        (TargetBackendFamily::Native, HostProjectionBoundary::Native),
    ] {
        assert!(nyar_emitter::bundled_backend_capability_descriptor(family).is_none());
        let result = nyar_emitter::lowering::lower_fragment_to_driver_input(
            &nyar_emitter::FragmentSubmission::default(),
            family,
            boundary,
            std::path::PathBuf::new(),
            "",
            nyar_emitter::nyar_backend_wasi::WasmPackageKind::Binary,
        );
        let error = match result {
            Ok(_) => panic!("冻结目标不得存在正式 lowering"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("拒绝切换目标或补造产物"), "{error}");
    }
    assert!(driver_compiler_names().iter().all(|name| matches!(*name, "wasm-binary" | "nyar-vm")));
}

#[cfg(not(feature = "nyar-vm-lane"))]
#[test]
fn disabled_vm_does_not_advertise_capability() {
    assert!(nyar_emitter::bundled_backend_capability_descriptor(TargetBackendFamily::NyarVm).is_none());
}

use std::path::PathBuf;

use nyar::{BackendInputKind, HostProjectionBoundary, TargetBackendFamily, TargetFamily, TargetLane};
use nyar_emitter::{
    FragmentSubmission, bundled_backend_capability_descriptor, driver_compiler_names, lowering::lower_fragment_to_driver_input,
    nyar_backend_wasi::WasmPackageKind,
};

#[test]
fn jvm_family_advertises_capability_and_driver_compiler() {
    let descriptor = bundled_backend_capability_descriptor(TargetBackendFamily::Jvm).expect("jvm capability");
    assert_eq!(descriptor.backend_name, "jvm-jar");
    assert_eq!(descriptor.interpreter, "jvm.classfile");
    assert_eq!(descriptor.lane, TargetLane::Jvm);
    assert_eq!(descriptor.input_kind, Some(BackendInputKind::JvmClassFile));
    assert_eq!(descriptor.target_family, TargetFamily::Jvm);
    assert!(descriptor.supported_host_boundaries.contains(&HostProjectionBoundary::Jvm));
    assert!(driver_compiler_names().contains(&"jvm-jar"), "{:?}", driver_compiler_names());
}

#[test]
fn frozen_families_have_neither_capability_nor_lowering() {
    for (family, boundary) in [(TargetBackendFamily::Clr, HostProjectionBoundary::Clr), (TargetBackendFamily::Native, HostProjectionBoundary::Native)] {
        assert!(bundled_backend_capability_descriptor(family).is_none());
        let result =
            lower_fragment_to_driver_input(&FragmentSubmission::default(), family, boundary, PathBuf::new(), "", WasmPackageKind::Binary);
        let error = match result {
            Ok(_) => panic!("冻结目标不得存在正式 lowering"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("拒绝切换目标或补造产物"), "{error}");
    }
}

#[test]
fn jvm_lowering_rejects_fragment_without_executable_operations() {
    let result = lower_fragment_to_driver_input(
        &FragmentSubmission::default(),
        TargetBackendFamily::Jvm,
        HostProjectionBoundary::Jvm,
        PathBuf::new(),
        "",
        WasmPackageKind::Binary,
    );
    let error = result.expect_err("empty fragment must not produce a class");
    assert!(error.to_string().contains("没有可导出的可执行函数"), "{error}");
}

#[cfg(not(feature = "ovm-lane"))]
#[test]
fn disabled_vm_does_not_advertise_capability() {
    assert!(bundled_backend_capability_descriptor(TargetBackendFamily::Ovm).is_none());
}

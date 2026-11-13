use nyar::abstractions::{
    BinaryArch, BinaryFlavor, BinaryTarget, CanonicalAbi, CanonicalArch, CanonicalSpecification, CanonicalTarget, CanonicalVendor, TargetFamily,
};

#[test]
fn parses_short_aliases() {
    assert_eq!(CanonicalTarget::parse("clr").unwrap().to_string(), "clr-microsoft-unknown-managed");
    assert_eq!(CanonicalTarget::parse("jvm").unwrap().to_string(), "jvm-openjdk-unknown-managed");
    assert_eq!(CanonicalTarget::parse("wasm").unwrap().to_string(), "wasm32-unknown-unknown-wasm");
    assert_eq!(CanonicalTarget::parse("browser").unwrap().to_string(), "wasm32-unknown-browser-wasm");
    assert_eq!(CanonicalTarget::parse("ovm").unwrap().to_string(), "ovm-unknown-unknown-managed");
    assert_eq!(CanonicalTarget::parse("odin-vm").unwrap().to_string(), "ovm-unknown-unknown-managed");
    assert!(CanonicalTarget::parse("nyar").is_err(), "the legacy `nyar` alias must not resolve");
    assert!(CanonicalTarget::parse("nyar-vm").is_err(), "the legacy `nyar-vm` alias must not resolve");
    assert!(CanonicalTarget::parse("nyarvm").is_err(), "the legacy `nyarvm` alias must not resolve");
}

#[test]
fn parses_full_canonical_targets() {
    let target = CanonicalTarget::parse("x86_64-pc-windows-msvc").unwrap();
    assert_eq!(target.arch, CanonicalArch::X86_64);
    assert_eq!(target.vendor, CanonicalVendor::Pc);
    assert_eq!(target.specification, CanonicalSpecification::Windows);
    assert_eq!(target.abi, Some(CanonicalAbi::Msvc));
}

#[test]
#[cfg(target_os = "windows")]
fn native_alias_resolves_to_host_canonical_target() {
    let target = CanonicalTarget::parse("native").unwrap();
    assert_eq!(target.to_string(), CanonicalTarget::parse("x86_64-pc-windows-msvc").unwrap().to_string());
}

#[test]
fn projects_to_binary_target() {
    let binary = CanonicalTarget::parse("clr").unwrap().to_binary_target();
    assert_eq!(binary.family, TargetFamily::Clr);
    assert_eq!(binary.arch, BinaryArch::Any);
    assert_eq!(binary.flavor, BinaryFlavor::ManagedClr);
}

/// 规范四元组的 OS 规格与 ABI 必须贯穿到产物目标，不能只留下 `arch`。
#[test]
fn binary_target_preserves_specification_and_abi() {
    let target = CanonicalTarget::parse("x86_64-pc-windows-msvc").unwrap();
    let binary = target.to_binary_target();
    assert_eq!(binary.specification, Some(CanonicalSpecification::Windows));
    assert_eq!(binary.abi, Some(CanonicalAbi::Msvc));
    assert_eq!(BinaryTarget::from(target), binary, "`From` 与 `to_binary_target` 必须同源");

    let linux = CanonicalTarget::parse("x86_64-unknown-linux-gnu").unwrap().to_binary_target();
    assert_eq!(linux.specification, Some(CanonicalSpecification::Linux));
    assert_eq!(linux.abi, Some(CanonicalAbi::Gnu));
    assert!(!linux.matches(&binary), "OS/ABI 不同的目标不得互相匹配");
}

/// 未细化规格的后端声明只约束家族/架构/口味，不被请求目标的 OS/ABI 误判。
#[test]
fn coarse_backend_target_matches_refined_request() {
    let coarse = BinaryTarget::new(TargetFamily::Native, BinaryArch::X64, BinaryFlavor::Native);
    let refined = CanonicalTarget::parse("x86_64-pc-windows-msvc").unwrap().to_binary_target();
    assert!(coarse.matches(&refined));

    let mut declared = coarse.clone();
    declared.specification = Some(CanonicalSpecification::Linux);
    assert!(!declared.matches(&refined), "后端显式声明的 OS 必须参与约束");
}

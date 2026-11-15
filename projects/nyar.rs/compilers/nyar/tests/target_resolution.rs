//! 作者面目标占位符（`native` / `node` / `python`）的统一解析：平台身份与宿主合同正交。

use nyar::{ArtifactContract, CanonicalAbi, CanonicalTarget, CanonicalTargetParseError, TargetRequest, TargetResolver};

fn resolver() -> TargetResolver {
    TargetResolver::for_build_host().expect("build host must resolve")
}

#[test]
fn placeholders_share_platform_but_keep_distinct_contracts() {
    let resolver = resolver();
    let native = resolver.resolve_label("native").expect("native");
    let node = resolver.resolve_label("node").expect("node");
    let python = resolver.resolve_label("python").expect("python");

    assert_eq!(native.platform, resolver.host());
    assert_eq!(node.platform, native.platform, "同一宿主上三个占位符得到同一平台身份");
    assert_eq!(python.platform, native.platform);

    assert_eq!(native.contract, ArtifactContract::Native);
    assert_eq!(node.contract, ArtifactContract::NodeApi);
    assert!(matches!(python.contract, ArtifactContract::Cpython(_)));

    assert_ne!(native.identity(), node.identity(), "同一平台的普通产物与 Node-API 扩展身份必须可区分");
    assert_ne!(node.identity(), python.identity());
    assert_eq!(node.platform.abi, Some(resolver.host().resolved_abi()), "解析结果必须带冻结 ABI");
}

#[test]
fn explicit_platform_overrides_host_probe() {
    let resolver = TargetResolver::with_host(CanonicalTarget::parse("aarch64-apple-macos").expect("apple host"));
    let windows = CanonicalTarget::parse("x86_64-pc-windows-msvc").expect("windows quad");

    let node = resolver.resolve_with_platform(TargetRequest::Node, windows).expect("node on windows");
    assert_eq!(node.platform, windows, "显式目标必须覆盖本机探测");
    assert_eq!(node.contract, ArtifactContract::NodeApi);
    assert_ne!(node.platform, resolver.host());

    let python = resolver.resolve_with_platform(TargetRequest::Python, windows).expect("cpython on windows");
    assert_eq!(python.platform, windows);
    assert!(matches!(python.contract, ArtifactContract::Cpython(_)));
}

#[test]
fn placeholder_rejects_non_machine_platforms() {
    let resolver = resolver();
    let wasm = CanonicalTarget::parse("wasm").expect("wasm");

    let error = resolver.resolve_with_platform(TargetRequest::Node, wasm).expect_err("node cannot expand to wasm");
    assert!(error.to_string().contains("node"), "{error}");
    let error = resolver.resolve_with_platform(TargetRequest::Python, wasm).expect_err("python cannot expand to wasm");
    assert!(error.to_string().contains("python"), "{error}");
}

#[test]
fn explicit_platform_freezes_abi_once() {
    let resolver = resolver();
    let target = resolver.resolve_label("x86_64-pc-windows").expect("triple without abi");
    assert_eq!(target.platform.abi, Some(CanonicalAbi::Msvc));
    assert_eq!(target.contract, ArtifactContract::Native);

    let arm_windows = resolver.resolve_label("aarch64-pc-windows").expect("arm64 windows triple");
    assert_eq!(arm_windows.platform.abi, Some(CanonicalAbi::Msvc), "Windows ARM64 也使用 MSVC ABI");
}

#[test]
fn platform_lanes_keep_their_own_artifact_contract() {
    let resolver = resolver();
    for label in ["wasm", "clr", "jvm", "ovm", "wasm-node"] {
        let resolved = resolver.resolve_label(label).unwrap_or_else(|error| panic!("`{label}`: {error}"));
        assert_eq!(resolved.contract, ArtifactContract::Platform, "`{label}` 不应被包装成原生宿主产物");
    }
}

#[test]
fn node_alias_only_resolves_as_a_placeholder() {
    assert_eq!(TargetRequest::parse("node").expect("node placeholder"), TargetRequest::Node);
    assert_eq!(TargetRequest::parse("python").expect("python placeholder"), TargetRequest::Python);
    assert_eq!(TargetRequest::parse("native").expect("native placeholder"), TargetRequest::Native);

    let parsed = TargetRequest::parse("wasm-node").expect("explicit wasm host");
    assert!(matches!(parsed, TargetRequest::Platform(target) if target == CanonicalTarget::parse("wasm-node").unwrap()));

    let error: CanonicalTargetParseError = "node".parse::<CanonicalTarget>().expect_err("node is not a platform identity");
    assert!(error.to_string().contains("TargetRequest"), "{error}");
}

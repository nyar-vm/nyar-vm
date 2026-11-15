//! 作者面目标占位符（`native` / `node` / `python`）的统一解析：平台身份与宿主合同正交。

use nyar::{
    ArtifactContract, CanonicalAbi, CanonicalSpecification, CanonicalTarget, CanonicalTargetParseError, NodeHostProbe, PythonHostProbe,
    TargetRequest, TargetResolver,
};

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

#[test]
fn node_host_probe_maps_process_tokens() {
    let windows = NodeHostProbe::from_process_tokens("win32", "x64").expect("windows node");
    assert_eq!(windows.platform.to_string(), "x86_64-pc-windows-msvc");

    let apple = NodeHostProbe::from_process_tokens("darwin", "arm64").expect("apple node");
    assert_eq!(apple.platform.arch, nyar::CanonicalArch::AArch64);
    assert_eq!(apple.platform.specification, CanonicalSpecification::MacOs);
    assert_eq!(apple.platform.abi, Some(CanonicalAbi::Aapcs64));

    let linux = NodeHostProbe::from_process_tokens("linux", "x64").expect("linux node");
    assert_eq!(linux.platform.to_string(), "x86_64-unknown-linux-gnu");

    assert!(NodeHostProbe::from_process_tokens("sunos", "x64").is_err());
    assert!(NodeHostProbe::from_process_tokens("linux", "sparc").is_err());
}

#[test]
fn python_host_probe_requires_host_supplied_config() {
    let linux = PythonHostProbe::from_tokens("linux", "x86_64", ".cpython-312-x86_64-linux-gnu.so", None).expect("linux python");
    assert_eq!(linux.platform.specification, CanonicalSpecification::Linux);
    assert_eq!(linux.config.module_suffix.as_deref(), Some(".cpython-312-x86_64-linux-gnu.so"));
    assert_eq!(linux.config.import_library, None);

    let windows = PythonHostProbe::from_tokens("win32", "AMD64", ".cp312-win_amd64.pyd", Some("python312.dll")).expect("windows python");
    assert_eq!(windows.platform.specification, CanonicalSpecification::Windows);
    assert_eq!(windows.config.import_library.as_deref(), Some("python312.dll"));

    assert!(
        PythonHostProbe::from_tokens("win32", "AMD64", ".cp312-win_amd64.pyd", None).is_err(),
        "Windows 导入库名不能由编译器猜"
    );
    assert!(
        PythonHostProbe::from_tokens("linux", "x86_64", "", None).is_err(),
        "扩展后缀不能只凭 OS 推导"
    );
}

#[test]
fn probed_resolution_uses_the_real_host_process() {
    let resolver = resolver();
    match NodeHostProbe::probe() {
        Ok(probe) => {
            let resolved = resolver.resolve_label_probed("node").expect("probed node");
            assert_eq!(resolved.platform, probe.platform, "`node` 必须展开为真实 Node 进程平台");
            assert_eq!(resolved.contract, ArtifactContract::NodeApi);
        }
        Err(error) => eprintln!("skip: node 不可用（{error}）"),
    }
    match PythonHostProbe::probe() {
        Ok(probe) => {
            let resolved = resolver.resolve_label_probed("python").expect("probed python");
            assert_eq!(resolved.platform, probe.platform, "`python` 必须展开为真实解释器平台");
            assert!(matches!(resolved.contract, ArtifactContract::Cpython(_)));
        }
        Err(error) => eprintln!("skip: python 不可用（{error}）"),
    }
}

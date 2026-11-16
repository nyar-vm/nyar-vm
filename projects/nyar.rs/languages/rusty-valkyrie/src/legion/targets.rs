//! 作者面目标标签的统一解析入口。
//!
//! `native` / `node` / `python` 是占位符，必须经 [`nyar::TargetResolver`] 展开成
//! 平台身份与宿主合同；Legion 的构建、检查与测试入口都从这里取解析结果，不各自
//! 再写一次平台猜测。

use crate::legion::{
    ArtifactContract, CanonicalTarget, ResolvedTarget, TargetResolver,
    cmds::error::{LegionError, LegionResult},
};

/// 把作者面目标标签（占位符、三元组或四元组）解析为冻结目标。
pub fn resolve_authoring_target(label: &str) -> LegionResult<ResolvedTarget> {
    let resolver = TargetResolver::for_build_host().map_err(|error| LegionError::msg(format!("无法探测构建宿主：{error}")))?;
    resolver.resolve_label(label).map_err(|error| LegionError::msg(format!("无法解析目标 `{label}`：{error}")))
}

/// 解析目标标签为冻结目标（平台身份 + 产物合同）。
///
/// `native` / `node` / `python` 占位符在这里展开；平台 lane 与宿主合同保持
/// 正交，构建 lane 按合同推导产物用途，测试 lane 等无法装载宿主扩展的入口
/// 用 [`resolve_platform_only_target`] 明确拒绝。
pub fn resolve_platform_target(label: &str) -> LegionResult<ResolvedTarget> {
    resolve_authoring_target(label)
}

/// 解析目标标签，只接受平台/原生合同，返回完整 `ResolvedTarget`。
///
/// Node-API / CPython 扩展需要宿主装载（`require` / `import`），测试运行器
/// 没有这条装载链；在这里明确失败，不退回普通原生可执行文件。
pub fn resolve_platform_only_target(label: &str) -> LegionResult<ResolvedTarget> {
    let resolved = resolve_authoring_target(label)?;
    match &resolved.contract {
        ArtifactContract::Platform | ArtifactContract::Native => Ok(resolved),
        contract @ (ArtifactContract::NodeApi | ArtifactContract::Cpython(_)) => Err(LegionError::msg(format!(
            "目标 `{label}` 解析为 `{}` 宿主合同（`{}`），测试运行器没有宿主装载链；\
             构建宿主扩展请使用 `legion build --target {label}`，\
             Node 承载的 core Wasm 请使用 `wasm-node`",
            contract.as_str(),
            resolved.identity()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_contract_placeholders_resolve_with_platform_identity() {
        // `node` / `python` 是占位符：解析结果同时携带宿主合同与平台身份。
        let resolved = resolve_platform_target("node").expect("`node` 应解析为宿主合同目标");
        assert_eq!(resolved.contract, ArtifactContract::NodeApi);
        let python = resolve_platform_target("python").expect("`python` 应解析为宿主合同目标");
        assert!(matches!(python.contract, ArtifactContract::Cpython(_)));
        assert_ne!(python.identity(), resolved.identity(), "同一平台上的不同宿主合同必须可区分");

        assert!(resolve_platform_target("wasm-node").is_ok());
        assert!(resolve_platform_target("native").is_ok());
        assert!(resolve_platform_target("x86_64-pc-windows-msvc").is_ok());
    }

    #[test]
    fn platform_only_lane_rejects_host_contract() {
        let error = resolve_platform_only_target("node").expect_err("`node` 是宿主合同，不是平台 lane");
        assert!(error.to_string().contains("宿主装载"), "错误必须说明测试 lane 无宿主装载链: {error}");
        assert!(error.to_string().contains("wasm-node"), "错误必须给出可用的替代写法: {error}");

        assert!(resolve_platform_only_target("wasm-node").is_ok());
        assert!(resolve_platform_only_target("native").is_ok());
    }

    #[test]
    fn product_alias_resolves_through_the_shared_service() {
        let resolved = resolve_platform_target("ovm").expect("ovm");
        assert_eq!(resolved.platform, nyar::CanonicalTarget::parse("ovm").expect("canonical ovm"));
    }

    #[test]
    fn platform_only_target_keeps_contract_and_platform_together() {
        // 平台 lane 的解析结果仍是完整 `ResolvedTarget`：合同不在这个接口丢失。
        let resolved = resolve_platform_only_target("native").expect("native 是平台/原生合同");
        assert_eq!(resolved.contract, ArtifactContract::Native);
        let wasm = resolve_platform_only_target("wasm-node").expect("wasm-node");
        assert_eq!(wasm.contract, ArtifactContract::Platform);
    }
}

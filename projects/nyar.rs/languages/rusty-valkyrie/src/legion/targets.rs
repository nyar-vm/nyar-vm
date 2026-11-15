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

/// 解析目标标签，并只接受当前编译 lane 能承载的合同。
///
/// Node-API / CPython 扩展需要宿主入口、值转换与生命周期包装，属于独立的产物
/// lane；未接入时在这里明确失败，不退回普通原生可执行文件。
pub fn resolve_platform_target(label: &str) -> LegionResult<CanonicalTarget> {
    let resolved = resolve_authoring_target(label)?;
    match &resolved.contract {
        ArtifactContract::Platform | ArtifactContract::Native => Ok(resolved.platform),
        contract @ (ArtifactContract::NodeApi | ArtifactContract::Cpython(_)) => Err(LegionError::msg(format!(
            "目标 `{label}` 解析为 `{}` 宿主合同（`{}`），当前 Legion 构建 lane 尚未接入该产物；\
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
    fn platform_lanes_reject_host_contract_placeholders() {
        let error = resolve_platform_target("node").expect_err("`node` 是宿主合同，不是平台 lane");
        assert!(error.to_string().contains("未接入"), "unexpected error: {error}");
        assert!(error.to_string().contains("wasm-node"), "错误必须给出可用的替代写法: {error}");

        assert!(resolve_platform_target("wasm-node").is_ok());
        assert!(resolve_platform_target("native").is_ok());
        assert!(resolve_platform_target("x86_64-pc-windows-msvc").is_ok());
    }

    #[test]
    fn product_alias_resolves_through_the_shared_service() {
        let target = resolve_platform_target("ovm").expect("ovm");
        assert_eq!(target, nyar::CanonicalTarget::parse("ovm").expect("canonical ovm"));
    }
}

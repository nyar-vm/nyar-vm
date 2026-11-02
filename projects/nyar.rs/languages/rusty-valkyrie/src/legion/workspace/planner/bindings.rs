use crate::compiler::emitter::pipeline::{CompilerGatedOutHostProvider, CompilerHostProviderBinding};

use super::types::{PlannedHostProvider, PlannedHostProviderExclusion, PlannerError, PlannerResult};

/// 将 Resolver 选定的 host provider 计划项转为 Compiler 构建上下文绑定。
pub(crate) fn compiler_host_provider_bindings(providers: &[PlannedHostProvider]) -> PlannerResult<Vec<CompilerHostProviderBinding>> {
    providers
        .iter()
        .map(|provider| {
            let contract = provider.contract_key.clone().ok_or_else(|| PlannerError::UnknownHostProviderContract {
                provider: provider.symbol.clone(),
                contract: provider.contract.clone(),
                source_file: provider.source_file.clone(),
                line: provider.line,
            })?;
            Ok(CompilerHostProviderBinding {
                contract,
                provider: provider.key.clone(),
                required_capabilities: provider.required_capabilities.clone(),
                foreign_bindings: provider.foreign_bindings.clone(),
            })
        })
        .collect()
}

/// 将被 feature gate 排除的 provider 转为编译期诊断事实（合同名 / provider 名 / feature）。
pub(crate) fn compiler_gated_out_host_providers(exclusions: &[PlannedHostProviderExclusion]) -> Vec<CompilerGatedOutHostProvider> {
    exclusions
        .iter()
        .map(|exclusion| CompilerGatedOutHostProvider {
            contract: super::host::normalized_contract_reference(&exclusion.provider.contract),
            provider: exclusion.provider.symbol.clone(),
            feature: exclusion.feature.clone(),
        })
        .collect()
}

use crate::compiler::emitter::pipeline::CompilerHostProviderBinding;

use super::types::{PlannedHostProvider, PlannerError, PlannerResult};

/// 将 Resolver 选定的 host provider 计划项转为 Compiler 构建上下文绑定。
pub(crate) fn compiler_host_provider_bindings(providers: &[PlannedHostProvider]) -> PlannerResult<Vec<CompilerHostProviderBinding>> {
    providers
        .iter()
        .map(|provider| {
            let contract = provider
                .contract_key
                .clone()
                .ok_or_else(|| PlannerError::UnknownHostProviderContract {
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

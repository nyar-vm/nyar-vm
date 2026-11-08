use crate::{DriverBackendInput, DriverCompileReport, DriverCompileRequest, DriverLibraryExport, DriverRunContract, Result, bundled_backend_capability_descriptor, nyar_backend_clr::ClrBinaryBackend};
use nyar::{
    BackendInputKind, HostProjectionBoundary, PartitionBackendRequirement, TargetBackendFamily, TargetFamily, TargetLane,
    backends::TargetCodeGenBackend,
};

use super::BundledBackendCompiler;

/// `CLR` 家族后端编译器：唯一宿主边界是 [`HostProjectionBoundary::Clr`]。
pub(super) struct ClrFamilyCompiler;

/// 判断给定的后端需求是否可由 `CLR` 家族满足。
pub(super) fn supports_requirement(requirement: &PartitionBackendRequirement) -> bool {
    bundled_backend_capability_descriptor(TargetBackendFamily::Clr).is_some_and(|descriptor| descriptor.supports_requirement(requirement))
}

impl BundledBackendCompiler for ClrFamilyCompiler {
    fn compile(&self, request: DriverCompileRequest<'_>) -> Result<DriverCompileReport> {
        let DriverBackendInput::Clr(mut input) = request.input
        else {
            return Err(format!("`CLR` 家族请求必须携带 `ClrBinaryBackendInput`"));
        };
        let requirement = &request.requirement;
        if requirement.lane != TargetLane::Clr {
            return Err(format!("`CLR` 后端需求 lane 不一致：requirement={:?}", requirement.lane));
        }
        if requirement.input_kind != BackendInputKind::MsilText {
            return Err(format!("`CLR` 后端需求输入种类不一致：requirement={:?}", requirement.input_kind));
        }
        if requirement.target.family != TargetFamily::Clr {
            return Err(format!("`CLR` 后端需求目标家族不一致：requirement={:?}", requirement.target.family));
        }
        if requirement.host_boundary != HostProjectionBoundary::Clr {
            return Err(format!("`CLR` 后端输入宿主与规划需求不一致：requirement={:?}", requirement.host_boundary));
        }
        let has_entry_point = input.module.global_methods.iter().any(|method| method.is_entry_point);
        let provided_contracts = std::mem::take(&mut input.run_contracts);
        let mut managed_abi = input.managed_abi.take();
        // 库导出面自带合成入口点，产物必须保持 `.dll`；无导出时按入口点推断镜像口味。
        if managed_abi.is_some() && input.image_kind.is_none() {
            input.image_kind = Some(nyar::backends::clr::ClrImageKind::DynamicLibrary);
        }
        let image_kind = input.image_kind.unwrap_or_else(|| nyar::backends::clr::ClrImageKind::infer(has_entry_point));
        let artifact_file = format!("{}.{}", request.artifact_name, image_kind.file_extension());
        if let Some(contract) = managed_abi.as_mut() {
            // 产物文件名由驱动层决定；契约摘要必须覆盖最终文件名。
            contract.set_artifact(artifact_file.clone());
        }
        let backend = ClrBinaryBackend::new();
        backend.validate(&input)?;
        let artifacts = backend.compile(input, request.options)?;
        // 库产物必须有导出面：缺失 managed ABI 入口时结构化失败，不得回退 `main`
        // 或静默返回空运行契约。
        if request.library_mode {
            match managed_abi.as_ref() {
                None => return Err("`artifact: library` 的 `clr` 产物缺少 managed ABI 契约；拒绝回退 `main`".to_string()),
                Some(contract) if contract.entries.is_empty() => {
                    return Err("`artifact: library` 的 `clr` managed ABI 契约为空；拒绝产出空运行契约".to_string());
                }
                Some(_) => {}
            }
        }
        // managed ABI 入口逐个提供 argv 分发合同；无导出时才回退到单一 `main` 入口。
        let run_contracts = if !provided_contracts.is_empty() {
            provided_contracts
        }
        else if let Some(contract) = &managed_abi {
            contract
                .entries
                .iter()
                .map(|entry| DriverRunContract {
                    logical_entry: entry.logical_entry.clone(),
                    physical_entry: artifact_file.clone(),
                    invocation: "dotnet".to_string(),
                    validate: format!("dotnet exec {} {}", artifact_file, entry.logical_entry),
                })
                .collect()
        }
        else if has_entry_point {
            vec![DriverRunContract {
                logical_entry: "main".to_string(),
                physical_entry: artifact_file.clone(),
                invocation: "dotnet".to_string(),
                validate: format!("dotnet exec {}", artifact_file),
            }]
        }
        else {
            Vec::new()
        };
        let library_exports = if request.library_mode {
            managed_abi.as_ref().map(|contract| {
                contract
                    .entries
                    .iter()
                    .map(|entry| DriverLibraryExport {
                        logical_name: entry.logical_entry.clone(),
                        physical_artifact: artifact_file.clone(),
                        invocation: "dotnet".to_string(),
                        abi_digest: contract.contract_digest.clone(),
                    })
                    .collect()
            }).unwrap_or_default()
        }
        else {
            Vec::new()
        };
        Ok(DriverCompileReport { artifacts, entry_symbol: None, run_contracts, managed_abi, ovm_abi: Vec::new(), library_exports })
    }
}

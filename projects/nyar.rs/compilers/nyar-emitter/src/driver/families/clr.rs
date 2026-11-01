use crate::{Result, bundled_backend_capability_descriptor, nyar_backend_clr::ClrBinaryBackend};
use nyar::{
    BackendInputKind, HostProjectionBoundary, PartitionBackendRequirement, TargetBackendFamily, TargetFamily, TargetLane,
    backends::TargetCodeGenBackend,
};

use super::BundledBackendCompiler;
use crate::{DriverBackendInput, DriverCompileReport, DriverCompileRequest, DriverRunContract};

/// `CLR` 家族后端编译器：唯一宿主边界是 [`HostProjectionBoundary::Clr`]。
pub(super) struct ClrFamilyCompiler;

/// 判断给定的后端需求是否可由 `CLR` 家族满足。
pub(super) fn supports_requirement(requirement: &PartitionBackendRequirement) -> bool {
    bundled_backend_capability_descriptor(TargetBackendFamily::Clr).is_some_and(|descriptor| descriptor.supports_requirement(requirement))
}

impl BundledBackendCompiler for ClrFamilyCompiler {
    fn compile(&self, request: DriverCompileRequest<'_>) -> Result<DriverCompileReport> {
        let DriverBackendInput::Clr(input) = request.input
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
        let backend = ClrBinaryBackend::new();
        backend.validate(&input)?;
        let artifacts = backend.compile(input, request.options)?;
        // 无入口点的库镜像没有可执行入口合同；只有可执行镜像才提供 `dotnet exec` 合同。
        let run_contracts = if has_entry_point {
            vec![DriverRunContract {
                logical_entry: "main".to_string(),
                physical_entry: format!("{}.exe", request.artifact_name),
                invocation: "dotnet".to_string(),
                validate: format!("dotnet exec {}.exe", request.artifact_name),
            }]
        }
        else {
            Vec::new()
        };
        Ok(DriverCompileReport { artifacts, entry_symbol: None, run_contracts, managed_abi: None })
    }
}

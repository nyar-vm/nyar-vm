//! `native` 家族后端编译器：唯一宿主边界是 [`HostProjectionBoundary::Native`]。
//!
//! 输入是语言 × 目标适配器已经编码好的原生镜像；本家族只负责按冻结的目标四元组
//! 选择容器并写出产物，不重新生成机器码，也不解释语言语义。

use crate::{
    DriverBackendInput, DriverCompileReport, DriverCompileRequest, DriverRunContract, Result, bundled_backend_capability_descriptor,
    nyar_backend_native::NativeBinaryBackend,
};
use nyar::{
    BackendInputKind, HostProjectionBoundary, PartitionBackendRequirement, TargetBackendFamily, TargetFamily, TargetLane,
    backends::TargetCodeGenBackend,
};

use super::BundledBackendCompiler;

/// `native` 家族后端编译器。
pub(super) struct NativeFamilyCompiler;

/// 判断给定的后端需求是否可由 `native` 家族满足。
pub(super) fn supports_requirement(requirement: &PartitionBackendRequirement) -> bool {
    bundled_backend_capability_descriptor(TargetBackendFamily::Native).is_some_and(|descriptor| descriptor.supports_requirement(requirement))
}

impl BundledBackendCompiler for NativeFamilyCompiler {
    fn compile(&self, request: DriverCompileRequest<'_>) -> Result<DriverCompileReport> {
        let DriverBackendInput::Native(input) = request.input
        else {
            return Err(format!("`native` 家族请求必须携带 `NativeBinaryBackendInput`"));
        };
        let requirement = &request.requirement;
        if requirement.lane != TargetLane::Native {
            return Err(format!("`native` 后端需求 lane 不一致：requirement={:?}", requirement.lane));
        }
        if requirement.input_kind != BackendInputKind::PeImage {
            return Err(format!("`native` 后端需求输入种类不一致：requirement={:?}", requirement.input_kind));
        }
        if requirement.target.family != TargetFamily::Native {
            return Err(format!("`native` 后端需求目标家族不一致：requirement={:?}", requirement.target.family));
        }
        if requirement.host_boundary != HostProjectionBoundary::Native {
            return Err(format!("`native` 后端输入宿主与规划需求不一致：requirement={:?}", requirement.host_boundary));
        }
        let backend = NativeBinaryBackend::new();
        backend.validate(&input).map_err(|error| error.to_string())?;
        let artifacts = backend.compile(input, request.options).map_err(|error| error.to_string())?;
        // 原生可执行文件就是宿主入口：运行契约直接指向产物本身，不拼接命令前缀。
        let run_contracts = artifacts
            .artifacts
            .first()
            .map(|artifact| DriverRunContract {
                logical_entry: "main".to_string(),
                physical_entry: artifact.path.clone(),
                invocation: artifact.path.clone(),
                validate: artifact.path.clone(),
            })
            .into_iter()
            .collect();
        Ok(DriverCompileReport { artifacts, entry_symbol: None, run_contracts, managed_abi: Vec::new(), ovm_abi: Vec::new(), library_exports: Vec::new() })
    }
}

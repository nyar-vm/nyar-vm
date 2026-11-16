//! `native` 家族后端编译器：唯一宿主边界是 [`HostProjectionBoundary::MachineCode`]。
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
    bundled_backend_capability_descriptor(TargetBackendFamily::MachineCode).is_some_and(|descriptor| descriptor.supports_requirement(requirement))
}

impl BundledBackendCompiler for NativeFamilyCompiler {
    fn compile(&self, request: DriverCompileRequest<'_>) -> Result<DriverCompileReport> {
        let DriverBackendInput::Native(input) = request.input
        else {
            return Err(format!("`machine-code` 家族请求必须携带 `NativeBinaryBackendInput`"));
        };
        let requirement = &request.requirement;
        if requirement.lane != TargetLane::MachineCode {
            return Err(format!("`machine-code` 后端需求 lane 不一致：requirement={:?}", requirement.lane));
        }
        if requirement.input_kind != BackendInputKind::PeImage {
            return Err(format!("`machine-code` 后端需求输入种类不一致：requirement={:?}", requirement.input_kind));
        }
        if requirement.target.family != TargetFamily::MachineCode {
            return Err(format!("`machine-code` 后端需求目标家族不一致：requirement={:?}", requirement.target.family));
        }
        if requirement.host_boundary != HostProjectionBoundary::MachineCode {
            return Err(format!("`machine-code` 后端输入宿主与规划需求不一致：requirement={:?}", requirement.host_boundary));
        }
        let backend = NativeBinaryBackend::new();
        backend.validate(&input).map_err(|error| error.to_string())?;
        let entry_symbol = input.entry_symbol.clone();
        // 用途决定容器：输入必须已携带与用途一致的产物事实（扩展名、导出面），
        // 包装层不得把可执行镜像改标为库或扩展。
        match request.purpose {
            nyar::ArtifactPurpose::Library => {
                return Err("`machine-code` 库交付尚未接线；拒绝把可执行镜像改标为库产物".to_string());
            }
            nyar::ArtifactPurpose::HostExtension => {
                if input.artifact_extension.is_none() {
                    return Err("native 宿主扩展输入缺少产物扩展名（`.node`/`.pyd`/`.so`）；拒绝写出不具名的共享库".to_string());
                }
                if input.host_exports.is_empty() {
                    return Err("native 宿主扩展缺少冻结的宿主导出面；拒绝产出无导出的扩展交付".to_string());
                }
            }
            nyar::ArtifactPurpose::Application if !input.host_exports.is_empty() => {
                return Err("native 可执行交付不得携带宿主扩展导出面；用途与导出面必须一致".to_string());
            }
            _ => {}
        }
        let artifacts = backend.compile(input.clone(), request.options).map_err(|error| error.to_string())?;
        // 宿主扩展没有进程入口：不产 CLI 运行合同，交付事实是宿主装载入口符号
        // 与按名调用的导出索引（合法无 CLI，区别于合同缺失）。
        if request.purpose == nyar::ArtifactPurpose::HostExtension {
            let physical_artifact = artifacts
                .artifacts
                .first()
                .map(|artifact| artifact.path.clone())
                .ok_or_else(|| "native 宿主扩展没有产出共享库镜像".to_string())?;
            let invocation = match input.artifact_extension.as_deref() {
                Some(".node") => "node require",
                _ => "python import",
            };
            let library_exports = input
                .host_exports
                .iter()
                .map(|name| crate::DriverLibraryExport {
                    logical_name: name.clone(),
                    physical_artifact: physical_artifact.clone(),
                    invocation: invocation.to_string(),
                    abi_digest: String::new(),
                })
                .collect();
            return Ok(DriverCompileReport {
                artifacts,
                entry_symbol: Some(entry_symbol),
                run_contracts: Vec::new(),
                managed_abi: Vec::new(),
                ovm_abi: Vec::new(),
                library_exports,
            });
        }
        // 原生可执行文件就是宿主入口：运行契约直接指向产物本身，不拼接命令前缀，
        // 逻辑入口取适配器冻结的入口符号。
        let run_contracts = artifacts
            .artifacts
            .first()
            .map(|artifact| DriverRunContract {
                logical_entry: entry_symbol.clone(),
                physical_entry: artifact.path.clone(),
                invocation: artifact.path.clone(),
                validate: artifact.path.clone(),
            })
            .into_iter()
            .collect();
        Ok(DriverCompileReport { artifacts, entry_symbol: None, run_contracts, managed_abi: Vec::new(), ovm_abi: Vec::new(), library_exports: Vec::new() })
    }
}

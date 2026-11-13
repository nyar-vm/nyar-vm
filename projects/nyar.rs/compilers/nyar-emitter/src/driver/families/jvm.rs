use crate::{
    DriverBackendInput, DriverCompileReport, DriverCompileRequest, DriverLibraryExport, DriverRunContract, Result,
    bundled_backend_capability_descriptor, nyar_backend_jvm::JvmBinaryBackend,
};
use nyar::{
    BackendInputKind, HostProjectionBoundary, PartitionBackendRequirement, TargetBackendFamily, TargetFamily, TargetLane,
    backends::TargetCodeGenBackend,
};

use super::BundledBackendCompiler;

/// `JVM` 家族后端编译器：唯一宿主边界是 [`HostProjectionBoundary::Jvm`]。
pub(super) struct JvmFamilyCompiler;

/// 判断给定的后端需求是否可由 `JVM` 家族满足。
pub(super) fn supports_requirement(requirement: &PartitionBackendRequirement) -> bool {
    bundled_backend_capability_descriptor(TargetBackendFamily::Jvm).is_some_and(|descriptor| descriptor.supports_requirement(requirement))
}

impl BundledBackendCompiler for JvmFamilyCompiler {
    fn compile(&self, request: DriverCompileRequest<'_>) -> Result<DriverCompileReport> {
        let DriverBackendInput::Jvm(mut input) = request.input
        else {
            return Err(format!("`JVM` 家族请求必须携带 `JvmBinaryBackendInput`"));
        };
        let requirement = &request.requirement;
        if requirement.lane != TargetLane::Jvm {
            return Err(format!("`JVM` 后端需求 lane 不一致：requirement={:?}", requirement.lane));
        }
        if requirement.input_kind != BackendInputKind::JvmClassFile {
            return Err(format!("`JVM` 后端需求输入种类不一致：requirement={:?}", requirement.input_kind));
        }
        if requirement.target.family != TargetFamily::Jvm {
            return Err(format!("`JVM` 后端需求目标家族不一致：requirement={:?}", requirement.target.family));
        }
        if requirement.host_boundary != HostProjectionBoundary::Jvm {
            return Err(format!("`JVM` 后端输入宿主与规划需求不一致：requirement={:?}", requirement.host_boundary));
        }
        let has_launcher_entry =
            input.class_file.methods.iter().any(|method| method.name == "main" && method.descriptor.parameter_types.is_empty());
        let provided_contracts = std::mem::take(&mut input.run_contracts);
        let mut managed_abi = input.managed_abi.take();
        if let Some(contract) = managed_abi.as_mut() {
            // 产物文件名由驱动层决定；契约摘要必须覆盖最终文件名。
            contract.set_artifact(format!("{}.jar", request.artifact_name));
        }
        let backend = JvmBinaryBackend::new();
        backend.validate(&input).map_err(|error| error.to_string())?;
        let artifacts = backend.compile(input, request.options).map_err(|error| error.to_string())?;
        // 库产物必须有导出面：缺失 managed ABI 入口时结构化失败，不得回退 `main`
        // 或静默返回空运行契约。
        if request.library_mode {
            match managed_abi.as_ref() {
                None => return Err("`artifact: library` 的 `jvm` 产物缺少 managed ABI 契约；拒绝回退 `main`".to_string()),
                Some(contract) if contract.entries.is_empty() => {
                    return Err("`artifact: library` 的 `jvm` managed ABI 契约为空；拒绝产出空运行契约".to_string());
                }
                Some(_) => {}
            }
        }
        // 库产物没有零参 `main` 时 `JvmBinaryBackend` 不补造 launcher；此时每个
        // managed ABI 入口各自持有可执行的 argv launcher 类与 run contract。
        let run_contracts = if !provided_contracts.is_empty() {
            provided_contracts
        }
        else if let Some(contract) = &managed_abi {
            contract
                .entries
                .iter()
                .map(|entry| DriverRunContract {
                    logical_entry: entry.logical_entry.clone(),
                    physical_entry: format!("{}.jar", request.artifact_name),
                    invocation: "java".to_string(),
                    validate: format!("java -cp {}.jar {}", request.artifact_name, entry.launcher_class.replace('/', ".")),
                })
                .collect()
        }
        else if has_launcher_entry {
            vec![DriverRunContract {
                logical_entry: "main".to_string(),
                physical_entry: format!("{}.jar", request.artifact_name),
                invocation: "java".to_string(),
                validate: format!("java -jar {}.jar", request.artifact_name),
            }]
        }
        else {
            Vec::new()
        };
        let library_exports = if request.library_mode {
            managed_abi
                .as_ref()
                .map(|contract| {
                    contract
                        .entries
                        .iter()
                        .map(|entry| DriverLibraryExport {
                            logical_name: entry.logical_entry.clone(),
                            physical_artifact: format!("{}.jar", request.artifact_name),
                            invocation: "java".to_string(),
                            abi_digest: contract.contract_digest.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        }
        else {
            Vec::new()
        };
        Ok(DriverCompileReport {
            artifacts,
            entry_symbol: None,
            run_contracts,
            managed_abi: managed_abi.into_iter().collect(),
            ovm_abi: Vec::new(),
            library_exports,
        })
    }
}

use crate::{Result, bundled_backend_capability_descriptor, nyar_backend_jvm::JvmBinaryBackend};
use nyar::{
    BackendInputKind, HostProjectionBoundary, PartitionBackendRequirement, TargetBackendFamily, TargetFamily, TargetLane,
    backends::TargetCodeGenBackend,
};

use super::BundledBackendCompiler;
use crate::{DriverBackendInput, DriverCompileReport, DriverCompileRequest, DriverRunContract};

/// `JVM` 家族后端编译器：唯一宿主边界是 [`HostProjectionBoundary::Jvm`]。
pub(super) struct JvmFamilyCompiler;

/// 判断给定的后端需求是否可由 `JVM` 家族满足。
pub(super) fn supports_requirement(requirement: &PartitionBackendRequirement) -> bool {
    bundled_backend_capability_descriptor(TargetBackendFamily::Jvm).is_some_and(|descriptor| descriptor.supports_requirement(requirement))
}

impl BundledBackendCompiler for JvmFamilyCompiler {
    fn compile(&self, request: DriverCompileRequest<'_>) -> Result<DriverCompileReport> {
        let DriverBackendInput::Jvm(input) = request.input
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
        let backend = JvmBinaryBackend::new();
        backend.validate(&input)?;
        let artifacts = backend.compile(input, request.options)?;
        // 库产物没有零参 `main` 时 `JvmBinaryBackend` 不补造 launcher，因此也没有可执行入口合同。
        let run_contracts = if has_launcher_entry {
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
        Ok(DriverCompileReport { artifacts, entry_symbol: None, run_contracts })
    }
}

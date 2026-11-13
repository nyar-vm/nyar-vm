use crate::{DriverBackendInput, DriverCompileReport, DriverCompileRequest, DriverLibraryExport, DriverRunContract, Result, bundled_backend_capability_descriptor, nyar_backend_wasi::WasmBinaryBackend};
use nyar::{HostProjectionBoundary, PartitionBackendRequirement, TargetBackendFamily, backends::TargetCodeGenBackend};

use super::BundledBackendCompiler;

/// `WASM` 家族后端编译器，统一调度 `WasmJsGlue` 与 `WasiComponent` 两种宿主边界。
pub(super) struct WasmFamilyCompiler;

/// 判断给定的后端需求是否可由 `WASM` 家族满足。
pub(super) fn supports_requirement(requirement: &PartitionBackendRequirement) -> bool {
    bundled_backend_capability_descriptor(TargetBackendFamily::Wasm).is_some_and(|descriptor| descriptor.supports_requirement(requirement))
}

impl BundledBackendCompiler for WasmFamilyCompiler {
    fn compile(&self, request: DriverCompileRequest<'_>) -> Result<DriverCompileReport> {
        let DriverBackendInput::Wasm(input) = request.input
        else {
            return Err(format!("`WASM` 家族请求必须携带 `WasmBinaryBackendInput`"));
        };
        let expected_boundary = input.host_boundary;
        if request.requirement.host_boundary != expected_boundary {
            return Err(format!(
                "`WASM` 后端输入宿主与规划需求不一致：input={:?}, requirement={:?}",
                expected_boundary, request.requirement.host_boundary
            ));
        }
        let backend = WasmBinaryBackend::new();
        backend.validate(&input).map_err(|error| error.to_string())?;
        let package_as_wasi_command = input.package_as_wasi_command;
        let wasi_preview = input.wasi_preview;
        // `artifact: library`：导出通过 glue 的 `callExport` 调用，不产生可执行
        // run contract；同时记录每个导出的物理产物与 ABI digest。
        let library_exports = if request.library_mode {
            if input.library_wasm_exports.is_empty() {
                return Err("`artifact: library` 的 `wasm` 产物没有导出面".to_string());
            }
            let abi_digest = library_invoke_digest(&input.module).unwrap_or_default();
            let physical_name = wasm_physical_artifact_name(request.artifact_name);
            input
                .library_wasm_exports
                .iter()
                .map(|name| DriverLibraryExport {
                    logical_name: name.clone(),
                    physical_artifact: format!("{physical_name}.wasm"),
                    invocation: "wasm callExport".to_string(),
                    abi_digest: abi_digest.clone(),
                })
                .collect()
        }
        else {
            Vec::new()
        };
        let artifacts = backend.compile(input, request.options).map_err(|error| error.to_string())?;
        let run_contracts = if request.library_mode {
            Vec::new()
        }
        else {
            match request.requirement.host_boundary {
                HostProjectionBoundary::WasiComponent if !package_as_wasi_command => Vec::new(),
                boundary => vec![wasm_run_contract(request.artifact_name, boundary, wasi_preview)],
            }
        };
        Ok(DriverCompileReport { artifacts, entry_symbol: None, run_contracts, managed_abi: None, ovm_abi: Vec::new(), library_exports })
    }
}

fn wasm_run_contract(
    artifact_name: &str,
    boundary: HostProjectionBoundary,
    wasi_preview: crate::nyar_backend_wasi::WasiPreview,
) -> DriverRunContract {
    let physical_name = wasm_physical_artifact_name(artifact_name);
    match boundary {
        HostProjectionBoundary::WasmJsGlue => DriverRunContract {
            logical_entry: "main".to_string(),
            physical_entry: format!("{}.mjs", physical_name),
            invocation: "node".to_string(),
            validate: format!("node {}.mjs", physical_name),
        },
        HostProjectionBoundary::WasiComponent => {
            let p3_flag = match wasi_preview {
                crate::nyar_backend_wasi::WasiPreview::Preview3 => " -S p3",
                crate::nyar_backend_wasi::WasiPreview::Preview2 => "",
            };
            DriverRunContract {
                logical_entry: "_start".to_string(),
                physical_entry: format!("{}.wasi", physical_name),
                invocation: "wasmtime".to_string(),
                validate: format!("wasmtime run -W gc -W wmemcheck -W max-memory-size=16777216{p3_flag} {}.wasi", physical_name),
            }
        }
        other => unreachable!("unexpected wasm host boundary: {:?}", other),
    }
}

/// 从 `.wasm` 的 `nyar.library_invoke` 自定义段读取 ABI digest。
///
/// 该段由 lowering 在库模式下写入；本函数只读取已冻结事实，不重新推导。
fn library_invoke_digest(module: &crate::nyar_backend_wasi::WasmBinaryModule) -> Option<String> {
    let section = module.sections.iter().find(|section| section.name.as_deref() == Some("nyar.library_invoke"))?;
    let payload: serde_json::Value = serde_json::from_slice(&section.bytes).ok()?;
    payload.get("abi_digest")?.as_str().map(str::to_string)
}

/// 将逻辑产物名规范化为 `WASM` 物理产物名。
///
/// 通用规则：直接使用 `artifact_name` 作为物理产物名，不对任何项目名做特殊分支。
/// 此函数是 `DriverRunContract` 引用物理文件名的唯一来源，必须与
/// `backend::wasi::canonical_wasm_artifact_stem` 产出相同的结果。
fn wasm_physical_artifact_name(artifact_name: &str) -> String {
    artifact_name.to_string()
}

use std::{fs, path::Path};

use crate::{
    DriverBackendInput, DriverCompileReport, DriverCompileRequest, DriverLibraryExport, DriverRunContract, OvmAbiSidecar, Result,
    bundled_backend_capability_descriptor, nyar_backend_vm::emit_nyar_module,
};
use nyar::{ArtifactDescriptor, ArtifactFormat, ArtifactKind, ArtifactSet, PartitionBackendRequirement, TargetBackendFamily, TargetLane};
use odin_bytecode::{OdinExportKind, OdinModuleData, decode_module};

use super::BundledBackendCompiler;

pub(super) struct OvmFamilyCompiler;

pub(super) fn supports_requirement(requirement: &PartitionBackendRequirement) -> bool {
    bundled_backend_capability_descriptor(TargetBackendFamily::Ovm).is_some_and(|descriptor| descriptor.supports_requirement(requirement))
}

impl BundledBackendCompiler for OvmFamilyCompiler {
    fn compile(&self, request: DriverCompileRequest<'_>) -> Result<DriverCompileReport> {
        let DriverBackendInput::Ovm(input) = request.input
        else {
            return Err("`ovm` 家族请求必须携带 `OvmBackendInput`".to_string());
        };

        let mut artifacts = ArtifactSet::default();
        let mut entry_symbol = None;
        let mut run_contracts = Vec::new();
        let mut library_exports = Vec::new();
        let mut ovm_abi = Vec::new();

        fs::create_dir_all(&input.output_dir).map_err(|e| format!("创建输出目录失败：{}: {e}", input.output_dir.display()))?;

        if let Some(module) = input.nyar_module.as_ref() {
            let entries = resolve_entry_symbols(module, &input.library_public_exports, request.library_mode)?;
            reject_legacy_output(&input.output_dir, request.artifact_name)?;
            let odin_name = format!("{}.odin", request.artifact_name);
            let odin_path = input.output_dir.join(&odin_name);
            emit_nyar_module(module, &odin_path)?;
            // sidecar 冻结产物**整体**哈希：布局表、常量池与代码段都参与 ABI 解释，
            // 挑字段计算迟早漏掉参与解释的内容。
            let odin_bytes = fs::read(&odin_path).map_err(|error| format!("读取刚写出的 `.odin` 产物失败：{}: {error}", odin_path.display()))?;
            artifacts.push(ArtifactDescriptor {
                name: odin_name.clone(),
                path: format!("{}.odin", request.artifact_name),
                kind: ArtifactKind::Executable,
                format: ArtifactFormat::RawBinary,
                target: request.options.target.clone(),
                lane: TargetLane::Vm,
            });

            entry_symbol = entries.first().cloned();
            if request.library_mode {
                // 库导出面必须携带冻结的 ABI 合同；缺失即失败闭合，绝不写出
                // 无合同的导出索引让宿主按未校验的布局解释内存。
                let contract = input
                    .abi_contract
                    .as_ref()
                    .ok_or_else(|| "`artifact: library` 缺少 OVM ABI 合同；拒绝产出无合同的库导出面".to_string())?;
                library_exports = entries
                    .iter()
                    .map(|entry| DriverLibraryExport {
                        logical_name: entry.clone(),
                        physical_artifact: format!("{}.odin", request.artifact_name),
                        invocation: "ovm run --entry".to_string(),
                        abi_digest: contract.digest.clone(),
                    })
                    .collect();
            }
            // sidecar 与 `.odin` 同目录落盘，每个物理产物各持一份；宿主读取时先按整份产物重算哈希。
            if let Some(contract) = input.abi_contract.as_ref() {
                ovm_abi.push(OvmAbiSidecar::new(contract.clone(), odin_name.clone(), &odin_bytes));
            }
            for entry in entries {
                run_contracts.push(ovm_run_contract(request.artifact_name, &entry));
            }
        }

        Ok(DriverCompileReport { artifacts, entry_symbol, run_contracts, managed_abi: None, ovm_abi, library_exports })
    }
}

#[doc(hidden)]
pub fn resolve_entry_symbols(module: &OdinModuleData, library_public_exports: &[String], library_mode: bool) -> Result<Vec<String>> {
    if library_mode && library_public_exports.is_empty() {
        return Err("`artifact: library` 要求至少一个已解析的 `[export]`；拒绝回落 `main`".to_string());
    }
    let entries = if library_public_exports.is_empty() { vec!["main".to_owned()] } else { library_public_exports.to_vec() };
    for entry in &entries {
        let matches: Vec<_> =
            module.exports.iter().filter(|export| export.kind == OdinExportKind::Function && export.symbol_name == *entry).collect();
        if matches.len() != 1 {
            return Err(format!("Nyar 入口 `{entry}` 必须对应唯一明确函数导出"));
        }
        let index = usize::try_from(matches[0].function_index).map_err(|_| format!("Nyar 入口 `{entry}` 的函数索引无效"))?;
        if module.functions.get(index).is_none() {
            return Err(format!("Nyar 入口 `{entry}` 的函数索引越界"));
        }
    }
    Ok(entries)
}

/// 拒绝在旧产物之上静默重建：旧 `.nyar` 执行产物与不兼容的既有 `.odin` 都必须先移除。
fn reject_legacy_output(output_dir: &Path, artifact_name: &str) -> Result<()> {
    if let Ok(entries) = fs::read_dir(output_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("nyar") && path.is_file() {
                return Err(format!(
                    "拒绝重建：输出目录存在旧 `.nyar` 执行产物 `{}`；`.odin` 是不同产物身份，请删除旧产物后重建",
                    path.display()
                ));
            }
        }
    }
    let target = output_dir.join(format!("{artifact_name}.odin"));
    if target.is_file() {
        let bytes = fs::read(&target).map_err(|error| format!("读取既有 `.odin` 产物失败：{}: {error}", target.display()))?;
        decode_module(&bytes)
            .map_err(|error| format!("拒绝覆盖格式不兼容的既有 `.odin` 产物 `{}`：{error}；请删除后重建", target.display()))?;
    }
    Ok(())
}

fn ovm_run_contract(artifact_name: &str, entry: &str) -> DriverRunContract {
    DriverRunContract {
        logical_entry: entry.to_string(),
        physical_entry: format!("{artifact_name}.odin"),
        invocation: "ovm".to_string(),
        validate: format!("ovm run {artifact_name}.odin --entry {entry}"),
    }
}

use nyar::{TargetBackendFamily, TargetLane};

use crate::{ArtifactPartition, BundledBackendCapabilityDescriptor, DriverCompileReport, bundled_backend_capability_descriptor};

pub(crate) fn backend_family_for_partition(partition: &ArtifactPartition) -> TargetBackendFamily {
    match partition.lane {
        TargetLane::Clr => TargetBackendFamily::Clr,
        TargetLane::Jvm => TargetBackendFamily::Jvm,
        TargetLane::Wasm => TargetBackendFamily::Wasm,
        TargetLane::Native => TargetBackendFamily::Native,
        TargetLane::Vm => TargetBackendFamily::Ovm,
        TargetLane::Gpu => TargetBackendFamily::Gpu,
    }
}

pub(crate) fn backend_descriptor_for_partition(partition: &ArtifactPartition) -> Option<BundledBackendCapabilityDescriptor> {
    bundled_backend_capability_descriptor(backend_family_for_partition(partition))
}

pub(crate) fn partition_artifact_name(base_name: &str, partition: &ArtifactPartition, partition_count: usize) -> String {
    if partition_count <= 1 {
        return base_name.to_string();
    }

    let suffix = partition
        .name
        .rsplit("::")
        .next()
        .unwrap_or(partition.name.as_str())
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '_' })
        .collect::<String>();
    format!("{base_name}__{suffix}")
}

pub(crate) fn merge_partition_reports(
    reports: Vec<(String, DriverCompileReport)>,
    primary_partition_name: Option<&str>,
) -> crate::Result<DriverCompileReport> {
    let mut merged = DriverCompileReport::default();
    let mut primary_contracts = Vec::new();
    let mut secondary_contracts = Vec::new();
    let mut managed_abi: Vec<(String, crate::ManagedAbiContract)> = Vec::new();
    let mut ovm_abi: Vec<(String, crate::OvmAbiSidecar)> = Vec::new();
    for (partition_name, report) in reports {
        merged.artifacts.artifacts.extend(report.artifacts.artifacts);
        if merged.entry_symbol.is_none() {
            merged.entry_symbol = report.entry_symbol.clone();
        }
        // 每个物理产物各持一份 managed ABI 合同；不同分区各自拥有产物是合法的，
        // 只有两个分区声称同一 `(lane, artifact)` 时才是不可合并的歧义。
        for contract in report.managed_abi {
            if let Some((existing_partition, existing)) =
                managed_abi.iter().find(|(_, existing)| existing.lane == contract.lane && existing.artifact == contract.artifact)
            {
                return Err(format!(
                    "多个分区同时声称同一物理产物 `{}` 的 managed ABI 合同（lane `{}`）：`{existing_partition}` 与 `{partition_name}`；拒绝合并歧义合同",
                    existing.artifact, existing.lane
                ));
            }
            managed_abi.push((partition_name.clone(), contract));
        }
        // 每个物理产物各持一份 ABI sidecar；不同分区各自拥有产物是合法的，只有
        // 两个分区声称同一物理产物时才是不可合并的歧义（产物归属/哈希无法唯一）。
        for sidecar in report.ovm_abi {
            if let Some((existing_partition, existing)) = ovm_abi.iter().find(|(_, existing)| existing.artifact == sidecar.artifact) {
                return Err(format!(
                    "多个分区同时声称同一物理产物 `{}` 的 OVM ABI sidecar：`{existing_partition}` 与 `{partition_name}`；拒绝合并歧义 ABI 合同",
                    existing.artifact
                ));
            }
            ovm_abi.push((partition_name.clone(), sidecar));
        }
        if primary_partition_name == Some(partition_name.as_str()) {
            primary_contracts.extend(report.run_contracts);
        }
        else {
            secondary_contracts.extend(report.run_contracts);
        }
        merged.library_exports.extend(report.library_exports);
    }
    merged.run_contracts = primary_contracts;
    merged.run_contracts.extend(secondary_contracts);
    merged.managed_abi = managed_abi.into_iter().map(|(_, contract)| contract).collect();
    merged.ovm_abi = ovm_abi.into_iter().map(|(_, sidecar)| sidecar).collect();
    Ok(merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ManagedAbiContract;

    fn report_with_managed_abi(lane: &str, artifact: &str) -> DriverCompileReport {
        DriverCompileReport { managed_abi: vec![ManagedAbiContract::new(lane, artifact, Vec::new())], ..DriverCompileReport::default() }
    }

    /// 不同物理产物各自拥有 managed ABI；合并不得丢弃第二份。
    #[test]
    fn merges_one_managed_abi_per_physical_artifact() {
        let merged = merge_partition_reports(
            vec![
                ("client".to_string(), report_with_managed_abi("jvm", "client.jar")),
                ("server".to_string(), report_with_managed_abi("clr", "server.dll")),
            ],
            None,
        )
        .expect("不同产物必须可合并");
        assert_eq!(merged.managed_abi.len(), 2);
        assert_eq!(merged.managed_abi.iter().map(|contract| contract.artifact.as_str()).collect::<Vec<_>>(), vec!["client.jar", "server.dll"]);
    }

    /// 两个分区声称同一 `(lane, artifact)` 是归属歧义，必须失败。
    #[test]
    fn rejects_two_partitions_claiming_the_same_managed_artifact() {
        let error = merge_partition_reports(
            vec![
                ("left".to_string(), report_with_managed_abi("jvm", "app.jar")),
                ("right".to_string(), report_with_managed_abi("jvm", "app.jar")),
            ],
            None,
        )
        .expect_err("同一产物被两个分区声称必须失败");
        assert!(error.contains("managed ABI"), "{error}");
        assert!(error.contains("app.jar"), "{error}");
    }
}

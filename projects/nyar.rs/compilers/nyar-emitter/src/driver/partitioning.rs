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
    let mut managed_abi: Option<(String, crate::ManagedAbiContract)> = None;
    for (partition_name, report) in reports {
        merged.artifacts.artifacts.extend(report.artifacts.artifacts);
        if merged.entry_symbol.is_none() {
            merged.entry_symbol = report.entry_symbol.clone();
        }
        if let Some(contract) = report.managed_abi {
            if let Some((existing_partition, _)) = &managed_abi {
                return Err(format!("多个分区同时产出 managed ABI 契约：`{existing_partition}` 与 `{partition_name}`；拒绝合并歧义契约"));
            }
            managed_abi = Some((partition_name.clone(), contract));
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
    merged.managed_abi = managed_abi.map(|(_, contract)| contract);
    Ok(merged)
}

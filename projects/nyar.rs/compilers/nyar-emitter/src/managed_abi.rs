//! Managed run 入口的 ABI 契约。
//!
//! 库产物（例如 `.jar`）没有零参 `main`，但仍需要机器可校验的执行入口。
//! 本模块只描述已经由目标 lowering 冻结的入口事实：逻辑入口名、主类中的
//! 静态方法、JVM 方法描述符，以及独立 launcher 类；不补造未降低的入口。
//!
//! `contract_digest` 覆盖 lane、产物文件名、调用约定与按 `logical_entry`
//! 排序后的全部入口，任何入口事实变化都会改变摘要。

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// managed ABI 契约 schema 版本。
pub const MANAGED_ABI_SCHEMA_VERSION: u32 = 1;

/// managed 入口 ABI 契约。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAbiContract {
    /// schema 版本，固定为 [`MANAGED_ABI_SCHEMA_VERSION`]。
    pub schema_version: u32,
    /// 目标 lane，`jvm` 或 `clr`。
    pub lane: String,
    /// 产物文件名（仅文件名，不含目录）。
    pub artifact: String,
    /// 调用约定名。
    pub calling_convention: String,
    /// 已冻结的入口列表，按 `logical_entry` 排序。
    pub entries: Vec<ManagedAbiEntry>,
    /// 对小写十六进制 sha256 摘要。
    pub contract_digest: String,
}

impl ManagedAbiContract {
    /// 构造契约并计算摘要；`entries` 会按 `logical_entry` 稳定排序。
    pub fn new(lane: impl Into<String>, artifact: impl Into<String>, mut entries: Vec<ManagedAbiEntry>) -> Self {
        entries.sort_by(|left, right| left.logical_entry.cmp(&right.logical_entry));
        let mut contract = Self {
            schema_version: MANAGED_ABI_SCHEMA_VERSION,
            lane: lane.into(),
            artifact: artifact.into(),
            calling_convention: "static-argv-launcher".to_string(),
            entries,
            contract_digest: String::new(),
        };
        contract.refresh_digest();
        contract
    }

    /// 设置产物文件名并重新计算摘要。
    pub fn set_artifact(&mut self, artifact: impl Into<String>) {
        self.artifact = artifact.into();
        self.refresh_digest();
    }

    /// 重新计算 [`Self::contract_digest`]。
    pub fn refresh_digest(&mut self) {
        self.contract_digest = self.compute_digest();
    }

    /// 生成摘要的规范文本；入口已按 `logical_entry` 排序。
    pub fn canonical_digest_input(&self) -> String {
        let mut lines = Vec::with_capacity(self.entries.len() + 3);
        lines.push(format!("lane={}", self.lane));
        lines.push(format!("artifact={}", self.artifact));
        lines.push(format!("calling_convention={}", self.calling_convention));
        for entry in &self.entries {
            lines.push(format!(
                "{}|{}|{}|{}|{}|{}|{}",
                entry.logical_entry,
                entry.owner,
                entry.method,
                entry.signature,
                entry.parameter_types.join(","),
                entry.return_type,
                entry.launcher_class,
            ));
        }
        lines.join("\n")
    }

    fn compute_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.canonical_digest_input().as_bytes());
        hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

/// 单个 managed 入口。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedAbiEntry {
    /// 逻辑入口名（导出名）。
    pub logical_entry: String,
    /// 主类内部名（JVM 形态带 `/`）。
    pub owner: String,
    /// 主类中的静态方法名。
    pub method: String,
    /// managed 方法描述符，例如 `(J)J`。
    pub signature: String,
    /// 形参的 managed 标量类型名。
    pub parameter_types: Vec<String>,
    /// 返回值的 managed 标量类型名。
    pub return_type: String,
    /// launcher 类内部名。
    pub launcher_class: String,
}

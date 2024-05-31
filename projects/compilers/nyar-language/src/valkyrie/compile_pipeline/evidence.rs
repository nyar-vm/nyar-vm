//! Compiler 成功路径的可复现证据（digest / provenance）。

use nyar_types::{CompiledProgram, combined_content_hash};

use crate::CompilerSourceGroup;

use super::context::CompilerHostProviderBinding;

/// 从源码闭包到 Canonical 表面的稳定编译证据。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CompilerCompileEvidence {
    /// Resolver 提交的语义源码闭包摘要。
    pub semantic_closure_hash: String,
    /// 已验证 `CompiledProgram` 的导出 / 入口 / 函数表面摘要。
    pub canonical_surface_hash: String,
    /// Resolver 选定的 host provider 绑定摘要。
    pub host_provider_binding_hash: String,
}

impl CompilerCompileEvidence {
    /// 从源码组、成功载荷与 host 绑定构造证据。
    pub fn from_success(groups: &[CompilerSourceGroup], program: &CompiledProgram, host_bindings: &[CompilerHostProviderBinding]) -> Self {
        Self {
            semantic_closure_hash: semantic_closure_hash(groups),
            canonical_surface_hash: program.surface_digest(),
            host_provider_binding_hash: host_provider_binding_hash(host_bindings),
        }
    }
}

/// 从 Resolver 提交的源码组计算语义闭包摘要（供 freshness 比对）。
pub fn semantic_closure_hash_for_groups(groups: &[CompilerSourceGroup]) -> String {
    semantic_closure_hash(groups)
}

/// 从 host provider 绑定计算摘要（供 freshness 比对）。
pub fn host_provider_binding_hash_for_bindings(host_bindings: &[CompilerHostProviderBinding]) -> String {
    host_provider_binding_hash(host_bindings)
}

fn semantic_closure_hash(groups: &[CompilerSourceGroup]) -> String {
    // 闭包和摘要使用同一 Compiler 生产者，不能信任调用方提交的摘要。
    let closure = crate::valkyrie::semantic_closure::source_group_closure(groups);
    let mut parts = vec![closure.digest];
    for group in groups {
        parts.push(group.dependency_key.clone());
        parts.push(group.name.clone());
        for dependency in &group.direct_dependencies {
            parts.push(dependency.clone());
        }
    }
    let borrowed = parts.iter().map(String::as_str).collect::<Vec<_>>();
    combined_content_hash(&borrowed)
}

fn host_provider_binding_hash(host_bindings: &[CompilerHostProviderBinding]) -> String {
    let mut parts = Vec::new();
    for binding in host_bindings {
        parts.push(binding.contract.clone());
        parts.push(binding.symbol.clone());
    }
    let borrowed = parts.iter().map(String::as_str).collect::<Vec<_>>();
    combined_content_hash(&borrowed)
}

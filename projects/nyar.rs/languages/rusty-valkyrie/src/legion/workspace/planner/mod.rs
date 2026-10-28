mod ast_attributes;
mod bindings;
mod capability;
mod condition_eval;
mod source_file_conditions;
mod target_gate;
mod target_when;
mod closure_demand;
mod dependencies;
mod host;
mod host_inventory_ast;
mod resolver;
mod sources;
mod types;

pub(crate) use bindings::compiler_host_provider_bindings;
pub(crate) use closure_demand::{compile_arch_key, demanded_declaration_keys};

pub use dependencies::*;
pub use sources::*;
pub use types::*;

/// 把已选定的 host provider 集合序列化为稳定 SHA-256 digest，纳入构建缓存身份。
///
/// 输入 providers 应已按 `contract + symbol + source_file + line` 排序
/// （`select_host_providers` 已保证这一点）。digest 仅使用 contract、symbol、
/// required_capabilities、when/gate attributes、foreign_bindings 等稳定字段，
/// **不**依赖源码 checkout 位置的绝对路径，因此跨机器同构构建会得到相同结果。
///
/// 空集合返回全零 hex 字符串（长度 64）。
pub fn provider_set_digest(providers: &[PlannedHostProvider]) -> String {
    use nyar_workspace::combined_hash;

    if providers.is_empty() {
        return "0".repeat(64);
    }

    // 每个 provider 贡献一组字符串 part，字段之间用明确分隔符连接，
    // 避免字符串拼接带来的歧义。
    let mut parts: Vec<String> = Vec::new();
    for p in providers {
        // contract（源码书写形式）
        parts.push(format!("C:{}", p.contract));
        // symbol（声明限定名）
        parts.push(format!("S:{}", p.symbol));
        // required_capabilities（已在 capability.rs 排序 + dedup）
        parts.push(format!("RC:{}", p.required_capabilities.join("|")));
        // foreign_bindings
        parts.push(format!(
            "FB:{}",
            p.foreign_bindings
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join("|")
        ));
        // when_attributes：每个 (key=value) 用逗号分隔
        parts.push(format!(
            "W:{}",
            p.when_attributes
                .iter()
                .map(|w| format!("{}={}", w.key, w.value))
                .collect::<Vec<_>>()
                .join(",")
        ));
        // gate_attributes：每个 (key=value) 用逗号分隔
        parts.push(format!(
            "G:{}",
            p.gate_attributes
                .iter()
                .map(|g| format!("{}={}", g.key, g.value))
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    let ref_parts: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
    combined_hash(&ref_parts)
}

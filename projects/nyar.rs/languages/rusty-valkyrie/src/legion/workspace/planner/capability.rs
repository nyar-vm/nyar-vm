use std::collections::BTreeSet;

use nyar::{CanonicalAbi, CanonicalSpecification, CanonicalTarget, CanonicalVendor, TargetBackendFamily, TargetHostKind};
use serde::{Deserialize, Serialize};

use super::types::PlannedHostProvider;

/// Oak attribute 上的一个外部绑定（`js` / `wasm` / … 的参数表达式）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForeignBinding {
    Js { symbol: String },
    JsBuiltin { symbol: String },
    Wasm { module: String, field: String },
    Clr { symbol: String },
    Jvm { symbol: String },
}

impl ForeignBinding {
    pub(crate) fn display(&self) -> String {
        match self {
            Self::Js { symbol } => format!("js(\"{symbol}\")"),
            Self::JsBuiltin { symbol } => format!("js_builtin(\"{symbol}\")"),
            Self::Wasm { module, field } => format!("wasm(\"{module}\", \"{field}\")"),
            Self::Clr { symbol } => format!("clr(\"{symbol}\")"),
            Self::Jvm { symbol } => format!("jvm(\"{symbol}\")"),
        }
    }
}

/// 从 `[js("fetch")]`、`[js_builtin("wx.request")]` 等 attribute 推导目标能力 id。
pub(crate) fn js_capability_id(symbol: &str) -> String {
    let trimmed = symbol.trim();
    if trimmed.is_empty() {
        return "js".to_string();
    }
    format!("js.{trimmed}")
}

fn wasm_capability_id(module: &str, field: &str) -> String {
    format!("wasm.{module}.{field}")
}

/// 判断单个 capability 是否被 target 集合满足（`wasm.env.*` 可由 `wasm.env` 模块级授权覆盖）。
pub(crate) fn capability_available(capability: &str, available: &BTreeSet<String>) -> bool {
    if available.contains(capability) {
        return true;
    }
    if let Some(rest) = capability.strip_prefix("wasm.") {
        let segments: Vec<_> = rest.split('.').collect();
        if segments.len() >= 2 {
            let module_grant = format!("wasm.{}", segments[0]);
            if available.contains(&module_grant) {
                return true;
            }
        }
    }
    false
}

/// 由外部绑定列表生成 provider 所需 capability id。
pub(crate) fn required_capabilities_from_foreign_bindings(bindings: &[ForeignBinding]) -> Vec<String> {
    let mut required = Vec::new();
    for binding in bindings {
        match binding {
            ForeignBinding::Js { symbol } | ForeignBinding::JsBuiltin { symbol } => required.push(js_capability_id(symbol)),
            ForeignBinding::Wasm { module, field } => required.push(wasm_capability_id(module, field)),
            ForeignBinding::Clr { symbol } => required.push(format!("clr.{symbol}")),
            ForeignBinding::Jvm { symbol } => required.push(format!("jvm.{symbol}")),
        }
    }
    required.sort();
    required.dedup();
    required
}

/// 由规范化目标四元组（`CanonicalTarget::to_profile`）推导当前构建可用的 host capability 集合。
///
/// 不读取 `publish`、源码路径或 namespace。`js.*` 仅出现在 **Wasm + JS glue** 目标（browser / miniprogram / node wasm），
/// 不出现在 WASI（WASI 走 `wasm.env` / component import，与 browser JS 无关）。
pub(crate) fn target_capability_set(target: CanonicalTarget) -> BTreeSet<String> {
    let profile = target.to_profile(None);
    let mut available = BTreeSet::new();

    for tag in profile.capability_tags {
        available.insert(tag);
    }

    match profile.backend_family {
        TargetBackendFamily::Wasm => match profile.host_kind {
            TargetHostKind::Browser => {
                available.insert("js.fetch".to_string());
                available.insert("js.random".to_string());
            }
            TargetHostKind::JavaScript => {
                available.insert("js.fetch".to_string());
                available.insert("js.random".to_string());
                available.insert("node.fs".to_string());
            }
            TargetHostKind::Wasi => {
                available.insert("wasm.wasi".to_string());
                available.insert("wasm.env".to_string());
            }
            _ => {}
        },
        TargetBackendFamily::Clr => {
            available.insert("clr.host".to_string());
        }
        TargetBackendFamily::Jvm => {
            available.insert("jvm.host".to_string());
        }
        _ => {}
    }

    if target.specification == CanonicalSpecification::Miniprogram {
        available.insert("js.wx.request".to_string());
        available.remove("js.fetch");
        available.remove("fetch");
    }

    if matches!(target.vendor, CanonicalVendor::Node | CanonicalVendor::Deno | CanonicalVendor::Bun) {
        available.insert("node.fs".to_string());
    }

    available
}

/// 解析 provider 在指定 target 下声明的能力需求（显式 attribute 优先；托管后端可缺省为 `clr.host` / `jvm.host`）。
pub(crate) fn provider_required_capabilities(provider: &PlannedHostProvider, target: CanonicalTarget) -> Vec<String> {
    if !provider.required_capabilities.is_empty() {
        return provider.required_capabilities.clone();
    }
    match target.to_profile(None).backend_family {
        TargetBackendFamily::Clr => vec!["clr.host".to_string()],
        TargetBackendFamily::Jvm => vec!["jvm.host".to_string()],
        _ => Vec::new(),
    }
}

/// provider 是否满足当前 target capability 集合。
pub(crate) fn provider_matches_target(provider: &PlannedHostProvider, available: &BTreeSet<String>, target: CanonicalTarget) -> bool {
    let required = provider_required_capabilities(provider, target);
    if required.is_empty() {
        // WASI adaptor provider：无 `[js]` / `[wasm]` 时由 target 规格门控（正式 WASI binding 未落地前）。
        return matches!(target.abi, Some(CanonicalAbi::Wasi) | Some(CanonicalAbi::WasiP3));
    }
    required.iter().all(|capability| capability_available(capability, available))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyar::CanonicalTarget;

    #[test]
    fn js_capability_id_maps_fetch_and_wx_request() {
        assert_eq!(js_capability_id("fetch"), "js.fetch");
        assert_eq!(js_capability_id("wx.request"), "js.wx.request");
    }

    #[test]
    fn browser_wasm_includes_fetch_not_wx() {
        let available = target_capability_set(CanonicalTarget::wasm());
        assert!(available.contains("js.fetch"));
        assert!(available.contains("js.random"));
        assert!(!available.contains("js.wx.request"));
    }

    #[test]
    fn miniprogram_quadruple_enables_wx_request_not_fetch() {
        let target: CanonicalTarget = "wasm32-unknown-miniprogram-wasm".parse().unwrap();
        let available = target_capability_set(target);
        assert!(available.contains("js.wx.request"));
        assert!(!available.contains("js.fetch"));
    }

    #[test]
    fn wasi_target_excludes_js_capabilities() {
        let available = target_capability_set(CanonicalTarget::wasi());
        assert!(!available.contains("js.fetch"));
        assert!(!available.contains("js.random"));
        assert!(available.contains("wasm.wasi"));
        assert!(available.contains("wasm.env"));
    }

    #[test]
    fn wasm_binding_maps_env_import_to_capability_id() {
        let caps = required_capabilities_from_foreign_bindings(&[ForeignBinding::Wasm {
            module: "env".into(),
            field: "random_next_i32".into(),
        }]);
        assert_eq!(caps, vec!["wasm.env.random_next_i32".to_string()]);
        let available = target_capability_set(CanonicalTarget::wasi());
        assert!(capability_available("wasm.env.random_next_i32", &available));
    }
}

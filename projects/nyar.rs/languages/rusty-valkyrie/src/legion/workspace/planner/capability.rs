use std::collections::BTreeSet;

use nyar::{CanonicalTarget, CanonicalVendor, TargetBackendFamily, TargetHostKind};
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

/// 判断单个 capability 是否被 target 集合满足。
///
/// 精确匹配优先；`wasm.env.*` 可由 `wasm.env` 模块级授权覆盖；此外，
/// 如果 available 里存在**capability 空间标记**（如 `js`、`wasi`、`node`），
/// 则该空间下的具体 capability（`js.fetch`、`js.random` …）视为可用。
pub(crate) fn capability_available(capability: &str, available: &BTreeSet<String>) -> bool {
    if available.contains(capability) {
        return true;
    }
    // wasm.env.* → wasm.env 空间
    if let Some(rest) = capability.strip_prefix("wasm.") {
        let segments: Vec<_> = rest.split('.').collect();
        if segments.len() >= 2 {
            let module_grant = format!("wasm.{}", segments[0]);
            if available.contains(&module_grant) {
                return true;
            }
        }
    }
    // 通用 capability 空间匹配：`prefix.*` 能在 available 含 `prefix` 时通过
    if let Some(space) = capability.split_once('.').map(|(prefix, _)| prefix) {
        if !space.is_empty() && available.contains(space) {
            return true;
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

/// 由规范化目标四元组推导**宿主原生 capability 空间**——
/// 返回"这个 target 能提供什么类型的能力空间"（如 `js`、`node`、`wasi`），
/// **不**列出 `js.fetch` / `js.random` 等具体 capability contract。
///
/// 具体 capability（如 `js.fetch`）必须由 provider 的 `[js("fetch")]` 声明，
/// planner 根据 provider 声明填进可用集。本函数只负责声明宿主具备哪些能力类型，
/// 避免 target 的默认 capability 集合变成隐式宿主功能白名单。
///
/// # 过渡说明
///
/// 当前 provider 系统尚未完整接入"capability 空间 → 具体 capability"的显式映射：
/// 暂时通过 `capability_available` 的通用前缀规则（`js` 空间包含所有 `js.*`）
/// 来满足具体 capability 匹配。未来 provider registry 细化后应移除前缀规则，
/// 让每个具体 capability 都有明确的 provider 声明。
pub(crate) fn target_host_capability_space(target: CanonicalTarget) -> BTreeSet<String> {
    let profile = target.to_profile(None);
    let mut space = BTreeSet::new();

    // backend 基础空间：Wasm backend 自身的能力载体
    match profile.backend_family {
        TargetBackendFamily::Wasm => {
            space.insert("wasm".to_string());
        }
        TargetBackendFamily::Clr => {
            space.insert("clr".to_string());
        }
        TargetBackendFamily::Jvm => {
            space.insert("jvm".to_string());
        }
        TargetBackendFamily::Native => {
            space.insert("native".to_string());
        }
        TargetBackendFamily::Ovm => {
            space.insert("ovm".to_string());
        }
        _ => {}
    }

    // host 提供的 capability 空间（这才是核心）
    match profile.backend_family {
        TargetBackendFamily::Wasm => match profile.host_kind {
            // 浏览器宿主（specification=Browser）：JS 能力空间
            TargetHostKind::Browser => {
                space.insert("js".to_string());
            }
            // JS 宿主（Node / Deno / Bun / Wechat）：JS + vendor 能力空间
            TargetHostKind::JavaScript => {
                space.insert("js".to_string());
                match target.vendor {
                    nyar::CanonicalVendor::Node | nyar::CanonicalVendor::Deno | nyar::CanonicalVendor::Bun => {
                        space.insert("node".to_string());
                    }
                    nyar::CanonicalVendor::Wechat => {
                        // WeChat runtime：只给 js 空间，不注入额外 "wx" 子空间
                    }
                    _ => {}
                }
            }
            // WASI 宿主：WASI + wasm.env 能力空间
            TargetHostKind::Wasi => {
                space.insert("wasi".to_string());
                space.insert("wasm.env".to_string());
            }
            // Neutral Wasm / Unknown host：只有 backend 空间，无宿主
            _ => {}
        },
        // 托管后端：host 自身就是能力空间
        TargetBackendFamily::Clr => {
            space.insert("clr.host".to_string());
        }
        TargetBackendFamily::Jvm => {
            space.insert("jvm.host".to_string());
        }
        _ => {}
    }

    space
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
        // `[wasm_operator]` / `[vm_opcode]` 等无宿主导入的 adaptor：由 backend family 门控，不靠 `js.*` capability。
        return matches!(
            target.to_profile(None).backend_family,
            TargetBackendFamily::Wasm | TargetBackendFamily::Ovm | TargetBackendFamily::Native
        );
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
    fn browser_wasm_has_js_capability_space_not_wx() {
        // Browser host 提供 JS capability 空间，但不声明任何具体 fetch/random。
        // 具体 capability 由 provider 的 [js("fetch")] / [js("random")] 声明。
        let space = target_host_capability_space(CanonicalTarget::browser());
        assert!(space.contains("js"));
        assert!(space.contains("wasm"));
        assert!(!space.contains("wx"));
        // 具体 capability 不应被 target 隐式注入
        assert!(!space.contains("js.fetch"));
        assert!(!space.contains("js.random"));
    }

    #[test]
    fn wechat_vendor_has_js_but_no_wx_subspace() {
        // WeChat vendor（小程序 runtime）：JS 能力空间，不注入额外 "wx" 子空间。
        let target = CanonicalTarget::wechat();
        let space = target_host_capability_space(target);
        assert!(space.contains("js"));
        assert!(space.contains("wasm"));
        assert!(!space.contains("node"));
        assert!(!space.contains("wx"));
    }

    #[test]
    fn wasi_target_has_wasi_and_wasm_env_space() {
        // WASI：WASI component model 空间 + wasm.env（canonical import）空间。
        let space = target_host_capability_space(CanonicalTarget::wasi());
        assert!(space.contains("wasi"));
        assert!(space.contains("wasm.env"));
        assert!(space.contains("wasm"));
        assert!(!space.contains("js"));
        // 具体的 wasi.* / wasm.env.* 应由 provider 声明
        assert!(!space.contains("wasm.wasi"));
    }

    #[test]
    fn neutral_wasm_has_only_backend_space_no_host() {
        // Neutral wasm：只有 WASM backend 载体，没有任何宿主 capability 空间。
        // 这是 wasm() 别名的新语义——不注入任何 JS/Node/WASI 宿主。
        let space = target_host_capability_space(CanonicalTarget::neutral_wasm());
        assert!(space.contains("wasm"));
        assert!(!space.contains("js"));
        assert!(!space.contains("wasi"));
        assert!(!space.contains("node"));
    }

    #[test]
    fn capability_available_resolves_through_host_capability_space() {
        // 过渡机制：有 "js" 空间 → 所有 js.* capability 可用。
        // 这让 provider 声明的 [js("fetch")] 在 browser target 下能匹配成功。
        let browser_space = target_host_capability_space(CanonicalTarget::browser());
        assert!(capability_available("js.fetch", &browser_space));
        assert!(capability_available("js.random", &browser_space));
        assert!(capability_available("js.wx.request", &browser_space)); // 通用前缀匹配

        let wasi_space = target_host_capability_space(CanonicalTarget::wasi());
        assert!(!capability_available("js.fetch", &wasi_space));
        assert!(capability_available("wasm.env.random_next_i32", &wasi_space));
    }

    #[test]
    fn wasm_opcode_provider_matches_browser_wasm_without_js_binding() {
        use crate::compiler::semantic::closure::{DeclarationKey, DeclarationKind};
        let target = CanonicalTarget::browser();
        let space = target_host_capability_space(target);
        let provider = PlannedHostProvider {
            contract: "core::primitive::__f64_add".into(),
            contract_key: None,
            symbol: "std::adaptor::wasm::math::f64_add".into(),
            key: DeclarationKey {
                dependency_key: "std.adaptor.wasm".into(),
                source_path: "math/arithmetic.v".into(),
                namespace: vec!["std".into(), "adaptor".into(), "wasm".into(), "math".into()],
                kind: DeclarationKind::Micro,
                owner: None,
                name: "f64_add".into(),
            },
            source_file: std::path::PathBuf::from("arithmetic.v"),
            line: 1,
            required_capabilities: Vec::new(),
            foreign_bindings: Vec::new(),
            when_attributes: Vec::new(),
            gate_attributes: Vec::new(),
        };
        assert!(provider_matches_target(&provider, &space, target));
    }

    #[test]
    fn wasm_binding_maps_env_import_to_capability_id() {
        let caps = required_capabilities_from_foreign_bindings(&[ForeignBinding::Wasm {
            module: "env".into(),
            field: "random_next_i32".into(),
        }]);
        assert_eq!(caps, vec!["wasm.env.random_next_i32".to_string()]);
        let space = target_host_capability_space(CanonicalTarget::wasi());
        // wasm.env capability 空间通过 wasm.env 模块前缀匹配
        assert!(capability_available("wasm.env.random_next_i32", &space));
    }
}

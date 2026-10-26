//! 测试目标解析。
use crate::legion::{
    CanonicalTarget,
    cmds::error::{LegionError, LegionResult},
};

/// 解析 `--target` 参数（默认 `legion`，`all` 展开为 legion/clr/jvm/node）。
pub fn resolve_test_targets(target: Option<&str>) -> Vec<String> {
    match target {
        None | Some("") => vec!["legion".into()],
        Some(value) if value.eq_ignore_ascii_case("all") => {
            vec!["legion".into(), "clr".into(), "jvm".into(), "node".into()]
        }
        Some(value) => value.split(',').map(str::trim).filter(|item| !item.is_empty()).map(|item| item.to_ascii_lowercase()).collect(),
    }
}

/// Legion 产品自己的目标标签别名。
///
/// `legion` 是 Legion 产品标签，映射到中性的 `ovm` 架构目标；
/// 该别名由产品层持有，Nyar 的 `CanonicalTarget` 不认识 Legion。
fn product_target_alias(label: &str) -> Option<&'static str> {
    match label.to_ascii_lowercase().as_str() {
        "legion" => Some("ovm"),
        _ => None,
    }
}

/// 将短标签解析为 `CanonicalTarget`。
pub fn parse_target_label(label: &str) -> LegionResult<CanonicalTarget> {
    let canonical = product_target_alias(label).unwrap_or(label);
    canonical.parse::<CanonicalTarget>().map_err(|error| LegionError::msg(report!("无法解析目标 '{label}': {error}")))
}

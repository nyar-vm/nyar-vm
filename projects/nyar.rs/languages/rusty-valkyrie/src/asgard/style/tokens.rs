//! 样式令牌注册表：颜色令牌与文本样式令牌是合同值域的一部分。
//!
//! 令牌是平台无关的语义名；各后端把令牌降级到自身主题系统（dom 后端降级为
//! 具体色值与字体参数，native 后端映射到平台主题资源）。首轮注册表内置，
//! 作者面只能引用、不能重定义。

use super::{Property, StyleContractError};

/// 颜色令牌值（sRGB，dom 降级直接消费；native 映射平台主题）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColorToken {
    /// sRGB 十六进制色值。
    pub hex: &'static str,
}

/// 文本样式令牌：字号（逻辑像素）/ 字重 / 行高倍率。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextToken {
    /// 字号（逻辑像素）。
    pub size: i32,
    /// 字重。
    pub weight: i32,
    /// 行高倍率。
    pub line_height: f32,
}

/// 颜色令牌表。
pub fn color_token(name: &str) -> Option<ColorToken> {
    Some(match name {
        "surface" => ColorToken { hex: "#0f172a" },
        "panel" => ColorToken { hex: "#1e293b" },
        "panel-hover" => ColorToken { hex: "#334155" },
        "brand" => ColorToken { hex: "#38bdf8" },
        "brand-strong" => ColorToken { hex: "#0ea5e9" },
        "ink" => ColorToken { hex: "#f8fafc" },
        "ink-soft" => ColorToken { hex: "#e2e8f0" },
        "ink-muted" => ColorToken { hex: "#94a3b8" },
        "ok" => ColorToken { hex: "#4ade80" },
        "warn" => ColorToken { hex: "#f97316" },
        "danger" => ColorToken { hex: "#ef4444" },
        _ => return None,
    })
}

/// 文本样式令牌表。
pub fn text_token(name: &str) -> Option<TextToken> {
    Some(match name {
        "hero" => TextToken { size: 32, weight: 700, line_height: 1.2 },
        "title" => TextToken { size: 20, weight: 700, line_height: 1.3 },
        "value" => TextToken { size: 26, weight: 700, line_height: 1.2 },
        "body" => TextToken { size: 15, weight: 400, line_height: 1.5 },
        "caption" => TextToken { size: 13, weight: 400, line_height: 1.4 },
        _ => return None,
    })
}

/// 校验令牌引用；未知令牌 fail-closed。
pub fn require_token(property: Property, name: &str) -> Result<(), StyleContractError> {
    let known = match property {
        Property::Background | Property::Color => color_token(name).is_some(),
        Property::Text => text_token(name).is_some(),
        _ => false,
    };
    if known {
        Ok(())
    }
    else {
        Err(StyleContractError::BadValue {
            property: property.name().to_string(),
            value: format!("@{name}"),
            reason: "令牌不在注册表内".to_string(),
        })
    }
}

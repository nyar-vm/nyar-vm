//! native / 自渲染后端的样式合同降级：合同 → 样式结构体，不生成任何 CSS 文本。
//!
//! 阶段三把样式结构体喂平台控件或自有布局引擎；阶段四喂自渲染绘制原语。
//! `web_only` 块在非 dom 后端必须 fail-closed（能力矩阵声明为不支持）。

use super::{Declaration, Property, Selector, State, StyleContract, Value, handle_class};
use crate::asgard::error::{AsgardError, AsgardResult};

/// 平台无关的样式值（native 结构体消费）。
#[derive(Debug, Clone, PartialEq)]
pub enum NativeValue {
    /// 逻辑像素数值。
    Points(i32),
    /// 解析后的颜色值。
    ColorRgb {
        /// 十六进制色值。
        hex: String,
    },
    /// 解析后的文本字阶。
    TextRamp {
        /// 字号（逻辑像素）。
        size: i32,
        /// 字重。
        weight: i32,
        /// 行高倍率。
        line_height: f32,
    },
    /// 封闭关键字。
    Keyword(String),
}

/// 一条 native 样式规则：合同句柄 + 状态 + 属性表。
#[derive(Debug, Clone, PartialEq)]
pub struct NativeStyleRule {
    /// 合同句柄类名。
    pub handle: String,
    /// 可选状态。
    pub state: Option<State>,
    /// 属性表（顺序保留）。
    pub properties: Vec<(Property, NativeValue)>,
}

/// native 样式表：直接喂布局引擎/平台主题的结构体树根。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NativeStyleSheet {
    /// 规则列表。
    pub rules: Vec<NativeStyleRule>,
}

/// 合同 → native 样式结构体；`web_only` 存在即 fail-closed。
pub fn lower_contract(contract: &StyleContract) -> AsgardResult<NativeStyleSheet> {
    if !contract.web_only.is_empty() {
        return Err(AsgardError::msg(format!(
            "样式合同包含 {} 个 web_only 块：非 dom/html 后端必须 fail-closed（移植成本条目见 manifest），不允许静默忽略",
            contract.web_only.len()
        )));
    }
    let rules = contract
        .rules
        .iter()
        .map(|rule| NativeStyleRule {
            handle: handle_class(&rule.selector),
            state: rule.state,
            properties: rule.declarations.iter().filter_map(lower_declaration).collect(),
        })
        .collect();
    Ok(NativeStyleSheet { rules })
}

fn lower_declaration(declaration: &Declaration) -> Option<(Property, NativeValue)> {
    let value = match (&declaration.property, &declaration.value) {
        (Property::Background | Property::Color, Value::Token(name)) => {
            NativeValue::ColorRgb { hex: super::tokens::color_token(name)?.hex.to_string() }
        }
        (Property::Text, Value::Token(name)) => {
            let token = super::tokens::text_token(name)?;
            NativeValue::TextRamp { size: token.size, weight: token.weight, line_height: token.line_height }
        }
        (_, Value::Points(points)) => NativeValue::Points(*points),
        (_, Value::Keyword(word)) => NativeValue::Keyword(word.clone()),
        (_, Value::Token(_)) => return None,
    };
    Some((declaration.property, value))
}

/// 选择器句柄（与 dom 降级共用同一映射，保证跨后端句柄一致）。
pub fn handle_of(selector: &Selector) -> String {
    handle_class(selector)
}

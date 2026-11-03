//! dom/html 后端的样式合同降级：合同 → per-component CSS chunk 文本。
//!
//! CSS 属性名与伪类只允许出现在本模块：它们是 dom 后端的 lowering 实现，
//! 不是合同。状态语义降级为伪类（`pressed→:active` 等）；`web_only` 块原样
//! 追加并登记为移植成本条目。webview 阶段复用本降级。

use super::{Declaration, Property, Selector, State, StyleContract, Value, handle_class, matrix, matrix::StyleCapability, tokens};
use crate::asgard::{
    awsl::LoweredComponent,
    error::{AsgardError, AsgardResult},
    layout::{LayoutBinding, LayoutSpec, capability::BackendStage},
};

/// 组件的 dom 后端样式产物：合同降级 CSS + 度量行盒尺寸。
pub fn component_web_css(component: &LoweredComponent) -> String {
    let Some(contract) = &component.style_contract
    else {
        return String::new();
    };
    let mut out = lower_contract(contract);
    for binding in &component.layout {
        if let LayoutBinding::MeasuredText { class, metrics } = binding {
            out.push_str(&format!(
                ".{class} {{ width: {}px; height: {}px; }}\n",
                metrics.width.0.round() as i32,
                metrics.height.0.round() as i32
            ));
        }
    }
    out
}

/// 合同 → CSS 文本（选择器/状态/属性逐项降级）。
pub fn lower_contract(contract: &StyleContract) -> String {
    let mut out = String::new();
    for rule in &contract.rules {
        let selector = format!(".{}", handle_class(&rule.selector));
        let state_suffix = rule.state.map(state_pseudo_class).unwrap_or_default();
        let body = lower_declarations(&rule.declarations);
        let body = body.trim();
        if body.is_empty() {
            continue;
        }
        out.push_str(&format!("{selector}{state_suffix} {{ {body} }}\n"));
    }
    for raw in &contract.web_only {
        out.push_str(raw);
        out.push('\n');
    }
    out
}

/// dom 后端能力校验：合同用到的能力超出 dom 声明即 fail-closed。
pub fn validate_for_dom(contract: &StyleContract) -> AsgardResult<Vec<&'static str>> {
    let mut capabilities: Vec<StyleCapability> = contract
        .rules
        .iter()
        .flat_map(|rule| {
            rule.declarations
                .iter()
                .map(|declaration| StyleCapability::Property(declaration.property))
                .chain(rule.state.map(StyleCapability::State))
        })
        .collect();
    if !contract.web_only.is_empty() {
        capabilities.push(StyleCapability::WebOnlyEscape);
    }
    matrix::validate(&capabilities, BackendStage::DomHtml).map_err(|error| AsgardError::msg(error.report_text()))
}

/// `web_only` 块的移植成本条目（manifest 登记用）。
pub fn porting_cost_entries(contract: &StyleContract) -> Vec<String> {
    contract
        .web_only
        .iter()
        .enumerate()
        .map(|(index, raw)| format!("web_only[{}]: {} bytes of raw css, dom/html only", index, raw.len()))
        .collect()
}

fn state_pseudo_class(state: State) -> &'static str {
    match state {
        State::Pressed => ":active",
        State::Hover => ":hover",
        State::Focus => ":focus-visible",
        State::Disabled => ":disabled",
    }
}

fn lower_declarations(declarations: &[Declaration]) -> String {
    let mut out = String::new();
    let has_layout = declarations.iter().any(|declaration| matches!(declaration.property, Property::Direction | Property::MainAlign | Property::CrossAlign | Property::Spacing));
    if has_layout {
        out.push_str("display: flex; ");
    }
    for declaration in declarations {
        out.push_str(&lower_declaration(declaration));
    }
    out
}

fn lower_declaration(declaration: &Declaration) -> String {
    let value_text = |value: &Value| match value {
        Value::Points(points) => format!("{points}px"),
        Value::Token(_) | Value::Keyword(_) => String::new(),
    };
    match (&declaration.property, &declaration.value) {
        (Property::Direction, Value::Keyword(word)) => format!("flex-direction: {}; ", if word == "horizontal" { "row" } else { "column" }),
        (Property::MainAlign, Value::Keyword(word)) => format!(
            "justify-content: {}; ",
            match word.as_str() {
                "start" => "flex-start",
                "end" => "flex-end",
                other => other,
            }
        ),
        (Property::CrossAlign, Value::Keyword(word)) => format!(
            "align-items: {}; ",
            match word.as_str() {
                "start" => "flex-start",
                "end" => "flex-end",
                other => other,
            }
        ),
        (Property::Spacing, Value::Points(points)) => format!("gap: {points}px; "),
        (Property::Inset, Value::Points(points)) => format!("padding: {points}px; "),
        (Property::Width, value) => format!("width: {}; ", extent_css(value)),
        (Property::Height, value) => format!("height: {}; ", extent_css(value)),
        (Property::MaxWidth, Value::Points(points)) => format!("max-width: {points}px; "),
        (Property::Radius, Value::Points(points)) => format!("border-radius: {points}px; "),
        (Property::Background, Value::Token(name)) => format!("background: {}; ", tokens::color_token(name).map(|token| token.hex.to_string()).unwrap_or_default()),
        (Property::Color, Value::Token(name)) => format!("color: {}; ", tokens::color_token(name).map(|token| token.hex.to_string()).unwrap_or_default()),
        (Property::Text, Value::Token(name)) => tokens::text_token(name)
            .map(|token| format!("font-size: {}px; font-weight: {}; line-height: {}; ", token.size, token.weight, token.line_height))
            .unwrap_or_default(),
        (Property::Measure, _) => String::new(),
        other => format!("/* unparsed {:?} {} */ ", other.0, value_text(&other.1)),
    }
}

fn extent_css(value: &Value) -> String {
    match value {
        Value::Points(points) => format!("{points}px"),
        Value::Keyword(word) if word == "fill" => "100%".to_string(),
        Value::Keyword(_) => "auto".to_string(),
        Value::Token(_) => "auto".to_string(),
    }
}

/// 从规则声明构造布局声明（容器类选择器的布局子集）。
pub fn layout_spec_of(rule_declarations: &[Declaration]) -> Option<LayoutSpec> {
    let pairs: Vec<(String, String)> = rule_declarations
        .iter()
        .filter(|declaration| {
            matches!(
                declaration.property,
                Property::Direction | Property::MainAlign | Property::CrossAlign | Property::Spacing | Property::Inset | Property::Width | Property::Height
            )
        })
        .map(|declaration| {
            let value = match &declaration.value {
                Value::Points(points) => points.to_string(),
                Value::Keyword(word) => word.clone(),
                Value::Token(token) => format!("@{token}"),
            };
            (declaration.property.name().to_string(), value)
        })
        .collect();
    if pairs.is_empty() {
        return None;
    }
    crate::asgard::layout::LayoutSpec::parse(&pairs, crate::asgard::layout::Direction::Vertical).ok()?
}

/// 选择器是否为容器类（root/slot），id 目标也可以是容器；度量只作用于文本目标。
pub fn selector_handle(selector: &Selector) -> String {
    handle_class(selector)
}

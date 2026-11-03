//! 样式合同 DSL（原型）：`<style>` 块的内容不是 CSS，是本合同的最小文法。
//!
//! 文法（三类选择器 + 状态前缀 + 封闭属性集 + 显式 web 逃生口）：
//!
//! ```text
//! root            { direction: vertical  spacing: 24  background: @surface }
//! slot tabs       { direction: horizontal  spacing: 12 }
//! #hero-title     { text: @hero  color: @ink }
//! state hover on #tab   { background: @panel-hover }
//! web_only        { .legacy { backdrop-filter: blur(4px); } }
//! ```
//!
//! 禁止项一律编译期 fail-closed：标签选择器、后代组合器、通配符、`!important`、
//! CSS 伪类/伪元素、未定义属性、依赖宿主盒模型的相对单位（`%` / `vw` / `em` …）。
//! 状态只用合同语义 `pressed` / `hover` / `focus` / `disabled`，由各后端自行降级。
//! `web_only` 块内容是原始 CSS，但构建到任何非 dom 后端必须 fail-closed，并记入
//! manifest 的移植成本条目（见 [`web::porting_cost_entries`]）。

pub mod matrix;
pub mod native;
pub mod tokens;
pub mod web;

use crate::asgard::error::{AsgardError, AsgardResult};

/// 选择器：只允许三类合同句柄。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selector {
    /// 组件根节点。
    Root,
    /// 命名 slot（元素上的 `slot="name"` 属性）。
    Slot(String),
    /// 显式 id（元素上的 `id="name"` 属性）。
    Id(String),
}

/// 合同状态语义；CSS 伪类只是 dom 后端的降级细节，不进合同。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// 按下态。
    Pressed,
    /// 悬停态。
    Hover,
    /// 焦点态。
    Focus,
    /// 禁用态。
    Disabled,
}

/// 封闭属性集：布局四类 + 文本度量 + 视觉令牌引用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Property {
    /// 主轴方向。
    Direction,
    /// 主轴对齐。
    MainAlign,
    /// 交叉轴对齐。
    CrossAlign,
    /// 子项间距。
    Spacing,
    /// 容器内边距。
    Inset,
    /// 水平轴尺寸约束。
    Width,
    /// 垂直轴尺寸约束。
    Height,
    /// 文本度量请求（构建期行盒计算）。
    Measure,
    /// 度量宽度约束。
    MaxWidth,
    /// 背景色令牌。
    Background,
    /// 前景色令牌。
    Color,
    /// 文本样式令牌。
    Text,
    /// 圆角半径。
    Radius,
}

impl Property {
    fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "direction" => Self::Direction,
            "main-align" => Self::MainAlign,
            "cross-align" => Self::CrossAlign,
            "spacing" => Self::Spacing,
            "inset" => Self::Inset,
            "width" => Self::Width,
            "height" => Self::Height,
            "measure" => Self::Measure,
            "max-width" => Self::MaxWidth,
            "background" => Self::Background,
            "color" => Self::Color,
            "text" => Self::Text,
            "radius" => Self::Radius,
            _ => return None,
        })
    }

    /// 属性名的合同拼写（诊断与矩阵共用）。
    pub fn name(&self) -> &'static str {
        match self {
            Self::Direction => "direction",
            Self::MainAlign => "main-align",
            Self::CrossAlign => "cross-align",
            Self::Spacing => "spacing",
            Self::Inset => "inset",
            Self::Width => "width",
            Self::Height => "height",
            Self::Measure => "measure",
            Self::MaxWidth => "max-width",
            Self::Background => "background",
            Self::Color => "color",
            Self::Text => "text",
            Self::Radius => "radius",
        }
    }
}

/// 合同值：逻辑像素数值、令牌引用、封闭关键字。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// 逻辑像素（裸数值；单位换算归各后端）。
    Points(i32),
    /// 令牌引用（`@name`）。
    Token(String),
    /// 封闭关键字（`horizontal` / `fill` / `content` / `wrap` …）。
    Keyword(String),
}

/// 一条声明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declaration {
    /// 封闭属性集内的属性。
    pub property: Property,
    /// 合同值。
    pub value: Value,
}

/// 一条规则：选择器 + 可选状态 + 声明列表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyleRule {
    /// 三类合同选择器之一。
    pub selector: Selector,
    /// 可选状态前缀。
    pub state: Option<State>,
    /// 声明列表（顺序保留）。
    pub declarations: Vec<Declaration>,
}

/// 解析后的样式合同。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StyleContract {
    /// 规则列表（顺序保留）。
    pub rules: Vec<StyleRule>,
    /// `web_only` 块的原始 CSS 文本（逐块保留，供 dom 降级与移植成本登记）。
    pub web_only: Vec<String>,
}

/// 合同解析错误：全部 fail-closed，不忽略未知属性。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StyleContractError {
    /// 选择器形态非法（标签/组合器/通配符/伪类等）。
    BadSelector {
        /// 被拒选择器原文。
        text: String,
    },
    /// 状态名不在合同语义内。
    BadState {
        /// 被拒状态名原文。
        text: String,
    },
    /// 属性不在封闭集合内。
    UnknownProperty {
        /// 被拒属性名。
        name: String,
    },
    /// 属性取值非法（含相对单位、未知关键字、未知令牌）。
    BadValue {
        /// 属性名。
        property: String,
        /// 被拒取值原文。
        value: String,
        /// 拒绝原因。
        reason: String,
    },
    /// 出现 `!important`。
    ImportantForbidden,
    /// 文法结构错误（括号不配对等）。
    Malformed {
        /// 结构错误细节。
        detail: String,
    },
}

impl StyleContractError {
    /// 结构化诊断文本。
    pub fn report_text(&self) -> String {
        match self {
            Self::BadSelector { text } => format!("样式合同拒绝选择器 `{text}`：只允许 root / slot <name> / #<id>"),
            Self::BadState { text } => format!("样式合同拒绝状态 `{text}`：只允许 pressed / hover / focus / disabled"),
            Self::UnknownProperty { name } => format!("样式合同拒绝未定义属性 `{name}`：属性集合封闭，不做忽略"),
            Self::BadValue { property, value, reason } => format!("样式合同拒绝 `{property}: {value}`：{reason}"),
            Self::ImportantForbidden => "样式合同禁止 `!important`".to_string(),
            Self::Malformed { detail } => format!("样式合同文法错误：{detail}"),
        }
    }

    /// 结构化诊断。
    pub fn report(&self) -> AsgardError {
        AsgardError::msg(self.report_text())
    }
}

/// 解析 `<style>` 内容为样式合同；任何越界写法 fail-closed。
pub fn parse_style_contract(source: &str) -> Result<StyleContract, StyleContractError> {
    if source.contains("!important") {
        return Err(StyleContractError::ImportantForbidden);
    }
    let mut contract = StyleContract::default();
    let mut rest = source.trim();
    while !rest.is_empty() {
        let open = rest.find('{').ok_or_else(|| StyleContractError::Malformed { detail: "缺少 `{`".to_string() })?;
        let head = rest[..open].trim();
        let close = find_block_end(&rest[open..]).ok_or_else(|| StyleContractError::Malformed { detail: "括号不配对".to_string() })?;
        let body = &rest[open + 1..open + close - 1];
        rest = rest[open + close..].trim();
        if head == "web_only" {
            contract.web_only.push(body.trim().to_string());
            continue;
        }
        contract.rules.push(parse_rule(head, body)?);
    }
    Ok(contract)
}

/// 找到与首个 `{` 配对的 `}` 的偏移（含该 `}`，长度语义）。
fn find_block_end(block: &str) -> Option<usize> {
    let mut depth = 0;
    for (offset, ch) in block.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(offset + 1);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_rule(head: &str, body: &str) -> Result<StyleRule, StyleContractError> {
    let (state, selector_text) = match head.strip_prefix("state ") {
        Some(tail) => {
            let (state_name, selector_part) =
                tail.split_once(" on ").ok_or_else(|| StyleContractError::Malformed { detail: format!("状态规则缺少 `on`：`{head}`") })?;
            let state = match state_name.trim() {
                "pressed" => State::Pressed,
                "hover" => State::Hover,
                "focus" => State::Focus,
                "disabled" => State::Disabled,
                other => return Err(StyleContractError::BadState { text: other.to_string() }),
            };
            (Some(state), selector_part.trim())
        }
        None => (None, head),
    };
    let selector = parse_selector(selector_text)?;
    let mut declarations = Vec::new();
    for line in body.split([';', '\n']) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (name, value) = line.split_once(':').ok_or_else(|| StyleContractError::Malformed { detail: format!("声明缺少 `:`：`{line}`") })?;
        declarations.push(parse_declaration(name.trim(), value.trim())?);
    }
    Ok(StyleRule { selector, state, declarations })
}

fn parse_selector(text: &str) -> Result<Selector, StyleContractError> {
    if text.contains("::") || text.contains('*') || text.contains('>') || text.contains('~') || text.contains('+') {
        return Err(StyleContractError::BadSelector { text: text.to_string() });
    }
    if text == "root" {
        return Ok(Selector::Root);
    }
    if let Some(name) = text.strip_prefix("slot ") {
        let name = name.trim();
        if !name.is_empty() && !name.contains(' ') {
            return Ok(Selector::Slot(name.to_string()));
        }
        return Err(StyleContractError::BadSelector { text: text.to_string() });
    }
    if let Some(name) = text.strip_prefix('#') {
        let name = name.trim();
        if !name.is_empty() && !name.contains(' ') {
            return Ok(Selector::Id(name.to_string()));
        }
        return Err(StyleContractError::BadSelector { text: text.to_string() });
    }
    Err(StyleContractError::BadSelector { text: text.to_string() })
}

fn parse_declaration(name: &str, value: &str) -> Result<Declaration, StyleContractError> {
    let property = Property::parse(name).ok_or_else(|| StyleContractError::UnknownProperty { name: name.to_string() })?;
    for unit in ["%", "vw", "vh", "em", "rem", "px"] {
        if value.ends_with(unit) {
            return Err(StyleContractError::BadValue {
                property: property.name().to_string(),
                value: value.to_string(),
                reason: format!("禁止依赖宿主盒模型的单位 `{unit}`；数值即逻辑像素，弹性尺寸走 width/height 合同关键字"),
            });
        }
    }
    let value = match property {
        Property::Direction => keyword(value, &["horizontal", "vertical"])?,
        Property::MainAlign => keyword(value, &["start", "center", "end", "space-between", "space-around"])?,
        Property::CrossAlign => keyword(value, &["start", "center", "end", "stretch"])?,
        Property::Width | Property::Height => match value {
            "fill" | "content" => Value::Keyword(value.to_string()),
            other => Value::Points(parse_points(other)?),
        },
        Property::Measure => keyword(value, &["wrap"])?,
        Property::Spacing | Property::Inset | Property::MaxWidth | Property::Radius => Value::Points(parse_points(value)?),
        Property::Background | Property::Color | Property::Text => {
            let token = value
                .strip_prefix('@')
                .ok_or_else(|| StyleContractError::BadValue {
                    property: property.name().to_string(),
                    value: value.to_string(),
                    reason: "颜色/文本样式只接受令牌引用 `@name`".to_string(),
                })?
                .to_string();
            tokens::require_token(property, &token)?;
            Value::Token(token)
        }
    };
    Ok(Declaration { property, value })
}

fn keyword(value: &str, allowed: &[&str]) -> Result<Value, StyleContractError> {
    if allowed.contains(&value) {
        Ok(Value::Keyword(value.to_string()))
    }
    else {
        Err(StyleContractError::BadValue {
            property: value.to_string(),
            value: value.to_string(),
            reason: format!("关键字不在封闭集合 {:?}", allowed),
        })
    }
}

fn parse_points(value: &str) -> Result<i32, StyleContractError> {
    value.parse::<i32>().map_err(|_| StyleContractError::BadValue {
        property: value.to_string(),
        value: value.to_string(),
        reason: "必须是逻辑像素整数".to_string(),
    })
}

/// 合同错误的诊断出口。
pub fn contract_result<T>(value: Result<T, StyleContractError>) -> AsgardResult<T> {
    value.map_err(|error| error.report())
}

/// 选择器对应的合同句柄类名（dom 降级与元素降级共用同一映射）。
pub fn handle_class(selector: &Selector) -> String {
    match selector {
        Selector::Root => "sc-root".to_string(),
        Selector::Slot(name) => format!("sc-slot-{name}"),
        Selector::Id(name) => format!("sc-id-{name}"),
    }
}

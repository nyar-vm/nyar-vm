//! 平台无关布局合同（原型）。
//!
//! Asgard 的宿主路线是 `dom/html → webview → native → 自渲染` 四阶段；本合同是
//! 四个阶段共享的布局语义唯一事实源，只覆盖四类概念加文本度量：
//! 方向（主轴/交叉轴）、间距、对齐、尺寸约束、文本度量。
//!
//! 合同层**禁止**出现任何宿主专有名词：CSS 属性名、盒模型怪癖、样式块或选择器
//! 都不得进入本模块的类型与语言 API。CSS 只是 dom/html 后端（以及复用它的
//! webview 阶段）的 lowering 实现之一，见 [`web`]；native 与自渲染阶段各自降级，
//! 见 [`native`]。文本度量由布局层自持：测量结果是布局层输入，字体只是度量
//! 后端（[`measure::FontMetricsBackend`]），行盒计算不交给任何宿主。
//!
//! 每个后端阶段声明能力矩阵（[`capability`]）；合同用到的能力超出当前后端
//! 声明范围时 fail-closed 报错，不允许静默降级成「看起来差不多」。

pub mod capability;
pub mod fonts;
pub mod measure;
pub mod native;
pub mod web;

use crate::asgard::error::{AsgardError, AsgardResult};

/// 主轴方向。`Column` 语义即 `Vertical`，`Row` 语义即 `Horizontal`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// 主轴水平。
    Horizontal,
    /// 主轴垂直。
    Vertical,
}

/// 主轴对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainAlignment {
    /// 主轴起点对齐。
    Start,
    /// 主轴居中。
    Center,
    /// 主轴终点对齐。
    End,
    /// 主轴两端对齐、中间均分。
    SpaceBetween,
    /// 主轴每项两侧均分。
    SpaceAround,
}

/// 交叉轴对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossAlignment {
    /// 交叉轴起点对齐。
    Start,
    /// 交叉轴居中。
    Center,
    /// 交叉轴终点对齐。
    End,
    /// 交叉轴拉伸填满。
    Stretch,
}

/// 绝对长度，单位 point（与设备像素解耦，由各后端按自身密度换算）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Points(pub f32);

/// 间距：子项之间的主/交叉轴间距与容器内边距。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Spacing {
    /// 子项间距；缺席表示未声明。
    pub between: Option<Points>,
    /// 容器内边距；缺席表示未声明。
    pub inset: Option<Points>,
}

/// 单轴尺寸约束。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extent {
    /// 固定尺寸（逻辑像素）。
    Fixed(i32),
    /// 占满父容器该轴可用空间。
    Fill,
    /// 由内容（含文本度量结果）决定。
    Content,
}

impl Default for Extent {
    fn default() -> Self {
        Extent::Content
    }
}

/// 两轴尺寸约束。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SizeConstraint {
    /// 主轴/水平轴尺寸约束。
    pub width: Extent,
    /// 交叉轴/垂直轴尺寸约束。
    pub height: Extent,
}

/// 一个容器的完整布局声明：四类平台无关概念。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutSpec {
    /// 主轴方向。
    pub direction: Direction,
    /// 主轴对齐。
    pub main_alignment: MainAlignment,
    /// 交叉轴对齐。
    pub cross_alignment: CrossAlignment,
    /// 间距声明。
    pub spacing: Spacing,
    /// 尺寸约束。
    pub size: SizeConstraint,
}

/// 合同错误：作者面取值越界或度量输入非法，一律结构化失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutContractError {
    /// 属性取值不在合同枚举内。
    UnknownValue {
        /// 属性名。
        attr: String,
        /// 被拒取值原文。
        value: String,
    },
    /// 数值属性不是合法 point 数。
    BadLength {
        /// 属性名。
        attr: String,
        /// 被拒长度原文。
        value: String,
    },
    /// 度量请求的输入非法（如动态文本请求构建期度量）。
    Unmeasurable {
        /// 拒绝原因。
        reason: String,
    },
}

impl LayoutContractError {
    /// 结构化诊断文本。
    pub fn report_text(&self) -> String {
        match self {
            Self::UnknownValue { attr, value } => format!("布局合同拒绝 `{attr}` 的取值 `{value}`：不在平台无关枚举内"),
            Self::BadLength { attr, value } => format!("布局合同拒绝 `{attr}` 的长度 `{value}`：必须是 point 数值"),
            Self::Unmeasurable { reason } => format!("布局合同拒绝文本度量请求：{reason}"),
        }
    }

    /// 结构化诊断。
    pub fn report(&self) -> AsgardError {
        AsgardError::msg(self.report_text())
    }
}

/// 布局合同绑定：降级阶段消费的最小单元。
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutBinding {
    /// 容器布局声明，`class` 是降级阶段寻址该节点的合同句柄。
    Container {
        /// 合同句柄类名。
        class: String,
        /// 布局声明。
        spec: LayoutSpec,
    },
    /// 构建期已完成度量的文本节点（行盒是布局层产物，不是宿主产物）。
    MeasuredText {
        /// 合同句柄类名。
        class: String,
        /// 度量结果（行盒与 intrinsic 尺寸）。
        metrics: measure::TextMetrics,
    },
    /// 度量请求被拒绝（如动态文本）：降级阶段必须 fail-closed。
    MeasureRejected {
        /// 合同句柄类名。
        class: String,
        /// 拒绝原因。
        reason: String,
    },
}

impl LayoutSpec {
    /// 从语言面属性解析布局声明；无布局属性返回 `None`。
    ///
    /// 属性名是平台无关词：`direction` / `main-align` / `cross-align` /
    /// `spacing` / `inset` / `width` / `height`。未知取值与非法长度 fail-closed。
    pub fn parse(attrs: &[(String, String)], default_direction: Direction) -> Result<Option<Self>, LayoutContractError> {
        let mut seen = false;
        let mut spec = LayoutSpec {
            direction: default_direction,
            main_alignment: MainAlignment::Start,
            cross_alignment: CrossAlignment::Start,
            spacing: Spacing::default(),
            size: SizeConstraint::default(),
        };
        for (name, value) in attrs {
            let value = value.trim();
            match name.as_str() {
                "direction" => {
                    spec.direction = match value {
                        "horizontal" => Direction::Horizontal,
                        "vertical" => Direction::Vertical,
                        other => return Err(LayoutContractError::UnknownValue { attr: name.clone(), value: other.to_string() }),
                    };
                    seen = true;
                }
                "main-align" => {
                    spec.main_alignment = match value {
                        "start" => MainAlignment::Start,
                        "center" => MainAlignment::Center,
                        "end" => MainAlignment::End,
                        "space-between" => MainAlignment::SpaceBetween,
                        "space-around" => MainAlignment::SpaceAround,
                        other => return Err(LayoutContractError::UnknownValue { attr: name.clone(), value: other.to_string() }),
                    };
                    seen = true;
                }
                "cross-align" => {
                    spec.cross_alignment = match value {
                        "start" => CrossAlignment::Start,
                        "center" => CrossAlignment::Center,
                        "end" => CrossAlignment::End,
                        "stretch" => CrossAlignment::Stretch,
                        other => return Err(LayoutContractError::UnknownValue { attr: name.clone(), value: other.to_string() }),
                    };
                    seen = true;
                }
                "spacing" => {
                    spec.spacing.between = Some(parse_points(name, value)?);
                    seen = true;
                }
                "inset" => {
                    spec.spacing.inset = Some(parse_points(name, value)?);
                    seen = true;
                }
                "width" => {
                    spec.size.width = parse_extent(name, value)?;
                    seen = true;
                }
                "height" => {
                    spec.size.height = parse_extent(name, value)?;
                    seen = true;
                }
                _ => {}
            }
        }
        Ok(seen.then_some(spec))
    }

    /// 本声明用到的合同能力集合（供能力矩阵校验）。
    pub fn capabilities(&self) -> Vec<capability::LayoutCapability> {
        use capability::LayoutCapability::*;
        let mut caps = vec![Direction, Spacing, Alignment, SizeConstraint];
        if matches!(self.size.width, Extent::Content) || matches!(self.size.height, Extent::Content) {
            caps.push(TextMeasurement);
        }
        caps
    }
}

fn parse_points(attr: &str, value: &str) -> Result<Points, LayoutContractError> {
    value
        .parse::<f32>()
        .map(Points)
        .map_err(|_| LayoutContractError::BadLength { attr: attr.to_string(), value: value.to_string() })
}

fn parse_extent(attr: &str, value: &str) -> Result<Extent, LayoutContractError> {
    match value {
        "fill" => Ok(Extent::Fill),
        "content" => Ok(Extent::Content),
        other => other
            .parse::<i32>()
            .map(Extent::Fixed)
            .map_err(|_| LayoutContractError::UnknownValue { attr: attr.to_string(), value: other.to_string() }),
    }
}

/// 合同错误的诊断出口。
pub fn contract_result<T>(value: Result<T, LayoutContractError>) -> AsgardResult<T> {
    value.map_err(|error| error.report())
}

/// 布局属性名集合：语言面只暴露这些平台无关词。
pub const LAYOUT_ATTR_NAMES: &[&str] = &["direction", "main-align", "cross-align", "spacing", "inset", "width", "height"];

/// 文本度量属性名：`measure="wrap"` 请求构建期度量，`max-width` 给出度量约束。
pub const MEASURE_ATTR_NAMES: &[&str] = &["measure", "max-width"];

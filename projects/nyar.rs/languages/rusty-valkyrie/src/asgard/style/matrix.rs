//! 能力矩阵（草案）：样式/布局合同属性 × 四个后端阶段的覆盖声明。
//!
//! 阶段路线：`dom/html → webview → native → 自渲染`。矩阵是防还债机制：
//! 合同用到的能力超出当前后端声明范围时 fail-closed，不允许静默降级。
//!
//! | 能力 | dom/html（阶段1 定死） | webview（阶段2） | native（阶段3 桩） | 自渲染（阶段4 桩） |
//! |---|---|---|---|---|
//! | direction | 支持·定死 | 支持 | 支持 | 支持 |
//! | spacing / inset | 支持·定死 | 支持 | 支持 | 支持 |
//! | main-align / cross-align | 支持·定死 | 支持 | 近似 | 支持 |
//! | width / height | 支持·定死 | 支持 | 近似 | 支持 |
//! | background / color / radius | 支持 | 支持 | 近似 | 支持 |
//! | text 令牌 | 支持 | 支持 | 近似 | 支持 |
//! | 状态 pressed/focus/disabled | 支持 | 支持 | 近似 | 支持 |
//! | 状态 hover | 支持 | 支持 | 近似（触摸宿主无 hover） | 支持 |
//! | 文本度量 measure/max-width | 近似（静态可构建期度量；动态待宿主度量后端） | 近似 | 不支持（桩） | 支持（自持） |
//! | web_only 逃生口 | 支持 | 不支持·fail-closed | 不支持·fail-closed | 不支持·fail-closed |

use super::{Property, State};
use crate::asgard::layout::capability::{BackendStage, Support};

/// 合同能力项：属性族或状态族。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StyleCapability {
    /// 单个封闭属性的能力。
    Property(Property),
    /// 单个状态语义的能力。
    State(State),
    /// web_only 逃生口的能力（仅 dom/html）。
    WebOnlyEscape,
}

/// 查询某能力在某后端阶段的声明覆盖。
pub fn support(capability: StyleCapability, stage: BackendStage) -> Support {
    use BackendStage::*;
    use Support::*;
    match capability {
        StyleCapability::WebOnlyEscape => match stage {
            DomHtml => Supported,
            _ => Unsupported { reason: "web_only 块只属于 dom/html 后端；非 dom 构建必须 fail-closed 并登记移植成本" },
        },
        StyleCapability::State(State::Hover) => match stage {
            DomHtml | WebView => Supported,
            Native => Approximate { reason: "触摸类平台控件没有 hover 语义，映射到 pressed 或忽略由平台合同决定" },
            SelfRendered => Supported,
        },
        StyleCapability::State(_) => match stage {
            DomHtml | WebView => Supported,
            Native => Approximate { reason: "平台控件状态集合各异，映射表由平台合同冻结" },
            SelfRendered => Supported,
        },
        StyleCapability::Property(property) => match property {
            Property::Direction | Property::Spacing | Property::Inset => Supported,
            Property::MainAlign | Property::CrossAlign | Property::Width | Property::Height => match stage {
                Native => Approximate { reason: "平台控件对齐/尺寸集合受限，近似映射须显式声明" },
                _ => Supported,
            },
            Property::Background | Property::Color | Property::Radius | Property::Text => match stage {
                Native => Approximate { reason: "映射到平台主题资源，色域与字阶差异由平台合同冻结" },
                _ => Supported,
            },
            Property::Measure | Property::MaxWidth => match stage {
                DomHtml | WebView => Approximate { reason: "静态文本构建期度量；动态文本运行期度量待宿主度量后端" },
                Native => Unsupported { reason: "native 桩阶段没有度量后端接入" },
                SelfRendered => Supported,
            },
        },
    }
}

/// 阶段一（dom/html）定死的能力集合：语义自此冻结，后续阶段不得改义。
pub const STAGE1_FROZEN: &[Property] = &[
    Property::Direction,
    Property::Spacing,
    Property::Inset,
    Property::MainAlign,
    Property::CrossAlign,
    Property::Width,
    Property::Height,
];

/// 校验一组能力在某阶段是否可用；`Unsupported` fail-closed，`Approximate` 收集返回。
pub fn validate(capabilities: &[StyleCapability], stage: BackendStage) -> Result<Vec<&'static str>, super::StyleContractError> {
    let mut approximations = Vec::new();
    for capability in capabilities {
        match support(*capability, stage) {
            Support::Supported => {}
            Support::Approximate { reason } => approximations.push(reason),
            Support::Unsupported { reason } => {
                return Err(super::StyleContractError::BadValue {
                    property: format!("{capability:?}"),
                    value: format!("{stage:?}"),
                    reason: reason.to_string(),
                })
            }
        }
    }
    Ok(approximations)
}

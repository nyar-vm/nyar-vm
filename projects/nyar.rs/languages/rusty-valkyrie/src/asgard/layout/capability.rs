//! 后端阶段与能力声明：布局合同的 fail-closed 闸门。

/// 宿主路线四阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendStage {
    /// 阶段一：dom/html（CSS 只是本阶段的 lowering 实现之一）。
    DomHtml,
    /// 阶段二：webview（复用合同与 CSS 降级；事件/窗口语义另立合同）。
    WebView,
    /// 阶段三：native（平台控件或自有布局引擎）。
    Native,
    /// 阶段四：自渲染（布局引擎 + 绘制原语，文本度量自持）。
    SelfRendered,
}

/// 能力覆盖声明。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// 完整支持。
    Supported,
    /// 近似支持：语义有损但映射显式声明，原因必须可追溯。
    Approximate {
        /// 近似原因（进入构建报告与诊断）。
        reason: &'static str,
    },
    /// 不支持：合同用到即 fail-closed。
    Unsupported {
        /// 不支持原因（进入 fail-closed 诊断）。
        reason: &'static str,
    },
}

/// 布局合同的四类能力加文本度量。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutCapability {
    /// 主轴/交叉轴方向。
    Direction,
    /// 子项间距与容器内边距。
    Spacing,
    /// 主轴/交叉轴对齐。
    Alignment,
    /// 两轴尺寸约束。
    SizeConstraint,
    /// 文本度量（行盒与 intrinsic 尺寸）。
    TextMeasurement,
}

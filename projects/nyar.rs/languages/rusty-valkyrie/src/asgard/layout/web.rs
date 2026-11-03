//! dom/html 后端的布局降级：合同 → CSS 声明片段。
//!
//! CSS 属性名只允许出现在本模块（与 [`crate::asgard::style::web`]）：它们是
//! lowering 实现细节，不是合同。webview 阶段复用本降级；native / 自渲染不经过这里。

use super::{CrossAlignment, Direction, Extent, LayoutSpec, MainAlignment};

/// 把布局声明降级为 CSS 声明块内容（不含选择器与花括号）。
pub fn lower_spec_css(spec: &LayoutSpec) -> String {
    let mut out = String::from("display: flex;");
    out.push_str(match spec.direction {
        Direction::Horizontal => " flex-direction: row;",
        Direction::Vertical => " flex-direction: column;",
    });
    out.push_str(match spec.main_alignment {
        MainAlignment::Start => " justify-content: flex-start;",
        MainAlignment::Center => " justify-content: center;",
        MainAlignment::End => " justify-content: flex-end;",
        MainAlignment::SpaceBetween => " justify-content: space-between;",
        MainAlignment::SpaceAround => " justify-content: space-around;",
    });
    out.push_str(match spec.cross_alignment {
        CrossAlignment::Start => " align-items: flex-start;",
        CrossAlignment::Center => " align-items: center;",
        CrossAlignment::End => " align-items: flex-end;",
        CrossAlignment::Stretch => " align-items: stretch;",
    });
    if let Some(gap) = spec.spacing.between {
        out.push_str(&format!(" gap: {}px;", gap.0));
    }
    if let Some(inset) = spec.spacing.inset {
        out.push_str(&format!(" padding: {}px;", inset.0));
    }
    out.push_str(&extent_css("width", &spec.size.width));
    out.push_str(&extent_css("height", &spec.size.height));
    out
}

fn extent_css(axis: &str, extent: &Extent) -> String {
    match extent {
        Extent::Fixed(points) => format!(" {axis}: {points}px;"),
        // Fill 在主轴与交叉轴的 lowering 不同：主轴靠 stretch 语义，交叉靠 100%；
        // 这是 dom 后端的盒模型细节，合同层只有 Fill 一个语义。
        Extent::Fill => format!(" {axis}: 100%;"),
        Extent::Content => format!(" {axis}: auto;"),
    }
}

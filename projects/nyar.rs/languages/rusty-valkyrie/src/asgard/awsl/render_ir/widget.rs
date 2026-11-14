//! intrinsic 组件的冻结身份与 Web 宿主投影。
//!
//! 模板标签到 `IntrinsicWidget` 的映射是**唯一**的内置组件身份表：lowering 用它
//! 判定 `TemplateNodeKind::Intrinsic`，RenderIR 在 canonicalize 时把身份冻结进
//! [`RenderElement`](super::RenderElement)，各 codegen 只按身份投影到自己的宿主
//! 原语，不再各自持有标签字符串表。

/// 内置组件身份。
///
/// 身份在 canonicalize 阶段由标签一次性冻结；下游不按拼写二次分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntrinsicWidget {
    /// 纵向容器。
    Column,
    /// 通用容器盒。
    Box,
    /// 横向容器。
    Row,
    /// 文本。
    Text,
    /// 按钮。
    Button,
    /// 输入框。
    Input,
    /// 弹性容器。
    Flex,
    /// 插槽占位。
    Slot,
    /// 列表容器。
    List,
    /// 列表项。
    Item,
    /// 复选框。
    Checkbox,
    /// 单选项。
    Radio,
    /// 单选组。
    RadioGroup,
}

impl IntrinsicWidget {
    /// 按源码标签解析内置组件身份。
    ///
    /// 大小写不敏感（同时接受 `Column` 与 `column`、`RadioGroup` 与 `radiogroup`），
    /// 这是唯一一处标签拼写表；`None` 表示不是内置组件。
    pub fn from_tag(tag: &str) -> Option<Self> {
        let widget = match tag.to_ascii_lowercase().as_str() {
            "column" => Self::Column,
            "box" => Self::Box,
            "row" => Self::Row,
            "text" => Self::Text,
            "button" => Self::Button,
            "input" => Self::Input,
            "flex" => Self::Flex,
            "slot" => Self::Slot,
            "list" => Self::List,
            "item" => Self::Item,
            "checkbox" => Self::Checkbox,
            "radio" => Self::Radio,
            "radiogroup" => Self::RadioGroup,
            _ => return None,
        };
        Some(widget)
    }

    /// Web/DOM 宿主元素投影（HTML 与 SSG/VRender 发射器共用）。
    ///
    /// 当前 Web 宿主没有 `list`/`item`/`checkbox`/`radio`/`radiogroup` 的原生
    /// 元素，保留其小写同名标签即为既有宿主事实；投影变化必须同时改所有 Web 发射器。
    pub fn html_element(self) -> &'static str {
        match self {
            Self::Text => "span",
            Self::Button => "button",
            Self::Input => "input",
            Self::Column | Self::Box | Self::Row | Self::Flex | Self::Slot => "div",
            Self::List => "list",
            Self::Item => "item",
            Self::Checkbox => "checkbox",
            Self::Radio => "radio",
            Self::RadioGroup => "radiogroup",
        }
    }

    /// 序列化载荷里的稳定身份码。
    ///
    /// 宿主运行时按该码分派组件，不再对序列化的标签字符串做二次匹配。
    /// 码值属于 wire 合同：已发布值不得复用或重排。
    pub fn wire_tag(self) -> u8 {
        match self {
            Self::Column => 1,
            Self::Box => 2,
            Self::Row => 3,
            Self::Text => 4,
            Self::Button => 5,
            Self::Input => 6,
            Self::Flex => 7,
            Self::Slot => 8,
            Self::List => 9,
            Self::Item => 10,
            Self::Checkbox => 11,
            Self::Radio => 12,
            Self::RadioGroup => 13,
        }
    }

    /// 按 wire 身份码还原组件身份；未知码返回 `None`。
    pub fn from_wire_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            1 => Self::Column,
            2 => Self::Box,
            3 => Self::Row,
            4 => Self::Text,
            5 => Self::Button,
            6 => Self::Input,
            7 => Self::Flex,
            8 => Self::Slot,
            9 => Self::List,
            10 => Self::Item,
            11 => Self::Checkbox,
            12 => Self::Radio,
            13 => Self::RadioGroup,
            _ => return None,
        })
    }
}

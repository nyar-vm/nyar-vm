//! 文本载体身份归一的唯一判定点。
//!
//! 文本编码身份只有一个正式来源：std 声明携带的 `[primitive("std::text::…")]` 自举
//! 注解（与数值 primitive 同一通道）。本模块把该规范身份映射到编码家族，HIR 类型关系、
//! 控制流校验与 MIR 验证共用这一份映射；用户自定义的同名声明没有该身份，不获得文本语义。
//! 只归一同一编码：`Utf8` 不接受 `Utf16Text`，反之亦然；共享载体 `std::text::text`
//! 两族都接受，字面量默认落 `Utf8`。带泛型实参的名义声明由调用方先行排除。

use crate::compiler::types::hir::{HirTextEncoding, ValkyrieType};

/// `[primitive("…")]` 文本身份对应的编码家族。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextPrimitiveFamily {
    /// 仅 UTF-8 载体（`std::text::utf8`）。
    Utf8,
    /// 仅 UTF-16 载体（`std::text::utf16`）。
    Utf16,
    /// 两种编码共享的载体（`std::text::text`）。
    Shared,
}

/// 按规范身份查文本家族；表外一律 `None`，调用方不得回退到声明拼写。
pub(crate) fn text_primitive_family(identity: &str) -> Option<TextPrimitiveFamily> {
    match identity {
        "std::text::utf8" => Some(TextPrimitiveFamily::Utf8),
        "std::text::utf16" => Some(TextPrimitiveFamily::Utf16),
        "std::text::text" => Some(TextPrimitiveFamily::Shared),
        _ => None,
    }
}

impl TextPrimitiveFamily {
    /// 该文本身份是否锚定这个内建编码 carrier。
    pub(crate) fn anchors(self, carrier: &ValkyrieType) -> bool {
        matches!(
            (self, carrier),
            (Self::Utf8, ValkyrieType::Utf8) | (Self::Utf16, ValkyrieType::Utf16) | (Self::Shared, ValkyrieType::Utf8 | ValkyrieType::Utf16)
        )
    }

    /// 字面量/期望的编码：共享载体按默认规则落 `Utf8`。
    pub(crate) fn encoding(self) -> HirTextEncoding {
        match self {
            Self::Utf16 => HirTextEncoding::Utf16,
            Self::Utf8 | Self::Shared => HirTextEncoding::Utf8,
        }
    }
}

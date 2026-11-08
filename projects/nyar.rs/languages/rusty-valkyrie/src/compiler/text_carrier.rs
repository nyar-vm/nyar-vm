//! 文本载体身份归一的唯一判定点。
//!
//! 源码 `type utf8 = Utf8Text` 把编码锚定到名义声明上；生成组按 §4 规则不回填该
//! 别名，`utf8` 位置回落到内建 carrier `Utf8`（`utf16` 同理）。HIR 绑定与 MIR
//! 验证必须共用同一份名集，不在各层各建白名单。只归一同一编码：`Utf8` 不接受
//! `Utf16Text`，反之亦然；带泛型实参的名义声明由调用方先行排除。

use crate::compiler::types::hir::ValkyrieType;

/// 内建文本 carrier 是否锚定到该名义声明名。
pub(crate) fn carrier_matches_declaration_name(builtin: &ValkyrieType, declaration_name: &str) -> bool {
    match builtin {
        // `text` 是两种编码共享的载体名。
        ValkyrieType::Utf8 => matches!(declaration_name, "text" | "utf8" | "Utf8Text" | "std::text::text" | "std.text.text"),
        ValkyrieType::Utf16 => matches!(declaration_name, "text" | "utf16" | "Utf16Text" | "std::text::text" | "std.text.text"),
        _ => false,
    }
}

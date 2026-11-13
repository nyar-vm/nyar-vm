//! 程序级文本值载体表示事实的唯一判定。
//!
//! 文本值空间只有两种正式来源：源程序在 `std.text` 下声明并登记了 canonical
//! 文本载体（[`crate::canonical_program::LinkedSemanticProgram::text_encoding_types`]）时为托管对象；未登记
//! 任何载体时，`utf8` / `utf16` 值就是宿主编码基元本身。判定只读这份声明与值类型
//! 事实，不读布局注册表——布局缺失必须作为目标层的 missing 事实失败闭合，不能
//! 反过来成为 raw 的依据。

use super::{CanonicalConstant, CanonicalOperation, CanonicalPrimitiveType, CanonicalProgram, CanonicalTypeKind};
use crate::{TypeId, semantic_ids::layout_choice::TextRepresentation};

/// 由 canonical 声明与值类型事实判定程序级文本表示。
///
/// - 登记了 `text_encoding_types` 时返回 [`TextRepresentation::Managed`]；
/// - 未登记载体但存在文本值（`value_types` / `parameters` / `return_type` 是
///   `utf8` / `utf16` 基元，或任何 `LoadConstant` 携带 UTF-8 / UTF-16 文本）时
///   返回 [`TextRepresentation::Raw`]；
/// - 程序没有任何文本值时为 `None`。
///
/// 判定只读声明与值类型事实，不读布局表。
pub fn declared_text_representation(canonical: &CanonicalProgram) -> Option<TextRepresentation> {
    if !canonical.linked.text_encoding_types.is_empty() {
        return Some(TextRepresentation::Managed);
    }
    let is_text_type = |type_id: TypeId| is_text_primitive(canonical.linked.types.get(&type_id).map(|record| &record.kind));
    let has_text_values = canonical.mir.functions.values().any(|function| {
        function.value_types.values().any(|ty| is_text_type(*ty))
            || function.parameters.iter().any(|(_, ty)| is_text_type(*ty))
            || is_text_type(function.return_type)
            || function.blocks.values().any(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(
                        &instruction.operation,
                        CanonicalOperation::LoadConstant { constant: CanonicalConstant::Utf8(_) | CanonicalConstant::Utf16(_) }
                    )
                })
            })
    });
    has_text_values.then_some(TextRepresentation::Raw)
}

fn is_text_primitive(kind: Option<&CanonicalTypeKind>) -> bool {
    matches!(kind, Some(CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Utf8 | CanonicalPrimitiveType::Utf16)))
}

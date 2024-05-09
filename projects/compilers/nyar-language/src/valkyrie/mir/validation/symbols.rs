//! 语言运算符与内建符号身份判定（迁移期显示名查找，不维护闭式字符串表）。

use crate::{mir::ssa::builtin_helpers::resolve_intrinsic_id, types::NamePath};
use nyar_types::builtin_operator;

/// Language operators lower as `Call` to display names（迁移期）；后端展开，不进函数注册表。
///
/// 身份由 [`builtin_operator`] 注册表判定，不再维护封闭字符串表。
pub fn is_language_operator_symbol(symbol: &NamePath) -> bool {
    is_language_operator_name(symbol.parts().last().map(|part| part.as_str()).unwrap_or(""))
}

/// 显示名是否为已播种内建运算符（迁移期查找键）。
pub fn is_language_operator_name(name: &str) -> bool {
    builtin_operator::lookup_display_name(name).is_some()
}

/// Language builtins / private intrinsic seeds lower as `Call` but expand in backends。
///
/// 身份由 [`resolve_intrinsic_id`] → [`nyar_types::IntrinsicId`] 判定。
pub fn is_language_builtin_symbol(symbol: &NamePath) -> bool {
    resolve_intrinsic_id(symbol).is_some()
}

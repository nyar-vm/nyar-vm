//! 语言内建符号身份判定。

use crate::{mir::ssa::builtin_helpers::resolve_intrinsic_id, types::NamePath};

/// Language builtins / private intrinsic seeds lower as `Call` but expand in backends。
///
/// 身份由 [`resolve_intrinsic_id`] → [`nyar_types::IntrinsicId`] 判定。
pub fn is_language_builtin_symbol(symbol: &NamePath) -> bool {
    resolve_intrinsic_id(symbol).is_some()
}

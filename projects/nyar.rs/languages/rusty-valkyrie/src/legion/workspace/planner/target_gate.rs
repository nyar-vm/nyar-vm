//! `[gate(expression)]`：功能/构建开关条件（与 `when` 独立解析、独立记录）。
//!
//! 第一版仅解析并保存单条 `feature = "…"` 比较；不在此模块组合多个 `gate` item，也不与 `when` 求交。

use crate::compiler::syntax::oak::ast::Attribute;

use super::ast_attributes::term_string_literal;
use super::types::GatePredicate;

impl GatePredicate {
    pub(crate) fn from_gate_attribute(attribute: &Attribute) -> Option<Self> {
        if attribute.name.name != "gate" {
            return None;
        }
        let argument = attribute.args.first()?;
        let key = argument.key.as_ref()?.name.as_str();
        let value = term_string_literal(&argument.value)?;
        if value.is_empty() {
            return None;
        }
        Some(Self { key: key.to_string(), value })
    }
}

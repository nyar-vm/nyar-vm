//! 从 Oak `Attribute` 读取 planner 事实（不渲染 attribute 文本、不做二次 parse）。

use crate::compiler::syntax::oak::ast::{Attribute, StringLiteral, StringSegment, TermExpression};

use super::capability::ForeignBinding;
use super::types::{GatePredicate, WhenPredicate};

pub(crate) fn has_host_contract_attribute(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| attribute.name.name == "host_contract")
}

pub(crate) fn when_attributes_from_items(attributes: &[Attribute]) -> Vec<WhenPredicate> {
    attributes.iter().filter_map(WhenPredicate::from_when_attribute).collect()
}

pub(crate) fn gate_attributes_from_items(attributes: &[Attribute]) -> Vec<GatePredicate> {
    attributes.iter().filter_map(GatePredicate::from_gate_attribute).collect()
}

pub(crate) fn foreign_bindings_from_attributes(attributes: &[Attribute]) -> Vec<ForeignBinding> {
    attributes.iter().filter_map(foreign_binding_from_attribute).collect()
}

fn foreign_binding_from_attribute(attribute: &Attribute) -> Option<ForeignBinding> {
    let name = attribute.name.name.as_str();
    match name {
        "js" => {
            let symbol = attribute.args.first().and_then(|arg| term_string_literal(&arg.value))?;
            Some(ForeignBinding::Js { symbol })
        }
        "js_builtin" => {
            let symbol = attribute.args.first().and_then(|arg| term_string_literal(&arg.value))?;
            Some(ForeignBinding::JsBuiltin { symbol })
        }
        "wasm" => {
            let module = attribute.args.first().and_then(|arg| term_string_literal(&arg.value))?;
            let field = attribute.args.get(1).and_then(|arg| term_string_literal(&arg.value))?;
            Some(ForeignBinding::Wasm { module, field })
        }
        "clr" => {
            let symbol = attribute.args.first().and_then(|arg| term_string_literal(&arg.value))?;
            Some(ForeignBinding::Clr { symbol })
        }
        "jvm" => {
            let symbol = attribute.args.first().and_then(|arg| term_string_literal(&arg.value))?;
            Some(ForeignBinding::Jvm { symbol })
        }
        _ => None,
    }
}

pub(crate) fn host_provider_contract_targets(attributes: &[Attribute], source: &str) -> Vec<String> {
    attributes
        .iter()
        .filter(|attribute| attribute.name.name == "host_provider")
        .filter_map(|attribute| attribute.args.first())
        .filter_map(|argument| host_provider_contract_reference(&argument.value, source))
        .collect()
}

/// `host_provider` 参数：字符串字面量走 AST 解码，其余用表达式 span 在源码中的切片（不拼 attribute 文本）。
pub(crate) fn host_provider_contract_reference(expr: &TermExpression, source: &str) -> Option<String> {
    match expr {
        TermExpression::StringLiteral(literal) => string_literal_value(literal),
        expr => {
            let span = expr.span();
            source
                .get(span.start..span.end)
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
        }
    }
}

pub(crate) fn term_string_literal(expr: &TermExpression) -> Option<String> {
    match expr {
        TermExpression::StringLiteral(literal) => string_literal_value(literal),
        _ => None,
    }
}

fn string_literal_value(literal: &StringLiteral) -> Option<String> {
    let mut value = String::new();
    for segment in &literal.segments {
        match segment {
            StringSegment::Text(text) => value.push_str(&text.content),
            StringSegment::Interpolation(_) => return None,
        }
    }
    Some(value)
}

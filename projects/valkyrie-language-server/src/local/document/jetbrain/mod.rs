use std::str::FromStr;

use crate::utils::ColoredWriter;

use super::*;

impl TypedDocument {
    pub fn render_jetbrain(&self) -> Result<Json<ODocument>, StatusCode> {
        match self {
            TypedDocument::Nothing => Err(StatusCode::NOT_FOUND),
            TypedDocument::Keywords(s) => Ok(Json(ODocument { content: render_keywords(s) })),
            TypedDocument::Operator(s) => Ok(Json(ODocument { content: render_operator(s) })),
        }
    }
}

fn render_keywords(s: &str) -> String {
    let mut w = ColoredWriter::default();
    let keyword = ValkyrieKeyword::from_str(s).unwrap();
    if keyword.is_valid() {
        return format!("Invalid keyword: {}", s);
    }
    w.write_keyword("keyword");
    w.write_text(" ");
    w.write_attribute(keyword.name());
    w.write_newline();
    w.write_text("A keyword is a reserved word that cannot be used as an identifier.");
    w.finish()
}

fn render_operator(s: &str) -> String {
    let mut w = ColoredWriter::default();
    let operator = ValkyrieOperator::from_str(s).unwrap();
    if operator.is_valid() {
        return format!("Invalid operator: {}", s);
    }
    w.write_keyword("operator");
    w.write_text(" ");
    w.write_attribute(s);
    w.write_newline();
    w.write_text("An operator is a symbol that tells the compiler to perform specific mathematical or logical manipulations.");
    w.finish()
}

pub enum ValkyrieKeyword {
    Invalid,
    Class,
    Union,
    Trait,
    Namespace,
    NamespaceMajor,
}

pub enum ValkyrieOperator {
    Invalid,
    Add,
}

impl FromStr for ValkyrieKeyword {
    type Err = !;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "class" | "struct" => Ok(ValkyrieKeyword::Class),
            "union" | "enum" => Ok(ValkyrieKeyword::Union),
            "trait" => Ok(ValkyrieKeyword::Trait),
            _ => Ok(ValkyrieKeyword::Invalid),
        }
    }
}

impl FromStr for ValkyrieOperator {
    type Err = !;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "+" => Ok(ValkyrieOperator::Add),
            _ => Ok(ValkyrieOperator::Invalid),
        }
    }
}

impl ValkyrieKeyword {
    pub fn is_valid(&self) -> bool {
        matches!(self, ValkyrieKeyword::Invalid)
    }
    pub fn name(&self) -> &str {
        match self {
            ValkyrieKeyword::Invalid => "",
            ValkyrieKeyword::Class => "class",
            ValkyrieKeyword::Union => "union",
            ValkyrieKeyword::Trait => "trait",
            ValkyrieKeyword::Namespace => "namespace",
            ValkyrieKeyword::NamespaceMajor => "major namespace",
        }
    }
}

impl ValkyrieOperator {
    pub fn is_valid(&self) -> bool {
        matches!(self, ValkyrieOperator::Invalid)
    }
    pub fn name(&self) -> &str {
        match self {
            ValkyrieOperator::Invalid => "",
            ValkyrieOperator::Add => "plus",
        }
    }
}

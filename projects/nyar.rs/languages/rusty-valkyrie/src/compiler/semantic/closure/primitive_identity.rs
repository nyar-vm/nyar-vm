//! Primitive 自举身份表：`[primitive("std::primitive::i32")]` 注解的唯一消费侧。
//!
//! 数值等 primitive 类型不是语言内建，而是 std 的 `structure` 声明；在 std 经语义闭包
//! 可见之前，编译器必须能按声明携带的身份识别类型、分配 carrier 布局、参与数值/字面量
//! 判定——这正是 `[primitive(...)]` 注解存在的意义（解决先有鸡还是先有蛋）。
//!
//! 本表在闭包源码索引完成后、可见性与需求展开之前建立：键是声明携带的身份字符串
//! （不是短名），同一编译内身份必须唯一。重复或畸形身份结构化失败，不允许名字表放行。

use std::collections::BTreeMap;

use nyar_types::compiler_input::DeclarationKey;

use crate::compiler::syntax::oak::ast;

use super::SemanticClosureError;

/// 一条 primitive 自举身份：身份字符串 → 携带它的声明身份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveIdentity {
    /// 注解冻结的身份字符串，例如 `std::primitive::i32`。
    pub identity: String,
    /// 携带 `[primitive(...)]` 注解的声明身份。
    pub declaration: DeclarationKey,
}

/// 可见性与需求展开之前从源码 inventory 建立的自举身份表。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrimitiveIdentityTable {
    by_identity: BTreeMap<String, PrimitiveIdentity>,
}

impl PrimitiveIdentityTable {
    /// 按身份字符串查询登记记录。
    pub fn get(&self, identity: &str) -> Option<&PrimitiveIdentity> {
        self.by_identity.get(identity)
    }

    /// 按声明身份反查其携带的 primitive 身份字符串。
    pub fn identity_of(&self, declaration: &DeclarationKey) -> Option<&str> {
        self.by_identity.values().find(|item| &item.declaration == declaration).map(|item| item.identity.as_str())
    }

    /// 按身份字符串序迭代全部登记记录。
    pub fn iter(&self) -> impl Iterator<Item = &PrimitiveIdentity> {
        self.by_identity.values()
    }

    /// 登记条数。
    pub fn len(&self) -> usize {
        self.by_identity.len()
    }

    /// 是否为空表。
    pub fn is_empty(&self) -> bool {
        self.by_identity.is_empty()
    }
}

/// 从声明的 `[primitive("…")]` 注解读出身份字符串。
///
/// 只认「单个位置字符串字面量参数」形态：带命名键、含插值或没有 `primitive` 注解时
/// 返回 `None`。形态合法性（`::` 限定名段）由 [`build_primitive_identity_table`] 统一校验。
pub(crate) fn primitive_annotation_identity(annotations: &[ast::Attribute]) -> Option<String> {
    for attribute in annotations {
        if attribute.name.name != "primitive" {
            continue;
        }
        let argument = attribute.args.first()?;
        if argument.key.is_some() {
            return None;
        }
        let ast::TermExpression::StringLiteral(string) = &argument.value
        else {
            return None;
        };
        let mut content = String::new();
        for segment in &string.segments {
            match segment {
                ast::StringSegment::Text(text) => content.push_str(&text.content),
                ast::StringSegment::Interpolation(_) => return None,
            }
        }
        return Some(content);
    }
    None
}

/// 校验身份字符串形态并登记成表；重复或畸形身份结构化失败。
pub(crate) fn build_primitive_identity_table(
    records: impl IntoIterator<Item = (DeclarationKey, String)>,
) -> Result<PrimitiveIdentityTable, SemanticClosureError> {
    let mut table = PrimitiveIdentityTable::default();
    for (declaration, identity) in records {
        validate_primitive_identity_format(&declaration, &identity)?;
        if let Some(existing) = table.by_identity.get(&identity) {
            return Err(SemanticClosureError::DuplicatePrimitiveIdentity {
                identity,
                declarations: vec![existing.declaration.encoded(), declaration.encoded()],
            });
        }
        table.by_identity.insert(identity.clone(), PrimitiveIdentity { identity, declaration });
    }
    Ok(table)
}

/// 身份字符串必须是 `::` 分隔的限定名：至少两段，每段为非空标识符。
fn validate_primitive_identity_format(declaration: &DeclarationKey, identity: &str) -> Result<(), SemanticClosureError> {
    let malformed = || SemanticClosureError::MalformedPrimitiveIdentity { declaration: declaration.encoded(), identity: identity.to_string() };
    let segments: Vec<&str> = identity.split("::").collect();
    if segments.len() < 2 {
        return Err(malformed());
    }
    for segment in segments {
        let mut chars = segment.chars();
        let valid = match chars.next() {
            Some(first) if first.is_alphabetic() || first == '_' => chars.all(|ch| ch.is_alphanumeric() || ch == '_'),
            _ => false,
        };
        if !valid {
            return Err(malformed());
        }
    }
    Ok(())
}

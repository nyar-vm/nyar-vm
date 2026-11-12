//! Primitive 自举身份表：`[primitive("std::primitive::i32")]` 注解的唯一消费侧。
//!
//! 数值等 primitive 类型不是语言内建，而是 std 的 `structure` 声明；在 std 经语义闭包
//! 可见之前，编译器必须能按声明携带的身份识别类型、分配 carrier 布局、参与数值/字面量
//! 判定——这正是 `[primitive(...)]` 注解存在的意义（解决先有鸡还是先有蛋）。
//!
//! 本表在闭包源码索引完成后、可见性与需求展开之前建立：键是声明携带的身份字符串
//! （不是短名）。身份唯一性按可见面（依赖组）成立：同一依赖组（`DeclarationKey.dependency_key`）
//! 内重复或畸形身份结构化失败，不允许名字表放行。跨依赖组携带同一身份时，规范 owner 由
//! 身份引用本身裁决：身份是 `::` 限定名，声明全称（namespace 段 + 短名）恰好等于身份的
//! 声明才是规范 owner，其余携带者是别名载体（封闭生成单元为宿主调用重声明自己用到的
//! 标量拼写，它不是依赖包声明的第二个 primitive）。没有唯一规范 owner 的并列冒领——
//! 多个全称命中，或多个携带者而无人持有全称——结构化拒绝；裁决只看声明自身事实，
//! 不依赖组名或登记顺序。

use std::collections::BTreeMap;

use nyar_types::compiler_input::DeclarationKey;

use crate::compiler::syntax::oak::ast;

use super::SemanticClosureError;

/// 一条 primitive 自举身份：身份字符串 → 规范 owner 与其别名载体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveIdentity {
    /// 注解冻结的身份字符串，例如 `std::primitive::i32`。
    pub identity: String,
    /// 规范 owner 声明身份：声明全称恰好等于身份引用；唯一携带者无竞争者时即规范 owner。
    pub declaration: DeclarationKey,
    /// 别名载体：跨可见面携带同一身份引用、但自身全称不等于身份的声明（按声明身份有序）。
    pub aliases: Vec<DeclarationKey>,
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
        let ast::TermExpression::QuoteLiteral(literal) = &argument.value
        else {
            return None;
        };
        // 符号身份要求常量文本：与表达式位同一套捕获解码，含插值即拒绝。
        use crate::compiler::hir::lowering::quote_literal::{DecodedQuoteCapture, decode_quote_capture};
        return match decode_quote_capture(literal, false).ok()? {
            DecodedQuoteCapture::PlainText(text) => Some(text),
            DecodedQuoteCapture::Character(value) => Some(char::try_from(u32::from(value)).expect("char value is a valid scalar").to_string()),
            _ => None,
        };
    }
    None
}

/// 校验身份字符串形态并按可验证的规范 owner 引用登记成表。
///
/// 同一依赖组（同一可见面）内重复身份结构化失败，不得择一登记。跨依赖组携带同一身份
/// 时，规范 owner 是「声明全称恰好等于身份引用」的唯一声明，其余携带者登记为别名载体；
/// 多个全称命中、或多个携带者而无人持有全称，都是独立 owner 冒领同一身份，结构化拒绝。
/// 先按身份聚合再裁决，结果与输入登记顺序无关。
pub(crate) fn build_primitive_identity_table(
    records: impl IntoIterator<Item = (DeclarationKey, String)>,
) -> Result<PrimitiveIdentityTable, SemanticClosureError> {
    let mut claims: BTreeMap<String, Vec<DeclarationKey>> = BTreeMap::new();
    for (declaration, identity) in records {
        validate_primitive_identity_format(&declaration, &identity)?;
        claims.entry(identity).or_default().push(declaration);
    }
    let mut table = PrimitiveIdentityTable::default();
    for (identity, mut declarations) in claims {
        declarations.sort();
        // 同一可见面内重复携带：无论全称是否命中，都不允许择一。
        let mut surfaces: BTreeMap<&str, Vec<&DeclarationKey>> = BTreeMap::new();
        for declaration in &declarations {
            surfaces.entry(declaration.dependency_key.as_str()).or_default().push(declaration);
        }
        if let Some(duplicates) = surfaces.values().find(|items| items.len() > 1) {
            return Err(SemanticClosureError::DuplicatePrimitiveIdentity {
                identity,
                declarations: duplicates.iter().map(|declaration| declaration.encoded()).collect(),
            });
        }
        let canonical: Vec<&DeclarationKey> =
            declarations.iter().filter(|declaration| declaration_qualifies_as(*declaration, &identity)).collect();
        let owner = match canonical.len() {
            1 => canonical[0],
            // 唯一携带者没有竞争者：bootstrap 场景（规范包尚未进入输入）它即规范 owner。
            0 if declarations.len() == 1 => &declarations[0],
            // 多个全称命中，或多个携带者而无人持有全称：独立 owner 冒领同一身份。
            _ => {
                return Err(SemanticClosureError::DuplicatePrimitiveIdentity {
                    identity,
                    declarations: declarations.iter().map(DeclarationKey::encoded).collect(),
                });
            }
        };
        let aliases = declarations.iter().filter(|declaration| *declaration != owner).cloned().collect();
        table.by_identity.insert(identity.clone(), PrimitiveIdentity { identity, declaration: owner.clone(), aliases });
    }
    Ok(table)
}

/// 声明全称（namespace 段按 `.` 展开 + 短名）是否恰好等于身份引用。
fn declaration_qualifies_as(declaration: &DeclarationKey, identity: &str) -> bool {
    let Some((namespace, name)) = identity.rsplit_once("::")
    else {
        return false;
    };
    declaration.name == name && declaration.namespace.iter().flat_map(|segment| segment.split('.')).eq(namespace.split("::"))
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

//! Primitive 自举身份表：`[primitive("std::primitive::i32")]` 注解的唯一消费侧。
//!
//! 数值等 primitive 类型不是语言内建，而是 std 的 `structure` 声明；在 std 经语义闭包
//! 可见之前，编译器必须能按声明携带的身份识别类型、分配 carrier 布局、参与数值/字面量
//! 判定——这正是 `[primitive(...)]` 注解存在的意义（解决先有鸡还是先有蛋）。
//!
//! 本表在闭包源码索引完成后、可见性与需求展开之前建立：键是声明携带的身份字符串
//! （不是短名）。身份唯一性按可见面（依赖组）成立：同一依赖组（`DeclarationKey.dependency_key`）
//! 内重复或畸形身份结构化失败，不允许名字表放行；跨依赖组携带同一身份是别名载体——
//! 封闭生成单元会为宿主调用重声明自己用到的标量拼写，它不是依赖包声明的第二个 primitive，
//! 规范 owner 优先非生成组。

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

/// 封闭生成单元的依赖组键（`asgard::compile::compiler_input_for` 分配的组身份）。
///
/// 生成源码组不能继承依赖包的语义导出，会为重声明自己用到的标量拼写；这类声明是
/// 同一 primitive 身份的生成侧载体，登记时让位于非生成组的规范 owner。
const GENERATED_SOURCE_GROUP_KEY: &str = "asgard.generated";

/// 声明是否属于封闭生成单元。
fn is_generated_source_group(declaration: &DeclarationKey) -> bool {
    declaration.dependency_key == GENERATED_SOURCE_GROUP_KEY
}

/// 校验身份字符串形态并登记成表。
///
/// 同一依赖组（同一可见面）内重复身份结构化失败，不得择一登记；跨依赖组携带同一身份
/// 是别名载体，规范 owner 优先非生成组（已有记录是生成组且候选不是时改登记候选）。
/// 两个非生成组跨组登记时保持先登记：inventory 按依赖组键有序遍历，结果确定。
pub(crate) fn build_primitive_identity_table(
    records: impl IntoIterator<Item = (DeclarationKey, String)>,
) -> Result<PrimitiveIdentityTable, SemanticClosureError> {
    let mut table = PrimitiveIdentityTable::default();
    for (declaration, identity) in records {
        validate_primitive_identity_format(&declaration, &identity)?;
        // 先取出已有记录的 surface 事实，避免持有表内引用时改写表。
        let existing = table.by_identity.get(&identity).map(|item| {
            (item.declaration.dependency_key.clone(), item.declaration.encoded(), is_generated_source_group(&item.declaration))
        });
        if let Some((surface, encoded, existing_is_generated)) = existing {
            if surface == declaration.dependency_key {
                return Err(SemanticClosureError::DuplicatePrimitiveIdentity {
                    identity,
                    declarations: vec![encoded, declaration.encoded()],
                });
            }
            // 跨 surface：别名载体。只有「生成组让位给非生成组」会改写规范 owner，
            // 其余跨组登记保持先登记，别名声明不进入本表。
            if existing_is_generated && !is_generated_source_group(&declaration) {
                table.by_identity.insert(identity.clone(), PrimitiveIdentity { identity, declaration });
            }
            continue;
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

//! Primitive 自举身份表：`[primitive("std::primitive::i32")]` 注解的唯一消费侧。
//!
//! 数值等 primitive 类型不是语言内建，而是 std 的 `structure` 声明；在 std 经语义闭包
//! 可见之前，编译器必须能按声明携带的身份识别类型、分配 carrier 布局、参与数值/字面量
//! 判定——这正是 `[primitive(...)]` 注解存在的意义（解决先有鸡还是先有蛋）。
//!
//! 本表在闭包源码索引完成后、需求展开之前建立：键是声明携带的身份字符串（不是短名）。
//! 身份唯一性按可见面（依赖组）成立：同一依赖组（`DeclarationKey.dependency_key`）
//! 内重复或畸形身份结构化失败，不允许名字表放行。
//!
//! owner 与 aliases 一律以 Resolver 产出的可验证来源引用登记，不以字符串相等认领：
//! 每个携带者的身份引用（`::` 限定名）在携带者自身的可见面（本组 + 直接依赖组）内按
//! 绝对限定位置解析，并穿透类型别名的目标链接（`type utf8 = Utf8Text` 是同一声明的
//! 透明拼写，不是第二个声明），解析结果就是它的来源声明。来源恰好是携带者自身时，它
//! 占据身份的规范位置，是规范 owner 候选；来源是可见面内的另一声明时，携带者是该声明
//! 的投影，登记为别名载体。来源解析为空、同一位置歧义、投影目标不是规范 owner、或多个
//! 携带者并列占据规范位置，都是无法证明同源的冒领，结构化拒绝（fail-closed）；唯一携带者
//! 不享受任何豁免——全称不匹配、引用解析落空的单携带者同样被拒绝。裁决只看声明索引与
//! 可见性事实，不依赖组名或登记顺序。

use std::collections::{BTreeMap, BTreeSet};

use nyar_types::compiler_input::{DeclarationKey, ReferencePath};

use crate::compiler::syntax::oak::ast;

use super::{DeclarationIndexes, SemanticClosureError};

/// 一条 primitive 自举身份：身份字符串 → 规范 owner 与其别名载体。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimitiveIdentity {
    /// 注解冻结的身份字符串，例如 `std::primitive::i32`。
    pub identity: String,
    /// 规范 owner 声明身份：身份引用（穿透透明类型别名后）在其可见面内唯一命中自身。
    pub declaration: DeclarationKey,
    /// 别名载体：身份引用在其可见面内解析命中规范 owner 的投影声明（按声明身份有序）。
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

/// 校验身份字符串形态，并按 Resolver 产出的可验证来源引用登记成表。
///
/// 同一依赖组（同一可见面）内重复身份结构化失败，不得择一登记。跨依赖组携带同一身份
/// 时，每个携带者的身份引用必须在携带者自身的可见面内按绝对限定位置解析出唯一来源
/// 声明：来源恰为自身的是规范 owner，来源为 owner 的是别名载体（同一声明的投影）。
/// 来源解析落空、位置歧义、投影目标不是 owner、或多个携带者并列自命中，都无法证明
/// 同源，结构化拒绝（fail-closed）；唯一携带者没有豁免。先按身份聚合再裁决，结果与
/// 输入登记顺序无关。
pub(crate) fn build_primitive_identity_table(
    records: impl IntoIterator<Item = (DeclarationKey, String)>,
    indexes: &DeclarationIndexes,
    visibility: &BTreeMap<String, BTreeSet<String>>,
) -> Result<PrimitiveIdentityTable, SemanticClosureError> {
    let mut claims: BTreeMap<String, Vec<DeclarationKey>> = BTreeMap::new();
    for (declaration, identity) in records {
        validate_primitive_identity_format(&declaration, &identity)?;
        claims.entry(identity).or_default().push(declaration);
    }
    let empty_visibility = BTreeSet::new();
    let mut table = PrimitiveIdentityTable::default();
    for (identity, mut declarations) in claims {
        declarations.sort();
        // 同一可见面内重复携带：无论引用能否解析，都不允许择一。
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
        let reference = identity_reference(&identity);
        // 逐携带者解析来源引用：来源声明是「同一声明的投影」的唯一可验证证据。
        // 身份是拼写，可能是声明本身，也可能是指向它的透明类型别名（`type utf8 = Utf8Text`）：
        // 两者落在同一冻结声明身份，别名链由 Resolver 的链接事实穿透，不做短名近似。
        let mut owners: Vec<&DeclarationKey> = Vec::new();
        let mut projections: Vec<(&DeclarationKey, DeclarationKey)> = Vec::new();
        for declaration in &declarations {
            let visible = visibility.get(declaration.dependency_key.as_str()).unwrap_or(&empty_visibility);
            let sources = indexes.resolve_identity_source(&reference, visible);
            match sources.len() {
                0 => {
                    return Err(SemanticClosureError::UnprovenPrimitiveIdentitySource {
                        identity,
                        declaration: declaration.encoded(),
                        resolved_source: None,
                    });
                }
                1 => {
                    let source = sources.into_iter().next().expect("single hit");
                    if source == *declaration {
                        owners.push(declaration);
                    }
                    else {
                        projections.push((declaration, source));
                    }
                }
                _ => {
                    return Err(SemanticClosureError::AmbiguousPrimitiveIdentitySource {
                        identity,
                        declaration: declaration.encoded(),
                        sources: sources.iter().map(DeclarationKey::encoded).collect(),
                    });
                }
            }
        }
        // 规范 owner 必须唯一：并列自命中是独立声明在同一规范位置冒领同一身份。
        let owner = match owners.len() {
            1 => owners[0],
            0 => {
                // 没有携带者能证明自己是规范声明：投影目标不携带身份，身份失去规范 owner。
                let (declaration, source) = projections.first().expect("claims is non-empty");
                return Err(SemanticClosureError::UnprovenPrimitiveIdentitySource {
                    identity,
                    declaration: declaration.encoded(),
                    resolved_source: Some(source.encoded()),
                });
            }
            _ => {
                return Err(SemanticClosureError::DuplicatePrimitiveIdentity {
                    identity,
                    declarations: owners.iter().map(|declaration| declaration.encoded()).collect(),
                });
            }
        };
        // 别名载体只能是 owner 的可验证投影：来源指向其他声明同样是冒领。
        for (declaration, source) in &projections {
            if source != owner {
                return Err(SemanticClosureError::UnprovenPrimitiveIdentitySource {
                    identity,
                    declaration: declaration.encoded(),
                    resolved_source: Some(source.encoded()),
                });
            }
        }
        let aliases = projections.iter().map(|(declaration, _)| (*declaration).clone()).collect();
        table.by_identity.insert(identity.clone(), PrimitiveIdentity { identity, declaration: owner.clone(), aliases });
    }
    Ok(table)
}

/// 身份字符串的限定名引用段：`std::primitive::i32` → `["std","primitive","i32"]`。
///
/// 与声明 namespace 段同一口径按 `.` 再切分，绝对位置匹配不引入第二份名字事实。
fn identity_reference(identity: &str) -> ReferencePath {
    identity.split("::").flat_map(|segment| segment.split('.')).map(str::to_string).collect()
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

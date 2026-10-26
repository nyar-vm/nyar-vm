//! 入口驱动的语义闭包规划：可见性合同下的声明图、body lowering 边界与内容摘要。
//!
//! 闭包是「目标模块能看见的声明」的上界，由声明身份与依赖可见性驱动，而不是文件短名近似：
//!
//! - 可见性的事实来源是 Resolver 给出的 `direct_dependencies`：一条引用只能命中本组声明
//!   或本组直接依赖的声明。闭包不做跨组全局名字回退，也不把整个依赖包的 `.v` 文件拉进闭包；
//! - 种子是入口组的声明；依赖组只在自身声明被需求时进入 body lowering；
//! - 自由名引用按「限定路径 → 当前 namespace → 父 namespace」解析；链上未命中的单段名
//!   只在可见组内按短名回退，且只接受非成员声明：方法/imply 成员没有名字回退索引；
//! - 成员选择名不建立全库同名索引：限定形式 `<聚合>.<成员>` 先解析聚合身份再取成员身份；
//!   接收者类型在索引阶段不可知的选择名，只在该名字确实出现在闭包需求中、且其拥有聚合
//!   已被需求时，作为**该聚合的成员**进入闭包；
//! - 运算符是普通成员调用：`a + b` / `!a` / `a[i]` 登记为 `infix +` / `prefix !` / `suffix [ ]`
//!   的成员引用，成员名与 HIR lowering 共用 `operator_members` 这一唯一映射；
//! - 闭包只做上界判定，不做歧义判定：精确调用解析与歧义归 HIR；
//! - 摘要绑定规划版本、依赖身份、被编译声明身份与这些声明的源文件内容。

use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// Declaration 索引中立数据结构（生产权在本 crate 的 frontend inventory）
// 定义在 nyar_types::compiler_input，本模块 re-export 供内部使用。
// ---------------------------------------------------------------------------
pub use nyar_types::compiler_input::{DeclarationIndexEntry, DeclarationKey, DeclarationKind, DeclarationLink, EntryPointKind, ReferencePath};

/// 单组语义闭包切片。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupSemanticClosure {
    /// 依赖组身份。
    pub dependency_key: String,
    /// 进入 body lowering 的仓相对路径。
    pub body_lowering_source_paths: BTreeSet<String>,
    /// 本组 body lowering 覆盖的声明身份。
    pub body_lowering_declarations: BTreeSet<DeclarationKey>,
}

/// 入口驱动的完整语义闭包：声明索引 + body lowering 边界 + 内容摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticClosure {
    /// 各依赖组的 body lowering 切片。
    pub groups: Vec<GroupSemanticClosure>,
    /// 全 inventory 声明索引（含不可达文件）。
    pub declaration_index: BTreeMap<DeclarationKey, DeclarationIndexEntry>,
    /// 入口闭包内被引用的声明身份。
    pub demanded_declarations: BTreeSet<DeclarationKey>,
    /// 未能建立声明索引的 inventory 文件（`依赖组::路径` → 失败原因）。
    pub unindexed_source_files: BTreeMap<String, String>,
    /// 闭包稳定摘要。
    pub digest: String,
}

/// 单个源码组的闭包规划输入：组身份、模块名、直接依赖身份、是否入口组、组内文件。
///
/// `direct_dependencies` 是可见性合同的唯一事实来源：它决定本组引用可以命中哪些组的声明。
pub struct ClosureGroupInput<'a> {
    /// 依赖组身份。
    pub dependency_key: &'a str,
    /// 语义组模块名；无 namespace 的声明用它形成与 HIR 稳定符号一致的限定名。
    pub module_name: &'a str,
    /// 本组可直接引用的依赖组身份。
    pub direct_dependencies: &'a [&'a str],
    /// 是否为唯一入口组。
    pub is_entry: bool,
    /// 组内文件（仓相对路径、源码）。
    pub files: &'a [(String, String)],
}

/// 语义闭包规划的结构化失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticClosureError {
    /// 入口组没有提供任何 `[export]` / `[main]` 声明：不得把整组文件当作可执行程序。
    MissingEntryDeclaration {
        /// 失败入口组身份。
        dependency_key: String,
        /// 入口组内未能建立索引的文件（路径、原因）；为空表示文件可解析但没有入口注解。
        unindexed_source_files: Vec<(String, String)>,
    },
    /// Resolver 选定的 provider 符号没有命中唯一声明身份；被需求合同不得缺实现。
    HostProviderNotResolved {
        /// 绑定中的 provider 稳定符号。
        symbol: String,
    },
    /// 同一 provider 符号命中多个声明身份：不得择一继续。
    HostProviderAmbiguous {
        /// 绑定中的 provider 稳定符号。
        symbol: String,
        /// 命中的声明身份（依赖组::路径::种类::namespace::拥有者::名字）。
        declarations: Vec<String>,
    },
}

impl std::fmt::Display for SemanticClosureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingEntryDeclaration { dependency_key, unindexed_source_files } => {
                write!(formatter, "entry source group `{dependency_key}` declares no `[export]` or `[main]` entry")?;
                if !unindexed_source_files.is_empty() {
                    write!(formatter, "; unindexed sources:")?;
                    for (path, reason) in unindexed_source_files {
                        write!(formatter, " {path}: {reason};")?;
                    }
                }
                Ok(())
            }
            Self::HostProviderNotResolved { symbol } => {
                write!(formatter, "host provider `{symbol}` 未在语义源码闭包中解析到唯一声明身份")
            }
            Self::HostProviderAmbiguous { symbol, declarations } => {
                write!(formatter, "host provider `{symbol}` 在语义源码闭包中命中多个声明身份: {declarations:?}")
            }
        }
    }
}

impl std::error::Error for SemanticClosureError {}

mod collect;
mod index_file;
mod indexes;
mod plan;
mod retain;

pub(crate) use collect::imply_owner_name;
pub(crate) use index_file::name_path_segments;
pub(crate) type ScopedReference = (Vec<String>, ReferencePath);

pub use plan::{micro_declaration_key, plan_semantic_closure, plan_semantic_closure_with_host_bindings};
pub(crate) use plan::{source_group_closure, source_group_closure_with_host_bindings};
pub use retain::retain_demanded_declarations;

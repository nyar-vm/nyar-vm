//! 可扩展属性注册表。
//!
//! 解析后语义路径只持有 [`AttributeId`]；名称仅存旁表。
//!
//! 运算符没有注册表：语言运算符在 HIR 早期展开为普通的接收者成员调用，
//! 与用户声明的同名成员走同一条 callable 解析链。

use std::collections::BTreeMap;

use crate::semantic_ids::{AttributeId, AttributeRegistration, builtin_attribute};

/// 属性注册表错误（失败关闭，禁止静默覆盖）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttributeRegistryError {
    /// 同名属性已绑定不同 id。
    DuplicateName {
        /// 冲突的属性名。
        name: String,
        /// 已有 id。
        existing: AttributeId,
    },
}

impl std::fmt::Display for AttributeRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateName { name, existing } => {
                write!(f, "duplicate attribute name `{name}` already bound to {existing}")
            }
        }
    }
}

impl std::error::Error for AttributeRegistryError {}

/// 可扩展属性注册表：内建播种后可 `intern` 用户属性。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttributeRegistry {
    by_name: BTreeMap<String, AttributeId>,
    by_id: BTreeMap<AttributeId, AttributeRegistration>,
    next_index: u32,
}

impl AttributeRegistry {
    /// 播种内建 `export` / `main` / `test` / `benchmark` / `workload_phase`。
    pub fn with_builtins() -> Self {
        let mut registry = Self::default();
        for row in builtin_attribute::seed_registrations() {
            registry.by_name.insert(row.name.clone(), row.id);
            let index = row.id.index();
            registry.next_index = registry.next_index.max(index.saturating_add(1));
            registry.by_id.insert(row.id, row);
        }
        registry
    }

    /// 按名查找（含用户 intern 项）。
    pub fn lookup(&self, name: &str) -> Option<AttributeId> {
        self.by_name.get(name).copied()
    }

    /// 登记或复用属性名；同名已存在则返回已有 id（幂等）。
    ///
    /// 若要以「同名必须失败」策略接入，改用 [`AttributeRegistry::intern_unique`]。
    pub fn intern(&mut self, name: impl Into<String>) -> AttributeId {
        let name = name.into();
        if let Some(id) = self.by_name.get(&name).copied() {
            return id;
        }
        let id = AttributeId::from_index(self.next_index).expect("attribute id space");
        self.next_index = self.next_index.saturating_add(1);
        self.by_name.insert(name.clone(), id);
        self.by_id.insert(id, AttributeRegistration { id, name });
        id
    }

    /// 登记新属性名；若名已存在则失败关闭。
    pub fn intern_unique(&mut self, name: impl Into<String>) -> Result<AttributeId, AttributeRegistryError> {
        let name = name.into();
        if let Some(existing) = self.by_name.get(&name).copied() {
            return Err(AttributeRegistryError::DuplicateName { name, existing });
        }
        Ok(self.intern(name))
    }

    /// 旁表查询。
    pub fn registration(&self, id: AttributeId) -> Option<&AttributeRegistration> {
        self.by_id.get(&id)
    }
}

pub use chomsky_extract::IKunTree;
use nyar_gc::{MarkContext, Trace};
use oak_core::Language;
use serde::{Deserialize, Serialize};

pub mod errors;
pub use crate::errors::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QualifiedName {
    pub parts: Vec<String>,
}

impl QualifiedName {
    pub fn new(parts: Vec<String>) -> Self {
        Self { parts }
    }
    pub fn push(&mut self, part: String) {
        self.parts.push(part);
    }
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
    pub fn last(&self) -> Option<&String> {
        self.parts.last()
    }
}

impl FromIterator<String> for QualifiedName {
    fn from_iter<T: IntoIterator<Item = String>>(iter: T) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

impl From<&str> for QualifiedName {
    fn from(s: &str) -> Self {
        s.split("::").map(|s| s.to_string()).collect()
    }
}

impl From<String> for QualifiedName {
    fn from(s: String) -> Self {
        s.as_str().into()
    }
}

impl Trace for QualifiedName {
    fn trace(&self, _ctx: &mut MarkContext) {
        // Strings are not GC-managed by nyar-gc, but this satisfies the Trace trait
    }
}

impl std::fmt::Display for QualifiedName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.parts.join("::"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct SourceLocation {
    pub source_id: u32,
    pub offset: u32,
}

impl SourceLocation {
    pub fn new(source_id: u32, offset: u32) -> Self {
        Self { source_id, offset }
    }
}

impl std::fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "source:{}:{}", self.source_id, self.offset)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectInfo {
    pub name: QualifiedName,
    pub location: SourceLocation,
}

pub trait NyarFrontend: Default {
    type Language: Language;

    fn parse(&self, source: &str) -> Result<<Self::Language as Language>::TypedRoot, NyarError>;

    /// 统一的接入接口，支持 EGraph 优化流
    fn lower_unified<V: Vfs>(&self, ast: &<Self::Language as Language>::TypedRoot, ctx: &mut NyarContext<V>) -> Id;

    /// 默认实现：利用 lower_unified 生成 IKunTree
    fn lower<V: Vfs>(&self, ast: &<Self::Language as Language>::TypedRoot, vfs: &V) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::new();
        let mut ctx = NyarContext::new(&mut egraph, vfs, 1);
        let root_id = self.lower_unified(ast, &mut ctx);

        // 此处可以插入统一的优化流程
        // egraph.rebuild();

        let extractor = chomsky_extract::IKunExtractor::new(&egraph, chomsky_cost::DEFAULT_COST_MODEL.clone());
        Ok(extractor.extract(root_id))
    }

    /// 利用 Gaia 编译到特定目标
    fn compile_to_gaia<V: Vfs>(&self, ast: &<Self::Language as Language>::TypedRoot, vfs: &V, target: &str) -> Result<chomsky_extract::BackendArtifact, NyarError> {
        let tree = self.lower(ast, vfs)?;
        let emitter = chomsky_emit::GaiaEmitter::new(target).standalone();
        use chomsky_extract::Backend;
        emitter.generate(&tree).map_err(|e| NyarError::Compile(format!("Gaia error: {:?}", e)))
    }
}

use std::collections::HashMap;
use chomsky_uir::{EGraph, IKun, Id, IntentBuilder};
pub use oak_vfs::Vfs;
pub use chomsky_types::Loc;

/// 统一的作用域管理器，负责符号混淆和遮蔽
#[derive(Debug, Default, Clone)]
pub struct ScopeManager {
    scopes: Vec<HashMap<String, String>>,
    next_id: u32,
}

impl ScopeManager {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
            next_id: 0,
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
        eprintln!("DEBUG: pushed scope, depth: {}", self.scopes.len());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
            eprintln!("DEBUG: popped scope, depth: {}", self.scopes.len());
        } else {
            eprintln!("DEBUG: attempt to pop global scope ignored");
        }
    }

    /// 声明一个类成员（不进行混淆）
    pub fn declare_member(&mut self, name: &str) -> String {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), name.to_string());
        }
        name.to_string()
    }

    /// 声明一个变量并返回混淆后的名称
    pub fn declare_variable(&mut self, name: &str) -> String {
        // If it's already declared in current scope as global or nonlocal, use that mangled name
        if let Some(scope) = self.scopes.last() {
            if let Some(mangled) = scope.get(name) {
                return mangled.clone();
            }
        }

        let mangled = format!("{}_{}", name, self.next_id);
        self.next_id += 1;
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), mangled.clone());
        }
        mangled
    }

    /// 声明一个全局变量引用
    pub fn declare_global(&mut self, name: &str) {
        let mangled = if let Some(m) = self.scopes[0].get(name) {
            m.clone()
        } else {
            let m = format!("{}_{}", name, self.next_id);
            self.next_id += 1;
            self.scopes[0].insert(name.to_string(), m.clone());
            m
        };

        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), mangled);
        }
    }

    /// 声明一个非局部变量引用
    pub fn declare_nonlocal(&mut self, name: &str) {
        let mut mangled = None;
        // Search in outer scopes, but not the global scope (index 0)
        for i in (1..self.scopes.len() - 1).rev() {
            if let Some(m) = self.scopes[i].get(name) {
                mangled = Some(m.clone());
                break;
            }
        }

        if let Some(m) = mangled {
            if let Some(scope) = self.scopes.last_mut() {
                scope.insert(name.to_string(), m);
            }
        }
    }

    /// 解析变量名称，返回混淆后的名称或原始名称
    pub fn resolve_variable(&self, name: &str) -> String {
        for scope in self.scopes.iter().rev() {
            if let Some(mangled) = scope.get(name) {
                return mangled.clone();
            }
        }
        name.to_string()
    }
}

/// 统一的前端接入上下文
pub struct NyarContext<'a, V: Vfs, A: chomsky_uir::Analysis<IKun> = ()> {
    pub egraph: &'a mut EGraph<IKun, A>,
    pub scopes: ScopeManager,
    pub source_id: u32,
    pub vfs: &'a V,
}

impl<'a, V: Vfs, A: chomsky_uir::Analysis<IKun>> NyarContext<'a, V, A> {
    pub fn new(egraph: &'a mut EGraph<IKun, A>, vfs: &'a V, source_id: u32) -> Self {
        Self {
            egraph,
            scopes: ScopeManager::new(),
            source_id,
            vfs,
        }
    }

    pub fn builder(&mut self) -> IntentBuilder<'_, A> {
        IntentBuilder::new(self.egraph)
    }

    pub fn loc(&self, start: u32, end: u32) -> Loc {
        Loc::new(self.source_id, start, end)
    }
}

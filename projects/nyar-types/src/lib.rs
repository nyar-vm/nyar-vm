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
    fn lower_unified(&self, ast: &<Self::Language as Language>::TypedRoot, ctx: &mut NyarContext) -> Id;

    /// 默认实现：利用 lower_unified 生成 IKunTree
    fn lower(&self, ast: &<Self::Language as Language>::TypedRoot) -> Result<IKunTree, NyarError> {
        let mut egraph = EGraph::new();
        let mut ctx = NyarContext::new(&mut egraph, 1);
        let root_id = self.lower_unified(ast, &mut ctx);

        // 此处可以插入统一的优化流程
        // egraph.rebuild();

        let extractor = chomsky_extract::IKunExtractor::new(&egraph, chomsky_cost::DEFAULT_COST_MODEL.clone());
        Ok(extractor.extract(root_id))
    }
}

use std::collections::HashMap;
use chomsky_uir::{EGraph, IKun, Id, IntentBuilder};
use chomsky_source::Loc;

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
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    /// 声明一个变量并返回混淆后的名称
    pub fn declare_variable(&mut self, name: &str) -> String {
        let mangled = format!("{}_{}", name, self.next_id);
        self.next_id += 1;
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), mangled.clone());
        }
        mangled
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
pub struct NyarContext<'a, A: chomsky_uir::Analysis<IKun> = ()> {
    pub egraph: &'a mut EGraph<IKun, A>,
    pub scopes: ScopeManager,
    pub source_id: u32,
}

impl<'a, A: chomsky_uir::Analysis<IKun>> NyarContext<'a, A> {
    pub fn new(egraph: &'a mut EGraph<IKun, A>, source_id: u32) -> Self {
        Self {
            egraph,
            scopes: ScopeManager::new(),
            source_id,
        }
    }

    pub fn builder(&mut self) -> IntentBuilder<'_, A> {
        IntentBuilder::new(self.egraph)
    }

    pub fn loc(&self, start: u32, end: u32) -> Loc {
        Loc::new(self.source_id, start, end)
    }

    /// 尝试将函数调用映射到标准 Intrinsics
    pub fn map_intrinsic(&mut self, name: &str, args: Vec<Id>, loc: Loc) -> Option<Id> {
        match name {
            "printf" | "print" | "println" | "System.Console.WriteLine" | "fmt.Printf" => {
                Some(self.builder().cross_lang_call("nyar", "std::io::print", args, loc))
            }
            "exit" | "os.Exit" | "System.Environment.Exit" => {
                Some(self.builder().cross_lang_call("nyar", "std::sys::exit", args, loc))
            }
            "sin" | "Math.Sin" => {
                Some(self.builder().cross_lang_call("nyar", "std::math::sin", args, loc))
            }
            "cos" | "Math.Cos" => {
                Some(self.builder().cross_lang_call("nyar", "std::math::cos", args, loc))
            }
            _ => None,
        }
    }
}

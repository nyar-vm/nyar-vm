use crate::ast::Stmt;
use crate::hir::{HIRModule, HirClass, HirFunc, HirImpl, HirTrait};

#[derive(Clone, Debug)]
pub struct MIRModule {
    pub functions: Vec<HirFunc>,
    pub classes: Vec<HirClass>,
    pub enums: Vec<Stmt>,
    pub traits: Vec<HirTrait>,
    pub impls: Vec<HirImpl>,
    pub main: Vec<Stmt>,
}

pub fn lower_hir_to_mir(hir: &HIRModule) -> MIRModule {
    // For now, MIR mirrors HIR. Future optimizations and canonicalization belong here.
    MIRModule {
        functions: hir.functions.clone(),
        classes: hir.classes.clone(),
        enums: hir.enums.clone(),
        traits: hir.traits.clone(),
        impls: hir.impls.clone(),
        main: hir.main.clone(),
    }
}

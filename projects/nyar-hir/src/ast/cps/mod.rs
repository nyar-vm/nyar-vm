use crate::{ASTKind, ASTNode};
use crate::ast::ASTAtom;
use crate::ast::let_bind::LetBind;

impl ASTNode {
    pub fn cps_transform(&self) -> ASTNode {
        ASTNode {
            kind: self.kind.cps_transform(),
            meta: self.meta.clone(),
        }
    }
}

impl ASTKind {
    pub fn cps_transform(&self) -> ASTKind {
        match self {
            ASTKind::Nothing => { ASTKind::Nothing }
            ASTKind::ASTAtom(atom) => {
                atom.cps_transform()
            }
            ASTKind::LetBind(bind) => { bind.cps_transform() }
        }
    }
}

impl ASTAtom {
    pub fn cps_transform(&self) -> ASTKind {
        todo!()
    }
}

impl LetBind {
    pub fn cps_transform(&self) -> ASTKind {
        todo!()
    }
}
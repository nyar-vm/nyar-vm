use crate::{
    files::{FunctionID, NyarProgram, ParameterID, Symbol},
    NyarData,
};
use valkyrie_ast::StatementKind;

#[salsa::tracked]
pub struct NyarStructureData {
    #[id]
    pub name: NyarStructure,
    // #[return_ref]
    // pub parameters: Vec<ParameterID>,
}

#[salsa::interned]
pub struct NyarStructure {
    #[return_ref]
    pub name: Vec<Symbol>,
}

// #[salsa::tracked]
pub fn find_structure(db: &dyn NyarData, program: NyarProgram) -> Vec<NyarStructureData> {
    for term in program.statements(db) {
        match term {
            StatementKind::Nothing => {}
            StatementKind::Document(_) => {}
            StatementKind::Annotation(_) => {}
            StatementKind::Namespace(_) => {}
            StatementKind::Import(_) => {}
            StatementKind::Class(_) => {}
            StatementKind::Union(_) => {}
            StatementKind::Enumerate(_) => {}
            StatementKind::Trait(_) => {}
            StatementKind::Extends(_) => {}
            StatementKind::Function(_) => {}
            StatementKind::Variable(_) => {}
            StatementKind::Guard(_) => {}
            StatementKind::While(_) => {}
            StatementKind::For(_) => {}
            StatementKind::Control(_) => {}
            StatementKind::Expression(_) => {}
        }
    }
    vec![]
}

pub enum Definitions {
    Class(NyarStructureData),
}

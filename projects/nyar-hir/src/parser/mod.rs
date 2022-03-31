use crate::{
    diagnostics::NyarDiagnostic,
    files::{NyarFile, NyarProgram},
    Failure, NyarData, Success,
};

use valkyrie_parser::ProgramContext;

#[salsa::tracked(return_ref)]
pub fn parse_program(db: &dyn NyarData, source: NyarFile) -> NyarProgram {
    let state = ProgramContext { file: Default::default() };
    match state.parse_custom(source.contents(db)) {
        Success { value, diagnostics } => {
            diagnostics.into_iter().for_each(|e| NyarDiagnostic::push(db, e));
            NyarProgram::new(db, value.statements)
        }
        Failure { fatal, diagnostics } => {
            diagnostics.into_iter().for_each(|e| NyarDiagnostic::push(db, e));
            NyarDiagnostic::push(db, fatal);
            NyarProgram::new(db, vec![])
        }
    }
}

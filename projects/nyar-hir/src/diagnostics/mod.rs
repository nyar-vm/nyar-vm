use crate::{
    files::{NyarFile, NyarFunction, NyarProgram},
    jars::NyarDatabase,
    parser::parse_program,
};
use nyar_error::NyarError;
use salsa::{debug::DebugWithDb, ParallelDatabase};
use std::fmt::{Debug, Formatter};

#[salsa::accumulator]
pub struct NyarDiagnostic(pub NyarError);

impl Debug for NyarDiagnostic {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

fn parse_string(input: &str) -> String {
    let db = NyarDatabase::default();
    let source_program =
        NyarFile::new(&db, crate::files::NyarFileLocation::Snippet("<test.nyar>".to_string()), input.to_string());
    let statements = parse_program(&db, source_program);
    let accumulated = parse_program::accumulated::<NyarDiagnostic>(&db, source_program);

    format!("{:#?}", (statements.debug_all(&db), accumulated))
}

#[test]
fn parse_print() {
    let actual = parse_string("print +");
    println!("{}", actual);
}

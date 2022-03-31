use salsa::{
    cycle::CycleRecoveryStrategy, database::AsSalsaDatabase, key::DependencyIndex, runtime::local_state::QueryOrigin,
    storage::HasJarsDyn, Database, DatabaseKeyIndex, DbWithJar, DebugWithDb, Event, EventKind, Id, IngredientIndex,
    ParallelDatabase, Revision, Runtime, Snapshot, Storage,
};
use std::{
    fmt::Formatter,
    sync::{Arc, Mutex},
};

pub trait NyarData: DbWithJar<Jar> {}

#[salsa::jar(db = NyarData)]
pub struct Jar(
    crate::diagnostics::NyarDiagnostic,
    crate::files::NyarFile,
    crate::files::Symbol,
    crate::files::NyarFunction,
    crate::files::FunctionID,
    crate::files::NyarProgram,
    crate::files::ParameterID,
    // crate::ir::FunctionId,
    // crate::ir::Function,
    // crate::ir::Diagnostics,
    // crate::ir::Span,
    crate::parser::parse_program,
    crate::structures::NyarStructure,
    crate::structures::NyarStructureData,
    // crate::type_check::find_function,
);

impl<Data> NyarData for Data where Data: ?Sized + DbWithJar<Jar> {}

#[derive(Default)]
#[salsa::db(Jar)]
pub(crate) struct NyarDatabase {
    storage: Storage<Self>,
    logs: Option<Arc<Mutex<Vec<String>>>>,
}

impl Database for NyarDatabase {
    fn salsa_event(&self, event: Event) {
        // Log interesting events, if logging is enabled
        if let Some(logs) = &self.logs {
            // don't log boring events
            if let EventKind::WillExecute { .. } = event.kind {
                logs.lock().unwrap().push(format!("Event: {:?}", event.debug(self)));
            }
        }
    }
}

impl ParallelDatabase for NyarDatabase {
    fn snapshot(&self) -> Snapshot<Self> {
        Snapshot::new(NyarDatabase { storage: self.storage.snapshot(), logs: self.logs.clone() })
    }
}

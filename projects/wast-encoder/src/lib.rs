use std::{ops::AddAssign, sync::Arc};

pub use crate::{
    dag::{DependencyLogger, DependentRegistry, ResolveDependencies},
    encoder::{encode_id, encode_kebab, WastEncoder},
    enumerations::{WasiEnumeration, WasiEnumerationItem},
    functions::{ExternalFunction, WasiParameter},
    resources::WasiResource,
    symbols::{
        identifiers::Identifier,
        wasi_publisher::{WasiModule, WasiPublisher},
    },
    variants::{VariantItem, VariantType},
    wasi_module::WasiInstance,
    wasi_types::WasiType,
};

mod dag;
mod encoder;
mod enumerations;
mod functions;
mod resources;
mod symbols;
mod variants;
mod wasi_module;
mod wasi_types;

pub struct CanonicalWasi {
    pub name: Arc<str>,
    pub imports: Vec<CanonicalImport>,
    pub type_signatures: bool,
}

pub enum CanonicalImport {
    Instance(WasiInstance),
}

impl Default for CanonicalWasi {
    fn default() -> Self {
        Self { name: Arc::from("root"), imports: vec![], type_signatures: true }
    }
}

impl AddAssign<WasiInstance> for CanonicalWasi {
    fn add_assign(&mut self, rhs: WasiInstance) {
        self.imports.push(CanonicalImport::Instance(rhs));
    }
}

impl CanonicalWasi {
    pub fn add_instance(&mut self, instance: WasiInstance) {
        self.imports.push(CanonicalImport::Instance(instance));
    }
    pub fn encode(&self) -> String {
        let mut output = String::with_capacity(1024);
        let mut encoder = WastEncoder::new(&self, &mut output);
        encoder.encode().unwrap();
        output
    }
}

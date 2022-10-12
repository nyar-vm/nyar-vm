use std::{ops::AddAssign, sync::Arc};

use encoder::WastEncoder;
pub use wasi_module::WasiInstance;

mod encoder;
mod wasi_module;
mod wasi_types;

pub struct WasiCanonical {
    pub name: Arc<str>,
    pub imports: Vec<CanonicalImport>,
}

pub enum CanonicalImport {
    Instance(WasiInstance),
}

impl Default for WasiCanonical {
    fn default() -> Self {
        Self { name: Arc::from("Root"), imports: vec![] }
    }
}

impl AddAssign<WasiInstance> for WasiCanonical {
    fn add_assign(&mut self, rhs: WasiInstance) {
        self.imports.push(CanonicalImport::Instance(rhs));
    }
}

impl WasiCanonical {
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

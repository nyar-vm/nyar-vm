use std::{fmt::Write, sync::Arc};

use indexmap::IndexMap;

use crate::encoder::WastEncoder;

pub enum WasiType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Option {
        inner: Box<WasiType>,
    },
    Result {
        success: Box<WasiType>,
        failure: Box<WasiType>,
    },
    Resource {
        /// Resource language name
        name: Arc<str>,
    },
    Variant {
        /// Variant language name
        name: Arc<str>,
        variant: IndexMap<Arc<str>, WasiType>,
    },
    /// A referenced type, the real type needs to be found later
    TypeAlias {
        /// Type language name
        name: Arc<str>,
    },
}

pub struct WasiVariantItem {
    name: Arc<str>,
    variant: Arc<str>,
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub(crate) fn alias_outer(&mut self, ty: &WasiType) -> std::fmt::Result {
        let root = self.encode_id(&self.source.name);
        match ty {
            WasiType::Resource { name } => {
                let name = self.encode_id(name);
                write!(self, "(alias outer {root} {name} (type {name}))")?
            }
            WasiType::Variant { name, .. } => {
                let name = self.encode_id(name);
                let wasi_name = self.encode_kebab(&name);
                write!(self, "(alias outer {root} {name} (type {name}?))")?;
                write!(self, "(export {name} {wasi_name} (type (eq {name}?)))")?
            }
            _ => {}
        }
        Ok(())
    }
}

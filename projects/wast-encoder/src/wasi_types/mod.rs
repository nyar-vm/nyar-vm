use std::{fmt::Write, sync::Arc};

use indexmap::IndexMap;

use crate::encoder::WastEncoder;

#[derive(Clone, Debug)]
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
        owned: bool,
    },
    Variant(WasiVariant),
    /// A referenced type, the real type needs to be found later
    TypeAlias {
        /// Type language name
        name: Arc<str>,
    },
}

#[derive(Clone, Debug)]
pub struct WasiVariant {
    /// Variant language name
    name: Arc<str>,
    variant: IndexMap<Arc<str>, WasiType>,
}

#[derive(Clone, Debug)]
pub struct WasiVariantItem {
    name: Arc<str>,
    variant: Arc<str>,
}

///         (export "[method]output-stream.blocking-write-and-flush"
///             (func (param "self" (borrow $output-stream)) (param "contents" (list u8)) (result $stream-result))
///         )
#[derive(Clone, Debug)]
pub struct WasiFunction {
    name: Arc<str>,
    wasi_name: String,
    inputs: Vec<WasiParameter>,
    result: Option<WasiType>,
}

impl WasiFunction {
    pub fn new(name: Arc<str>, wasi_class: &str, wasi_name: &str) -> Self {
        let wasi_name = format!("{}.{}", wasi_class, wasi_name);
        Self { name, wasi_name, inputs: vec![], result: None }
    }
    pub fn constructor(name: Arc<str>, wasi_class: &str) -> Self {
        let wasi_name = format!("[constructor]{}", wasi_class);
        Self { name, wasi_name, inputs: vec![], result: None }
    }
    pub fn static_method(name: Arc<str>, wasi_class: &str, wasi_name: &str) -> Self {
        let wasi_name = format!("[static]{}.{}", wasi_class, wasi_name);
        Self { name, wasi_name, inputs: vec![], result: None }
    }
    pub fn method(name: Arc<str>, wasi_class: &str, wasi_name: &str) -> Self {
        let wasi_name = format!("[method]{}.{}", wasi_class, wasi_name);
        Self { name, wasi_name, inputs: vec![], result: None }
    }
    pub fn destructor(name: Arc<str>, wasi_class: &str) -> Self {
        let wasi_name = format!("[resource-drop]{}", wasi_class);
        Self { name, wasi_name, inputs: vec![], result: None }
    }
}

#[derive(Clone, Debug)]
pub struct WasiParameter {
    name: Arc<str>,
    r#type: WasiType,
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub(crate) fn alias_outer(&mut self, ty: &WasiType) -> std::fmt::Result {
        let root = self.encode_id(&self.source.name);
        match ty {
            WasiType::Resource { name, .. } => {
                let name = self.encode_id(name);
                write!(self, "(alias outer {root} {name} (type {name}))")?
            }
            WasiType::Variant(variant) => {
                let name = self.encode_id(&variant.name);
                let wasi_name = self.encode_kebab(&name);
                write!(self, "(alias outer {root} {name} (type {name}?))")?;
                write!(self, "(export {name} {wasi_name} (type (eq {name}?)))")?
            }
            _ => {}
        }
        Ok(())
    }
}

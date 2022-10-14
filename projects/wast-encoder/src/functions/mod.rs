use std::{ops::AddAssign, sync::Arc};

use crate::WasiType;

mod arithmetic;

///         (export "[method]output-stream.blocking-write-and-flush"
///             (func (param "self" (borrow $output-stream)) (param "contents" (list u8)) (result $stream-result))
///         )
#[derive(Clone, Debug)]
pub struct WasiFunction {
    pub name: Arc<str>,
    pub wasi_name: String,
    pub inputs: Vec<WasiParameter>,
    pub result: Option<WasiType>,
}

#[derive(Clone, Debug)]
pub struct WasiParameter {
    pub name: Arc<str>,
    pub r#type: WasiType,
}

impl WasiFunction {
    pub fn new<S>(name: S, wasi_name: &str) -> Self
    where
        S: Into<Arc<str>>,
    {
        Self { name: name.into(), wasi_name: wasi_name.to_string(), inputs: vec![], result: None }
    }
    pub fn constructor<S>(name: S, wasi_class: &str) -> Self
    where
        S: Into<Arc<str>>,
    {
        let wasi_name = format!("[constructor]{}", wasi_class);
        Self { name: name.into(), wasi_name, inputs: vec![], result: None }
    }
    pub fn static_method<S>(name: S, wasi_class: &str, wasi_name: &str) -> Self
    where
        S: Into<Arc<str>>,
    {
        let wasi_name = format!("[static]{}.{}", wasi_class, wasi_name);
        Self { name: name.into(), wasi_name, inputs: vec![], result: None }
    }
    pub fn method<S>(name: S, wasi_class: &str, wasi_name: &str) -> Self
    where
        S: Into<Arc<str>>,
    {
        let wasi_name = format!("[method]{}.{}", wasi_class, wasi_name);
        Self { name: name.into(), wasi_name, inputs: vec![], result: None }
    }
    pub fn destructor<S>(name: S, wasi_class: &str) -> Self
    where
        S: Into<Arc<str>>,
    {
        let wasi_name = format!("[resource-drop]{}", wasi_class);
        Self { name: name.into(), wasi_name, inputs: vec![], result: None }
    }
}

impl WasiParameter {
    pub fn new<S>(name: S, r#type: WasiType) -> Self
    where
        S: Into<Arc<str>>,
    {
        Self { name: name.into(), r#type }
    }
}

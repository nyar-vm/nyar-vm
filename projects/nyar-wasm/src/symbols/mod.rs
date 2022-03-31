use std::{
    fmt::{Debug, Display, Formatter},
    sync::Arc,
};

mod convert;
mod display;

#[derive(Clone)]
pub struct WasmSymbol {
    inner: Arc<str>,
}

impl WasmSymbol {
    pub fn new(name: &str) -> Self {
        Self { inner: Arc::from(name) }
    }
}

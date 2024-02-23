use std::{
    fmt::{Display, Formatter, Write},
    sync::Arc,
};

mod encoder;
mod wasi_module;

pub struct WasiCanonical {
    component: Arc<str>,
}

impl Default for WasiCanonical {
    fn default() -> Self {
        Self { component: Arc::from("Root") }
    }
}

#[test]
fn test() {}

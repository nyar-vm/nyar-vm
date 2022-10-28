pub use crate::vm::NyarVM;

wasmtime::component::bindgen!();

mod host;
mod vm;

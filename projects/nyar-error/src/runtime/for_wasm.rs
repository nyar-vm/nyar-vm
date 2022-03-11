use crate::NyarError;

impl From<wasmtime::Error> for NyarError {
    fn from(value: wasmtime::Error) -> Self {
        NyarError::runtime_error(value.to_string())
    }
}

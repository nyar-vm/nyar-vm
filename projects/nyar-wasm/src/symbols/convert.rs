use super::*;

impl AsRef<str> for WasmSymbol {
    fn as_ref(&self) -> &str {
        self.inner.as_ref()
    }
}

impl From<String> for WasmSymbol {
    fn from(value: String) -> Self {
        Self { inner: Arc::from(value) }
    }
}
impl From<Arc<str>> for WasmSymbol {
    fn from(value: Arc<str>) -> Self {
        Self { inner: value }
    }
}

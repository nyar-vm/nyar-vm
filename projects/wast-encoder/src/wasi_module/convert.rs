use super::*;

impl From<&str> for WasiModule {
    fn from(value: &str) -> Self {
        Self { name: value.into(), package: None, version: None }
    }
}

impl AddAssign<WasiFunction> for WasiInstance {
    fn add_assign(&mut self, rhs: WasiFunction) {
        self.functions.insert(rhs.name.clone(), rhs);
    }
}

impl AddAssign<WasiResource> for WasiInstance {
    fn add_assign(&mut self, rhs: WasiResource) {
        self.resources.insert(rhs.name.clone(), rhs);
    }
}

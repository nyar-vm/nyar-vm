use super::*;

impl AddAssign<ExternalFunction> for WasiInstance {
    fn add_assign(&mut self, rhs: ExternalFunction) {
        self.functions.insert(rhs.name.clone(), rhs);
    }
}

impl AddAssign<WasiResource> for WasiInstance {
    fn add_assign(&mut self, rhs: WasiResource) {
        self.resources.insert(rhs.symbol.clone(), rhs);
    }
}

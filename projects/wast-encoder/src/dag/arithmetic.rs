use super::*;

impl AddAssign<WasiResource> for DependentRegistry {
    fn add_assign(&mut self, rhs: WasiResource) {
        self.types.insert(rhs.symbol.clone(), WasiType::Resource(rhs));
    }
}

impl AddAssign<ExternalFunction> for DependentRegistry {
    fn add_assign(&mut self, rhs: ExternalFunction) {
        self.functions.insert(rhs.name.clone(), rhs);
    }
}

use super::*;

impl AddAssign<WasiResource> for DependentRegistry {
    fn add_assign(&mut self, rhs: WasiResource) {
        self.resources.insert(rhs.name.clone(), rhs);
    }
}

impl AddAssign<WasiParameter> for DependentRegistry {
    fn add_assign(&mut self, rhs: WasiParameter) {
        self.types.insert(rhs.name.clone(), rhs);
    }
}

impl AddAssign<WasiFunction> for DependentRegistry {
    fn add_assign(&mut self, rhs: WasiFunction) {
        self.functions.insert(rhs.name.clone(), rhs);
    }
}

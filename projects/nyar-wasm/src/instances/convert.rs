use super::*;

impl AddAssign<WasiResource> for WasiInstance {
    fn add_assign(&mut self, rhs: WasiResource) {
        self.resources.insert(rhs.symbol.clone(), rhs);
    }
}

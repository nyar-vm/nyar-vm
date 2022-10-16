use super::*;

impl AddAssign<WasiVariant> for WasiVariant {
    fn add_assign(&mut self, rhs: WasiVariant) {
        self.variants.extend(rhs.variants);
    }
}

use super::*;

impl AddAssign<WasiVariantType> for WasiVariantType {
    fn add_assign(&mut self, rhs: WasiVariantType) {
        self.variants.extend(rhs.variants);
    }
}

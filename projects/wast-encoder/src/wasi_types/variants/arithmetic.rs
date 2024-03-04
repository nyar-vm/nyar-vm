use super::*;

impl AddAssign<WasiVariantItem> for WasiVariantType {
    fn add_assign(&mut self, rhs: WasiVariantItem) {
        self.variants.insert(rhs.symbol.clone(), rhs);
    }
}
impl AddAssign<WasiVariantType> for WasiVariantType {
    fn add_assign(&mut self, rhs: WasiVariantType) {
        self.variants.extend(rhs.variants);
    }
}

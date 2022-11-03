use super::*;

impl AddAssign<VariantItem> for WasiVariantType {
    fn add_assign(&mut self, rhs: VariantItem) {
        self.variants.insert(rhs.symbol.clone(), rhs);
    }
}
impl AddAssign<WasiVariantType> for WasiVariantType {
    fn add_assign(&mut self, rhs: WasiVariantType) {
        self.variants.extend(rhs.variants);
    }
}

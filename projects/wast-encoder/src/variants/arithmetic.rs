use super::*;

impl AddAssign<VariantItem> for VariantType {
    fn add_assign(&mut self, rhs: VariantItem) {
        self.variants.insert(rhs.symbol.clone(), rhs);
    }
}
impl AddAssign<VariantType> for VariantType {
    fn add_assign(&mut self, rhs: VariantType) {
        self.variants.extend(rhs.variants);
    }
}

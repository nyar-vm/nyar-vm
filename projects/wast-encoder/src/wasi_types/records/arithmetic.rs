use super::*;

impl Eq for WasiRecordType {}

impl PartialEq for WasiRecordType {
    fn eq(&self, other: &Self) -> bool {
        self.symbol.eq(&other.symbol)
    }
}

impl PartialOrd for WasiRecordType {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.symbol.partial_cmp(&other.symbol)
    }
}

impl Ord for WasiRecordType {
    fn cmp(&self, other: &Self) -> Ordering {
        self.symbol.cmp(&other.symbol)
    }
}

impl Hash for WasiRecordType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.symbol.hash(state)
    }
}
impl AddAssign<WasiRecordField> for WasiRecordType {
    fn add_assign(&mut self, rhs: WasiRecordField) {
        self.fields.insert(rhs.name.clone(), rhs);
    }
}

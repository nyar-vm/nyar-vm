use super::*;

impl AddAssign<WasiParameter> for ExternalFunction {
    fn add_assign(&mut self, rhs: WasiParameter) {
        self.inputs.push(rhs);
    }
}

impl AddAssign<WasiType> for ExternalFunction {
    fn add_assign(&mut self, rhs: WasiType) {
        self.output = Some(rhs);
    }
}

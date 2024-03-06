use super::*;

impl AddAssign<WasiParameter> for WasiFunction {
    fn add_assign(&mut self, rhs: WasiParameter) {
        self.inputs.push(rhs);
    }
}

impl AddAssign<WasiType> for WasiFunction {
    fn add_assign(&mut self, rhs: WasiType) {
        self.output = vec![WasiParameter::new("output", rhs)];
    }
}

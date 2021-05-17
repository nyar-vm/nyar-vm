// use std::collections::VecDeque;
//
// use nyar_error::NyarError;
//
// use super::*;

// pub struct Maybe {
//     value: SharedValue,
//     error: AnyList,
// }
//
// pub struct Validation {}
//
// impl Value {
//     pub fn is_unit(&self) -> bool {
//         matches!(self, Value::Unit)
//     }
// }
//
// impl Maybe {
//     pub fn is_option(&self) -> Result<bool> {
//         self.rhs.read().is_unit()
//     }
//     pub fn is_result(&self) -> Result<bool> {
//         self.rhs.read().is_unit()
//     }
//     pub fn is_either(&self) -> Result<bool> {
//         self.rhs.read().is_unit()
//     }
//     pub fn is_validation(&self) -> Result<bool> {
//         self.rhs.read().is_unit()
//     }
// }

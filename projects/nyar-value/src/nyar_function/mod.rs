use std::collections::BTreeMap;


pub trait NyarFunction {
    fn get_table() {}
    fn get_principal() {}
}

pub enum NyarFunctionTable<T> {
    Principal(T),
    Overload(BTreeMap<usize, T>),
    Dispatch(BTreeMap<usize, T>),
}


pub struct NyarFunctionApply {}
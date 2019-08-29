use std::collections::LinkedList;

use crate::NyarValue;

#[derive(Debug, Scan)]
pub struct AnyList {
    inner: LinkedList<NyarValue>,
}

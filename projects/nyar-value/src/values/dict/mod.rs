use crate::NyarValue;
use shredder::Scan;
use std::collections::HashMap;

#[derive(Clone, Debug, Scan)]
pub struct NyarDict {
    // TODO: using some faster non-safe hasher
    inner: HashMap<String, NyarValue>,
}

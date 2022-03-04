use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    ops::Not,
    sync::Arc,
};

use indexmap::IndexMap;

use crate::{types::ValkyrieMetaType, ValkyrieTypeLegacy, ValkyrieValue};

impl ValkyrieTypeLegacy for bool {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Boolean(self)
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut meta = ValkyrieMetaType::default();
        meta.set_namepath("std.primitive.Boolean");
        Arc::new(meta)
    }
}

pub struct TypeManager {
    types: IndexMap<u64, Arc<ValkyrieMetaType>>,
}

impl TypeManager {
    pub fn get_or_insert(&mut self, info: Arc<ValkyrieMetaType>) -> Arc<ValkyrieMetaType> {
        let mut hasher = DefaultHasher::new();
        info.hash(&mut hasher);
        let hash = hasher.finish();
        match self.types.get(&hash) {
            Some(t) => t.clone(),
            None => {
                self.types.insert(hash, info.clone());
                info
            }
        }
    }
}

pub struct ValkyrieFunction {
    document: String,
    parameters: Vec<ValkyrieMetaType>,
    return_type: ValkyrieMetaType,
    function_ptr: ValkyrieFunctionInstance,
}

pub enum ValkyrieFunctionInstance {
    Normal { apply: fn(Vec<ValkyrieValue>) -> ValkyrieValue },
    Curry { apply: fn(Vec<ValkyrieValue>) -> ValkyrieValue, parameters: Vec<ValkyrieValue> },
}

impl From<bool> for ValkyrieValue {
    fn from(value: bool) -> Self {
        ValkyrieValue::Boolean(value)
    }
}

// std.primitive.Boolean
pub fn not(args: Vec<ValkyrieValue>, kws: IndexMap<String, ValkyrieValue>) -> ValkyrieValue {
    if !kws.is_empty() {
        panic!("Invalid keyword arguments");
    }
    match args.get(0) {
        Some(ValkyrieValue::Boolean(p)) => return ValkyrieValue::from(p.not()),
        _ => panic!("Invalid type"),
    }
}

pub fn and(p: bool, q: bool) -> bool {
    p && q
}

pub fn or(p: bool, q: bool) -> bool {
    p || q
}

pub fn logic_gate(p: bool, q: bool, mask: u8) -> Result<bool, String> {
    let ok = match mask {
        0 => false,
        1 => p && q,
        2 => p && !q,
        3 => p,
        4 => !p && q,
        5 => q,
        6 => p ^ q,
        7 => p || q,
        8 => !p && !q,
        9 => p == q,
        10 => !q,
        11 => p || !q,
        12 => !p,
        13 => !p || q,
        14 => !p || !q,
        15 => true,
        _ => return Err(format!("Invalid mask: {}", mask)),
    };
    Ok(ok)
}

use std::{
    fmt::{Debug, Display},
    hash::{Hash, Hasher},
    sync::Arc,
};

use itertools::Itertools;

use crate::{ValkyrieClass, ValkyrieClassType, ValkyrieValue, ValkyrieVariantType};

pub mod atomic_type;
pub mod class_type;
pub mod literal_type;
pub mod tuple_type;
pub mod union_type;
pub mod variant_type;

// rtti of valkyrie type
#[derive(Debug, Default)]
pub struct ValkyrieMetaType {
    namepath: Vec<String>,
    document: String,
    generic_types: Vec<Arc<ValkyrieMetaType>>,
}

impl ValkyrieTypeLegacy for () {
    fn boxed(self) -> ValkyrieValue {
        ValkyrieValue::Class(Arc::new(ValkyrieClass::tuple()))
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        this.set_namepath("std.primitive.Unit");
        Arc::new(this)
    }
}

impl Default for ValkyrieValue {
    fn default() -> Self {
        ValkyrieValue::Never
    }
}

impl ValkyrieTypeLegacy for ValkyrieValue {
    fn boxed(self) -> ValkyrieValue {
        self
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        let mut this = ValkyrieMetaType::default();
        match self {
            ValkyrieValue::Never => {
                this.set_namepath("std.primitive.Any");
                Arc::new(this)
            }
            ValkyrieValue::Boolean(v) => v.type_info(),
            ValkyrieValue::Unsigned8(v) => v.type_info(),
            ValkyrieValue::Unsigned16(v) => v.type_info(),
            ValkyrieValue::Unsigned32(v) => v.type_info(),
            ValkyrieValue::Unsigned64(v) => v.type_info(),
            ValkyrieValue::Unsigned128(v) => v.type_info(),
            ValkyrieValue::Integer8(v) => v.type_info(),
            ValkyrieValue::Integer16(v) => v.type_info(),
            ValkyrieValue::Integer32(v) => v.type_info(),
            ValkyrieValue::Integer64(v) => v.type_info(),
            ValkyrieValue::Integer128(v) => v.type_info(),
            ValkyrieValue::Float32(v) => v.type_info(),
            ValkyrieValue::Float64(v) => v.type_info(),
            ValkyrieValue::Buffer(v) => v.type_info(),
            ValkyrieValue::String(v) => v.type_info(),
            ValkyrieValue::Class(v) => v.type_info(),
            ValkyrieValue::Variant(v) => v.type_info(),
        }
    }
}

impl ValkyrieMetaType {
    pub fn set_namepath(&mut self, namepath: &str) {
        self.namepath.clear();
        for s in namepath.split('.') {
            self.namepath.push(s.to_string());
        }
    }
    pub fn mut_namepath(&mut self) -> &mut Vec<String> {
        &mut self.namepath
    }
    pub fn mut_generic_types(&mut self) -> &mut Vec<Arc<ValkyrieMetaType>> {
        &mut self.generic_types
    }
}

pub enum ValkyrieType {
    Atomic(Arc<ValkyrieAtomicType>),
    Class(Arc<ValkyrieClassType>),
    Variant(Arc<ValkyrieVariantType>),
}

#[allow(clippy::wrong_self_convention)]
pub trait ValkyrieTypeLegacy
where
    Self: Sized,
{
    fn boxed(self) -> ValkyrieValue;
    fn type_info(&self) -> Arc<ValkyrieMetaType>;
}

impl ValkyrieMetaType {
    pub fn name(&self) -> String {
        assert_ne!(self.namepath.len(), 0, "namepath `{:?}` is not valid", self.namepath);
        self.namepath.last().unwrap().to_owned()
    }
    pub fn namespace(&self, join: &str) -> String {
        assert_ne!(self.namepath.len(), 0, "namepath `{:?}` is not valid", self.namepath);
        self.namepath[..self.namepath.len() - 1].join(join)
    }
    pub fn display_type(&self, full_path: bool) -> String {
        let this = match full_path {
            true => self.namepath.join("::"),
            false => self.name(),
        };
        if self.generic_types.is_empty() {
            return this;
        }
        format!("{}[{}]", this, self.generic_types.iter().map(|f| f.type_info().display_type(full_path)).join(", "))
    }
}

impl Display for ValkyrieMetaType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.display_type(false))
    }
}

impl Hash for ValkyrieMetaType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.display_type(true).as_bytes());
    }
}

impl ValkyrieTypeLegacy for ValkyrieMetaType {
    fn boxed(self) -> ValkyrieValue {
        todo!()
    }

    fn type_info(&self) -> Arc<ValkyrieMetaType> {
        todo!()
    }
}

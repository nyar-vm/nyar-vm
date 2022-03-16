use crate::functions::FunctionBuilder;
use indexmap::{map::Values, IndexMap};
use indexmap::map::Iter;
use nyar_error::{FileSpan, NyarError};
use nyar_hir::{Identifier, NyarType, NyarValue};
use wasm_encoder::{ConstExpr, GlobalSection, GlobalType, RefType, ValType};
use crate::helpers::IndexedIterator;

#[derive(Default)]
pub struct GlobalBuilder {
    items: IndexMap<String, GlobalItem>,
}

pub struct GlobalItem {
    pub(crate) name: Identifier,
    pub(crate) value: NyarValue,
}

impl GlobalBuilder {
    pub fn insert(&mut self, item: GlobalItem) -> Option<GlobalItem> {
        self.items.insert(item.name.to_string(), item)
    }
}

impl<'i> IntoIterator for &'i GlobalBuilder {
    type Item = ();
    type IntoIter = IndexedIterator<'i, GlobalItem>;

    fn into_iter(self) -> Self::IntoIter {
        let a: Iter<String, GlobalItem> = self.items.iter()
    }
}

impl GlobalItem {
    pub fn i32(name: Identifier, value: i32) -> Self {
        Self { name, value: NyarValue::I32(value) }
    }
    pub fn i64(name: Identifier, value: i32) -> Self {
        Self { name, value: NyarValue::I32(value) }
    }
    pub fn f32(name: Identifier, value: f32) -> Self {
        Self { name, value: NyarValue::F32(value) }
    }
    pub fn f64(name: Identifier, value: f32) -> Self {
        Self { name, value: NyarValue::F32(value) }
    }
    pub fn function(name: Identifier) -> Self {
        Self { name: name.clone(), value: NyarValue::Function(name) }
    }
    pub fn build(&self, section: &mut GlobalSection, functions: &FunctionBuilder) -> Result<(), NyarError> {
        match &self.value {
            NyarValue::I32(v) => {
                section.global(GlobalType { val_type: ValType::I32, mutable: true }, &ConstExpr::i32_const(*v))
            }
            NyarValue::I64(v) => {
                section.global(GlobalType { val_type: ValType::I64, mutable: true }, &ConstExpr::i64_const(*v))
            }
            NyarValue::F32(v) => {
                section.global(GlobalType { val_type: ValType::F32, mutable: true }, &ConstExpr::f32_const(*v))
            }
            NyarValue::F64(v) => {
                section.global(GlobalType { val_type: ValType::F64, mutable: true }, &ConstExpr::f64_const(*v))
            }
            NyarValue::Function(v) => {
                let id = functions.get_id(&v.to_string())?;
                section.global(GlobalType { val_type: ValType::FUNCREF, mutable: true }, &ConstExpr::ref_func(id as u32))
            }
        };
        Ok(())
    }
}

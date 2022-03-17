use crate::{
    functions::FunctionBuilder,
    helpers::{WasmBuilder, WasmEmitter},
};
use nyar_error::NyarError;
use nyar_hir::{GlobalBuilder, NamedValue, NyarValue};
use wasm_encoder::{ConstExpr, GlobalSection, GlobalType, Module, ValType};

impl WasmBuilder<GlobalSection> for GlobalBuilder {
    type Store = FunctionBuilder;
    fn build(&self, store: &Self::Store) -> Result<GlobalSection, NyarError> {
        let mut global = GlobalSection::default();
        for (_, _, value) in self.into_iter() {
            value.emit(&mut global, store)?;
        }
        Ok(global)
    }
}

impl WasmEmitter for GlobalBuilder {
    type Receiver = Module;
    type Store = FunctionBuilder;
    fn emit(&self, reviver: &mut Self::Receiver, store: &Self::Store) -> Result<(), NyarError> {
        reviver.section(&self.build(store)?);
        Ok(())
    }
}

impl WasmEmitter for NamedValue {
    type Receiver = GlobalSection;
    type Store = FunctionBuilder;
    fn emit(&self, reviver: &mut Self::Receiver, store: &Self::Store) -> Result<(), NyarError> {
        match self.value() {
            NyarValue::I32(v) => {
                reviver.global(GlobalType { val_type: ValType::I32, mutable: true }, &ConstExpr::i32_const(*v))
            }
            NyarValue::I64(v) => {
                reviver.global(GlobalType { val_type: ValType::I64, mutable: true }, &ConstExpr::i64_const(*v))
            }
            NyarValue::F32(v) => {
                reviver.global(GlobalType { val_type: ValType::F32, mutable: true }, &ConstExpr::f32_const(*v))
            }
            NyarValue::F64(v) => {
                reviver.global(GlobalType { val_type: ValType::F64, mutable: true }, &ConstExpr::f64_const(*v))
            }
            NyarValue::Function(v) => {
                let id = store.get_id(&v.to_string())?;
                reviver.global(GlobalType { val_type: ValType::FUNCREF, mutable: true }, &ConstExpr::ref_func(id as u32))
            }
        };
        Ok(())
    }
}

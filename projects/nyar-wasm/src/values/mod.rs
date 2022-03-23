use crate::helpers::{WasmBuilder, WasmEmitter};
use nyar_error::NyarError;
use nyar_hir::{FunctionRegister, GlobalBuilder, NamedValue, NyarValue};
use wasm_encoder::{CodeSection, ConstExpr, Function, GlobalSection, GlobalType, Instruction, Module, ValType};

impl WasmBuilder<GlobalSection> for GlobalBuilder {
    type Store = FunctionRegister;
    fn build(&self, store: &Self::Store) -> Result<GlobalSection, NyarError> {
        let mut global = GlobalSection::default();
        for (_, _, value) in self.into_iter() {
            value.emit_global(&mut global, store)?;
        }
        Ok(global)
    }
}

impl WasmEmitter for GlobalBuilder {
    type Receiver = Module;
    type Store = FunctionRegister;
    fn emit(&self, reviver: &mut Self::Receiver, store: &Self::Store) -> Result<(), NyarError> {
        reviver.section(&self.build(store)?);
        Ok(())
    }
}

pub trait WasmVariable {
    fn emit_global(&self, reviver: &mut GlobalSection, store: &FunctionRegister) -> Result<(), NyarError>;
    fn emit_local_def(&self, reviver: &mut Function, store: &FunctionRegister) -> Result<(), NyarError>;
}

impl WasmVariable for NamedValue {
    fn emit_global(&self, m: &mut GlobalSection, fs: &FunctionRegister) -> Result<(), NyarError> {
        match self.value() {
            NyarValue::U32(v) => {
                m.global(GlobalType { val_type: ValType::I32, mutable: self.mutable() }, &ConstExpr::i32_const(*v as i32))
            }
            NyarValue::I32(v) => {
                m.global(GlobalType { val_type: ValType::I32, mutable: self.mutable() }, &ConstExpr::i32_const(*v))
            }
            NyarValue::I64(v) => {
                m.global(GlobalType { val_type: ValType::I64, mutable: self.mutable() }, &ConstExpr::i64_const(*v))
            }
            NyarValue::F32(v) => {
                m.global(GlobalType { val_type: ValType::F32, mutable: self.mutable() }, &ConstExpr::f32_const(*v))
            }
            NyarValue::F64(v) => {
                m.global(GlobalType { val_type: ValType::F64, mutable: self.mutable() }, &ConstExpr::f64_const(*v))
            }
            NyarValue::Function(v) => {
                let id = fs.get_id(&v.to_string())?;
                m.global(GlobalType { val_type: ValType::FUNCREF, mutable: true }, &ConstExpr::ref_func(id as u32))
            }
            NyarValue::Any => {
                panic!()
            }
            NyarValue::Structure => {
                panic!()
            }
            NyarValue::Array => {
                panic!()
            }
        };
        Ok(())
    }

    fn emit_local_def(&self, f: &mut Function, store: &FunctionRegister) -> Result<(), NyarError> {
        // f.instruction(&Instruction::GlobalGet(10));
        // f.instruction(&Instruction::LocalSet(2));

        todo!()
    }
}

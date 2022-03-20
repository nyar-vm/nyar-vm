use crate::helpers::WasmEmitter;
use nyar_error::NyarError;
use nyar_hir::{FunctionBody, Operation};
use wasm_encoder::{Function, Instruction};

pub trait WasmFunctionBody {
    fn emit_function_body(&self) -> Result<Function, NyarError>;
}

pub trait WasmOperation {
    fn emit_operation(&self, f: &mut Function);
}

impl WasmOperation for Operation {
    fn emit_operation(&self, f: &mut Function) {
        match self {
            Self::GlobalGet { index } => {
                f.instruction(&Instruction::GlobalGet(*index));
            }
            Self::LocalGet { index } => {
                f.instruction(&Instruction::LocalGet(*index));
            }
            Self::LocalSet { index } => {
                f.instruction(&Instruction::LocalSet(*index));
            }
            Self::I32Add { lhs, rhs } => {
                lhs.emit_operation(f);
                rhs.emit_operation(f);
                f.instruction(&Instruction::I32Add);
            }
        }
    }
}

impl WasmFunctionBody for FunctionBody {
    fn emit_function_body(&self) -> Result<Function, NyarError> {
        let locals = vec![];
        let mut f = Function::new(locals);
        for i in self.into_iter() {
            i.emit_operation(&mut f)
        }
        f.instruction(&Instruction::End);

        Ok(f)
    }
}

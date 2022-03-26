use nyar_error::NyarError;
use nyar_hir::{FunctionBody, NativeDataType, NyarValue, Operation};
use wasm_encoder::{Function, Instruction};

pub trait WasmFunctionBody {
    fn emit_function_body(&self) -> Result<Function, NyarError>;
}

pub trait WasmOperation {
    fn emit(&self, f: &mut Function);
}

impl WasmOperation for Operation {
    fn emit(&self, f: &mut Function) {
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
            Self::NativeSum { native, terms } => match terms.as_slice() {
                [] => {
                    match native {
                        NativeDataType::I32 => f.instruction(&Instruction::I32Const(0)),
                        NativeDataType::I64 => f.instruction(&Instruction::I64Const(0)),
                        NativeDataType::F32 => f.instruction(&Instruction::F32Const(0.0)),
                        NativeDataType::F64 => f.instruction(&Instruction::F64Const(0.0)),
                    };
                }
                [first] => first.emit(f),
                [first, rest @ ..] => {
                    first.emit(f);
                    for i in rest {
                        i.emit(f);
                        match native {
                            NativeDataType::I32 => f.instruction(&Instruction::I32Add),
                            NativeDataType::I64 => f.instruction(&Instruction::I64Add),
                            NativeDataType::F32 => f.instruction(&Instruction::F32Add),
                            NativeDataType::F64 => f.instruction(&Instruction::F64Add),
                        };
                    }
                }
            },
            Self::NativeEqual { native, terms } => match terms.as_slice() {
                [] => panic!(),
                [_] => panic!(),
                [first, rest @ ..] => {
                    first.emit(f);
                    for i in rest {
                        i.emit(f);
                        match native {
                            NativeDataType::I32 => f.instruction(&Instruction::I32Eq),
                            NativeDataType::I64 => f.instruction(&Instruction::I64Eq),
                            NativeDataType::F32 => f.instruction(&Instruction::F32Eq),
                            NativeDataType::F64 => f.instruction(&Instruction::F64Eq),
                        };
                    }
                }
            },
            Operation::NativeEqualZero { native, term } => {
                term.emit(f);
                match native {
                    NativeDataType::I32 => f.instruction(&Instruction::I32Eqz),
                    NativeDataType::I64 => f.instruction(&Instruction::I64Eqz),
                    _ => panic!("unsupported operator {:?} for {:?}", self, native),
                };
            }
            Operation::Constant { value } => {
                match value {
                    NyarValue::U32(v) => f.instruction(&Instruction::I32Const(*v as i32)),
                    NyarValue::I32(v) => f.instruction(&Instruction::I32Const(*v)),
                    NyarValue::I64(v) => f.instruction(&Instruction::I64Const(*v)),
                    NyarValue::F32(v) => f.instruction(&Instruction::F32Const(*v)),
                    NyarValue::F64(v) => f.instruction(&Instruction::F64Const(*v)),
                    NyarValue::Function(_) => {
                        panic!()
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
            }
        }
    }
}

impl WasmFunctionBody for FunctionBody {
    fn emit_function_body(&self) -> Result<Function, NyarError> {
        let locals = vec![];
        let mut f = Function::new(locals);
        for i in self.into_iter() {
            i.emit(&mut f)
        }
        f.instruction(&Instruction::End);

        Ok(f)
    }
}

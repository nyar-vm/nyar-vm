use crate::bytecode::decoder::Instruction;
use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;

impl NyarVM {
    pub fn execute_float_op(&mut self, ins: Instruction) -> Result<(), VmError> {
        match ins {
            Instruction::F32Const(v) => {
                self.push(Value::float(v as f64));
            }
            Instruction::F32Add => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = ((lhs.as_float() as f32) + (rhs.as_float() as f32)) as f64;
                self.push(Value::float(r));
            }
            Instruction::F32Sub => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = ((lhs.as_float() as f32) - (rhs.as_float() as f32)) as f64;
                self.push(Value::float(r));
            }
            Instruction::F32Mul => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = ((lhs.as_float() as f32) * (rhs.as_float() as f32)) as f64;
                self.push(Value::float(r));
            }
            Instruction::F32Div => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = ((lhs.as_float() as f32) / (rhs.as_float() as f32)) as f64;
                self.push(Value::float(r));
            }
            Instruction::F32Neg => {
                let v = self.pop()?;
                let r = (-(v.as_float() as f32)) as f64;
                self.push(Value::float(r));
            }
            Instruction::F32Eq => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_float() as f32) == (rhs.as_float() as f32);
                self.push(Value::bool(r));
            }
            Instruction::F32Ne => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_float() as f32) != (rhs.as_float() as f32);
                self.push(Value::bool(r));
            }
            Instruction::F32Lt => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_float() as f32) < (rhs.as_float() as f32);
                self.push(Value::bool(r));
            }
            Instruction::F32Le => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_float() as f32) <= (rhs.as_float() as f32);
                self.push(Value::bool(r));
            }
            Instruction::F32Gt => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_float() as f32) > (rhs.as_float() as f32);
                self.push(Value::bool(r));
            }
            Instruction::F32Ge => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_float() as f32) >= (rhs.as_float() as f32);
                self.push(Value::bool(r));
            }
            Instruction::F32ToI32S => {
                let v = self.pop()?;
                let r = v.as_float() as i32;
                self.push(Value::int(r as i64));
            }
            Instruction::F32ToI32U => {
                let v = self.pop()?;
                let r = v.as_float() as u32;
                self.push(Value::int(r as i64));
            }
            Instruction::F32ToI64S => {
                let v = self.pop()?;
                let r = v.as_float() as i64;
                self.push(Value::int(r));
            }
            Instruction::F32ToI64U => {
                let v = self.pop()?;
                let r = v.as_float() as u64;
                self.push(Value::int(r as i64));
            }
            Instruction::F32ToF64 => {
                let v = self.pop()?;
                let r = v.as_float();
                self.push(Value::float(r));
            }
            Instruction::F64Const(v) => {
                self.push(Value::float(v));
            }
            Instruction::F64Add => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.as_float() + rhs.as_float();
                self.push(Value::float(r));
            }
            Instruction::F64Sub => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.as_float() - rhs.as_float();
                self.push(Value::float(r));
            }
            Instruction::F64Mul => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.as_float() * rhs.as_float();
                self.push(Value::float(r));
            }
            Instruction::F64Div => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.as_float() / rhs.as_float();
                self.push(Value::float(r));
            }
            Instruction::F64Neg => {
                let v = self.pop()?;
                let r = -v.as_float();
                self.push(Value::float(r));
            }
            Instruction::F64Eq => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.as_float() == rhs.as_float();
                self.push(Value::bool(r));
            }
            Instruction::F64Ne => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.as_float() != rhs.as_float();
                self.push(Value::bool(r));
            }
            Instruction::F64Lt => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.as_float() < rhs.as_float();
                self.push(Value::bool(r));
            }
            Instruction::F64Le => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.as_float() <= rhs.as_float();
                self.push(Value::bool(r));
            }
            Instruction::F64Gt => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.as_float() > rhs.as_float();
                self.push(Value::bool(r));
            }
            Instruction::F64Ge => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.as_float() >= rhs.as_float();
                self.push(Value::bool(r));
            }
            Instruction::F64ToI32S => {
                let v = self.pop()?;
                let r = v.as_float() as i32;
                self.push(Value::int(r as i64));
            }
            Instruction::F64ToI32U => {
                let v = self.pop()?;
                let r = v.as_float() as u32;
                self.push(Value::int(r as i64));
            }
            Instruction::F64ToI64S => {
                let v = self.pop()?;
                let r = v.as_float() as i64;
                self.push(Value::int(r));
            }
            Instruction::F64ToI64U => {
                let v = self.pop()?;
                let r = v.as_float() as u64;
                self.push(Value::int(r as i64));
            }
            Instruction::F64ToF32 => {
                let v = self.pop()?;
                let r = (v.as_float() as f32) as f64;
                self.push(Value::float(r));
            }
            _ => Err(VmError::InvalidOpcode),
        }
    }
}

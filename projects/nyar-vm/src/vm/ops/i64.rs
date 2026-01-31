use crate::bytecode::decoder::Instruction;
use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;

impl NyarVM {
    pub fn execute_i64_op(&mut self, ins: Instruction) -> Result<(), VmError> {
        match ins {
            Instruction::I64Const(v) => {
                self.push(Value::int(v));
            }
            Instruction::I64Add => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i64).wrapping_add(rhs.as_int() as i64);
                self.push(Value::int(r));
            }
            Instruction::I64Sub => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i64).wrapping_sub(rhs.as_int() as i64);
                self.push(Value::int(r));
            }
            Instruction::I64Mul => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i64).wrapping_mul(rhs.as_int() as i64);
                self.push(Value::int(r));
            }
            Instruction::I64DivS => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let (r, _) = (lhs.as_int() as i64).overflowing_div(rhs.as_int() as i64);
                self.push(Value::int(r));
            }
            Instruction::I64DivU => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let (r, _) = (lhs.as_int() as u64).overflowing_div(rhs.as_int() as u64);
                self.push(Value::int(r as i64));
            }
            Instruction::I64RemS => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let (r, _) = (lhs.as_int() as i64).overflowing_rem(rhs.as_int() as i64);
                self.push(Value::int(r));
            }
            Instruction::I64RemU => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let (r, _) = (lhs.as_int() as u64).overflowing_rem(rhs.as_int() as u64);
                self.push(Value::int(r as i64));
            }
            Instruction::I64Neg => {
                let v = self.pop()?;
                let r = -(v.as_int() as i64);
                self.push(Value::int(r));
            }
            Instruction::I64Eq => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i64) == (rhs.as_int() as i64);
                self.push(Value::bool(r));
            }
            Instruction::I64Ne => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i64) != (rhs.as_int() as i64);
                self.push(Value::bool(r));
            }
            Instruction::I64LtS => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i64) < (rhs.as_int() as i64);
                self.push(Value::bool(r));
            }
            Instruction::I64LtU => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as u64) < (rhs.as_int() as u64);
                self.push(Value::bool(r));
            }
            Instruction::I64LeS => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i64) <= (rhs.as_int() as i64);
                self.push(Value::bool(r));
            }
            Instruction::I64LeU => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as u64) <= (rhs.as_int() as u64);
                self.push(Value::bool(r));
            }
            Instruction::I64GtS => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i64) > (rhs.as_int() as i64);
                self.push(Value::bool(r));
            }
            Instruction::I64GtU => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as u64) > (rhs.as_int() as u64);
                self.push(Value::bool(r));
            }
            Instruction::I64GeS => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i64) >= (rhs.as_int() as i64);
                self.push(Value::bool(r));
            }
            Instruction::I64GeU => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as u64) >= (rhs.as_int() as u64);
                self.push(Value::bool(r));
            }
            Instruction::I64ToF32S => {
                let v = self.pop()?;
                let r = (v.as_int() as i64) as f32 as f64;
                self.push(Value::float(r));
            }
            Instruction::I64ToF32U => {
                let v = self.pop()?;
                let r = (v.as_int() as u64) as f32 as f64;
                self.push(Value::float(r));
            }
            Instruction::I64ToF64S => {
                let v = self.pop()?;
                let r = (v.as_int() as i64) as f64;
                self.push(Value::float(r));
            }
            Instruction::I64ToF64U => {
                let v = self.pop()?;
                let r = (v.as_int() as u64) as f64;
                self.push(Value::float(r));
            }
            _ => return Err(VmError::InvalidOpcode),
        }
        Ok(())
    }
}

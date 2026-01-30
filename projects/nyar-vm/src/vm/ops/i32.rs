use crate::bytecode::decoder::Instruction;
use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;

impl NyarVM {
    pub fn execute_i32_op(&mut self, ins: Instruction) -> Result<(), VmError> {
        match ins {
            Instruction::I32Const(v) => {
                self.push(Value::int(v as i64));
            }
            Instruction::I32DivS => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let (r, _) = (lhs.as_int() as i32).overflowing_div(rhs.as_int() as i32);
                self.push(Value::int(r as i64));
            }
            Instruction::I32DivU => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let (r, _) = (lhs.as_int() as u32).overflowing_div(rhs.as_int() as u32);
                self.push(Value::int(r as i64));
            }
            Instruction::I32RemS => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let (r, _) = (lhs.as_int() as i32).overflowing_rem(rhs.as_int() as i32);
                self.push(Value::int(r as i64));
            }
            Instruction::I32RemU => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let (r, _) = (lhs.as_int() as u32).overflowing_rem(rhs.as_int() as u32);
                self.push(Value::int(r as i64));
            }
            Instruction::I32Add => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i32).wrapping_add(rhs.as_int() as i32) as i64;
                self.push(Value::int(r));
            }
            Instruction::I32Sub => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i32).wrapping_sub(rhs.as_int() as i32) as i64;
                self.push(Value::int(r));
            }
            Instruction::I32Mul => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i32).wrapping_mul(rhs.as_int() as i32) as i64;
                self.push(Value::int(r));
            }
            Instruction::I32Neg => {
                let v = self.pop()?;
                let r = -(v.as_int() as i32) as i64;
                self.push(Value::int(r));
            }
            Instruction::I32Eq => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i32) == (rhs.as_int() as i32);
                self.push(Value::bool(r));
            }
            Instruction::I32Ne => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i32) != (rhs.as_int() as i32);
                self.push(Value::bool(r));
            }
            Instruction::I32LtS => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i32) < (rhs.as_int() as i32);
                self.push(Value::bool(r));
            }
            Instruction::I32LtU => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as u32) < (rhs.as_int() as u32);
                self.push(Value::bool(r));
            }
            Instruction::I32LeS => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i32) <= (rhs.as_int() as i32);
                self.push(Value::bool(r));
            }
            Instruction::I32LeU => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as u32) <= (rhs.as_int() as u32);
                self.push(Value::bool(r));
            }
            Instruction::I32GtS => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i32) > (rhs.as_int() as i32);
                self.push(Value::bool(r));
            }
            Instruction::I32GtU => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as u32) > (rhs.as_int() as u32);
                self.push(Value::bool(r));
            }
            Instruction::I32GeS => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as i32) >= (rhs.as_int() as i32);
                self.push(Value::bool(r));
            }
            Instruction::I32GeU => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = (lhs.as_int() as u32) >= (rhs.as_int() as u32);
                self.push(Value::bool(r));
            }
            Instruction::I32ToF32S => {
                let v = self.pop()?;
                let r = (v.as_int() as i32) as f32 as f64;
                self.push(Value::float(r));
            }
            Instruction::I32ToF32U => {
                let v = self.pop()?;
                let r = (v.as_int() as u32) as f32 as f64;
                self.push(Value::float(r));
            }
            Instruction::I32ToF64S => {
                let v = self.pop()?;
                let r = (v.as_int() as i32) as f64;
                self.push(Value::float(r));
            }
            Instruction::I32ToF64U => {
                let v = self.pop()?;
                let r = (v.as_int() as u32) as f64;
                self.push(Value::float(r));
            }
            Instruction::I32Extend64S => {
                let v = self.pop()?;
                let r = (v.as_int() as i32) as i64;
                self.push(Value::int(r));
            }
            Instruction::I32Extend64U => {
                let v = self.pop()?;
                let r = (v.as_int() as u32) as u64 as i64;
                self.push(Value::int(r));
            }
            Instruction::I32Trunc64SLow => {
                let v = self.pop()?;
                let low = (v.as_int() as u64) as u32;
                let r = low as i32;
                self.push(Value::int(r as i64));
            }
            Instruction::I32Trunc64S => {
                let v = self.pop()?;
                let r = (v.as_int() as i64) as i32;
                self.push(Value::int(r as i64));
            }
            Instruction::I32Trunc64U => {
                let v = self.pop()?;
                let r = (v.as_int() as u64) as u32;
                self.push(Value::int(r as i64));
            }
            _ => Err(VmError::InvalidOpcode),
        }
    }
}

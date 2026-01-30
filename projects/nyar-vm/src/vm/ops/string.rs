use crate::bytecode::decoder::Instruction;
use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;

impl NyarVM {
    pub fn execute_string_op(&mut self, ins: Instruction) -> Result<(), VmError> {
        match ins {
            Instruction::StringConst(s) => {
                self.push(Value::string(s, &self.gc));
            }
            Instruction::StringConcat => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let l = lhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
                let r = rhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
                let result = format!("{}{}", l, r);
                self.push(Value::string(result, &self.gc));
            }
            Instruction::StringLenBytes => {
                let v = self.pop()?;
                let n = v.try_as_str().ok_or(VmError::InvalidOpcode)?.len() as i64;
                self.push(Value::int(n));
            }
            Instruction::StringLenChars => {
                let v = self.pop()?;
                let n = v
                    .try_as_str()
                    .ok_or(VmError::InvalidOpcode)?
                    .chars()
                    .count() as i64;
                self.push(Value::int(n));
            }
            Instruction::StringEq => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.try_as_str().ok_or(VmError::InvalidOpcode)?
                    == rhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
                self.push(Value::bool(r));
            }
            Instruction::StringNe => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.try_as_str().ok_or(VmError::InvalidOpcode)?
                    != rhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
                self.push(Value::bool(r));
            }
            Instruction::StringLt => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.try_as_str().ok_or(VmError::InvalidOpcode)?
                    < rhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
                self.push(Value::bool(r));
            }
            Instruction::StringLe => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.try_as_str().ok_or(VmError::InvalidOpcode)?
                    <= rhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
                self.push(Value::bool(r));
            }
            _ => Err(VmError::InvalidOpcode),
        }
    }
}

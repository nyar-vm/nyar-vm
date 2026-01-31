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
            Instruction::StringGt => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.try_as_str().ok_or(VmError::InvalidOpcode)?
                    > rhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
                self.push(Value::bool(r));
            }
            Instruction::StringGe => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let r = lhs.try_as_str().ok_or(VmError::InvalidOpcode)?
                    >= rhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
                self.push(Value::bool(r));
            }
            Instruction::StringSubstr => {
                let len_v = self.pop()?;
                let start_v = self.pop()?;
                let s_v = self.pop()?;
                let s = s_v.try_as_str().ok_or(VmError::InvalidOpcode)?;
                let start = start_v.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
                let len = len_v.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
                let end = start.saturating_add(len);
                let end = end.min(s.len());
                let sub = if start <= end {
                    s[start..end].to_string()
                } else {
                    String::new()
                };
                self.push(Value::string(sub, &self.gc));
            }
            _ => Err(VmError::InvalidOpcode),
        }
    }
}

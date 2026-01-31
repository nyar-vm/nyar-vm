use crate::bytecode::decoder::Instruction;
use crate::vm::core::NyarVM;
use crate::vm::value::{BigInt, Value};
use crate::vm::VmError;
use num_bigint::{BigInt as NativeBigInt, Sign};

impl NyarVM {
    pub fn execute_bigint_op(&mut self, ins: Instruction) -> Result<(), VmError> {
        match ins {
            Instruction::BigIntConst { sign, bytes } => {
                let sign = if sign == 0 { Sign::Plus } else { Sign::Minus };
                let bi = NativeBigInt::from_bytes_le(sign, &bytes);
                self.push(Value::bigint(BigInt(bi), &self.gc));
            }
            Instruction::BigIntAdd => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let res = BigInt(&l.0 + &r.0);
                self.push(Value::bigint(res, &self.gc));
            }
            Instruction::BigIntSub => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let res = BigInt(&l.0 - &r.0);
                self.push(Value::bigint(res, &self.gc));
            }
            Instruction::BigIntMul => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let res = BigInt(&l.0 * &r.0);
                self.push(Value::bigint(res, &self.gc));
            }
            Instruction::BigIntDiv => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                if r.0 == NativeBigInt::from(0) {
                    return Err(VmError::DivisionByZero);
                }
                let res = BigInt(&l.0 / &r.0);
                self.push(Value::bigint(res, &self.gc));
            }
            Instruction::BigIntMod => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                if r.0 == NativeBigInt::from(0) {
                    return Err(VmError::DivisionByZero);
                }
                let res = BigInt(&l.0 % &r.0);
                self.push(Value::bigint(res, &self.gc));
            }
            Instruction::BigIntNeg => {
                let v = self.pop()?;
                let bi = v.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let res = BigInt(-&bi.0);
                self.push(Value::bigint(res, &self.gc));
            }
            Instruction::BigIntEq => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                self.push(Value::bool(l.0 == r.0));
            }
            Instruction::BigIntNe => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                self.push(Value::bool(l.0 != r.0));
            }
            Instruction::BigIntLt => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                self.push(Value::bool(l.0 < r.0));
            }
            Instruction::BigIntLe => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                self.push(Value::bool(l.0 <= r.0));
            }
            Instruction::BigIntGt => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                self.push(Value::bool(l.0 > r.0));
            }
            Instruction::BigIntGe => {
                let rhs = self.pop()?;
                let lhs = self.pop()?;
                let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                self.push(Value::bool(l.0 >= r.0));
            }
            Instruction::BigIntToI64 => {
                let v = self.pop()?;
                let b = v.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let i = b.to_i64();
                self.push(Value::int(i));
            }
            Instruction::BigIntFromI64 => {
                let v = self.pop()?;
                let i = v.try_as_int().ok_or(VmError::InvalidOpcode)?;
                self.push(Value::bigint_from_i64(i, &self.gc));
            }
            Instruction::BigIntToString => {
                let v = self.pop()?;
                let b = v.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
                let s = b.0.to_string();
                self.push(Value::string(s, &self.gc));
            }
            _ => return Err(VmError::InvalidOpcode),
        }
        Ok(())
    }
}

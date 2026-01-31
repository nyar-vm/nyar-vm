use crate::bytecode::instruction::Instruction;
use crate::vm::core::NyarVM;
use crate::vm::value::{BigInt, Value};
use crate::vm::VmError;
use num_bigint::{BigInt as NativeBigInt, Sign};

impl NyarVM {
    #[inline(always)]
    pub fn execute_bigint_const(&mut self, sign: u8, bytes: Vec<u8>) {
        let sign = if sign == 0 { Sign::Plus } else { Sign::Minus };
        let bi = NativeBigInt::from_bytes_le(sign, &bytes);
        self.push(Value::bigint(BigInt(bi), &self.gc));
    }

    #[inline(always)]
    pub fn execute_bigint_add(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let res = BigInt(&l.0 + &r.0);
        self.push(Value::bigint(res, &self.gc));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_bigint_sub(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let res = BigInt(&l.0 - &r.0);
        self.push(Value::bigint(res, &self.gc));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_bigint_mul(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let res = BigInt(&l.0 * &r.0);
        self.push(Value::bigint(res, &self.gc));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_bigint_div(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        if r.0 == NativeBigInt::from(0) {
            return Err(VmError::DivisionByZero);
        }
        let res = BigInt(&l.0 / &r.0);
        self.push(Value::bigint(res, &self.gc));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_bigint_mod(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        if r.0 == NativeBigInt::from(0) {
            return Err(VmError::DivisionByZero);
        }
        let res = BigInt(&l.0 % &r.0);
        self.push(Value::bigint(res, &self.gc));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_bigint_neg(&mut self) -> Result<(), VmError> {
        let v = self.pop()?;
        let bi = v.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let res = BigInt(-&bi.0);
        self.push(Value::bigint(res, &self.gc));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_bigint_eq(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        self.push(Value::bool(l.0 == r.0));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_bigint_ne(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        self.push(Value::bool(l.0 != r.0));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_bigint_lt(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        self.push(Value::bool(l.0 < r.0));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_bigint_le(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        self.push(Value::bool(l.0 <= r.0));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_bigint_gt(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        self.push(Value::bool(l.0 > r.0));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_bigint_ge(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let r = rhs.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        self.push(Value::bool(l.0 >= r.0));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_bigint_to_i64(&mut self) -> Result<(), VmError> {
        let v = self.pop()?;
        let b = v.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let i = b.to_i64();
        self.push(Value::int(i));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_bigint_from_i64(&mut self) -> Result<(), VmError> {
        let v = self.pop()?;
        let i = v.try_as_int().ok_or(VmError::InvalidOpcode)?;
        self.push(Value::bigint_from_i64(i, &self.gc));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_bigint_to_string(&mut self) -> Result<(), VmError> {
        let v = self.pop()?;
        let b = v.try_as_bigint().ok_or(VmError::InvalidOpcode)?;
        let s = b.0.to_string();
        self.push(Value::string(s, &self.gc));
        Ok(())
    }
}

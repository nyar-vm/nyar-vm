use crate::vm::core::NyarVM;
use crate::vm::value::{BigInt, Value};
use num_bigint::{BigInt as NativeBigInt, Sign};
use nyar_types::NyarError;

impl NyarVM {
    #[inline(always)]
    pub fn execute_bigint_const(&mut self, sign: u8, bytes: Vec<u8>) -> Result<Option<usize>, NyarError> {
        let sign = if sign == 0 { Sign::Plus } else { Sign::Minus };
        let bi = NativeBigInt::from_bytes_le(sign, &bytes);
        self.push(Value::bigint(BigInt(bi), &self.gc))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_add(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x01)))?;
        let r = rhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x01)))?;
        let res = BigInt(&l.0 + &r.0);
        self.push(Value::bigint(res, &self.gc))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_sub(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x02)))?;
        let r = rhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x02)))?;
        let res = BigInt(&l.0 - &r.0);
        self.push(Value::bigint(res, &self.gc))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_mul(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x03)))?;
        let r = rhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x03)))?;
        let res = BigInt(&l.0 * &r.0);
        self.push(Value::bigint(res, &self.gc))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_div(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x04)))?;
        let r = rhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x04)))?;
        if r.0 == NativeBigInt::from(0) {
            return Err(self.error(nyar_types::VmErrorKind::DivisionByZero));
        }
        let res = BigInt(&l.0 / &r.0);
        self.push(Value::bigint(res, &self.gc))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_mod(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x05)))?;
        let r = rhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x05)))?;
        if r.0 == NativeBigInt::from(0) {
            return Err(self.error(nyar_types::VmErrorKind::DivisionByZero));
        }
        let res = BigInt(&l.0 % &r.0);
        self.push(Value::bigint(res, &self.gc))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_neg(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let bi = v.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x08)))?;
        let res = BigInt(-&bi.0);
        self.push(Value::bigint(res, &self.gc))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_eq(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x10)))?;
        let r = rhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x10)))?;
        self.push(Value::bool(l.0 == r.0))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_ne(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x11)))?;
        let r = rhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x11)))?;
        self.push(Value::bool(l.0 != r.0))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_lt(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x12)))?;
        let r = rhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x12)))?;
        self.push(Value::bool(l.0 < r.0))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_le(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x13)))?;
        let r = rhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x13)))?;
        self.push(Value::bool(l.0 <= r.0))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_gt(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x14)))?;
        let r = rhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x14)))?;
        self.push(Value::bool(l.0 > r.0))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_ge(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x15)))?;
        let r = rhs.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x15)))?;
        self.push(Value::bool(l.0 >= r.0))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_to_i64(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let b = v.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x20)))?;
        let i = b.to_i64();
        self.push(Value::int(i))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_from_i64(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let i = v.try_as_int().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x21)))?;
        self.push(Value::bigint_from_i64(i, &self.gc))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_bigint_to_string(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let b = v.try_as_bigint().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x30)))?;
        let s = b.0.to_string();
        self.push(Value::string(s, &self.gc))?;
        Ok(None)
    }
}

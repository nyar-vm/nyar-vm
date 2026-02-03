use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use nyar_types::NyarError;

impl NyarVM {
    #[inline(always)]
    pub fn execute_i64_const(&mut self, v: i64) -> Result<Option<usize>, NyarError> {
        self.push(Value::int(v))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_add(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64).wrapping_add(rhs.as_int() as i64);
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_sub(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64).wrapping_sub(rhs.as_int() as i64);
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_mul(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64).wrapping_mul(rhs.as_int() as i64);
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_div_s(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let b = rhs.as_int() as i64;
        if b == 0 {
            return Err(self.error(nyar_types::VmErrorKind::DivisionByZero));
        }
        let (r, _) = (lhs.as_int() as i64).overflowing_div(b);
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_div_u(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let b = rhs.as_int() as u64;
        if b == 0 {
            return Err(self.error(nyar_types::VmErrorKind::DivisionByZero));
        }
        let (r, _) = (lhs.as_int() as u64).overflowing_div(b);
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_rem_s(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let b = rhs.as_int() as i64;
        if b == 0 {
            return Err(self.error(nyar_types::VmErrorKind::DivisionByZero));
        }
        let (r, _) = (lhs.as_int() as i64).overflowing_rem(b);
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_rem_u(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let b = rhs.as_int() as u64;
        if b == 0 {
            return Err(self.error(nyar_types::VmErrorKind::DivisionByZero));
        }
        let (r, _) = (lhs.as_int() as u64).overflowing_rem(b);
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_and(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) & (rhs.as_int() as i64);
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_or(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) | (rhs.as_int() as i64);
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_xor(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) ^ (rhs.as_int() as i64);
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_shl(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64).wrapping_shl(rhs.as_int() as u32);
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_shr_s(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64).wrapping_shr(rhs.as_int() as u32);
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_shr_u(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as u64).wrapping_shr(rhs.as_int() as u32);
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_not(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = !(v.as_int() as i64);
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_neg(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = -(v.as_int() as i64);
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_eq(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) == (rhs.as_int() as i64);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_ne(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) != (rhs.as_int() as i64);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_lt_s(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) < (rhs.as_int() as i64);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_lt_u(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as u64) < (rhs.as_int() as u64);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_le_s(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) <= (rhs.as_int() as i64);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_le_u(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as u64) <= (rhs.as_int() as u64);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_gt_s(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) > (rhs.as_int() as i64);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_gt_u(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as u64) > (rhs.as_int() as u64);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_ge_s(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) >= (rhs.as_int() as i64);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_ge_u(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as u64) >= (rhs.as_int() as u64);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_to_f32_s(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = (v.as_int() as i64) as f32;
        self.push(Value::f32(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_to_f32_u(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = (v.as_int() as u64) as f32;
        self.push(Value::f32(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_to_f64_s(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = (v.as_int() as i64) as f64;
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_to_f64_u(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = (v.as_int() as u64) as f64;
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_add_sat_s(&mut self) -> Result<Option<usize>, NyarError> {
        let b = self.pop()?.as_int() as i64;
        let a = self.pop()?.as_int() as i64;
        self.push(Value::int(a.saturating_add(b)))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i64_add_sat_u(&mut self) -> Result<Option<usize>, NyarError> {
        let b = self.pop()?.as_int() as u64;
        let a = self.pop()?.as_int() as u64;
        self.push(Value::int(a.saturating_add(b) as i64))?;
        Ok(None)
    }
}

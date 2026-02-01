use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use nyar_types::NyarError;

impl NyarVM {
    #[inline(always)]
    pub fn execute_i32_const(&mut self, v: i32) -> Result<Option<usize>, NyarError> {
        self.push(Value::int(v as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_add(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i32).wrapping_add(rhs.as_int() as i32) as i64;
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_sub(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i32).wrapping_sub(rhs.as_int() as i32) as i64;
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_mul(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i32).wrapping_mul(rhs.as_int() as i32) as i64;
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_div_s(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let b = rhs.as_int() as i32;
        if b == 0 {
            return Err(self.error(nyar_types::VmErrorKind::DivisionByZero));
        }
        let (r, _) = (lhs.as_int() as i32).overflowing_div(b);
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_div_u(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let b = rhs.as_int() as u32;
        if b == 0 {
            return Err(self.error(nyar_types::VmErrorKind::DivisionByZero));
        }
        let (r, _) = (lhs.as_int() as u32).overflowing_div(b);
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_rem_s(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let b = rhs.as_int() as i32;
        if b == 0 {
            return Err(self.error(nyar_types::VmErrorKind::DivisionByZero));
        }
        let (r, _) = (lhs.as_int() as i32).overflowing_rem(b);
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_rem_u(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let b = rhs.as_int() as u32;
        if b == 0 {
            return Err(self.error(nyar_types::VmErrorKind::DivisionByZero));
        }
        let (r, _) = (lhs.as_int() as u32).overflowing_rem(b);
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_neg(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = -(v.as_int() as i32) as i64;
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_eq(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i32) == (rhs.as_int() as i32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_ne(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i32) != (rhs.as_int() as i32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_lt_s(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i32) < (rhs.as_int() as i32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_lt_u(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as u32) < (rhs.as_int() as u32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_le_s(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i32) <= (rhs.as_int() as i32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_le_u(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as u32) <= (rhs.as_int() as u32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_gt_s(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i32) > (rhs.as_int() as i32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_gt_u(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as u32) > (rhs.as_int() as u32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_ge_s(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i32) >= (rhs.as_int() as i32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_ge_u(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as u32) >= (rhs.as_int() as u32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_to_f32_s(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = (v.as_int() as i32) as f32 as f64;
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_to_f32_u(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = (v.as_int() as u32) as f32 as f64;
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_to_f64_s(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = (v.as_int() as i32) as f64;
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_to_f64_u(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = (v.as_int() as u32) as f64;
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_extend64_s(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = (v.as_int() as i32) as i64;
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_extend64_u(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = (v.as_int() as u32) as u64 as i64;
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_trunc64_s_low(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let low = (v.as_int() as u64) as u32;
        let r = low as i32;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_trunc64_s(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = (v.as_int() as i64) as i32;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_trunc64_u(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let r = (v.as_int() as u64) as u32;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_add_sat_s(&mut self) -> Result<Option<usize>, NyarError> {
        let b = self.pop()?.as_int() as i32;
        let a = self.pop()?.as_int() as i32;
        self.push(Value::int(a.saturating_add(b) as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_add_sat_u(&mut self) -> Result<Option<usize>, NyarError> {
        let b = self.pop()?.as_int() as u32;
        let a = self.pop()?.as_int() as u32;
        self.push(Value::int(a.saturating_add(b) as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_sub_sat_s(&mut self) -> Result<Option<usize>, NyarError> {
        let b = self.pop()?.as_int() as i32;
        let a = self.pop()?.as_int() as i32;
        self.push(Value::int(a.saturating_sub(b) as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_i32_sub_sat_u(&mut self) -> Result<Option<usize>, NyarError> {
        let b = self.pop()?.as_int() as u32;
        let a = self.pop()?.as_int() as u32;
        self.push(Value::int(a.saturating_sub(b) as i64))?;
        Ok(None)
    }
}

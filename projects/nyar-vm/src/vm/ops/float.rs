use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;

impl NyarVM {
    #[inline(always)]
    pub fn execute_f32_const(&mut self, v: f32) -> Result<Option<usize>, VmError> {
        self.push(Value::float(v as f64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_add(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = ((lhs.as_float() as f32) + (rhs.as_float() as f32)) as f64;
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_sub(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = ((lhs.as_float() as f32) - (rhs.as_float() as f32)) as f64;
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_mul(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = ((lhs.as_float() as f32) * (rhs.as_float() as f32)) as f64;
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_div(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = ((lhs.as_float() as f32) / (rhs.as_float() as f32)) as f64;
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_neg(&mut self) -> Result<Option<usize>, VmError> {
        let v = self.pop()?;
        let r = (-(v.as_float() as f32)) as f64;
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_eq(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_float() as f32) == (rhs.as_float() as f32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_ne(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_float() as f32) != (rhs.as_float() as f32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_lt(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_float() as f32) < (rhs.as_float() as f32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_le(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_float() as f32) <= (rhs.as_float() as f32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_gt(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_float() as f32) > (rhs.as_float() as f32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_ge(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_float() as f32) >= (rhs.as_float() as f32);
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_to_i32_s(&mut self) -> Result<Option<usize>, VmError> {
        let v = self.pop()?;
        let r = v.as_float() as i32;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_to_i32_u(&mut self) -> Result<Option<usize>, VmError> {
        let v = self.pop()?;
        let r = v.as_float() as u32;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_to_i64_s(&mut self) -> Result<Option<usize>, VmError> {
        let v = self.pop()?;
        let r = v.as_float() as i64;
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_to_i64_u(&mut self) -> Result<Option<usize>, VmError> {
        let v = self.pop()?;
        let r = v.as_float() as u64;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_to_f64(&mut self) -> Result<Option<usize>, VmError> {
        let v = self.pop()?;
        let r = v.as_float();
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_const(&mut self, v: f64) -> Result<Option<usize>, VmError> {
        self.push(Value::float(v))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_add(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.as_float() + rhs.as_float();
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_sub(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.as_float() - rhs.as_float();
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_mul(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.as_float() * rhs.as_float();
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_div(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.as_float() / rhs.as_float();
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_neg(&mut self) -> Result<Option<usize>, VmError> {
        let v = self.pop()?;
        let r = -v.as_float();
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_eq(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.as_float() == rhs.as_float();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_ne(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.as_float() != rhs.as_float();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_lt(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.as_float() < rhs.as_float();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_le(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.as_float() <= rhs.as_float();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_gt(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.as_float() > rhs.as_float();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_ge(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.as_float() >= rhs.as_float();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_to_i32_s(&mut self) -> Result<Option<usize>, VmError> {
        let v = self.pop()?;
        let r = v.as_float() as i32;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_to_i32_u(&mut self) -> Result<Option<usize>, VmError> {
        let v = self.pop()?;
        let r = v.as_float() as u32;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_to_i64_s(&mut self) -> Result<Option<usize>, VmError> {
        let v = self.pop()?;
        let r = v.as_float() as i64;
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_to_i64_u(&mut self) -> Result<Option<usize>, VmError> {
        let v = self.pop()?;
        let r = v.as_float() as u64;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_to_f32(&mut self) -> Result<Option<usize>, VmError> {
        let v = self.pop()?;
        let r = (v.as_float() as f32) as f64;
        self.push(Value::float(r))?;
        Ok(None)
    }
}

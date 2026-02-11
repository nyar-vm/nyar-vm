use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use nyar_types::NyarError;

impl NyarVM {
    #[inline(always)]
    pub fn execute_f32_const(&mut self, v: f32) -> Result<Option<usize>, NyarError> {
        self.push(Value::f32(v))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_add(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r = lhs.as_f32() + rhs.as_f32();
        self.push(Value::f32(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_sub(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r = lhs.as_f32() - rhs.as_f32();
        self.push(Value::f32(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_mul(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r = lhs.as_f32() * rhs.as_f32();
        self.push(Value::f32(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_div(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r_val = rhs.as_f32();
        if r_val == 0.0 {
            return Err(self.error(nyar_types::VmErrorKind::DivisionByZero));
        }
        let r = lhs.as_f32() / r_val;
        self.push(Value::f32(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_neg(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if !v.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", v.tag()) }));
        }
        let r = -v.as_f32();
        self.push(Value::f32(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_eq(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f32() || !rhs.is_f32() {
            self.push(Value::bool(false))?;
            return Ok(None);
        }
        let r = lhs.as_f32() == rhs.as_f32();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_ne(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f32() || !rhs.is_f32() {
            self.push(Value::bool(true))?;
            return Ok(None);
        }
        let r = lhs.as_f32() != rhs.as_f32();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_lt(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r = lhs.as_f32() < rhs.as_f32();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_le(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r = lhs.as_f32() <= rhs.as_f32();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_gt(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r = lhs.as_f32() > rhs.as_f32();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_ge(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r = lhs.as_f32() >= rhs.as_f32();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_to_i32_s(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if !v.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", v.tag()) }));
        }
        let r = v.as_f32() as i32;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_to_i32_u(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if !v.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", v.tag()) }));
        }
        let r = v.as_f32() as u32;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_to_i64_s(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if !v.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", v.tag()) }));
        }
        let r = v.as_f32() as i64;
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_to_i64_u(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if !v.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", v.tag()) }));
        }
        let r = v.as_f32() as u64;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f32_to_f64(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if !v.is_f32() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f32".to_string(), found: format!("{:?}", v.tag()) }));
        }
        let r = v.as_f32() as f64;
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_const(&mut self, v: f64) -> Result<Option<usize>, NyarError> {
        self.push(Value::float(v))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_add(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r = lhs.as_f64() + rhs.as_f64();
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_sub(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r = lhs.as_f64() - rhs.as_f64();
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_mul(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r = lhs.as_f64() * rhs.as_f64();
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_div(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r_val = rhs.as_f64();
        if r_val == 0.0 {
            return Err(self.error(nyar_types::VmErrorKind::DivisionByZero));
        }
        let r = lhs.as_f64() / r_val;
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_neg(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if !v.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", v.tag()) }));
        }
        let r = -v.as_f64();
        self.push(Value::float(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_eq(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f64() || !rhs.is_f64() {
            self.push(Value::bool(false))?;
            return Ok(None);
        }
        let r = lhs.as_f64() == rhs.as_f64();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_ne(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f64() || !rhs.is_f64() {
            self.push(Value::bool(true))?;
            return Ok(None);
        }
        let r = lhs.as_f64() != rhs.as_f64();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_lt(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r = lhs.as_f64() < rhs.as_f64();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_le(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r = lhs.as_f64() <= rhs.as_f64();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_gt(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r = lhs.as_f64() > rhs.as_f64();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_ge(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        if !lhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", lhs.tag()) }));
        }
        if !rhs.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", rhs.tag()) }));
        }
        let r = lhs.as_f64() >= rhs.as_f64();
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_to_i32_s(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if !v.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", v.tag()) }));
        }
        let r = v.as_f64() as i32;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_to_i32_u(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if !v.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", v.tag()) }));
        }
        let r = v.as_f64() as u32;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_to_i64_s(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if !v.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", v.tag()) }));
        }
        let r = v.as_f64() as i64;
        self.push(Value::int(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_to_i64_u(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if !v.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", v.tag()) }));
        }
        let r = v.as_f64() as u64;
        self.push(Value::int(r as i64))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_f64_to_f32(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        if !v.is_f64() {
            return Err(self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "f64".to_string(), found: format!("{:?}", v.tag()) }));
        }
        let r = v.as_f64() as f32;
        self.push(Value::f32(r))?;
        Ok(None)
    }
}

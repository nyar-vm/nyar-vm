use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use nyar_types::NyarError;

impl NyarVM {
    #[inline(always)]
    pub fn execute_string_const(&mut self, s: String) -> Result<Option<usize>, NyarError> {
        self.push(Value::string(s, &self.gc))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_concat(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", lhs.tag()) }))?;
        let r = rhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", rhs.tag()) }))?;
        let result = format!("{}{}", l, r);
        self.push(Value::string(result, &self.gc))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_len_bytes(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let n = v.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", v.tag()) }))?.len() as i64;
        self.push(Value::int(n))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_len_chars(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let n = v.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", v.tag()) }))?.chars().count() as i64;
        self.push(Value::int(n))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_eq(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", lhs.tag()) }))?;
        let r = rhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", rhs.tag()) }))?;
        self.push(Value::bool(l == r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_ne(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", lhs.tag()) }))?;
        let r = rhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", rhs.tag()) }))?;
        self.push(Value::bool(l != r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_lt(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", lhs.tag()) }))?;
        let r = rhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", rhs.tag()) }))?;
        self.push(Value::bool(l < r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_le(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", lhs.tag()) }))?;
        let r = rhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", rhs.tag()) }))?;
        self.push(Value::bool(l <= r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_gt(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", lhs.tag()) }))?;
        let r = rhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", rhs.tag()) }))?;
        self.push(Value::bool(l > r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_ge(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", lhs.tag()) }))?;
        let r = rhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", rhs.tag()) }))?;
        self.push(Value::bool(l >= r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_substr(&mut self) -> Result<Option<usize>, NyarError> {
        let len_v = self.pop()?;
        let start_v = self.pop()?;
        let s_v = self.pop()?;
        let s = s_v.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "String".to_string(), found: format!("{:?}", s_v.tag()) }))?;
        let start_i = start_v.try_as_int().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "Int".to_string(), found: format!("{:?}", start_v.tag()) }))?;
        let len_i = len_v.try_as_int().ok_or_else(|| self.error(nyar_types::VmErrorKind::TypeMismatch { expected: "Int".to_string(), found: format!("{:?}", len_v.tag()) }))?;
        if start_i < 0 || len_i < 0 {
            return Err(self.error(nyar_types::VmErrorKind::RuntimeError(
                "string_substr expects non-negative start and len".to_string(),
            )));
        }
        let start = start_i as usize;
        let len = len_i as usize;
        let mut end = start.saturating_add(len);
        end = end.min(s.len());
        if !s.is_char_boundary(start) || !s.is_char_boundary(end) {
            return Err(self.error(nyar_types::VmErrorKind::RuntimeError(
                "string_substr expects UTF-8 boundary indices".to_string(),
            )));
        }
        let sub = s.get(start..end).unwrap_or("").to_string();
        self.push(Value::string(sub, &self.gc))?;
        Ok(None)
    }
}

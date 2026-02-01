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
        let l = lhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x01)))?;
        let r = rhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x01)))?;
        let result = format!("{}{}", l, r);
        self.push(Value::string(result, &self.gc))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_len_bytes(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let n = v.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x02)))?.len() as i64;
        self.push(Value::int(n))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_len_chars(&mut self) -> Result<Option<usize>, NyarError> {
        let v = self.pop()?;
        let n = v.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x20)))?.chars().count() as i64;
        self.push(Value::int(n))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_eq(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x10)))?;
        let r = rhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x10)))?;
        self.push(Value::bool(l == r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_ne(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x11)))?;
        let r = rhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x11)))?;
        self.push(Value::bool(l != r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_lt(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x12)))?;
        let r = rhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x12)))?;
        self.push(Value::bool(l < r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_le(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x13)))?;
        let r = rhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x13)))?;
        self.push(Value::bool(l <= r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_gt(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x14)))?;
        let r = rhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x14)))?;
        self.push(Value::bool(l > r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_ge(&mut self) -> Result<Option<usize>, NyarError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x15)))?;
        let r = rhs.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x15)))?;
        self.push(Value::bool(l >= r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_substr(&mut self) -> Result<Option<usize>, NyarError> {
        let len_v = self.pop()?;
        let start_v = self.pop()?;
        let s_v = self.pop()?;
        let s = s_v.try_as_str().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x03)))?;
        let start = start_v.try_as_int().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x03)))? as usize;
        let len = len_v.try_as_int().ok_or_else(|| self.error(nyar_types::VmErrorKind::InvalidOpcode(0x03)))? as usize;
        let end = start.saturating_add(len);
        let end = end.min(s.len());
        let sub = if start <= end { s[start..end].to_string() } else { String::new() };
        self.push(Value::string(sub, &self.gc))?;
        Ok(None)
    }
}

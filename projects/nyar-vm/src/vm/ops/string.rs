use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;

impl NyarVM {
    #[inline(always)]
    pub fn execute_string_const(&mut self, s: String) -> Result<Option<usize>, VmError> {
        self.push(Value::string(s, &self.gc))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_concat(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let l = lhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
        let r = rhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
        let result = format!("{}{}", l, r);
        self.push(Value::string(result, &self.gc))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_len_bytes(&mut self) -> Result<Option<usize>, VmError> {
        let v = self.pop()?;
        let n = v.try_as_str().ok_or(VmError::InvalidOpcode)?.len() as i64;
        self.push(Value::int(n))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_len_chars(&mut self) -> Result<Option<usize>, VmError> {
        let v = self.pop()?;
        let n = v.try_as_str().ok_or(VmError::InvalidOpcode)?.chars().count() as i64;
        self.push(Value::int(n))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_eq(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.try_as_str().ok_or(VmError::InvalidOpcode)? == rhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_ne(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.try_as_str().ok_or(VmError::InvalidOpcode)? != rhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_lt(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.try_as_str().ok_or(VmError::InvalidOpcode)? < rhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_le(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.try_as_str().ok_or(VmError::InvalidOpcode)? <= rhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_gt(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.try_as_str().ok_or(VmError::InvalidOpcode)? > rhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_ge(&mut self) -> Result<Option<usize>, VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = lhs.try_as_str().ok_or(VmError::InvalidOpcode)? >= rhs.try_as_str().ok_or(VmError::InvalidOpcode)?;
        self.push(Value::bool(r))?;
        Ok(None)
    }

    #[inline(always)]
    pub fn execute_string_substr(&mut self) -> Result<Option<usize>, VmError> {
        let len_v = self.pop()?;
        let start_v = self.pop()?;
        let s_v = self.pop()?;
        let s = s_v.try_as_str().ok_or(VmError::InvalidOpcode)?;
        let start = start_v.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
        let len = len_v.try_as_int().ok_or(VmError::InvalidOpcode)? as usize;
        let end = start.saturating_add(len);
        let end = end.min(s.len());
        let sub = if start <= end { s[start..end].to_string() } else { String::new() };
        self.push(Value::string(sub, &self.gc))?;
        Ok(None)
    }
}

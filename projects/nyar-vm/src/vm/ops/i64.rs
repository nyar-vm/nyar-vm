use crate::bytecode::instruction::Instruction;
use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use crate::vm::VmError;

impl NyarVM {
    #[inline(always)]
    pub fn execute_i64_const(&mut self, v: i64) {
        self.push(Value::int(v));
    }

    #[inline(always)]
    pub fn execute_i64_add(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64).wrapping_add(rhs.as_int() as i64);
        self.push(Value::int(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_sub(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64).wrapping_sub(rhs.as_int() as i64);
        self.push(Value::int(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_mul(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64).wrapping_mul(rhs.as_int() as i64);
        self.push(Value::int(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_div_s(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let (r, _) = (lhs.as_int() as i64).overflowing_div(rhs.as_int() as i64);
        self.push(Value::int(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_div_u(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let (r, _) = (lhs.as_int() as u64).overflowing_div(rhs.as_int() as u64);
        self.push(Value::int(r as i64));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_rem_s(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let (r, _) = (lhs.as_int() as i64).overflowing_rem(rhs.as_int() as i64);
        self.push(Value::int(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_rem_u(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let (r, _) = (lhs.as_int() as u64).overflowing_rem(rhs.as_int() as u64);
        self.push(Value::int(r as i64));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_neg(&mut self) -> Result<(), VmError> {
        let v = self.pop()?;
        let r = -(v.as_int() as i64);
        self.push(Value::int(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_eq(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) == (rhs.as_int() as i64);
        self.push(Value::bool(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_ne(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) != (rhs.as_int() as i64);
        self.push(Value::bool(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_lt_s(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) < (rhs.as_int() as i64);
        self.push(Value::bool(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_lt_u(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as u64) < (rhs.as_int() as u64);
        self.push(Value::bool(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_le_s(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) <= (rhs.as_int() as i64);
        self.push(Value::bool(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_le_u(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as u64) <= (rhs.as_int() as u64);
        self.push(Value::bool(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_gt_s(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) > (rhs.as_int() as i64);
        self.push(Value::bool(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_gt_u(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as u64) > (rhs.as_int() as u64);
        self.push(Value::bool(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_ge_s(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as i64) >= (rhs.as_int() as i64);
        self.push(Value::bool(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_ge_u(&mut self) -> Result<(), VmError> {
        let rhs = self.pop()?;
        let lhs = self.pop()?;
        let r = (lhs.as_int() as u64) >= (rhs.as_int() as u64);
        self.push(Value::bool(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_to_f32_s(&mut self) -> Result<(), VmError> {
        let v = self.pop()?;
        let r = (v.as_int() as i64) as f32 as f64;
        self.push(Value::float(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_to_f32_u(&mut self) -> Result<(), VmError> {
        let v = self.pop()?;
        let r = (v.as_int() as u64) as f32 as f64;
        self.push(Value::float(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_to_f64_s(&mut self) -> Result<(), VmError> {
        let v = self.pop()?;
        let r = (v.as_int() as i64) as f64;
        self.push(Value::float(r));
        Ok(())
    }

    #[inline(always)]
    pub fn execute_i64_to_f64_u(&mut self) -> Result<(), VmError> {
        let v = self.pop()?;
        let r = (v.as_int() as u64) as f64;
        self.push(Value::float(r));
        Ok(())
    }
}

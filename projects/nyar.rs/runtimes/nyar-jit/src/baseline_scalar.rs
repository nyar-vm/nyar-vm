//! 有限标量模式匹配 → NJ1 机器码 blob（无原生可执行页）。

use odin_bytecode::{OdinHeadCode, decode_at};

use crate::{
    JitCompileRequest, JitCompiledArtifact, JitCompiler, JitError, baseline_scalar_assumptions, build_baseline_deopt_map,
    build_conservative_stack_maps,
    machine_code::{
        I32Binop, I32Cmp, SelectArm, encode_ret_const_i32, encode_ret_i32_binop_imm_local, encode_ret_i32_binop_locals,
        encode_ret_i32_cmp_imm_local, encode_ret_i32_cmp_locals, encode_ret_i32_select_cmp_consts, encode_ret_i32_select_cmp_locals,
        encode_ret_i32_select_cmp_mixed, encode_ret_local, encode_ret_void,
    },
};

/// 识别极简外码模式并附带 NJ1 blob 的 JIT 后端。
#[derive(Debug, Default)]
pub struct BaselineScalarJit;

impl JitCompiler for BaselineScalarJit {
    fn enabled(&self) -> bool {
        true
    }

    fn compile_function(&mut self, request: &JitCompileRequest) -> Result<JitCompiledArtifact, JitError> {
        let code = request.function_code()?;
        let machine_code = match_scalar_program(code, &request.constant_i32).ok_or(JitError::Unsupported)?;
        let maps = build_conservative_stack_maps(request.function_index, request.function.local_count, &request.function.safepoint_indices);
        let deopt = build_baseline_deopt_map(request.function_index, request.function.local_count, &request.function.safepoint_indices);
        Ok(JitCompiledArtifact::with_machine_code(maps, deopt, machine_code)
            .with_assumptions(baseline_scalar_assumptions(request.module_version)))
    }
}

/// 尝试从外码切片匹配基线标量程序。
pub fn match_scalar_program(code: &[u8], constant_i32: &[Option<i32>]) -> Option<Vec<u8>> {
    if let Some(blob) = match_select_cmp_return(code, constant_i32) {
        return Some(blob);
    }
    if let Some(blob) = match_const_const_op_return(code, constant_i32) {
        return Some(blob);
    }
    if let Some(blob) = match_const_const_op_pop_void(code, constant_i32) {
        return Some(blob);
    }
    if let Some(blob) = match_load_dup_op_pop_void(code) {
        return Some(blob);
    }
    if let Some(blob) = match_load_dup_op_return(code) {
        return Some(blob);
    }
    if let Some(blob) = match_const_load_op_pop_void(code, constant_i32) {
        return Some(blob);
    }
    if let Some(blob) = match_const_load_op_return(code, constant_i32) {
        return Some(blob);
    }
    if let Some(blob) = match_load_load_binop_return(code) {
        return Some(blob);
    }
    if let Some(blob) = match_const_return(code, constant_i32) {
        return Some(blob);
    }
    if let Some(blob) = match_load_return(code) {
        return Some(blob);
    }
    if let Some(blob) = match_binop_pop_void_return(code) {
        return Some(blob);
    }
    if let Some(blob) = match_discard_pop_void_return(code) {
        return Some(blob);
    }
    if let Some(blob) = match_pop_void_return(code) {
        return Some(blob);
    }
    if let Some(blob) = match_void_return(code) {
        return Some(blob);
    }
    None
}

/// 仅 `Return` → void 叶。
fn match_void_return(code: &[u8]) -> Option<Vec<u8>> {
    let ret = decode_at(code, 0);
    if ret.size == 0 || ret.code != OdinHeadCode::Return || ret.size as usize != code.len() {
        return None;
    }
    Some(encode_ret_void())
}

/// `Pop; Return` → void 叶（丢弃栈顶后无返回值）。
fn match_pop_void_return(code: &[u8]) -> Option<Vec<u8>> {
    let mut pc = 0usize;
    let pop = decode_at(code, pc);
    if pop.size == 0 || pop.code != OdinHeadCode::Pop {
        return None;
    }
    pc += pop.size as usize;
    let ret = decode_at(code, pc);
    if ret.size == 0 || ret.code != OdinHeadCode::Return || pc + ret.size as usize != code.len() {
        return None;
    }
    Some(encode_ret_void())
}

/// `Load*|Const; Pop; Return` → void 叶（算完结果后丢弃）。
fn match_discard_pop_void_return(code: &[u8]) -> Option<Vec<u8>> {
    let mut pc = 0usize;
    let first = decode_at(code, pc);
    if first.size == 0 {
        return None;
    }
    match first.code {
        OdinHeadCode::LoadLocal | OdinHeadCode::LoadArg | OdinHeadCode::Constant => {}
        _ => return None,
    }
    pc += first.size as usize;
    let pop = decode_at(code, pc);
    if pop.size == 0 || pop.code != OdinHeadCode::Pop {
        return None;
    }
    pc += pop.size as usize;
    let ret = decode_at(code, pc);
    if ret.size == 0 || ret.code != OdinHeadCode::Return || pc + ret.size as usize != code.len() {
        return None;
    }
    Some(encode_ret_void())
}

/// `Load; Load; Binop|Cmp; Pop; Return` → void 叶（副作用仅为消费两 local）。
fn match_binop_pop_void_return(code: &[u8]) -> Option<Vec<u8>> {
    let mut pc = 0usize;
    let a_ins = decode_at(code, pc);
    if a_ins.size == 0 || load_slot(&a_ins).is_none() {
        return None;
    }
    pc += a_ins.size as usize;
    let b_ins = decode_at(code, pc);
    if b_ins.size == 0 || load_slot(&b_ins).is_none() {
        return None;
    }
    pc += b_ins.size as usize;
    let op = decode_at(code, pc);
    if op.size == 0 || (head_to_binop(op.code).is_none() && head_to_cmp(op.code).is_none()) {
        return None;
    }
    // 除数运行期才可知：除零 trap 不得被 void 折叠吞掉。
    if discards_trapping_binop(op.code) {
        return None;
    }
    pc += op.size as usize;
    let pop = decode_at(code, pc);
    if pop.size == 0 || pop.code != OdinHeadCode::Pop {
        return None;
    }
    pc += pop.size as usize;
    let ret = decode_at(code, pc);
    if ret.size == 0 || ret.code != OdinHeadCode::Return || pc + ret.size as usize != code.len() {
        return None;
    }
    Some(encode_ret_void())
}

/// `Const; Const; Op; Return` → 编译期折叠为 `RetConstI32`。
fn match_const_const_op_return(code: &[u8], constant_i32: &[Option<i32>]) -> Option<Vec<u8>> {
    let mut pc = 0usize;
    let a_ins = decode_at(code, pc);
    if a_ins.size == 0 || a_ins.code != OdinHeadCode::Constant || a_ins.operand1 < 0 {
        return None;
    }
    pc += a_ins.size as usize;
    let b_ins = decode_at(code, pc);
    if b_ins.size == 0 || b_ins.code != OdinHeadCode::Constant || b_ins.operand1 < 0 {
        return None;
    }
    pc += b_ins.size as usize;
    let op = decode_at(code, pc);
    if op.size == 0 {
        return None;
    }
    pc += op.size as usize;
    let ret = decode_at(code, pc);
    if ret.size == 0 || pc + ret.size as usize != code.len() || ret.code != OdinHeadCode::Return {
        return None;
    }
    let lhs = constant_i32.get(a_ins.operand1 as usize).copied().flatten()?;
    let rhs = constant_i32.get(b_ins.operand1 as usize).copied().flatten()?;
    if let Some(binop) = head_to_binop(op.code) {
        // 除零必须保留运行期 trap，不得折叠为常量。
        let value = eval_i32_binop(binop, lhs, rhs)?;
        return Some(encode_ret_const_i32(value));
    }
    if let Some(cmp) = head_to_cmp(op.code) {
        return Some(encode_ret_const_i32(i32::from(cmp.eval(lhs, rhs))));
    }
    None
}

/// `Const; Const; Op; Pop; Return` → 编译期丢弃结果，折叠为 void 叶。
fn match_const_const_op_pop_void(code: &[u8], constant_i32: &[Option<i32>]) -> Option<Vec<u8>> {
    let mut pc = 0usize;
    let a_ins = decode_at(code, pc);
    if a_ins.size == 0 || a_ins.code != OdinHeadCode::Constant || a_ins.operand1 < 0 {
        return None;
    }
    pc += a_ins.size as usize;
    let b_ins = decode_at(code, pc);
    if b_ins.size == 0 || b_ins.code != OdinHeadCode::Constant || b_ins.operand1 < 0 {
        return None;
    }
    pc += b_ins.size as usize;
    let op = decode_at(code, pc);
    if op.size == 0 || (head_to_binop(op.code).is_none() && head_to_cmp(op.code).is_none()) {
        return None;
    }
    // 确认常量池可解析（与 value-return 折叠同一合同），结果被 Pop 丢弃。
    let _ = constant_i32.get(a_ins.operand1 as usize).copied().flatten()?;
    let rhs = constant_i32.get(b_ins.operand1 as usize).copied().flatten()?;
    // 除零的 Pop 形态仍必须在运行期 trap，不得折叠为 void 叶。
    if let Some(binop) = head_to_binop(op.code) {
        if matches!(binop, I32Binop::DivS | I32Binop::RemS) && rhs == 0 {
            return None;
        }
    }
    pc += op.size as usize;
    let pop = decode_at(code, pc);
    if pop.size == 0 || pop.code != OdinHeadCode::Pop {
        return None;
    }
    pc += pop.size as usize;
    let ret = decode_at(code, pc);
    if ret.size == 0 || ret.code != OdinHeadCode::Return || pc + ret.size as usize != code.len() {
        return None;
    }
    Some(encode_ret_void())
}

fn eval_i32_binop(binop: I32Binop, lhs: i32, rhs: i32) -> Option<i32> {
    match binop {
        I32Binop::Add => Some(lhs.wrapping_add(rhs)),
        I32Binop::Sub => Some(lhs.wrapping_sub(rhs)),
        I32Binop::Mul => Some(lhs.wrapping_mul(rhs)),
        I32Binop::DivS => (rhs != 0).then(|| lhs.wrapping_div(rhs)),
        I32Binop::RemS => (rhs != 0).then(|| lhs.wrapping_rem(rhs)),
    }
}

/// 结果被丢弃的折叠形态中，除/余仍必须执行：除数可能为 0，trap 不得被 void 折叠吞掉。
fn discards_trapping_binop(op_code: OdinHeadCode) -> bool {
    matches!(head_to_binop(op_code), Some(I32Binop::DivS | I32Binop::RemS))
}

/// `Load; Dup; Op; Pop; Return` → 同槽运算结果丢弃为 void 叶。
fn match_load_dup_op_pop_void(code: &[u8]) -> Option<Vec<u8>> {
    let mut pc = 0usize;
    let load = decode_at(code, pc);
    if load.size == 0 || load_slot(&load).is_none() {
        return None;
    }
    pc += load.size as usize;
    let dup = decode_at(code, pc);
    if dup.size == 0 || dup.code != OdinHeadCode::Dup {
        return None;
    }
    pc += dup.size as usize;
    let op = decode_at(code, pc);
    if op.size == 0 || (head_to_binop(op.code).is_none() && head_to_cmp(op.code).is_none()) {
        return None;
    }
    // 同槽 `a/a`：除数运行期才可知，不得折叠吞掉除零 trap。
    if discards_trapping_binop(op.code) {
        return None;
    }
    pc += op.size as usize;
    let pop = decode_at(code, pc);
    if pop.size == 0 || pop.code != OdinHeadCode::Pop {
        return None;
    }
    pc += pop.size as usize;
    let ret = decode_at(code, pc);
    if ret.size == 0 || ret.code != OdinHeadCode::Return || pc + ret.size as usize != code.len() {
        return None;
    }
    Some(encode_ret_void())
}

/// `Load; Dup; Op; Return` → 同槽二元运算（如 `x*x`）。
fn match_load_dup_op_return(code: &[u8]) -> Option<Vec<u8>> {
    let mut pc = 0usize;
    let load = decode_at(code, pc);
    if load.size == 0 {
        return None;
    }
    let slot = load_slot(&load)?;
    pc += load.size as usize;
    let dup = decode_at(code, pc);
    if dup.size == 0 || dup.code != OdinHeadCode::Dup {
        return None;
    }
    pc += dup.size as usize;
    let op = decode_at(code, pc);
    if op.size == 0 {
        return None;
    }
    pc += op.size as usize;
    let ret = decode_at(code, pc);
    if ret.size == 0 || pc + ret.size as usize != code.len() || ret.code != OdinHeadCode::Return {
        return None;
    }
    if let Some(binop) = head_to_binop(op.code) {
        return Some(encode_ret_i32_binop_locals(binop, slot, slot));
    }
    if let Some(cmp) = head_to_cmp(op.code) {
        return Some(encode_ret_i32_cmp_locals(cmp, slot, slot));
    }
    None
}

/// `Const; Load; Op; Pop; Return` 或 `Load; Const; Op; Pop; Return` → void 叶。
fn match_const_load_op_pop_void(code: &[u8], constant_i32: &[Option<i32>]) -> Option<Vec<u8>> {
    let mut pc = 0usize;
    let first = decode_at(code, pc);
    if first.size == 0 {
        return None;
    }
    pc += first.size as usize;
    let second = decode_at(code, pc);
    if second.size == 0 {
        return None;
    }
    pc += second.size as usize;
    let op = decode_at(code, pc);
    if op.size == 0 || (head_to_binop(op.code).is_none() && head_to_cmp(op.code).is_none()) {
        return None;
    }
    match (first.code, second.code) {
        (OdinHeadCode::Constant, _) => {
            if first.operand1 < 0 {
                return None;
            }
            let _ = constant_i32.get(first.operand1 as usize).copied().flatten()?;
            let _ = load_slot(&second)?;
        }
        (_, OdinHeadCode::Constant) => {
            if second.operand1 < 0 {
                return None;
            }
            let _ = constant_i32.get(second.operand1 as usize).copied().flatten()?;
            let _ = load_slot(&first)?;
        }
        _ => return None,
    }
    if discards_trapping_binop(op.code) {
        // 仅当 rhs 是已确认非零的常量池值才可折叠为 void；否则除零 trap 必须保留执行。
        let rhs_safe = second.code == OdinHeadCode::Constant
            && second.operand1 >= 0
            && constant_i32.get(second.operand1 as usize).copied().flatten().is_some_and(|v| v != 0);
        if !rhs_safe {
            return None;
        }
    }
    pc += op.size as usize;
    let pop = decode_at(code, pc);
    if pop.size == 0 || pop.code != OdinHeadCode::Pop {
        return None;
    }
    pc += pop.size as usize;
    let ret = decode_at(code, pc);
    if ret.size == 0 || ret.code != OdinHeadCode::Return || pc + ret.size as usize != code.len() {
        return None;
    }
    Some(encode_ret_void())
}

/// `Const; Load; Op; Return` 或 `Load; Const; Op; Return`。
fn match_const_load_op_return(code: &[u8], constant_i32: &[Option<i32>]) -> Option<Vec<u8>> {
    let mut pc = 0usize;
    let first = decode_at(code, pc);
    if first.size == 0 {
        return None;
    }
    pc += first.size as usize;
    let second = decode_at(code, pc);
    if second.size == 0 {
        return None;
    }
    pc += second.size as usize;
    let op = decode_at(code, pc);
    if op.size == 0 {
        return None;
    }
    pc += op.size as usize;
    let ret = decode_at(code, pc);
    if ret.size == 0 || pc + ret.size as usize != code.len() || ret.code != OdinHeadCode::Return {
        return None;
    }

    let (imm, local, imm_on_left) = match (first.code, second.code) {
        (OdinHeadCode::Constant, _) => {
            if first.operand1 < 0 {
                return None;
            }
            let imm = constant_i32.get(first.operand1 as usize).copied().flatten()?;
            let local = load_slot(&second)?;
            (imm, local, true)
        }
        (_, OdinHeadCode::Constant) => {
            if second.operand1 < 0 {
                return None;
            }
            let imm = constant_i32.get(second.operand1 as usize).copied().flatten()?;
            let local = load_slot(&first)?;
            (imm, local, false)
        }
        _ => return None,
    };

    if let Some(binop) = head_to_binop(op.code) {
        return Some(encode_ret_i32_binop_imm_local(binop, imm, local, imm_on_left));
    }
    if let Some(cmp) = head_to_cmp(op.code) {
        return Some(encode_ret_i32_cmp_imm_local(cmp, imm, local, imm_on_left));
    }
    None
}

fn match_const_return(code: &[u8], constant_i32: &[Option<i32>]) -> Option<Vec<u8>> {
    let first = decode_at(code, 0);
    if first.size == 0 || first.code != OdinHeadCode::Constant {
        return None;
    }
    let second = decode_at(code, first.size as usize);
    if second.size == 0 || second.code != OdinHeadCode::Return {
        return None;
    }
    if first.size as usize + second.size as usize != code.len() {
        return None;
    }
    if first.operand1 < 0 {
        return None;
    }
    let value = constant_i32.get(first.operand1 as usize).copied().flatten()?;
    Some(encode_ret_const_i32(value))
}

fn match_load_return(code: &[u8]) -> Option<Vec<u8>> {
    let first = decode_at(code, 0);
    if first.size == 0 {
        return None;
    }
    let second = decode_at(code, first.size as usize);
    if second.size == 0 {
        return None;
    }
    if first.size as usize + second.size as usize != code.len() {
        return None;
    }
    if second.code != OdinHeadCode::Return {
        return None;
    }
    let slot = match first.code {
        OdinHeadCode::LoadLocal | OdinHeadCode::LoadArg if first.operand1 >= 0 => first.operand1 as u16,
        _ => return None,
    };
    Some(encode_ret_local(slot))
}

fn match_load_load_binop_return(code: &[u8]) -> Option<Vec<u8>> {
    let mut pc = 0usize;
    let a_ins = decode_at(code, pc);
    if a_ins.size == 0 {
        return None;
    }
    pc += a_ins.size as usize;
    let b_ins = decode_at(code, pc);
    if b_ins.size == 0 {
        return None;
    }
    pc += b_ins.size as usize;
    let op = decode_at(code, pc);
    if op.size == 0 {
        return None;
    }
    pc += op.size as usize;
    let ret = decode_at(code, pc);
    if ret.size == 0 || pc + ret.size as usize != code.len() {
        return None;
    }
    if ret.code != OdinHeadCode::Return {
        return None;
    }
    let a = load_slot(&a_ins)?;
    let b = load_slot(&b_ins)?;
    if let Some(binop) = head_to_binop(op.code) {
        return Some(encode_ret_i32_binop_locals(binop, a, b));
    }
    if let Some(cmp) = head_to_cmp(op.code) {
        return Some(encode_ret_i32_cmp_locals(cmp, a, b));
    }
    None
}

/// `Load a; Load b; Cmp; JumpIfFalse|JumpIfTrue else; (Load|Const) then; Return; (Load|Const) else; Return`
fn match_select_cmp_return(code: &[u8], constant_i32: &[Option<i32>]) -> Option<Vec<u8>> {
    let mut pc = 0usize;
    let a_ins = decode_at(code, pc);
    if a_ins.size == 0 {
        return None;
    }
    pc += a_ins.size as usize;
    let b_ins = decode_at(code, pc);
    if b_ins.size == 0 {
        return None;
    }
    pc += b_ins.size as usize;
    let cmp_ins = decode_at(code, pc);
    if cmp_ins.size == 0 {
        return None;
    }
    let cmp = head_to_cmp(cmp_ins.code)?;
    pc += cmp_ins.size as usize;
    let br_pc = pc;
    let br = decode_at(code, pc);
    if br.size == 0 || (br.code != OdinHeadCode::JumpIfFalse && br.code != OdinHeadCode::JumpIfTrue) {
        return None;
    }
    let invert = br.code == OdinHeadCode::JumpIfTrue;
    pc += br.size as usize;
    let then_ins = decode_at(code, pc);
    if then_ins.size == 0 {
        return None;
    }
    pc += then_ins.size as usize;
    let then_ret = decode_at(code, pc);
    if then_ret.size == 0 || then_ret.code != OdinHeadCode::Return {
        return None;
    }
    pc += then_ret.size as usize;
    let else_pc = br_pc.wrapping_add(br.operand1 as usize);
    if else_pc != pc {
        return None;
    }
    let else_ins = decode_at(code, pc);
    if else_ins.size == 0 {
        return None;
    }
    pc += else_ins.size as usize;
    let else_ret = decode_at(code, pc);
    if else_ret.size == 0 || else_ret.code != OdinHeadCode::Return {
        return None;
    }
    pc += else_ret.size as usize;
    if pc != code.len() {
        return None;
    }
    let a = load_slot(&a_ins)?;
    let b = load_slot(&b_ins)?;

    if let (Some(then_slot), Some(else_slot)) = (load_slot(&then_ins), load_slot(&else_ins)) {
        let (then_slot, else_slot) = if invert { (else_slot, then_slot) } else { (then_slot, else_slot) };
        return Some(encode_ret_i32_select_cmp_locals(cmp, a, b, then_slot, else_slot));
    }

    let then_imm = const_i32(&then_ins, constant_i32);
    let else_imm = const_i32(&else_ins, constant_i32);
    let then_slot = load_slot(&then_ins);
    let else_slot = load_slot(&else_ins);

    match (then_imm, else_imm, then_slot, else_slot) {
        (Some(then_imm), Some(else_imm), _, _) => {
            let (then_imm, else_imm) = if invert { (else_imm, then_imm) } else { (then_imm, else_imm) };
            Some(encode_ret_i32_select_cmp_consts(cmp, a, b, then_imm, else_imm))
        }
        (Some(then_imm), None, _, Some(else_slot)) => {
            let (then_arm, else_arm) = if invert {
                (SelectArm::Local(else_slot), SelectArm::Imm(then_imm))
            }
            else {
                (SelectArm::Imm(then_imm), SelectArm::Local(else_slot))
            };
            Some(encode_ret_i32_select_cmp_mixed(cmp, a, b, then_arm, else_arm))
        }
        (None, Some(else_imm), Some(then_slot), _) => {
            let (then_arm, else_arm) = if invert {
                (SelectArm::Imm(else_imm), SelectArm::Local(then_slot))
            }
            else {
                (SelectArm::Local(then_slot), SelectArm::Imm(else_imm))
            };
            Some(encode_ret_i32_select_cmp_mixed(cmp, a, b, then_arm, else_arm))
        }
        _ => None,
    }
}

fn const_i32(ins: &odin_bytecode::OdinInstruction, constant_i32: &[Option<i32>]) -> Option<i32> {
    if ins.code != OdinHeadCode::Constant || ins.operand1 < 0 {
        return None;
    }
    constant_i32.get(ins.operand1 as usize).copied().flatten()
}

fn load_slot(ins: &odin_bytecode::OdinInstruction) -> Option<u16> {
    match ins.code {
        OdinHeadCode::LoadLocal | OdinHeadCode::LoadArg if ins.operand1 >= 0 => Some(ins.operand1 as u16),
        _ => None,
    }
}

fn head_to_binop(code: OdinHeadCode) -> Option<I32Binop> {
    match code {
        OdinHeadCode::I32Add => Some(I32Binop::Add),
        OdinHeadCode::I32Sub => Some(I32Binop::Sub),
        OdinHeadCode::I32Mul => Some(I32Binop::Mul),
        OdinHeadCode::I32DivS => Some(I32Binop::DivS),
        OdinHeadCode::I32RemS => Some(I32Binop::RemS),
        _ => None,
    }
}

fn head_to_cmp(code: OdinHeadCode) -> Option<I32Cmp> {
    match code {
        OdinHeadCode::I32Eq => Some(I32Cmp::Eq),
        OdinHeadCode::I32Ne => Some(I32Cmp::Ne),
        OdinHeadCode::I32LtS => Some(I32Cmp::LtS),
        OdinHeadCode::I32LeS => Some(I32Cmp::LeS),
        OdinHeadCode::I32GtS => Some(I32Cmp::GtS),
        OdinHeadCode::I32GeS => Some(I32Cmp::GeS),
        _ => None,
    }
}

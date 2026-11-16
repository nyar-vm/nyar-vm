//! x86-64 机器码图元权威（机器码 lane 唯一字节表）。
//!
//! 所有权边界（01 §5）：`ExecutableFunction` 是语言语义事实，本模块不读取它。
//! 帧序言/尾声、寄存器前缀表、算子编码、`rel32` 回填与数据/导入寻址图元在这里
//! 唯一实现；[`super::x64_program`] 的程序装配器与 ELF 入口桩共享同一套图元，
//! 任何其他模块不得再手写同一 ISA 的指令字节。
//!
//! 栈槽固定为帧内 8 字节槽（`[rbp - 8 * (slot + 1)]`）；调用与跳转以 `rel32`
//! 占位记录，由程序装配器在全部函数布局完成后回填。

use crate::Result;

use super::{X64CallConv, x86_64::X86_64OperatorCode};

/// x86-64 通用寄存器（本 lane 目前只使用有固定编码职责的子集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum X64Reg {
    /// `rax`：表达式结果 / 返回载体。
    Rax,
    /// `rcx`：Win64 第 1 / SysV 第 4 个整数形参，也是算子右操作数。
    Rcx,
    /// `rdx`：Win64 第 2 / SysV 第 3 个整数形参。
    Rdx,
    /// `r8`：Win64 第 3 / SysV 第 5 个整数形参。
    R8,
    /// `r9`：Win64 第 4 / SysV 第 6 个整数形参。
    R9,
    /// `rdi`：SysV 第 1 个整数形参。
    Rdi,
    /// `rsi`：SysV 第 2 个整数形参。
    Rsi,
}

impl X64Reg {
    /// Win64 整数形参寄存器顺序。
    pub(super) const WIN64_ARGS: [Self; 4] = [Self::Rcx, Self::Rdx, Self::R8, Self::R9];
    /// SysV64（x86-64 ELF）整数形参寄存器顺序。
    pub(super) const SYSTEM_V_ARGS: [Self; 6] = [Self::Rdi, Self::Rsi, Self::Rdx, Self::Rcx, Self::R8, Self::R9];

    /// 按调用约定返回整数形参/实参的寄存器顺序。
    pub(super) fn argument_registers(abi: X64CallConv) -> &'static [Self] {
        match abi {
            X64CallConv::Win64 => &Self::WIN64_ARGS,
            X64CallConv::SystemV64 => &Self::SYSTEM_V_ARGS,
        }
    }

    /// `mov [rbp + disp32], reg` 的编码前缀（含 REX 与 ModRM）。
    fn frame_store_prefix(self) -> [u8; 3] {
        match self {
            Self::Rax => [0x48, 0x89, 0x85],
            Self::Rcx => [0x48, 0x89, 0x8D],
            Self::Rdx => [0x48, 0x89, 0x95],
            Self::R8 => [0x4C, 0x89, 0x85],
            Self::R9 => [0x4C, 0x89, 0x8D],
            Self::Rdi => [0x48, 0x89, 0xBD],
            Self::Rsi => [0x48, 0x89, 0xB5],
        }
    }

    /// `mov reg, [rbp + disp32]` 的编码前缀。
    fn frame_load_prefix(self) -> [u8; 3] {
        match self {
            Self::Rax => [0x48, 0x8B, 0x85],
            Self::Rcx => [0x48, 0x8B, 0x8D],
            Self::Rdx => [0x48, 0x8B, 0x95],
            Self::R8 => [0x4C, 0x8B, 0x85],
            Self::R9 => [0x4C, 0x8B, 0x8D],
            Self::Rdi => [0x48, 0x8B, 0xBD],
            Self::Rsi => [0x48, 0x8B, 0xB5],
        }
    }

    /// `mov reg, imm64` 的编码前缀。
    fn imm64_prefix(self) -> [u8; 2] {
        match self {
            Self::Rax => [0x48, 0xB8],
            Self::Rcx => [0x48, 0xB9],
            Self::Rdx => [0x48, 0xBA],
            Self::R8 => [0x49, 0xB8],
            Self::R9 => [0x49, 0xB9],
            Self::Rdi => [0x48, 0xBF],
            Self::Rsi => [0x48, 0xBE],
        }
    }

}

/// 局部槽到 `rbp` 的负位移：`[rbp - 8 * (slot + 1)]`。
fn frame_disp(slot: u16) -> i32 {
    -8 * (i32::from(slot) + 1)
}

/// 帧序言：`push rbp; mov rbp, rsp; sub rsp, frame_bytes`。
pub(super) fn emit_prologue(text: &mut Vec<u8>, frame_bytes: u32) {
    text.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5, 0x48, 0x81, 0xEC]);
    text.extend_from_slice(&frame_bytes.to_le_bytes());
}

/// 函数返回：`mov rsp, rbp; pop rbp; ret`（返回值已在 `rax`）。
pub(super) fn emit_function_return(text: &mut Vec<u8>) {
    text.extend_from_slice(&[0x48, 0x89, 0xEC, 0x5D, 0xC3]);
}

/// `ud2`：到达即故障。
pub(super) fn emit_trap(text: &mut Vec<u8>) {
    text.extend_from_slice(&[0x0F, 0x0B]);
}

/// `call rel32`：写操作码与占位位移，返回位移字段偏移。
pub(super) fn emit_call_placeholder(text: &mut Vec<u8>) -> usize {
    text.push(0xE8);
    let field = text.len();
    text.extend_from_slice(&[0, 0, 0, 0]);
    field
}

/// `jmp rel32`。
pub(super) fn emit_jump_placeholder(text: &mut Vec<u8>) -> usize {
    text.push(0xE9);
    let field = text.len();
    text.extend_from_slice(&[0, 0, 0, 0]);
    field
}

/// `test eax, eax` + `jz rel32`（条件值已在 `rax`，只按 32 位布尔载体判定）。
pub(super) fn emit_jump_if_zero_placeholder(text: &mut Vec<u8>) -> usize {
    text.extend_from_slice(&[0x85, 0xC0, 0x0F, 0x84]);
    let field = text.len();
    text.extend_from_slice(&[0, 0, 0, 0]);
    field
}

/// `call qword ptr [rip + disp32]`：导入间接调用；修复去向由容器决定。
pub(super) fn emit_call_import_placeholder(text: &mut Vec<u8>) -> usize {
    text.extend_from_slice(&[0xFF, 0x15]);
    let field = text.len();
    text.extend_from_slice(&[0, 0, 0, 0]);
    field
}

/// `lea rax, [rip + disp32]`：rip 相对取址，位移在布局确定后回填。
pub(super) fn emit_lea_rip_placeholder(text: &mut Vec<u8>) -> usize {
    text.extend_from_slice(&[0x48, 0x8D, 0x05]);
    let field = text.len();
    text.extend_from_slice(&[0, 0, 0, 0]);
    field
}

/// `rel32` 回填的唯一实现：位移 = 目标 − (字段 + 4)。
pub(super) fn patch_rel32(text: &mut [u8], field: usize, target: usize, out_of_range: impl FnOnce() -> String) -> Result<()> {
    let displacement = i64::try_from(target).unwrap_or(i64::MAX) - (i64::try_from(field).unwrap_or(i64::MAX) + 4);
    let displacement = i32::try_from(displacement).map_err(|_| out_of_range())?;
    text[field..field + 4].copy_from_slice(&displacement.to_le_bytes());
    Ok(())
}

/// `mov reg, imm64`。
pub(super) fn emit_mov_imm64(text: &mut Vec<u8>, register: X64Reg, value: i64) {
    text.extend_from_slice(&register.imm64_prefix());
    text.extend_from_slice(&(value as u64).to_le_bytes());
}

/// `mov reg, [rbp - 8 * (slot + 1)]`。
pub(super) fn emit_mov_from_frame(text: &mut Vec<u8>, register: X64Reg, slot: u16) {
    text.extend_from_slice(&register.frame_load_prefix());
    text.extend_from_slice(&frame_disp(slot).to_le_bytes());
}

/// `mov [rbp - 8 * (slot + 1)], reg`。
pub(super) fn emit_mov_to_frame(text: &mut Vec<u8>, register: X64Reg, slot: u16) {
    text.extend_from_slice(&register.frame_store_prefix());
    text.extend_from_slice(&frame_disp(slot).to_le_bytes());
}

/// `lea rax, [rbp - 8 * (slot + 1)]`：取帧槽地址。
pub(super) fn emit_lea_rax_from_frame(text: &mut Vec<u8>, slot: u16) {
    text.extend_from_slice(&[0x48, 0x8D, 0x85]);
    text.extend_from_slice(&frame_disp(slot).to_le_bytes());
}

/// 按调用约定取第 `index` 个整数形参寄存器；超出寄存器上限返回 `None`。
pub(super) fn argument_register(abi: X64CallConv, index: u8) -> Option<X64Reg> {
    X64Reg::argument_registers(abi).get(index as usize).copied()
}

/// `add rax, imm32`。
pub(super) fn emit_add_rax_imm32(text: &mut Vec<u8>, value: i32) {
    text.extend_from_slice(&[0x48, 0x81, 0xC0]);
    text.extend_from_slice(&value.to_le_bytes());
}

/// `sub rax, imm32`。
pub(super) fn emit_sub_rax_imm32(text: &mut Vec<u8>, value: i32) {
    text.extend_from_slice(&[0x48, 0x81, 0xE8]);
    text.extend_from_slice(&value.to_le_bytes());
}

/// `add rdx, imm32`。
pub(super) fn emit_add_rdx_imm32(text: &mut Vec<u8>, value: i32) {
    text.extend_from_slice(&[0x48, 0x81, 0xC2]);
    text.extend_from_slice(&value.to_le_bytes());
}

/// `neg rax`。
pub(super) fn emit_neg_rax(text: &mut Vec<u8>) {
    text.extend_from_slice(&[0x48, 0xF7, 0xD8]);
}

/// `rax = rax / 10`，余数进入 `rdx`（无符号除法；调用方保证非负）。
pub(super) fn emit_divide_by_ten(text: &mut Vec<u8>) {
    // `xor rdx, rdx; mov rcx, 10; div rcx`。
    text.extend_from_slice(&[0x48, 0x31, 0xD2, 0x48, 0xB9]);
    text.extend_from_slice(&10u64.to_le_bytes());
    text.extend_from_slice(&[0x48, 0xF7, 0xF1]);
}

/// `mov [rax], dl`：把 `rdx` 的低字节写入 `rax` 指向的缓冲区。
pub(super) fn emit_store_byte_at_rax_from_rdx(text: &mut Vec<u8>) {
    text.extend_from_slice(&[0x88, 0x10]);
}

/// `movzx eax, byte [rax]`：读取单字节并零扩展到 64 位。
pub(super) fn emit_load_byte_from_rax(text: &mut Vec<u8>) {
    text.extend_from_slice(&[0x0F, 0xB6, 0x00]);
}

/// `mov rax, [rax]`：读取机器字。
pub(super) fn emit_load_from_rax(text: &mut Vec<u8>) {
    text.extend_from_slice(&[0x48, 0x8B, 0x00]);
}

/// `mov [rax], rcx`。
pub(super) fn emit_store_rcx_to_rax(text: &mut Vec<u8>) {
    text.extend_from_slice(&[0x48, 0x89, 0x08]);
}

/// `mov [rcx], rax`。
pub(super) fn emit_store_rax_to_rcx(text: &mut Vec<u8>) {
    text.extend_from_slice(&[0x48, 0x89, 0x01]);
}

/// `mov dword ptr [rax], imm32`。
pub(super) fn emit_store_dword_imm_to_rax(text: &mut Vec<u8>, value: u32) {
    text.extend_from_slice(&[0xC7, 0x00]);
    text.extend_from_slice(&value.to_le_bytes());
}

/// `mov [rax], ecx`。
pub(super) fn emit_store_dword_rcx_to_rax(text: &mut Vec<u8>) {
    text.extend_from_slice(&[0x89, 0x08]);
}

/// `mov eax, [rax + 4]`：读字符串描述符里的 `u32` 长度（零扩展）。
pub(super) fn emit_load_descriptor_length(text: &mut Vec<u8>) {
    text.extend_from_slice(&[0x8B, 0x40, 0x04]);
}

/// 复制字节：`rax` = 目标、`rcx` = 源、`rdx` = 长度。
///
/// 保存/恢复 `rdi`/`rsi`（两种 ABI 都是被调用者保存），`rep movsb` 完成复制；
/// 方向标志由 ABI 保证为 0。
pub(super) fn emit_copy_bytes(text: &mut Vec<u8>) {
    text.extend_from_slice(&[0x57, 0x56, 0x48, 0x89, 0xC7, 0x48, 0x89, 0xCE, 0x48, 0x89, 0xD1, 0xF3, 0xA4, 0x5E, 0x5F]);
}

/// Linux `write(2)`：实参按 SysV64 位于 `rdi`/`rsi`/`rdx`。
pub(super) fn emit_syscall_write(text: &mut Vec<u8>) {
    text.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00, 0x0F, 0x05]);
}

/// Linux `exit(2)`：退出码取自 `rax`（前置 `mov rdi, rax` 由调用方发射）。
pub(super) fn emit_syscall_exit(text: &mut Vec<u8>) {
    text.extend_from_slice(&[0xB8, 0x3C, 0x00, 0x00, 0x00, 0x0F, 0x05]);
}

/// `mov rdi, rax`。
pub(super) fn emit_mov_rdi_from_rax(text: &mut Vec<u8>) {
    text.extend_from_slice(&[0x48, 0x89, 0xC7]);
}

/// Win64：`mov qword ptr [rsp + displacement], 0`（第 5+ 个栈实参置零）。
pub(super) fn emit_zero_stack_arg(text: &mut Vec<u8>, displacement: u8) {
    text.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, displacement, 0, 0, 0, 0]);
}

/// Win64：`mov [rsp + offset], rax`（栈实参周转）。
pub(super) fn emit_store_rax_to_stack(text: &mut Vec<u8>, offset: u32) {
    text.extend_from_slice(&[0x48, 0x89, 0x84, 0x24]);
    text.extend_from_slice(&offset.to_le_bytes());
}

/// 按冻结 x86-64 operator 编码（唯一算子字节表；操作数已按约定位于 `rax`/`rcx`）。
pub(super) fn encode_operator(text: &mut Vec<u8>, code: X86_64OperatorCode) {
    match code {
        X86_64OperatorCode::I64Add => text.extend_from_slice(&[0x48, 0x01, 0xC8]),
        X86_64OperatorCode::I64Subtract => text.extend_from_slice(&[0x48, 0x29, 0xC8]),
        X86_64OperatorCode::I64Multiply => text.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC1]),
        X86_64OperatorCode::I64Negate => text.extend_from_slice(&[0x48, 0xF7, 0xD8]),
        X86_64OperatorCode::I64BitAnd => text.extend_from_slice(&[0x48, 0x21, 0xC8]),
        X86_64OperatorCode::I64BitOr => text.extend_from_slice(&[0x48, 0x09, 0xC8]),
        X86_64OperatorCode::I64BitXor => text.extend_from_slice(&[0x48, 0x31, 0xC8]),
        X86_64OperatorCode::I64ShiftLeft => text.extend_from_slice(&[0x48, 0xD3, 0xE0]),
        X86_64OperatorCode::I64ShiftRight => text.extend_from_slice(&[0x48, 0xD3, 0xF8]),
        X86_64OperatorCode::I64CompareEqual => encode_compare_i64(text, 0x94),
        X86_64OperatorCode::I64CompareNotEqual => encode_compare_i64(text, 0x95),
        X86_64OperatorCode::I64CompareLessThan => encode_compare_i64(text, 0x9C),
        X86_64OperatorCode::I64CompareLessOrEqual => encode_compare_i64(text, 0x9E),
        X86_64OperatorCode::I64CompareGreaterThan => encode_compare_i64(text, 0x9F),
        X86_64OperatorCode::I64CompareGreaterOrEqual => encode_compare_i64(text, 0x9D),
        // 32 位编码不写 REX.W；x86 规则下 32 位结果自动零扩展到 64 位槽。
        X86_64OperatorCode::I32Add => text.extend_from_slice(&[0x01, 0xC8]),
        X86_64OperatorCode::I32Subtract => text.extend_from_slice(&[0x29, 0xC8]),
        X86_64OperatorCode::I32Multiply => text.extend_from_slice(&[0x0F, 0xAF, 0xC1]),
        X86_64OperatorCode::I32Negate => text.extend_from_slice(&[0xF7, 0xD8]),
        X86_64OperatorCode::I32BitAnd => text.extend_from_slice(&[0x21, 0xC8]),
        X86_64OperatorCode::I32BitOr => text.extend_from_slice(&[0x09, 0xC8]),
        X86_64OperatorCode::I32BitXor => text.extend_from_slice(&[0x31, 0xC8]),
        X86_64OperatorCode::I32ShiftLeft => text.extend_from_slice(&[0xD3, 0xE0]),
        X86_64OperatorCode::I32ShiftRight => text.extend_from_slice(&[0xD3, 0xF8]),
        X86_64OperatorCode::I32CompareEqual => encode_compare_i32(text, 0x94),
        X86_64OperatorCode::I32CompareNotEqual => encode_compare_i32(text, 0x95),
        X86_64OperatorCode::I32CompareLessThan => encode_compare_i32(text, 0x9C),
        X86_64OperatorCode::I32CompareLessOrEqual => encode_compare_i32(text, 0x9E),
        X86_64OperatorCode::I32CompareGreaterThan => encode_compare_i32(text, 0x9F),
        X86_64OperatorCode::I32CompareGreaterOrEqual => encode_compare_i32(text, 0x9D),
        X86_64OperatorCode::I64ShiftRightUnsigned => text.extend_from_slice(&[0x48, 0xD3, 0xE8]),
        X86_64OperatorCode::I32ShiftRightUnsigned => text.extend_from_slice(&[0xD3, 0xE8]),
        // `setb` = 无符号小于（CF=1）。
        X86_64OperatorCode::I64CompareLessThanUnsigned => encode_compare_i64(text, 0x92),
        X86_64OperatorCode::I32CompareLessThanUnsigned => encode_compare_i32(text, 0x92),
    }
}

/// `cmp rax, rcx` + `setcc al` + `movzx eax, al`：结果为 0/1 的 32 位载体。
fn encode_compare_i64(text: &mut Vec<u8>, setcc: u8) {
    text.extend_from_slice(&[0x48, 0x39, 0xC8]);
    text.extend_from_slice(&[0x0F, setcc, 0xC0]);
    text.extend_from_slice(&[0x0F, 0xB6, 0xC0]);
}

/// `cmp eax, ecx` + `setcc al` + `movzx eax, al`：32 位比较，结果 0/1。
fn encode_compare_i32(text: &mut Vec<u8>, setcc: u8) {
    text.extend_from_slice(&[0x39, 0xC8]);
    text.extend_from_slice(&[0x0F, setcc, 0xC0]);
    text.extend_from_slice(&[0x0F, 0xB6, 0xC0]);
}

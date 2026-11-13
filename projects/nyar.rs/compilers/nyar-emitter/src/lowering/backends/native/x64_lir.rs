//! x86-64 目标私有低层 IR（LIR）与机器码编码器。
//!
//! 所有权边界（01 §5）：`ExecutableFunction` 是语言语义事实，本模块不读取它；
//! [`super::x86_64`] 的降低器把已冻结的计划事实转成 [`X64Function`]，编码器只把
//! LIR 机械翻译成字节。标签、栈槽、寄存器与直接调用目标都在 LIR 内显式，编码器
//! 不再推导 SSA、布局或调用语义。
//!
//! 栈槽固定为 Win64 帧内 8 字节槽（`[rbp - 8 * (slot + 1)]`），出参周转区为
//! `[rsp + 8 * index]`；调用目标以重定位记录返回，由镜像装配在全部函数布局完成
//! 后回填 `rel32`。

use std::collections::BTreeMap;

use crate::{Result, backend_plan_views::ExecutableBlockRef as MirBlockRef};
use nyar_types::ItemInstanceId;

use super::x86_64::X86_64OperatorCode;

/// x86-64 通用寄存器（本 lane 目前只使用有固定编码职责的子集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum X64Reg {
    /// `rax`：表达式结果 / 返回载体。
    Rax,
    /// `rcx`：Win64 第 1 个形参与算子右操作数。
    Rcx,
    /// `rdx`：Win64 第 2 个形参。
    Rdx,
    /// `r8`：Win64 第 3 个形参。
    R8,
    /// `r9`：Win64 第 4 个形参。
    R9,
}

impl X64Reg {
    /// Win64 整数形参寄存器顺序。
    pub(super) const WIN64_ARGS: [Self; 4] = [Self::Rcx, Self::Rdx, Self::R8, Self::R9];

    fn index(self) -> u8 {
        match self {
            Self::Rax => 0,
            Self::Rcx => 1,
            Self::Rdx => 2,
            Self::R8 => 8,
            Self::R9 => 9,
        }
    }

    /// 64 位操作数的 REX 前缀（`r8`/`r9` 需要 REX.B）。
    fn rex_w(self) -> u8 {
        if self.index() & 0b1000 != 0 { 0x4C } else { 0x48 }
    }

    /// `mov [rbp + disp32], reg` 的编码前缀（含 REX 与 ModRM）。
    fn frame_store_prefix(self) -> [u8; 3] {
        match self {
            Self::Rax => [0x48, 0x89, 0x85],
            Self::Rcx => [0x48, 0x89, 0x8D],
            Self::Rdx => [0x48, 0x89, 0x95],
            Self::R8 => [0x4C, 0x89, 0x85],
            Self::R9 => [0x4C, 0x89, 0x8D],
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
        }
    }

    /// `mov reg, [rsp + disp8]` 的编码前缀 + SIB（`mod=01, rm=100`）。
    fn scratch_load_prefix(self) -> [u8; 4] {
        [self.rex_w(), 0x8B, 0x40 | ((self.index() & 0b111) << 3) | 0x04, 0x24]
    }

    /// `mov [rsp + disp8], reg` 的编码前缀 + SIB。
    fn scratch_store_prefix(self) -> [u8; 4] {
        [self.rex_w(), 0x89, 0x40 | ((self.index() & 0b111) << 3) | 0x04, 0x24]
    }
}

/// LIR 可寻址位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum X64Location {
    /// 帧内 8 字节局部槽：`[rbp - 8 * (slot + 1)]`。
    Frame(u16),
    /// 出参/块参数周转区：`[rsp + 8 * index]`。
    Scratch(u8),
}

/// LIR 源操作数。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum X64Value {
    /// 从帧槽或周转区读取。
    Location(X64Location),
    /// 已在寄存器中的值。
    Register(X64Reg),
    /// 64 位立即数（布尔按 0/1 位模式）。
    Immediate(i64),
}

/// 单条 x86-64 LIR 指令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum X64Instruction {
    /// 函数内块标签，供跳转回填。
    Label(MirBlockRef),
    /// `mov <帧槽>, <寄存器>`。
    StoreFrame { slot: u16, source: X64Reg },
    /// `mov <寄存器>, <值>`。
    Load { register: X64Reg, source: X64Value },
    /// `mov [rsp + 8 * index], <寄存器>`。
    StoreScratch { index: u8, source: X64Reg },
    /// 按冻结 x86-64 operator 编码执行（操作数已按约定位于 `rax`/`rcx`）。
    Operator(X86_64OperatorCode),
    /// 同片段直接调用；`rel32` 由镜像装配回填。
    Call(ItemInstanceId),
    /// 无条件跳转到函数内块。
    Jump(MirBlockRef),
    /// `test eax, eax` 后为零跳转（条件值已在 `rax`）。
    JumpIfZero(MirBlockRef),
    /// 函数返回：`mov rsp, rbp; pop rbp; ret`（返回值已在 `rax`）。
    Return,
    /// `ud2`：到达即故障。
    Trap,
}

/// 一个函数的完整 LIR。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct X64Function {
    /// 对齐后的帧大小（含 32 字节 shadow space）。
    pub(super) frame_bytes: u32,
    pub(super) instructions: Vec<X64Instruction>,
}

/// 直接调用目标在文本节中的回填位置。
pub(super) type X64Relocation = (usize, ItemInstanceId);

/// 把函数的 LIR 编码进文本节，并记录直接调用重定位。
///
/// 编码器只消费 LIR：不读 `ExecutableFunction`、计划、SSA 或类型布局。
pub(super) fn encode_x64_function(function: &X64Function, text: &mut Vec<u8>, relocations: &mut Vec<X64Relocation>) -> Result<()> {
    let start = text.len();
    // 序言：`push rbp; mov rbp, rsp; sub rsp, frame`。
    text.extend_from_slice(&[0x55]);
    text.extend_from_slice(&[0x48, 0x89, 0xE5]);
    text.extend_from_slice(&[0x48, 0x81, 0xEC]);
    text.extend_from_slice(&function.frame_bytes.to_le_bytes());

    let mut labels = BTreeMap::<MirBlockRef, usize>::new();
    let mut jumps = Vec::<(usize, MirBlockRef)>::new();

    for instruction in &function.instructions {
        match instruction {
            X64Instruction::Label(block) => {
                labels.insert(*block, text.len());
            }
            X64Instruction::StoreFrame { slot, source } => {
                text.extend_from_slice(&source.frame_store_prefix());
                text.extend_from_slice(&frame_disp(*slot).to_le_bytes());
            }
            X64Instruction::Load { register, source } => match source {
                X64Value::Register(source) => {
                    if source != register {
                        return Err(format!("x86_64 LIR 编码：寄存器到寄存器的 `Load` 未定义（{:?} → {:?}）", source, register));
                    }
                }
                X64Value::Immediate(value) => {
                    text.extend_from_slice(&register.imm64_prefix());
                    text.extend_from_slice(&(*value as u64).to_le_bytes());
                }
                X64Value::Location(X64Location::Frame(slot)) => {
                    text.extend_from_slice(&register.frame_load_prefix());
                    text.extend_from_slice(&frame_disp(*slot).to_le_bytes());
                }
                X64Value::Location(X64Location::Scratch(index)) => {
                    text.extend_from_slice(&register.scratch_load_prefix());
                    text.push(*index);
                }
            },
            X64Instruction::StoreScratch { index, source } => {
                text.extend_from_slice(&source.scratch_store_prefix());
                text.push(*index);
            }
            X64Instruction::Operator(code) => encode_operator(text, *code),
            X64Instruction::Call(instance) => {
                text.push(0xE8);
                // 记录 `rel32` 字段的绝对位置；相对位移在全部函数布局完成后回填。
                relocations.push((text.len(), *instance));
                text.extend_from_slice(&[0, 0, 0, 0]);
            }
            X64Instruction::Jump(target) => {
                text.push(0xE9);
                jumps.push((text.len(), *target));
                text.extend_from_slice(&[0, 0, 0, 0]);
            }
            X64Instruction::JumpIfZero(target) => {
                // `test eax, eax` 只按 32 位布尔载体判定。
                text.extend_from_slice(&[0x85, 0xC0]);
                text.extend_from_slice(&[0x0F, 0x84]);
                jumps.push((text.len(), *target));
                text.extend_from_slice(&[0, 0, 0, 0]);
            }
            X64Instruction::Return => {
                text.extend_from_slice(&[0x48, 0x89, 0xEC, 0x5D, 0xC3]);
            }
            X64Instruction::Trap => text.extend_from_slice(&[0x0F, 0x0B]),
        }
    }

    for (field, target) in jumps {
        let Some(target_offset) = labels.get(&target).copied()
        else {
            return Err(format!("x86_64 LIR 编码：跳转目标块 {:?} 缺少标签", target));
        };
        let displacement = i64::try_from(target_offset).unwrap_or(i64::MAX) - (i64::try_from(field).unwrap_or(i64::MAX) + 4);
        let displacement = i32::try_from(displacement).map_err(|_| format!("x86_64 LIR 编码：块跳转位移超出 `rel32`（函数起始 {start}）"))?;
        text[field..field + 4].copy_from_slice(&displacement.to_le_bytes());
    }
    Ok(())
}

/// 局部槽到 `rbp` 的负位移：`[rbp - 8 * (slot + 1)]`。
fn frame_disp(slot: u16) -> i32 {
    -8 * (i32::from(slot) + 1)
}

fn encode_operator(text: &mut Vec<u8>, code: X86_64OperatorCode) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::BlockRef;

    fn block(index: u32) -> MirBlockRef {
        BlockRef(index)
    }

    /// 编码器只消费 LIR：序言按 `frame_bytes`、直接调用产出重定位、跳转在函数内回填。
    #[test]
    fn encoder_consumes_only_lir_and_records_call_relocations() {
        let callee = ItemInstanceId::from_index(7).expect("callee instance");
        let function = X64Function {
            frame_bytes: 48,
            instructions: vec![
                X64Instruction::Label(block(0)),
                X64Instruction::Load { register: X64Reg::Rax, source: X64Value::Immediate(42) },
                X64Instruction::StoreFrame { slot: 0, source: X64Reg::Rax },
                X64Instruction::Call(callee),
                X64Instruction::Return,
                X64Instruction::Label(block(1)),
                X64Instruction::Load { register: X64Reg::Rcx, source: X64Value::Location(X64Location::Scratch(0)) },
                X64Instruction::JumpIfZero(block(0)),
            ],
        };

        let mut text = Vec::new();
        let mut relocations = Vec::new();
        encode_x64_function(&function, &mut text, &mut relocations).expect("LIR 必须可编码");

        assert_eq!(relocations.len(), 1, "直接调用必须产生一条重定位");
        assert_eq!(relocations[0].1, callee);
        assert_eq!(text[relocations[0].0 - 1], 0xE8, "重定位必须指向 `call rel32` 的位移字段");
        // `test eax, eax` + `0F 84` 后为目标块 0 的相对位移。
        let conditional = text.windows(2).position(|bytes| bytes == [0x0F, 0x84]).expect("条件跳转必须存在");
        let displacement = i32::from_le_bytes(text[conditional + 2..conditional + 6].try_into().expect("rel32"));
        // 标签紧随 11 字节序言（`push rbp` 1 + `mov rbp, rsp` 3 + `sub rsp, imm32` 7）。
        let label_offset = 11;
        assert_eq!(displacement, label_offset - (i32::try_from(conditional).expect("offset") + 6));
    }

    /// 目标块缺少标签是 LIR 结构违约，必须结构化失败而不是编造位移。
    #[test]
    fn jump_to_unlabeled_block_is_rejected() {
        let function = X64Function { frame_bytes: 32, instructions: vec![X64Instruction::Jump(block(9))] };
        let error = encode_x64_function(&function, &mut Vec::new(), &mut Vec::new()).expect_err("缺少标签必须失败");
        assert!(error.contains("缺少标签"), "{error}");
    }
}

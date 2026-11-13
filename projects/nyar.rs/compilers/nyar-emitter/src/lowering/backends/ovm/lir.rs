//! OVM 目标私有低层 IR（LIR）与 `.odin` 函数体编码器。
//!
//! 所有权边界（01 §5.5）：计划/MIR 的解读只发生在 `ovm/mir.rs` 的降低器；
//! 本模块持有目标私有指令序列与字节编码，不读取 `ExecutableFunction`、计划、
//! 布局或类型事实。块标签与跳转目标在 LIR 内显式，编码器只按标签偏移回填
//! `operand1`，不猜测控制流。

use std::collections::BTreeMap;

use crate::{Result, backend_plan_views::ExecutableBlockRef as MirBlockRef};
use acorn_odin::{OdinHeadCode, emit_imm1, emit_imm2, emit_plain};

/// 单条 OVM LIR 指令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OdinLirInstruction {
    /// 无立即数头码。
    Plain(OdinHeadCode),
    /// 单立即数头码（`Imm1` 形）。
    Imm1(OdinHeadCode, i32),
    /// 双立即数头码（`Imm2` 形）。
    Imm2(OdinHeadCode, i32, i32),
    /// 函数内块标签；不产生字节，仅供跳转回填。
    Label(MirBlockRef),
    /// 跳转到函数内块：编码器写入 `opcode` 与 `target - position`。
    Jump { opcode: OdinHeadCode, target: MirBlockRef },
}

impl OdinLirInstruction {
    /// 该指令在字节流中的固定长度；标签为 0。
    fn encoded_len(&self) -> usize {
        match self {
            Self::Label(_) => 0,
            Self::Plain(_) => 1,
            Self::Imm1(..) | Self::Jump { .. } => 5,
            Self::Imm2(..) => 9,
        }
    }
}

/// 把一个函数的 LIR 编码为函数体字节。
///
/// 跳转位移与既有 `.odin` 合同一致：`operand1 = target_offset - jump_offset`，
/// 两者都相对函数体起点。缺失标签是计划/LIR 不一致，结构化失败。
pub(crate) fn encode_odin_function(instructions: &[OdinLirInstruction]) -> Result<Vec<u8>> {
    let mut label_offsets = BTreeMap::<MirBlockRef, i32>::new();
    let mut offset = 0i32;
    for instruction in instructions {
        if let OdinLirInstruction::Label(block) = instruction {
            label_offsets.insert(*block, offset);
        }
        offset = advance(offset, instruction)?;
    }

    let mut code = Vec::<u8>::with_capacity(usize::try_from(offset).unwrap_or(0));
    let mut position = 0i32;
    for instruction in instructions {
        match instruction {
            OdinLirInstruction::Plain(opcode) => emit_plain(&mut code, *opcode),
            OdinLirInstruction::Imm1(opcode, operand) => emit_imm1(&mut code, *opcode, *operand),
            OdinLirInstruction::Imm2(opcode, first, second) => emit_imm2(&mut code, *opcode, *first, *second),
            OdinLirInstruction::Label(_) => {}
            OdinLirInstruction::Jump { opcode, target } => {
                let target_offset =
                    label_offsets.get(target).copied().ok_or_else(|| format!("OVM LIR 编码：跳转目标块 {target:?} 缺少标签"))?;
                emit_imm1(&mut code, *opcode, target_offset - position);
            }
        }
        position = advance(position, instruction)?;
    }
    Ok(code)
}

fn advance(position: i32, instruction: &OdinLirInstruction) -> Result<i32> {
    let length = i32::try_from(instruction.encoded_len()).map_err(|_| "OVM LIR 编码：函数体长度溢出 `i32`".to_string())?;
    position.checked_add(length).ok_or_else(|| "OVM LIR 编码：函数体长度溢出 `i32`".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::BlockRef;

    fn block(index: u32) -> MirBlockRef {
        BlockRef(index)
    }

    /// 编码器只消费 LIR：标签不产字节，跳转位移相对函数体起点回填。
    #[test]
    fn encodes_labels_and_relative_jumps() {
        let instructions = vec![
            OdinLirInstruction::Label(block(0)),
            OdinLirInstruction::Jump { opcode: OdinHeadCode::Jump, target: block(1) },
            OdinLirInstruction::Plain(OdinHeadCode::Nop),
            OdinLirInstruction::Label(block(1)),
        ];
        let code = encode_odin_function(&instructions).expect("LIR 必须可编码");
        assert_eq!(code.len(), 6, "标签不产生字节");
        assert_eq!(code[0], OdinHeadCode::Jump as u8);
        assert_eq!(i32::from_le_bytes(code[1..5].try_into().expect("operand1")), 6, "跳至 `Nop` 之后的标签");
    }

    /// 缺失标签的跳转是 LIR 结构违约，必须结构化失败。
    #[test]
    fn jump_to_missing_label_is_rejected() {
        let instructions = vec![OdinLirInstruction::Jump { opcode: OdinHeadCode::Jump, target: block(9) }];
        let error = encode_odin_function(&instructions).expect_err("缺少标签必须失败");
        assert!(error.contains("缺少标签"), "{error}");
    }
}

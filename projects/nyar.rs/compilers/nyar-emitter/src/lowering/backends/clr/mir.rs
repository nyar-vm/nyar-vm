//! CLR 数值指令绑定的唯一助记符表。
//!
//! `[clr_operator("i64.div")]` 只声明目标 operator 助记符；助记符到**冻结配方**
//! 的映射归 CLR backend 所有，并在 `TargetExecutionPlan` 构造时一次性写入计划
//! （arity、操作数发射配方、操作数/结果载体与 MSIL opcode）。emit 侧只编码计划
//! 里的配方，不再从助记符字符串推断 arity 或操作数改写。
//!
//! MSIL 算术/比较 opcode 不区分数值载体，助记符因此必须携带载体限定名
//! （`i32.add` / `i64.add` / `f64.add`），冻结表按限定名给出唯一配方。
//! 比较只冻结单 opcode 的 `eq/lt/gt`（`ceq`/`clt`/`cgt`）；`ne/le/ge` 在 MSIL
//! 上需要 `ceq + not` 之类的序列配方，补齐前不在本表，禁止猜默认指令。

use crate::{
    target_operator::{TargetOperandStep, TargetOperandType, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe},
    transitional::msil::MsilOpcode,
};

const fn binary(operand_type: TargetOperandType, result_type: TargetOperandType, opcode: MsilOpcode) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 2,
        operand_type,
        result_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: TargetOperatorEncoding::Clr(opcode),
    }
}

const fn unary(operand_type: TargetOperandType, opcode: MsilOpcode) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 1,
        operand_type,
        result_type: operand_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: TargetOperatorEncoding::Clr(opcode),
    }
}

/// `i64` 移位：MSIL `shl`/`shr` 的位移量操作数必须是 `int32`；声明签名携带 `i64`
/// 位移量，截窄（`conv.i4`）由冻结配方显式给出，emit 只机械执行。
const I64_SHIFT_STEPS: &[TargetOperandStep] = &[TargetOperandStep::Arguments, TargetOperandStep::NarrowTopI64ToI32];

const fn i64_shift(steps: &'static [TargetOperandStep], opcode: MsilOpcode) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 2,
        operand_type: TargetOperandType::I64,
        result_type: TargetOperandType::I64,
        recipe: TargetOperatorRecipe::Steps(steps),
        encoding: TargetOperatorEncoding::Clr(opcode),
    }
}

/// 唯一助记符表。未知助记符返回 `None`。
pub(crate) fn freeze_clr_operator_binding(mnemonic: &str) -> Option<TargetOperatorPlan> {
    Some(match mnemonic {
        // int32 算术：MSIL 单 opcode。
        "i32.add" => binary(TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::Add),
        "i32.sub" => binary(TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::Sub),
        "i32.mul" => binary(TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::Mul),
        "i32.div" => binary(TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::Div),
        "i32.rem" => binary(TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::Rem),
        "i32.neg" => unary(TargetOperandType::I32, MsilOpcode::Neg),
        // int32 位运算：MSIL 单 opcode，按位结果与 `u32` 载体等价。
        "i32.and" => binary(TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::And),
        "i32.or" => binary(TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::Or),
        "i32.xor" => binary(TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::Xor),
        "i32.shl" => binary(TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::Shl),
        // int64 算术：同一批 opcode 对宽载体同形。
        "i64.add" => binary(TargetOperandType::I64, TargetOperandType::I64, MsilOpcode::Add),
        "i64.sub" => binary(TargetOperandType::I64, TargetOperandType::I64, MsilOpcode::Sub),
        "i64.mul" => binary(TargetOperandType::I64, TargetOperandType::I64, MsilOpcode::Mul),
        "i64.div" => binary(TargetOperandType::I64, TargetOperandType::I64, MsilOpcode::Div),
        "i64.rem" => binary(TargetOperandType::I64, TargetOperandType::I64, MsilOpcode::Rem),
        "i64.neg" => unary(TargetOperandType::I64, MsilOpcode::Neg),
        // int64 位运算：MSIL 单 opcode，载体无关。
        "i64.and" => binary(TargetOperandType::I64, TargetOperandType::I64, MsilOpcode::And),
        "i64.or" => binary(TargetOperandType::I64, TargetOperandType::I64, MsilOpcode::Or),
        "i64.xor" => binary(TargetOperandType::I64, TargetOperandType::I64, MsilOpcode::Xor),
        // int64 移位：位移量按冻结配方先 `conv.i4` 截窄，再执行 `shl`/`shr`。
        "i64.shl" => i64_shift(I64_SHIFT_STEPS, MsilOpcode::Shl),
        "i64.shr" => i64_shift(I64_SHIFT_STEPS, MsilOpcode::Shr),
        // float64 算术：MSIL `neg` 对浮点保留 -0.0，与语义一致。
        "f64.add" => binary(TargetOperandType::F64, TargetOperandType::F64, MsilOpcode::Add),
        "f64.sub" => binary(TargetOperandType::F64, TargetOperandType::F64, MsilOpcode::Sub),
        "f64.mul" => binary(TargetOperandType::F64, TargetOperandType::F64, MsilOpcode::Mul),
        "f64.div" => binary(TargetOperandType::F64, TargetOperandType::F64, MsilOpcode::Div),
        "f64.neg" => unary(TargetOperandType::F64, MsilOpcode::Neg),
        // 比较：结果固定为 I32（0/1）；只冻结单 opcode 的 eq/lt/gt。
        "i32.eq" => binary(TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::Ceq),
        "i32.lt" => binary(TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::Clt),
        // 无符号 `int32` 小于：MSIL 原生 `clt.un`。
        "i32.lt.un" => binary(TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::CltUn),
        "i32.gt" => binary(TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::Cgt),
        "i64.eq" => binary(TargetOperandType::I64, TargetOperandType::I32, MsilOpcode::Ceq),
        "i64.lt" => binary(TargetOperandType::I64, TargetOperandType::I32, MsilOpcode::Clt),
        // 无符号 `int64` 小于：MSIL `clt.un` 对宽载体同形（ECMA-335 的 `clt.un` 接受 int64）。
        "i64.lt.un" => binary(TargetOperandType::I64, TargetOperandType::I32, MsilOpcode::CltUn),
        "i64.gt" => binary(TargetOperandType::I64, TargetOperandType::I32, MsilOpcode::Cgt),
        "f64.eq" => binary(TargetOperandType::F64, TargetOperandType::I32, MsilOpcode::Ceq),
        "f64.lt" => binary(TargetOperandType::F64, TargetOperandType::I32, MsilOpcode::Clt),
        "f64.gt" => binary(TargetOperandType::F64, TargetOperandType::I32, MsilOpcode::Cgt),
        _ => return None,
    })
}

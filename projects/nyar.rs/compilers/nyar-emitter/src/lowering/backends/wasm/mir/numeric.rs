//! WASM 数值指令绑定的唯一助记符表。
//!
//! `[wasm_operator("i32.add")]` 只声明目标指令助记符；助记符到**冻结配方**的映射归
//! wasm backend 所有，并在 `TargetExecutionPlan` 构造时一次性写入计划（arity、
//! 操作数发射配方、操作数/结果载体与 opcode）。emit 侧只编码计划里的配方，不再
//! 从助记符字符串推断 arity 或操作数改写。
//!
//! 本模块只接受下表列出的助记符，未知助记符返回 `None`，计划据此冻结为
//! `TargetOperatorBinding::Unfrozen`，由 emit 结构化失败，禁止猜默认指令。
//!
//! f32 合同（冻结，不可协商）：
//!
//! 1. **载体身份**：`f32` 运算的操作数与结果载体是 `TargetOperandType::F32`，
//!    wasm 侧对应独立 `VALTYPE_F32`（`0x7D`）。不得借用 `F64` 或 `I32` 槽。
//! 2. **转换规则**：`f32 → f64` 只能 `f64.promote_f32`（精确）；`f64 → f32` 只能
//!    `f32.demote_f64`（RNE，溢出到 `inf`、下溢到 0/次正规）。本表不冻结任何
//!    隐式拓宽，转换必须由调用方显式声明。
//! 3. **运算与位模式**：`f32.add/sub/mul/div/neg` 在单精度上做，不提升到 f64
//!    再降回；`i32.reinterpret_f32` / `f32.reinterpret_i32` 是 32 位、`i32`
//!    载体、NaN payload 23 位、`-0.0`/次正规保位。
//!
//! pin 的 `acorn_wasm`（e7ed84a）只导出部分 I32 / F64 变体与 `VALTYPE_I32/I64/F64`，
//! 没有 `VALTYPE_F32` 绑定与 f32 opcode 变体。这是**当前 acorn_wasm 库的实现限制**，
//! 不是 Wasm 原生缺少 f32 类型——Wasm 有完整的 f32 参数和指令集。载体常量由
//! `wasm::gc::VALTYPE_F32` 统一提供（`0x7D`）；opcode 按 Wasm 标准字节发射。
//!
//! 有 `WasmOpcode` 变体的字节走变体；其余按标准 WASM opcode 字节发射。
//! acorn 补齐这些变体后删除本地字节发射。

use acorn_wasm::WasmOpcode;

use crate::target_operator::{TargetOperandStep, TargetOperandType, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe};

/// 整数取负没有原生指令：计划显式冻结 `0 - x` 的发射配方。
const I32_NEGATE_STEPS: &[TargetOperandStep] =
    &[TargetOperandStep::IntegerConstant { operand_type: TargetOperandType::I32, value: 0 }, TargetOperandStep::Arguments];
const I64_NEGATE_STEPS: &[TargetOperandStep] =
    &[TargetOperandStep::IntegerConstant { operand_type: TargetOperandType::I64, value: 0 }, TargetOperandStep::Arguments];

const fn binary(operand_type: TargetOperandType, result_type: TargetOperandType, opcode: u8) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 2,
        operand_type,
        result_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: TargetOperatorEncoding::Wasm(opcode),
    }
}

const fn unary(operand_type: TargetOperandType, opcode: u8) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 1,
        operand_type,
        result_type: operand_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: TargetOperatorEncoding::Wasm(opcode),
    }
}

/// 单实参、结果载体与实参载体不同的目标指令（如位模式重解释）。
const fn unary_to(operand_type: TargetOperandType, result_type: TargetOperandType, opcode: u8) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 1,
        operand_type,
        result_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: TargetOperatorEncoding::Wasm(opcode),
    }
}

const fn integer_negate(operand_type: TargetOperandType, steps: &'static [TargetOperandStep], sub_opcode: u8) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 1,
        operand_type,
        result_type: operand_type,
        recipe: TargetOperatorRecipe::Steps(steps),
        encoding: TargetOperatorEncoding::Wasm(sub_opcode),
    }
}

/// 唯一助记符表。未知助记符返回 `None`。
pub(crate) fn freeze_wasm_numeric_binding(mnemonic: &str) -> Option<TargetOperatorPlan> {
    Some(match mnemonic {
        // i32 算术 / 位运算：结果与实参同型。
        "i32.add" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x6A),
        "i32.sub" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x6B),
        "i32.mul" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x6C),
        "i32.div_s" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x6D),
        "i32.div_u" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x6E),
        "i32.rem_s" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x6F),
        "i32.rem_u" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x70),
        "i32.and" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x71),
        "i32.or" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x72),
        "i32.xor" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x73),
        "i32.shl" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x74),
        "i32.shr_s" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x75),
        // `i32.shr_u` 是 0x76；0x77 是 `i32.rotl`，绑定成 0x77 会把逻辑右移发射成左旋。
        "i32.shr_u" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x76),
        // i32 比较：结果固定为 i32（0/1）。
        "i32.eq" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x46),
        "i32.ne" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x47),
        "i32.lt_s" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x48),
        "i32.lt_u" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x49),
        "i32.gt_s" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x4A),
        "i32.gt_u" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x4B),
        "i32.le_s" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x4C),
        "i32.le_u" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x4D),
        "i32.ge_s" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x4E),
        "i32.ge_u" => binary(TargetOperandType::I32, TargetOperandType::I32, 0x4F),
        // 整数 neg 没有原生指令，计划冻结为 `const 0` + `sub` 的显式发射配方。
        "i32.neg" => integer_negate(TargetOperandType::I32, I32_NEGATE_STEPS, 0x6B),

        // i64 算术 / 位运算：acorn 目前只导出 I64ShrU，其余按标准字节发射。
        "i64.add" => binary(TargetOperandType::I64, TargetOperandType::I64, 0x7C),
        "i64.sub" => binary(TargetOperandType::I64, TargetOperandType::I64, 0x7D),
        "i64.mul" => binary(TargetOperandType::I64, TargetOperandType::I64, 0x7E),
        "i64.div_s" => binary(TargetOperandType::I64, TargetOperandType::I64, 0x7F),
        "i64.div_u" => binary(TargetOperandType::I64, TargetOperandType::I64, 0x80),
        "i64.rem_s" => binary(TargetOperandType::I64, TargetOperandType::I64, 0x81),
        "i64.rem_u" => binary(TargetOperandType::I64, TargetOperandType::I64, 0x82),
        "i64.and" => binary(TargetOperandType::I64, TargetOperandType::I64, 0x83),
        "i64.or" => binary(TargetOperandType::I64, TargetOperandType::I64, 0x84),
        "i64.xor" => binary(TargetOperandType::I64, TargetOperandType::I64, 0x85),
        // Wasm 的 `i64.shl`/`i64.shr_s`/`i64.shr_u` 两个操作数都是 `i64`（位移量由指令
        // 自身按 64 取模），不存在 JVM/MSIL 那种「位移量必须窄成 i32」的合同，
        // 因此不得套用 `NarrowTopI64ToI32`，否则生成非法 wasm。
        "i64.shl" => binary(TargetOperandType::I64, TargetOperandType::I64, 0x86),
        "i64.shr_s" => binary(TargetOperandType::I64, TargetOperandType::I64, 0x87),
        "i64.shr_u" => binary(TargetOperandType::I64, TargetOperandType::I64, 0x88),
        // i64 比较：结果固定为 i32（0/1）。
        "i64.eq" => binary(TargetOperandType::I64, TargetOperandType::I32, 0x51),
        "i64.ne" => binary(TargetOperandType::I64, TargetOperandType::I32, 0x52),
        "i64.lt_s" => binary(TargetOperandType::I64, TargetOperandType::I32, 0x53),
        "i64.lt_u" => binary(TargetOperandType::I64, TargetOperandType::I32, 0x54),
        "i64.gt_s" => binary(TargetOperandType::I64, TargetOperandType::I32, 0x55),
        "i64.gt_u" => binary(TargetOperandType::I64, TargetOperandType::I32, 0x56),
        "i64.le_s" => binary(TargetOperandType::I64, TargetOperandType::I32, 0x57),
        "i64.le_u" => binary(TargetOperandType::I64, TargetOperandType::I32, 0x58),
        "i64.ge_s" => binary(TargetOperandType::I64, TargetOperandType::I32, 0x59),
        "i64.ge_u" => binary(TargetOperandType::I64, TargetOperandType::I32, 0x5A),
        "i64.neg" => integer_negate(TargetOperandType::I64, I64_NEGATE_STEPS, 0x7D),

        // f32 算术：在单精度上完成，不允许提升到 f64 再降回。
        "f32.add" => binary(TargetOperandType::F32, TargetOperandType::F32, 0x92),
        "f32.sub" => binary(TargetOperandType::F32, TargetOperandType::F32, 0x93),
        "f32.mul" => binary(TargetOperandType::F32, TargetOperandType::F32, 0x94),
        "f32.div" => binary(TargetOperandType::F32, TargetOperandType::F32, 0x95),
        // f32 比较：结果固定为 i32（0/1），判定本身在单精度上完成。
        "f32.eq" => binary(TargetOperandType::F32, TargetOperandType::I32, 0x5B),
        "f32.ne" => binary(TargetOperandType::F32, TargetOperandType::I32, 0x5C),
        "f32.lt" => binary(TargetOperandType::F32, TargetOperandType::I32, 0x5D),
        "f32.gt" => binary(TargetOperandType::F32, TargetOperandType::I32, 0x5E),
        "f32.le" => binary(TargetOperandType::F32, TargetOperandType::I32, 0x5F),
        "f32.ge" => binary(TargetOperandType::F32, TargetOperandType::I32, 0x60),
        // 浮点 neg 必须保留 -0.0 与 NaN 符号，走原生 f32.neg 而不是 `0.0 - x`。
        "f32.neg" => unary(TargetOperandType::F32, 0x8C),
        // 32 位位重解释：只搬位，不做数值转换。结果载体是 i32（`u32`/`i32` 同宽），
        // NaN payload 23 位与 -0.0 符号位原样保留。
        "i32.reinterpret_f32" => unary_to(TargetOperandType::F32, TargetOperandType::I32, 0xBC),
        "f32.reinterpret_i32" => unary_to(TargetOperandType::I32, TargetOperandType::F32, 0xBE),

        // f64 算术：acorn 有 add/sub/mul/div 变体。
        "f64.add" => binary(TargetOperandType::F64, TargetOperandType::F64, 0xA0),
        "f64.sub" => binary(TargetOperandType::F64, TargetOperandType::F64, 0xA1),
        "f64.mul" => binary(TargetOperandType::F64, TargetOperandType::F64, 0xA2),
        "f64.div" => binary(TargetOperandType::F64, TargetOperandType::F64, 0xA3),
        // f64 比较：结果固定为 i32（0/1）。
        "f64.eq" => binary(TargetOperandType::F64, TargetOperandType::I32, 0x61),
        "f64.ne" => binary(TargetOperandType::F64, TargetOperandType::I32, 0x62),
        "f64.lt" => binary(TargetOperandType::F64, TargetOperandType::I32, 0x63),
        "f64.le" => binary(TargetOperandType::F64, TargetOperandType::I32, 0x65),
        "f64.gt" => binary(TargetOperandType::F64, TargetOperandType::I32, 0x64),
        "f64.ge" => binary(TargetOperandType::F64, TargetOperandType::I32, 0x66),
        // 浮点 neg 必须保留 -0.0，走原生 f64.neg 而不是 `0.0 - x`。
        "f64.neg" => unary(TargetOperandType::F64, 0x9A),
        // 位模式重解释：只搬位，不做数值转换，因此实参/结果载体不同且 arity 为 1。
        // `i64.reinterpret_f64` 是无损 bitcast，NaN payload 与 -0.0 的符号位原样保留。
        "i64.reinterpret_f64" => unary_to(TargetOperandType::F64, TargetOperandType::I64, 0xBD),
        "f64.reinterpret_i64" => unary_to(TargetOperandType::I64, TargetOperandType::F64, 0xBF),

        _ => return None,
    })
}

/// 发射一条目标数值指令。
///
/// 有 `WasmOpcode` 变体时优先走变体；acorn（pin e7ed84a）尚未导出的字节按标准
/// opcode 编码，补齐后删除本地字节发射。
pub(super) fn emit_opcode(byte: u8, code: &mut Vec<u8>) {
    match WasmOpcode::from_u8(byte) {
        Some(opcode) => opcode.encode(code),
        None => code.push(byte),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 位重解释是结构性单指令配方：`F64 -> I64` / `I64 -> F64`，不允许按函数名特判。
    #[test]
    fn reinterpret_mnemonics_freeze_cross_carrier_unary_plans() {
        let to_bits = freeze_wasm_numeric_binding("i64.reinterpret_f64").expect("`i64.reinterpret_f64` 必须在助记符表内");
        assert_eq!(to_bits.arity, 1, "bitcast 只接受一个实参");
        assert_eq!(to_bits.operand_type, TargetOperandType::F64, "`f64.to_bits` 的实参必须是 f64 载体");
        assert_eq!(to_bits.result_type, TargetOperandType::I64, "`f64.to_bits` 必须产生 64 位整数载体");
        assert_eq!(to_bits.recipe, TargetOperatorRecipe::Operands, "bitcast 不得套用数值转换配方");
        assert_eq!(to_bits.encoding, TargetOperatorEncoding::Wasm(0xBD), "`i64.reinterpret_f64` 的标准 opcode 是 0xBD");

        let from_bits = freeze_wasm_numeric_binding("f64.reinterpret_i64").expect("`f64.reinterpret_i64` 必须在助记符表内");
        assert_eq!(from_bits.arity, 1, "bitcast 只接受一个实参");
        assert_eq!(from_bits.operand_type, TargetOperandType::I64, "`f64.from_bits` 的实参必须是 64 位整数载体");
        assert_eq!(from_bits.result_type, TargetOperandType::F64, "`f64.from_bits` 必须产生 f64 载体");
        assert_eq!(from_bits.recipe, TargetOperatorRecipe::Operands, "bitcast 不得套用数值转换配方");
        assert_eq!(from_bits.encoding, TargetOperatorEncoding::Wasm(0xBF), "`f64.reinterpret_i64` 的标准 opcode 是 0xBF");
    }

    /// f32 的算术与位重解释必须冻结在**独立 F32 载体**上，不能复用 F64/I32 槽。
    #[test]
    fn f32_mnemonics_freeze_independent_carrier_plans() {
        for (mnemonic, opcode) in [("f32.add", 0x92), ("f32.sub", 0x93), ("f32.mul", 0x94), ("f32.div", 0x95)] {
            let plan = freeze_wasm_numeric_binding(mnemonic).unwrap_or_else(|| panic!("`{mnemonic}` 必须在助记符表内"));
            assert_eq!(plan.arity, 2);
            assert_eq!(plan.operand_type, TargetOperandType::F32, "`{mnemonic}` 实参必须是 f32 载体");
            assert_eq!(plan.result_type, TargetOperandType::F32, "`{mnemonic}` 结果必须是 f32 载体");
            assert_eq!(plan.encoding, TargetOperatorEncoding::Wasm(opcode));
        }
        let neg = freeze_wasm_numeric_binding("f32.neg").expect("`f32.neg` 必须在助记符表内");
        assert_eq!(neg.arity, 1);
        assert_eq!(neg.operand_type, TargetOperandType::F32);
        assert_eq!(neg.result_type, TargetOperandType::F32);
        assert_eq!(neg.encoding, TargetOperatorEncoding::Wasm(0x8C), "`f32.neg` 必须走原生 neg 以保留 -0.0");

        let eq = freeze_wasm_numeric_binding("f32.eq").expect("`f32.eq` 必须在助记符表内");
        assert_eq!(eq.operand_type, TargetOperandType::F32, "比较操作数仍是 f32 载体");
        assert_eq!(eq.result_type, TargetOperandType::I32, "比较结果固定 i32 布尔");
        assert_eq!(eq.encoding, TargetOperatorEncoding::Wasm(0x5B));

        let to_bits = freeze_wasm_numeric_binding("i32.reinterpret_f32").expect("`i32.reinterpret_f32` 必须在助记符表内");
        assert_eq!(to_bits.arity, 1);
        assert_eq!(to_bits.operand_type, TargetOperandType::F32);
        assert_eq!(to_bits.result_type, TargetOperandType::I32, "f32 位模式是 32 位 i32 载体");
        assert_eq!(to_bits.encoding, TargetOperatorEncoding::Wasm(0xBC));

        let from_bits = freeze_wasm_numeric_binding("f32.reinterpret_i32").expect("`f32.reinterpret_i32` 必须在助记符表内");
        assert_eq!(from_bits.operand_type, TargetOperandType::I32);
        assert_eq!(from_bits.result_type, TargetOperandType::F32);
        assert_eq!(from_bits.encoding, TargetOperatorEncoding::Wasm(0xBE));

        // 反例：f32 不得静默走 64 位位重解释（那是把 32 位类型当 64 位载荷）。
        assert!(freeze_wasm_numeric_binding("i64.reinterpret_f32").is_none());
    }

    /// 未知助记符不得猜默认指令。
    #[test]
    fn unknown_mnemonic_stays_unfrozen() {
        assert!(freeze_wasm_numeric_binding("i64.reinterpret_f32").is_none());
        assert!(freeze_wasm_numeric_binding("f64.to_bits").is_none());
    }
}

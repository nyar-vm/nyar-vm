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
//! 冻结编码使用 `acorn-wasm` 的 [`WasmOpcode`] 变体，不再在 emitter 内保存裸字节
//! 数值表：opcode 身份归格式 crate 所有。
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

use acorn_wasm::WasmOpcode;

use crate::target_operator::{TargetOperandStep, TargetOperandType, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe};

/// 整数取负没有原生指令：计划显式冻结 `0 - x` 的发射配方。
const I32_NEGATE_STEPS: &[TargetOperandStep] =
    &[TargetOperandStep::IntegerConstant { operand_type: TargetOperandType::I32, value: 0 }, TargetOperandStep::Arguments];
const I64_NEGATE_STEPS: &[TargetOperandStep] =
    &[TargetOperandStep::IntegerConstant { operand_type: TargetOperandType::I64, value: 0 }, TargetOperandStep::Arguments];

const fn binary(operand_type: TargetOperandType, result_type: TargetOperandType, opcode: WasmOpcode) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 2,
        operand_type,
        result_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: TargetOperatorEncoding::Wasm(opcode),
    }
}

const fn unary(operand_type: TargetOperandType, opcode: WasmOpcode) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 1,
        operand_type,
        result_type: operand_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: TargetOperatorEncoding::Wasm(opcode),
    }
}

/// 单实参、结果载体与实参载体不同的目标指令（如位模式重解释）。
const fn unary_to(operand_type: TargetOperandType, result_type: TargetOperandType, opcode: WasmOpcode) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 1,
        operand_type,
        result_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: TargetOperatorEncoding::Wasm(opcode),
    }
}

const fn integer_negate(operand_type: TargetOperandType, steps: &'static [TargetOperandStep], sub_opcode: WasmOpcode) -> TargetOperatorPlan {
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
        "i32.add" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32Add),
        "i32.sub" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32Sub),
        "i32.mul" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32Mul),
        "i32.div_s" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32DivS),
        "i32.div_u" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32DivU),
        "i32.rem_s" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32RemS),
        "i32.rem_u" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32RemU),
        "i32.and" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32And),
        "i32.or" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32Or),
        "i32.xor" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32Xor),
        "i32.shl" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32Shl),
        "i32.shr_s" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32ShrS),
        "i32.shr_u" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32ShrU),
        // i32 比较：结果固定为 i32（0/1）。
        "i32.eq" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32Eq),
        "i32.ne" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32Ne),
        "i32.lt_s" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32LtS),
        "i32.lt_u" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32LtU),
        "i32.gt_s" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32GtS),
        "i32.gt_u" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32GtU),
        "i32.le_s" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32LeS),
        "i32.le_u" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32LeU),
        "i32.ge_s" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32GeS),
        "i32.ge_u" => binary(TargetOperandType::I32, TargetOperandType::I32, WasmOpcode::I32GeU),
        // 整数 neg 没有原生指令，计划冻结为 `const 0` + `sub` 的显式发射配方。
        "i32.neg" => integer_negate(TargetOperandType::I32, I32_NEGATE_STEPS, WasmOpcode::I32Sub),

        // i64 算术 / 位运算：结果与实参同型。
        "i64.add" => binary(TargetOperandType::I64, TargetOperandType::I64, WasmOpcode::I64Add),
        "i64.sub" => binary(TargetOperandType::I64, TargetOperandType::I64, WasmOpcode::I64Sub),
        "i64.mul" => binary(TargetOperandType::I64, TargetOperandType::I64, WasmOpcode::I64Mul),
        "i64.div_s" => binary(TargetOperandType::I64, TargetOperandType::I64, WasmOpcode::I64DivS),
        "i64.div_u" => binary(TargetOperandType::I64, TargetOperandType::I64, WasmOpcode::I64DivU),
        "i64.rem_s" => binary(TargetOperandType::I64, TargetOperandType::I64, WasmOpcode::I64RemS),
        "i64.rem_u" => binary(TargetOperandType::I64, TargetOperandType::I64, WasmOpcode::I64RemU),
        "i64.and" => binary(TargetOperandType::I64, TargetOperandType::I64, WasmOpcode::I64And),
        "i64.or" => binary(TargetOperandType::I64, TargetOperandType::I64, WasmOpcode::I64Or),
        "i64.xor" => binary(TargetOperandType::I64, TargetOperandType::I64, WasmOpcode::I64Xor),
        // Wasm 的 `i64.shl`/`i64.shr_s`/`i64.shr_u` 两个操作数都是 `i64`（位移量由指令
        // 自身按 64 取模），不存在 JVM/MSIL 那种「位移量必须窄成 i32」的合同，
        // 因此不得套用 `NarrowTopI64ToI32`，否则生成非法 wasm。
        "i64.shl" => binary(TargetOperandType::I64, TargetOperandType::I64, WasmOpcode::I64Shl),
        "i64.shr_s" => binary(TargetOperandType::I64, TargetOperandType::I64, WasmOpcode::I64ShrS),
        "i64.shr_u" => binary(TargetOperandType::I64, TargetOperandType::I64, WasmOpcode::I64ShrU),
        // i64 比较：结果固定为 i32（0/1）。
        "i64.eq" => binary(TargetOperandType::I64, TargetOperandType::I32, WasmOpcode::I64Eq),
        "i64.ne" => binary(TargetOperandType::I64, TargetOperandType::I32, WasmOpcode::I64Ne),
        "i64.lt_s" => binary(TargetOperandType::I64, TargetOperandType::I32, WasmOpcode::I64LtS),
        "i64.lt_u" => binary(TargetOperandType::I64, TargetOperandType::I32, WasmOpcode::I64LtU),
        "i64.gt_s" => binary(TargetOperandType::I64, TargetOperandType::I32, WasmOpcode::I64GtS),
        "i64.gt_u" => binary(TargetOperandType::I64, TargetOperandType::I32, WasmOpcode::I64GtU),
        "i64.le_s" => binary(TargetOperandType::I64, TargetOperandType::I32, WasmOpcode::I64LeS),
        "i64.le_u" => binary(TargetOperandType::I64, TargetOperandType::I32, WasmOpcode::I64LeU),
        "i64.ge_s" => binary(TargetOperandType::I64, TargetOperandType::I32, WasmOpcode::I64GeS),
        "i64.ge_u" => binary(TargetOperandType::I64, TargetOperandType::I32, WasmOpcode::I64GeU),
        "i64.neg" => integer_negate(TargetOperandType::I64, I64_NEGATE_STEPS, WasmOpcode::I64Sub),

        // f32 算术：在单精度上完成，不允许提升到 f64 再降回。
        "f32.add" => binary(TargetOperandType::F32, TargetOperandType::F32, WasmOpcode::F32Add),
        "f32.sub" => binary(TargetOperandType::F32, TargetOperandType::F32, WasmOpcode::F32Sub),
        "f32.mul" => binary(TargetOperandType::F32, TargetOperandType::F32, WasmOpcode::F32Mul),
        "f32.div" => binary(TargetOperandType::F32, TargetOperandType::F32, WasmOpcode::F32Div),
        // f32 比较：结果固定为 i32（0/1），判定本身在单精度上完成。
        "f32.eq" => binary(TargetOperandType::F32, TargetOperandType::I32, WasmOpcode::F32Eq),
        "f32.ne" => binary(TargetOperandType::F32, TargetOperandType::I32, WasmOpcode::F32Ne),
        "f32.lt" => binary(TargetOperandType::F32, TargetOperandType::I32, WasmOpcode::F32Lt),
        "f32.gt" => binary(TargetOperandType::F32, TargetOperandType::I32, WasmOpcode::F32Gt),
        "f32.le" => binary(TargetOperandType::F32, TargetOperandType::I32, WasmOpcode::F32Le),
        "f32.ge" => binary(TargetOperandType::F32, TargetOperandType::I32, WasmOpcode::F32Ge),
        // 浮点 neg 必须保留 -0.0 与 NaN 符号，走原生 f32.neg 而不是 `0.0 - x`。
        "f32.neg" => unary(TargetOperandType::F32, WasmOpcode::F32Neg),
        // 32 位位重解释：只搬位，不做数值转换。结果载体是 i32（`u32`/`i32` 同宽），
        // NaN payload 23 位与 -0.0 符号位原样保留。
        "i32.reinterpret_f32" => unary_to(TargetOperandType::F32, TargetOperandType::I32, WasmOpcode::I32ReinterpretF32),
        "f32.reinterpret_i32" => unary_to(TargetOperandType::I32, TargetOperandType::F32, WasmOpcode::F32ReinterpretI32),

        // f64 算术。
        "f64.add" => binary(TargetOperandType::F64, TargetOperandType::F64, WasmOpcode::F64Add),
        "f64.sub" => binary(TargetOperandType::F64, TargetOperandType::F64, WasmOpcode::F64Sub),
        "f64.mul" => binary(TargetOperandType::F64, TargetOperandType::F64, WasmOpcode::F64Mul),
        "f64.div" => binary(TargetOperandType::F64, TargetOperandType::F64, WasmOpcode::F64Div),
        // f64 比较：结果固定为 i32（0/1）。
        "f64.eq" => binary(TargetOperandType::F64, TargetOperandType::I32, WasmOpcode::F64Eq),
        "f64.ne" => binary(TargetOperandType::F64, TargetOperandType::I32, WasmOpcode::F64Ne),
        "f64.lt" => binary(TargetOperandType::F64, TargetOperandType::I32, WasmOpcode::F64Lt),
        "f64.le" => binary(TargetOperandType::F64, TargetOperandType::I32, WasmOpcode::F64Le),
        "f64.gt" => binary(TargetOperandType::F64, TargetOperandType::I32, WasmOpcode::F64Gt),
        "f64.ge" => binary(TargetOperandType::F64, TargetOperandType::I32, WasmOpcode::F64Ge),
        // 浮点 neg 必须保留 -0.0，走原生 f64.neg 而不是 `0.0 - x`。
        "f64.neg" => unary(TargetOperandType::F64, WasmOpcode::F64Neg),
        // 位模式重解释：只搬位，不做数值转换，因此实参/结果载体不同且 arity 为 1。
        // `i64.reinterpret_f64` 是无损 bitcast，NaN payload 与 -0.0 的符号位原样保留。
        "i64.reinterpret_f64" => unary_to(TargetOperandType::F64, TargetOperandType::I64, WasmOpcode::I64ReinterpretF64),
        "f64.reinterpret_i64" => unary_to(TargetOperandType::I64, TargetOperandType::F64, WasmOpcode::F64ReinterpretI64),

        _ => return None,
    })
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
        assert_eq!(to_bits.encoding, TargetOperatorEncoding::Wasm(WasmOpcode::I64ReinterpretF64));

        let from_bits = freeze_wasm_numeric_binding("f64.reinterpret_i64").expect("`f64.reinterpret_i64` 必须在助记符表内");
        assert_eq!(from_bits.arity, 1, "bitcast 只接受一个实参");
        assert_eq!(from_bits.operand_type, TargetOperandType::I64, "`f64.from_bits` 的实参必须是 64 位整数载体");
        assert_eq!(from_bits.result_type, TargetOperandType::F64, "`f64.from_bits` 必须产生 f64 载体");
        assert_eq!(from_bits.recipe, TargetOperatorRecipe::Operands, "bitcast 不得套用数值转换配方");
        assert_eq!(from_bits.encoding, TargetOperatorEncoding::Wasm(WasmOpcode::F64ReinterpretI64));
    }

    /// f32 的算术与位重解释必须冻结在**独立 F32 载体**上，不能复用 F64/I32 槽。
    #[test]
    fn f32_mnemonics_freeze_independent_carrier_plans() {
        for (mnemonic, opcode) in
            [("f32.add", WasmOpcode::F32Add), ("f32.sub", WasmOpcode::F32Sub), ("f32.mul", WasmOpcode::F32Mul), ("f32.div", WasmOpcode::F32Div)]
        {
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
        assert_eq!(neg.encoding, TargetOperatorEncoding::Wasm(WasmOpcode::F32Neg), "`f32.neg` 必须走原生 neg 以保留 -0.0");

        let eq = freeze_wasm_numeric_binding("f32.eq").expect("`f32.eq` 必须在助记符表内");
        assert_eq!(eq.operand_type, TargetOperandType::F32, "比较操作数仍是 f32 载体");
        assert_eq!(eq.result_type, TargetOperandType::I32, "比较结果固定 i32 布尔");
        assert_eq!(eq.encoding, TargetOperatorEncoding::Wasm(WasmOpcode::F32Eq));

        let to_bits = freeze_wasm_numeric_binding("i32.reinterpret_f32").expect("`i32.reinterpret_f32` 必须在助记符表内");
        assert_eq!(to_bits.arity, 1);
        assert_eq!(to_bits.operand_type, TargetOperandType::F32);
        assert_eq!(to_bits.result_type, TargetOperandType::I32, "f32 位模式是 32 位 i32 载体");
        assert_eq!(to_bits.encoding, TargetOperatorEncoding::Wasm(WasmOpcode::I32ReinterpretF32));

        let from_bits = freeze_wasm_numeric_binding("f32.reinterpret_i32").expect("`f32.reinterpret_i32` 必须在助记符表内");
        assert_eq!(from_bits.operand_type, TargetOperandType::I32);
        assert_eq!(from_bits.result_type, TargetOperandType::F32);
        assert_eq!(from_bits.encoding, TargetOperatorEncoding::Wasm(WasmOpcode::F32ReinterpretI32));

        // 反例：f32 不得静默走 64 位位重解释（那是把 32 位类型当 64 位载荷）。
        assert!(freeze_wasm_numeric_binding("i64.reinterpret_f32").is_none());
    }

    /// 未知助记符不得猜默认指令。
    #[test]
    fn unknown_mnemonic_stays_unfrozen() {
        assert!(freeze_wasm_numeric_binding("f32.unknown").is_none());
        assert!(freeze_wasm_numeric_binding("i64.rotl").is_none());
    }
}

//! JVM 数值指令绑定的唯一 operator 配方表。
//!
//! `[jvm_operator("long_divide")]` 只声明目标 operator 名；operator 名到**冻结配方**
//! 的映射归 JVM backend 所有，并在 `TargetExecutionPlan` 构造时一次性写入计划（arity、
//! 操作数发射配方、操作数/结果载体与冻结码）。emit 侧只编码计划里的配方，不再
//! 从 operator 名字符串推断 arity 或操作数改写。
//!
//! 本表只接受下列出的 operator 名，未知名字返回 `None`，计划据此冻结为
//! `TargetOperatorBinding::Unfrozen`，由 emit 结构化失败，禁止猜默认指令。
//!
//! JVM 没有 `dneg`（`double` 取负需要 `dconst_0` + `dsub` 序列，而当前配方层的
//! 常量步骤只支持整数载体），因此 `double.negate` 不在本表；补齐常量配方前不得
//! 声明该 provider，避免静默错误语义。

use super::JvmOperatorCode;
use crate::target_operator::{TargetOperandType, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe};

const fn binary(operand_type: TargetOperandType, result_type: TargetOperandType, code: JvmOperatorCode) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 2,
        operand_type,
        result_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: TargetOperatorEncoding::Jvm(code),
    }
}

const fn unary(operand_type: TargetOperandType, code: JvmOperatorCode) -> TargetOperatorPlan {
    TargetOperatorPlan {
        arity: 1,
        operand_type,
        result_type: operand_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: TargetOperatorEncoding::Jvm(code),
    }
}

/// 唯一 operator 配方表。未知名字返回 `None`。
pub(crate) fn freeze_jvm_operator_binding(operator_name: &str) -> Option<TargetOperatorPlan> {
    Some(match operator_name {
        // integer 算术：JVM 单指令（`iadd`/`isub`/`imul`/`idiv`/`irem`/`ineg`）。
        "integer.add" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerAdd),
        "integer.subtract" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerSubtract),
        "integer.multiply" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerMultiply),
        "integer.divide" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerDivide),
        "integer.remainder" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerRemainder),
        "integer.negate" => unary(TargetOperandType::I32, JvmOperatorCode::IntegerNegate),
        // long 算术：JVM 单指令（`ladd`/`lsub`/`lmul`/`ldiv`/`lrem`/`lneg`）。
        "long.add" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongAdd),
        "long.subtract" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongSubtract),
        "long.multiply" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongMultiply),
        "long.divide" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongDivide),
        "long.remainder" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongRemainder),
        "long.negate" => unary(TargetOperandType::I64, JvmOperatorCode::LongNegate),
        // double 算术：JVM 单指令（无 `dneg`，见模块头说明）。
        "double.add" => binary(TargetOperandType::F64, TargetOperandType::F64, JvmOperatorCode::DoubleAdd),
        "double.subtract" => binary(TargetOperandType::F64, TargetOperandType::F64, JvmOperatorCode::DoubleSubtract),
        "double.multiply" => binary(TargetOperandType::F64, TargetOperandType::F64, JvmOperatorCode::DoubleMultiply),
        "double.divide" => binary(TargetOperandType::F64, TargetOperandType::F64, JvmOperatorCode::DoubleDivide),
        // integer 比较：结果固定为 I32（0/1），由后端展开 `if_icmpxx` 分支序列。
        "integer.compare_equal" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerCompareEqual),
        "integer.compare_not_equal" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerCompareNotEqual),
        "integer.compare_less_than" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerCompareLessThan),
        "integer.compare_less_or_equal" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerCompareLessOrEqual),
        "integer.compare_greater_than" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerCompareGreaterThan),
        "integer.compare_greater_or_equal" => {
            binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerCompareGreaterOrEqual)
        }
        // long 比较：结果固定为 I32（0/1），由后端展开 `lcmp` + `ifxx` 序列。
        "long.compare_equal" => binary(TargetOperandType::I64, TargetOperandType::I32, JvmOperatorCode::LongCompareEqual),
        "long.compare_not_equal" => binary(TargetOperandType::I64, TargetOperandType::I32, JvmOperatorCode::LongCompareNotEqual),
        "long.compare_less_than" => binary(TargetOperandType::I64, TargetOperandType::I32, JvmOperatorCode::LongCompareLessThan),
        "long.compare_less_or_equal" => binary(TargetOperandType::I64, TargetOperandType::I32, JvmOperatorCode::LongCompareLessOrEqual),
        "long.compare_greater_than" => binary(TargetOperandType::I64, TargetOperandType::I32, JvmOperatorCode::LongCompareGreaterThan),
        "long.compare_greater_or_equal" => binary(TargetOperandType::I64, TargetOperandType::I32, JvmOperatorCode::LongCompareGreaterOrEqual),
        _ => return None,
    })
}

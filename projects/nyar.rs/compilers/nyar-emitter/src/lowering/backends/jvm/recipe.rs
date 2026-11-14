//! JVM 数值指令绑定的唯一 operator 配方表。
//!
//! `[jvm_operator("long_divide")]` 只声明目标 operator 名；operator 名到**冻结配方**
//! 的映射归 JVM backend 所有，并在 `TargetExecutionPlan` 构造时一次性写入计划（arity、
//! 操作数发射配方、操作数/结果载体与冻结码）。emit 侧只编码计划里的配方，不再
//! 从 operator 名字符串推断 arity 或操作数改写。
//!
//! 本表只接受下列出的 operator 名，未知名字返回 `None`，计划据此冻结为
//! `对应 lane 的 `Unfrozen` 绑定`，由 emit 结构化失败，禁止猜默认指令。
//!
//! JVM 没有 `dneg`（`double` 取负需要 `dconst_0` + `dsub` 序列，而当前配方层的
//! 常量步骤只支持整数载体），因此 `double.negate` 不在本表；补齐常量配方前不得
//! 声明该 provider，避免静默错误语义。

use super::JvmOperatorCode;
use crate::target_operator::{TargetOperandStep, TargetOperandType, TargetOperatorPlan, TargetOperatorRecipe};

const fn binary(operand_type: TargetOperandType, result_type: TargetOperandType, code: JvmOperatorCode) -> TargetOperatorPlan<JvmOperatorCode> {
    TargetOperatorPlan {
        arity: 2,
        operand_type,
        result_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: code,
    }
}

const fn unary(operand_type: TargetOperandType, code: JvmOperatorCode) -> TargetOperatorPlan<JvmOperatorCode> {
    TargetOperatorPlan {
        arity: 1,
        operand_type,
        result_type: operand_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding: code,
    }
}

/// `long` 移位：JVM `lshl`/`lshr` 的位移量操作数必须是 `int`；声明签名携带 `i64`
/// 位移量，截窄（`l2i`）由冻结配方显式给出，emit 只机械执行。
const LONG_SHIFT_STEPS: &[TargetOperandStep] = &[TargetOperandStep::Arguments, TargetOperandStep::NarrowTopI64ToI32];

const fn long_shift(steps: &'static [TargetOperandStep], code: JvmOperatorCode) -> TargetOperatorPlan<JvmOperatorCode> {
    TargetOperatorPlan {
        arity: 2,
        operand_type: TargetOperandType::I64,
        result_type: TargetOperandType::I64,
        recipe: TargetOperatorRecipe::Steps(steps),
        encoding: code,
    }
}

/// 唯一 operator 配方表。未知名字返回 `None`。
pub(crate) fn freeze_jvm_operator_binding(operator_name: &str) -> Option<TargetOperatorPlan<JvmOperatorCode>> {
    Some(match operator_name {
        // integer 算术：JVM 单指令（`iadd`/`isub`/`imul`/`idiv`/`irem`/`ineg`）。
        "integer.add" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerAdd),
        "integer.subtract" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerSubtract),
        "integer.multiply" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerMultiply),
        "integer.divide" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerDivide),
        "integer.remainder" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerRemainder),
        "integer.negate" => unary(TargetOperandType::I32, JvmOperatorCode::IntegerNegate),
        // integer 位运算：JVM 单指令（`iand`/`ior`/`ixor`）。
        "integer.and" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerBitAnd),
        "integer.or" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerBitOr),
        "integer.xor" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerBitXor),
        // integer 移位：`u32` 左移与有符号同形（`ishl`），右移必须是逻辑右移（`iushr`）。
        "integer.shift_left" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerShiftLeft),
        "integer.shift_right_unsigned" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerShiftRightUnsigned),
        // 无符号 `u32` 除/余：JVM 没有无符号 `idiv`/`irem`，冻结为 Java 8 标准库静态方法。
        "integer.divide_unsigned" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerDivideUnsigned),
        "integer.remainder_unsigned" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerRemainderUnsigned),
        // long 算术：JVM 单指令（`ladd`/`lsub`/`lmul`/`ldiv`/`lrem`/`lneg`）。
        "long.add" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongAdd),
        "long.subtract" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongSubtract),
        "long.multiply" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongMultiply),
        "long.divide" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongDivide),
        "long.remainder" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongRemainder),
        "long.negate" => unary(TargetOperandType::I64, JvmOperatorCode::LongNegate),
        // long 位运算：JVM 单指令（`land`/`lor`/`lxor`）。
        "long.and" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongBitAnd),
        "long.or" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongBitOr),
        "long.xor" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongBitXor),
        // long 移位：位移量按冻结配方先 `l2i` 截窄，再执行 `lshl`/`lshr`。
        "long.shift_left" => long_shift(LONG_SHIFT_STEPS, JvmOperatorCode::LongShiftLeft),
        "long.shift_right" => long_shift(LONG_SHIFT_STEPS, JvmOperatorCode::LongShiftRight),
        // 无符号 `u64` 右移：逻辑右移 `lushr`，位移量同样先截窄。
        "long.shift_right_unsigned" => long_shift(LONG_SHIFT_STEPS, JvmOperatorCode::LongShiftRightUnsigned),
        // 无符号 `u64` 除/余：JVM 没有无符号 `ldiv`/`lrem`，冻结为 Java 8 标准库静态方法。
        "long.divide_unsigned" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongDivideUnsigned),
        "long.remainder_unsigned" => binary(TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongRemainderUnsigned),
        // double 算术：JVM 单指令（无 `dneg`，见模块头说明）。
        "double.add" => binary(TargetOperandType::F64, TargetOperandType::F64, JvmOperatorCode::DoubleAdd),
        "double.subtract" => binary(TargetOperandType::F64, TargetOperandType::F64, JvmOperatorCode::DoubleSubtract),
        "double.multiply" => binary(TargetOperandType::F64, TargetOperandType::F64, JvmOperatorCode::DoubleMultiply),
        "double.divide" => binary(TargetOperandType::F64, TargetOperandType::F64, JvmOperatorCode::DoubleDivide),
        // integer 比较：结果固定为 I32（0/1），由后端展开 `if_icmpxx` 分支序列。
        "integer.compare_equal" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerCompareEqual),
        "integer.compare_not_equal" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerCompareNotEqual),
        "integer.compare_less_than" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerCompareLessThan),
        // 无符号 `i32` 小于：JVM 没有单条无符号比较指令，冻结为符号位翻转序列。
        "integer.compare_less_than_unsigned" => {
            binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerCompareLessThanUnsigned)
        }
        "integer.compare_less_or_equal" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerCompareLessOrEqual),
        "integer.compare_greater_than" => binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerCompareGreaterThan),
        "integer.compare_greater_or_equal" => {
            binary(TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerCompareGreaterOrEqual)
        }
        // long 比较：结果固定为 I32（0/1），由后端展开 `lcmp` + `ifxx` 序列。
        "long.compare_equal" => binary(TargetOperandType::I64, TargetOperandType::I32, JvmOperatorCode::LongCompareEqual),
        "long.compare_not_equal" => binary(TargetOperandType::I64, TargetOperandType::I32, JvmOperatorCode::LongCompareNotEqual),
        "long.compare_less_than" => binary(TargetOperandType::I64, TargetOperandType::I32, JvmOperatorCode::LongCompareLessThan),
        // 无符号 `u64` 小于：JVM 没有无符号 `lcmp`，冻结为符号位翻转序列（展开见
        // `JvmOperatorCode::LongCompareLessThanUnsigned`）；翻转中途经 scratch long 槽周转。
        "long.compare_less_than_unsigned" => {
            binary(TargetOperandType::I64, TargetOperandType::I32, JvmOperatorCode::LongCompareLessThanUnsigned)
        }
        "long.compare_less_or_equal" => binary(TargetOperandType::I64, TargetOperandType::I32, JvmOperatorCode::LongCompareLessOrEqual),
        "long.compare_greater_than" => binary(TargetOperandType::I64, TargetOperandType::I32, JvmOperatorCode::LongCompareGreaterThan),
        "long.compare_greater_or_equal" => binary(TargetOperandType::I64, TargetOperandType::I32, JvmOperatorCode::LongCompareGreaterOrEqual),
        _ => return None,
    })
}

/// JVM 平台私有的 operator 绑定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum JvmOperatorBinding {
    /// 已冻结的 JVM 配方。
    Frozen(TargetOperatorPlan<JvmOperatorCode>),
    /// 声明了 `[jvm_operator]`，但没有 JVM 可编码配方；emit 必须结构化失败。
    Unfrozen,
}

/// 按声明的导入签名冻结 JVM operator 绑定。
///
/// `None` 表示该导入不是 JVM operator；`Some(Unfrozen)` 表示绑定了 JVM operator
/// 但名字不在本 lane 的配方表内。两者都必须保留，禁止回落为宿主导入或默认成功。
pub(crate) fn freeze_jvm_operator(
    link: &nyar::ExternalImportLink,
    instance: nyar_types::ItemInstanceId,
    symbol: &str,
    parameter_types: &[nyar_types::NyarType],
    return_type: &nyar_types::NyarType,
) -> crate::Result<Option<JvmOperatorBinding>> {
    let Some(mnemonic) = crate::lowering::shared::interop::jvm_operator_target(link)
    else {
        return Ok(None);
    };
    let Some(plan) = freeze_jvm_operator_binding(mnemonic)
    else {
        return Ok(Some(JvmOperatorBinding::Unfrozen));
    };
    crate::target_operator::validate_frozen_operator(mnemonic, instance, symbol, parameter_types, return_type, &plan)?;
    Ok(Some(JvmOperatorBinding::Frozen(plan)))
}

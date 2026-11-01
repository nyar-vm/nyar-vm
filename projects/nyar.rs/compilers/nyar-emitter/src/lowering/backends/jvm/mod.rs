//! JVM lowering 后端：目标私有 operator 冻结配方与指令编码。
//!
//! `[jvm_operator("long_divide")]` 只声明目标 operator 名；operator 名到冻结配方的
//! 映射（[`recipe::freeze_jvm_operator_binding`]）与冻结码到 `class` 指令序列的展开
//! （[`JvmOperatorCode::instructions`]）都归本模块私有，公共层不携带编号或名字表。

#[doc(hidden)]
pub mod recipe;

use crate::transitional::binary::class::JvmInstruction;

/// 已冻结的 JVM operator 编码。
///
/// 算术码对应单条 JVM 指令；比较码在 JVM 上没有单指令形态（`int` 比较走
/// `if_icmpxx` 分支序列，`long` 比较走 `lcmp` + `ifxx` 序列），因此比较码
/// 冻结的是「后端展开知识」的标识，展开表是本模块的实现事实，不是名字推断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JvmOperatorCode {
    /// JVM `iadd`：integer 加法。
    IntegerAdd,
    /// JVM `isub`：integer 减法。
    IntegerSubtract,
    /// JVM `imul`：integer 乘法。
    IntegerMultiply,
    /// JVM `idiv`：integer 除法。
    IntegerDivide,
    /// JVM `irem`：integer 取余。
    IntegerRemainder,
    /// JVM `ineg`：integer 取负。
    IntegerNegate,
    /// JVM `ladd`：long 加法。
    LongAdd,
    /// JVM `lsub`：long 减法。
    LongSubtract,
    /// JVM `lmul`：long 乘法。
    LongMultiply,
    /// JVM `ldiv`：long 除法。
    LongDivide,
    /// JVM `lrem`：long 取余。
    LongRemainder,
    /// JVM `lneg`：long 取负。
    LongNegate,
    /// JVM `dadd`：double 加法。
    DoubleAdd,
    /// JVM `dsub`：double 减法。
    DoubleSubtract,
    /// JVM `dmul`：double 乘法。
    DoubleMultiply,
    /// JVM `ddiv`：double 除法。
    DoubleDivide,
    /// integer 相等比较（结果压 `int` 0/1）。
    IntegerCompareEqual,
    /// integer 不等比较（结果压 `int` 0/1）。
    IntegerCompareNotEqual,
    /// integer 小于比较（结果压 `int` 0/1）。
    IntegerCompareLessThan,
    /// integer 小于等于比较（结果压 `int` 0/1）。
    IntegerCompareLessOrEqual,
    /// integer 大于比较（结果压 `int` 0/1）。
    IntegerCompareGreaterThan,
    /// integer 大于等于比较（结果压 `int` 0/1）。
    IntegerCompareGreaterOrEqual,
    /// long 相等比较（结果压 `int` 0/1）。
    LongCompareEqual,
    /// long 不等比较（结果压 `int` 0/1）。
    LongCompareNotEqual,
    /// long 小于比较（结果压 `int` 0/1）。
    LongCompareLessThan,
    /// long 小于等于比较（结果压 `int` 0/1）。
    LongCompareLessOrEqual,
    /// long 大于比较（结果压 `int` 0/1）。
    LongCompareGreaterThan,
    /// long 大于等于比较（结果压 `int` 0/1）。
    LongCompareGreaterOrEqual,
}

impl JvmOperatorCode {
    /// 单指令算术码对应的 JVM 指令；比较码返回 `None`（必须走 [`Self::instructions`]）。
    pub const fn arithmetic_instruction(self) -> Option<JvmInstruction> {
        Some(match self {
            Self::IntegerAdd => JvmInstruction::IAdd,
            Self::IntegerSubtract => JvmInstruction::ISub,
            Self::IntegerMultiply => JvmInstruction::IMul,
            Self::IntegerDivide => JvmInstruction::IDiv,
            Self::IntegerRemainder => JvmInstruction::IRem,
            Self::IntegerNegate => JvmInstruction::INeg,
            Self::LongAdd => JvmInstruction::LAdd,
            Self::LongSubtract => JvmInstruction::LSub,
            Self::LongMultiply => JvmInstruction::LMul,
            Self::LongDivide => JvmInstruction::LDiv,
            Self::LongRemainder => JvmInstruction::LRem,
            Self::LongNegate => JvmInstruction::LNeg,
            Self::DoubleAdd => JvmInstruction::DAdd,
            Self::DoubleSubtract => JvmInstruction::DSub,
            Self::DoubleMultiply => JvmInstruction::DMul,
            Self::DoubleDivide => JvmInstruction::DDiv,
            _ => return None,
        })
    }

    /// 冻结码到 JVM 指令序列的展开。
    ///
    /// 算术码展开为单条指令；比较码展开为产生 `int` 0/1 的分支序列。
    /// 序列中的标签是发射期占位名，由调用方（lowering emit）替换为带后缀的
    /// 唯一标签；本函数只给出形状，不解析跳转。
    pub fn instructions(self) -> Vec<JvmInstruction> {
        if let Some(instruction) = self.arithmetic_instruction() {
            return vec![instruction];
        }
        // 比较序列统一形状：
        //   [比较前置] 分支到 false → iconst_1 goto end → false: iconst_0 → end:
        // `int` 比较用 `if_icmpxx` 直接比较栈顶两 `int`；`long` 比较先 `lcmp`
        // 压入 `int` 差值再用 `ifxx`。
        let (prelude, branch): (Vec<JvmInstruction>, Vec<JvmInstruction>) = match self {
            Self::IntegerCompareEqual => (vec![], vec![JvmInstruction::IfICmpNe("comparison_false".to_string())]),
            Self::IntegerCompareNotEqual => (vec![], vec![JvmInstruction::IfICmpEq("comparison_false".to_string())]),
            Self::IntegerCompareLessThan => (vec![], vec![JvmInstruction::IfICmpGe("comparison_false".to_string())]),
            Self::IntegerCompareLessOrEqual => (vec![], vec![JvmInstruction::IfICmpGt("comparison_false".to_string())]),
            Self::IntegerCompareGreaterThan => (vec![], vec![JvmInstruction::IfICmpLe("comparison_false".to_string())]),
            Self::IntegerCompareGreaterOrEqual => (vec![], vec![JvmInstruction::IfICmpLt("comparison_false".to_string())]),
            Self::LongCompareEqual => (vec![JvmInstruction::LCmp], vec![JvmInstruction::IfNe("comparison_false".to_string())]),
            Self::LongCompareNotEqual => (vec![JvmInstruction::LCmp], vec![JvmInstruction::IfEq("comparison_false".to_string())]),
            Self::LongCompareLessThan => (vec![JvmInstruction::LCmp], vec![JvmInstruction::IfGe("comparison_false".to_string())]),
            Self::LongCompareLessOrEqual => (vec![JvmInstruction::LCmp], vec![JvmInstruction::IfGt("comparison_false".to_string())]),
            Self::LongCompareGreaterThan => (vec![JvmInstruction::LCmp], vec![JvmInstruction::IfLe("comparison_false".to_string())]),
            Self::LongCompareGreaterOrEqual => (vec![JvmInstruction::LCmp], vec![JvmInstruction::IfLt("comparison_false".to_string())]),
            _ => return vec![],
        };
        let mut sequence = prelude;
        sequence.extend(branch);
        sequence.push(JvmInstruction::IConst(1));
        sequence.push(JvmInstruction::Goto("comparison_end".to_string()));
        sequence.push(JvmInstruction::Label("comparison_false".to_string()));
        sequence.push(JvmInstruction::IConst(0));
        sequence.push(JvmInstruction::Label("comparison_end".to_string()));
        sequence
    }
}

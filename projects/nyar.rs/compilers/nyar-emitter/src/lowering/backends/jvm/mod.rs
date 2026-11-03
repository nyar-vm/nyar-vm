//! JVM lowering 后端：目标私有 operator 冻结配方与指令编码。
//!
//! `[jvm_operator("long_divide")]` 只声明目标 operator 名；operator 名到冻结配方的
//! 映射（[`recipe::freeze_jvm_operator_binding`]）与冻结码到 `class` 指令序列的展开
//! （[`JvmOperatorCode::instructions`]）都归本模块私有，公共层不携带编号或名字表。

#[doc(hidden)]
pub mod recipe;

pub(crate) mod emit;

use crate::transitional::binary::class::{JvmInstruction, JvmMethodDescriptor, JvmMethodRef, JvmTypeDescriptor};

/// `java/lang/Integer` 上的无符号二元静态方法调用（参数与返回值均为 `int`）。
fn integer_unsigned_call(name: &str) -> JvmInstruction {
    JvmInstruction::InvokeStatic(JvmMethodRef {
        owner: "java/lang/Integer".to_string(),
        name: name.to_string(),
        descriptor: JvmMethodDescriptor::new(vec![JvmTypeDescriptor::Int, JvmTypeDescriptor::Int], JvmTypeDescriptor::Int),
    })
}

/// `java/lang/Long` 上的无符号二元静态方法调用（参数与返回值均为 `long`）。
fn long_unsigned_call(name: &str) -> JvmInstruction {
    JvmInstruction::InvokeStatic(JvmMethodRef {
        owner: "java/lang/Long".to_string(),
        name: name.to_string(),
        descriptor: JvmMethodDescriptor::new(vec![JvmTypeDescriptor::Long, JvmTypeDescriptor::Long], JvmTypeDescriptor::Long),
    })
}

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
    /// JVM `iand`：integer 按位与（`u32` 按位等价）。
    IntegerBitAnd,
    /// JVM `ior`：integer 按位或（`u32` 按位等价）。
    IntegerBitOr,
    /// JVM `ixor`：integer 按位异或（`u32` 按位等价）。
    IntegerBitXor,
    /// JVM `ishl`：integer 左移（`u32` 与有符号同形，位移量按低 5 位取模）。
    IntegerShiftLeft,
    /// JVM `iushr`：integer 逻辑右移（`u32` 的右移不能走有符号 `ishr`）。
    IntegerShiftRightUnsigned,
    /// `Integer.divideUnsigned(II)I`：`u32` 除法（JVM 无无符号 `idiv`）。
    IntegerDivideUnsigned,
    /// `Integer.remainderUnsigned(II)I`：`u32` 取余（JVM 无无符号 `irem`）。
    IntegerRemainderUnsigned,
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
    /// JVM `land`：long 按位与。
    LongBitAnd,
    /// JVM `lor`：long 按位或。
    LongBitOr,
    /// JVM `lxor`：long 按位异或。
    LongBitXor,
    /// JVM `lshl`：long 左移（位移量 `int`，截窄由冻结配方显式给出）。
    LongShiftLeft,
    /// JVM `lshr`：long 算术右移（位移量 `int`，截窄由冻结配方显式给出）。
    LongShiftRight,
    /// JVM `lushr`：long 逻辑右移（`u64` 的右移不能走有符号 `lshr`）。
    LongShiftRightUnsigned,
    /// `Long.divideUnsigned(JJ)J`：`u64` 除法（JVM 无无符号 `ldiv`）。
    LongDivideUnsigned,
    /// `Long.remainderUnsigned(JJ)J`：`u64` 取余（JVM 无无符号 `lrem`）。
    LongRemainderUnsigned,
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
    /// integer 无符号小于比较（结果压 `int` 0/1）：JVM 没有无符号 `if_icmp`，
    /// 展开为「两侧异或 `i32::MIN` 后按有符号比较」的显式序列。
    IntegerCompareLessThanUnsigned,
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
    /// long 无符号小于比较（结果压 `int` 0/1）：JVM 没有无符号 `lcmp`，
    /// 展开为「两侧异或 `i64::MIN` 翻转符号位后按有符号比较」的显式序列；
    /// JVM 没有 `swap2`，中间的翻转结果经发射方预留的 scratch long 槽周转。
    LongCompareLessThanUnsigned,
    /// long 小于等于比较（结果压 `int` 0/1）。
    LongCompareLessOrEqual,
    /// long 大于比较（结果压 `int` 0/1）。
    LongCompareGreaterThan,
    /// long 大于等于比较（结果压 `int` 0/1）。
    LongCompareGreaterOrEqual,
}

impl JvmOperatorCode {
    /// 单指令算术码对应的 JVM 指令；比较码返回 `None`（必须走 [`Self::instructions`]）。
    pub fn arithmetic_instruction(self) -> Option<JvmInstruction> {
        Some(match self {
            Self::IntegerAdd => JvmInstruction::IAdd,
            Self::IntegerSubtract => JvmInstruction::ISub,
            Self::IntegerMultiply => JvmInstruction::IMul,
            Self::IntegerDivide => JvmInstruction::IDiv,
            Self::IntegerRemainder => JvmInstruction::IRem,
            Self::IntegerNegate => JvmInstruction::INeg,
            Self::IntegerBitAnd => JvmInstruction::IAnd,
            Self::IntegerBitOr => JvmInstruction::IOr,
            Self::IntegerBitXor => JvmInstruction::IXor,
            Self::IntegerShiftLeft => JvmInstruction::IShl,
            Self::IntegerShiftRightUnsigned => JvmInstruction::IUShr,
            // JVM 没有无符号 `idiv`/`irem` 单指令；Java 8 起由标准库静态方法给出。
            Self::IntegerDivideUnsigned => integer_unsigned_call("divideUnsigned"),
            Self::IntegerRemainderUnsigned => integer_unsigned_call("remainderUnsigned"),
            Self::LongAdd => JvmInstruction::LAdd,
            Self::LongSubtract => JvmInstruction::LSub,
            Self::LongMultiply => JvmInstruction::LMul,
            Self::LongDivide => JvmInstruction::LDiv,
            Self::LongRemainder => JvmInstruction::LRem,
            Self::LongNegate => JvmInstruction::LNeg,
            Self::LongBitAnd => JvmInstruction::LAnd,
            Self::LongBitOr => JvmInstruction::LOr,
            Self::LongBitXor => JvmInstruction::LXor,
            Self::LongShiftLeft => JvmInstruction::LShl,
            Self::LongShiftRight => JvmInstruction::LShr,
            Self::LongShiftRightUnsigned => JvmInstruction::LUShr,
            // JVM 没有无符号 `ldiv`/`lrem` 单指令；Java 8 起由标准库静态方法给出。
            Self::LongDivideUnsigned => long_unsigned_call("divideUnsigned"),
            Self::LongRemainderUnsigned => long_unsigned_call("remainderUnsigned"),
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
    ///
    /// `scratch_long_slot` 是发射方在 `max_locals` 之外按需预留的 2 槽 `long`
    /// 周转槽；只有 [`Self::LongCompareLessThanUnsigned`] 消费它——`u64` 的序等价于
    /// 「两侧都翻转符号位后」的 `i64` 序，而 JVM 没有 `swap2`，翻转中途必须经局部槽
    /// 周转。其余码忽略该参数。
    pub fn instructions(self, scratch_long_slot: u16) -> Vec<JvmInstruction> {
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
            // 无符号比较：`u32` 的序等价于「两侧都翻转符号位后」的 `i32` 序。
            // 栈 `[a, b]` → `[a ^ MIN, b ^ MIN]`，再按有符号 `<` 展开。
            Self::IntegerCompareLessThanUnsigned => (
                vec![
                    JvmInstruction::IConst(i32::MIN),
                    JvmInstruction::IXor,
                    JvmInstruction::Swap,
                    JvmInstruction::IConst(i32::MIN),
                    JvmInstruction::IXor,
                    JvmInstruction::Swap,
                ],
                vec![JvmInstruction::IfICmpGe("comparison_false".to_string())],
            ),
            Self::IntegerCompareLessOrEqual => (vec![], vec![JvmInstruction::IfICmpGt("comparison_false".to_string())]),
            Self::IntegerCompareGreaterThan => (vec![], vec![JvmInstruction::IfICmpLe("comparison_false".to_string())]),
            Self::IntegerCompareGreaterOrEqual => (vec![], vec![JvmInstruction::IfICmpLt("comparison_false".to_string())]),
            Self::LongCompareEqual => (vec![JvmInstruction::LCmp], vec![JvmInstruction::IfNe("comparison_false".to_string())]),
            Self::LongCompareNotEqual => (vec![JvmInstruction::LCmp], vec![JvmInstruction::IfEq("comparison_false".to_string())]),
            Self::LongCompareLessThan => (vec![JvmInstruction::LCmp], vec![JvmInstruction::IfGe("comparison_false".to_string())]),
            // 无符号 `u64` 小于：栈 `[a, b]` → 先把 `b ^ MIN` 周转进 scratch 槽，
            // 再翻转 `a`，最后 `lcmp` 按有符号比较。`ldc2_w i64::MIN` + `lxor` 即符号位翻转。
            Self::LongCompareLessThanUnsigned => (
                vec![
                    JvmInstruction::LdcLong(i64::MIN),
                    JvmInstruction::LXor,
                    JvmInstruction::LStore(scratch_long_slot),
                    JvmInstruction::LdcLong(i64::MIN),
                    JvmInstruction::LXor,
                    JvmInstruction::LLoad(scratch_long_slot),
                    JvmInstruction::LCmp,
                ],
                vec![JvmInstruction::IfGe("comparison_false".to_string())],
            ),
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

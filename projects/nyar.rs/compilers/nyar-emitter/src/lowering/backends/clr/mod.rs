//! CLR lowering 后端：目标私有 operator 冻结配方与 MSIL 编码。
//!
//! `[clr_operator("i64.div")]` 只声明目标 operator 助记符；助记符到冻结配方的
//! 映射（[`mir::freeze_clr_operator_binding`]）归本模块私有，公共层不携带编号
//! 或名字表。MSIL 的算术/比较 opcode 与数值载体无关（`add` 对 `int32`/`int64`/
//! `float64` 同形），因此助记符必须携带载体限定名，冻结表按限定名给出唯一配方。

#[doc(hidden)]
pub mod mir;

pub(crate) mod emit;

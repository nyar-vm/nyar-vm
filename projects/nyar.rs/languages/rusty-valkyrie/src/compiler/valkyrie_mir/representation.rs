//! Valkyrie 表示义务的稀疏计划。
//!
//! 本模块只记录**语言语义义务**：某个调用如何绑定、某个值是否必须共享身份、
//! ADT 是否必须保留类型化聚合、文本值是否托管。物理载体（GC 对象、装箱帧、
//! 状态机、宿主 coroutine、tag 宽度、偏移）归各平台自己的计划与 LIR 决定，
//! 不在此提前选择，也不写占位事实。

use std::collections::BTreeMap;

use nyar_types::semantic_ids::{InstructionId, NominalInstanceId, ValueIdentity};

/// 可调用 / apply 位点的绑定方式（语言义务，不是目标指令）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvokeLowering {
    /// 对已知项的直接调用。
    Direct,
    /// 类型化 witness 调用。
    TypedWitness,
    /// 类型化间接 / 函数引用。
    TypedReference,
}

/// 值语义义务。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueRepresentation {
    /// 值语义：可内联复制，目标可自由选择标量或内联聚合载体。
    Specialized,
    /// 引用语义：必须保留对象身份；具体载体（GC 对象、宿主句柄等）归平台。
    Reified,
}

/// ADT 语义义务。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdtRepresentation {
    /// 必须保留类型化聚合身份；物理布局由平台解析。
    TypedAggregate,
}

/// 程序级文本值载体表示事实。
///
/// 文本值空间只有两种正式来源：源程序在 `std.text` 下声明并登记了文本载体
/// （`LinkedSemanticProgram::text_encoding_types`）时为托管对象；未登记任何
/// 载体时，`utf8` / `utf16` 值就是宿主编码基元本身。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextRepresentation {
    /// raw 宿主串。
    Raw,
    /// 托管文本对象。
    Managed,
}

/// 与目标无关的稀疏表示义务计划。
///
/// 以稳定语义 id 为键；不复制 CFG，也不携带平台 tag 宽度、偏移或载体枚举。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepresentationPlan {
    /// 按指令的 invoke / apply 绑定方式。
    pub invoke_lowerings: BTreeMap<InstructionId, InvokeLowering>,
    /// 按值的语义义务。
    pub value_reps: BTreeMap<ValueIdentity, ValueRepresentation>,
    /// 按名义实例的 ADT 义务。
    pub adt_reps: BTreeMap<NominalInstanceId, AdtRepresentation>,
    /// 程序级文本值载体表示；`None` 表示规划器尚未声明，后端必须失败闭合。
    pub text_representation: Option<TextRepresentation>,
}

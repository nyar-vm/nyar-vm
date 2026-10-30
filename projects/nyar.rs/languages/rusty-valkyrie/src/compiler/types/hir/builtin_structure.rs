//! 语言内建结构操作身份（frontend 私有）。

/// 语言内建结构操作身份。
///
/// 这是**语言内建结构操作身份**，只由两种来源产生：
///
/// 1. 源码声明上显式 `[intrinsic("<path>")]` 注解（经 [`Self::from_annotation_path`]）；
/// 2. 前端内建下标候选（`suffix [ ]` / `suffix ⁅ ⁆` 及其赋值形态）。
///
/// **它不是目标指令**：不带任何数值编码、没有 `bytecode_index`，也不得用于选择
/// wasm / OVM 指令。目标指令选择归各 backend 所有；backend 从自身冻结的
/// 计划与绑定事实出发，不消费本枚举做指令分派。
///
/// 数组下标 get/set、长度与 ref deref 是**有限、类型参数化的语言结构语义**，
/// 因此允许作为内建结构操作保留；不得按元素类型、tuple arity 或 target 扩展枚举。
///
/// 函数式 `push` 已退役为公共特殊语义：`[intrinsic("builtin.array.push")]` 注解不再
/// 产生身份，push 调用按普通声明 callable 走 `Invoke`（与 `array.insert` 等
/// 不产生身份的注解先例一致）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BuiltinStructureOp {
    /// `builtin.array.length`（别名 `builtin.array.len`）：数组 / 列表长度。
    ArrayLen,
    /// `builtin.array.get`：数组存储的下标读取。
    ArrayGet,
    /// `builtin.array.set`：数组存储的下标写入。
    ArraySet,
    /// `builtin.ref.deref`：引用解引用。
    RefDeref,
}

impl BuiltinStructureOp {
    /// 由 `[intrinsic("<path>")]` 注解的**精确渲染路径**解析身份。
    ///
    /// 只接受显式声明的注解路径，不做任何按名字末段的兜底猜测：
    /// `builtin.array.length`（别名 `builtin.array.len`）/
    /// `builtin.array.get` / `builtin.array.set` / `builtin.ref.deref`。
    ///
    /// 不落在该命名空间的注解路径（例如 `builtin.array.push`、`array.insert`）
    /// 不产生身份，返回 `None`。
    pub fn from_annotation_path(rendered: &str) -> Option<Self> {
        match rendered {
            "builtin.array.length" | "builtin.array.len" => Some(Self::ArrayLen),
            "builtin.array.get" => Some(Self::ArrayGet),
            "builtin.array.set" => Some(Self::ArraySet),
            "builtin.ref.deref" => Some(Self::RefDeref),
            _ => None,
        }
    }
}

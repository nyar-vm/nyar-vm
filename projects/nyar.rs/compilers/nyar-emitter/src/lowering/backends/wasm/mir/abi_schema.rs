//! 阶段 B：唯一的 Valkyrie → Wasm ABI 合同。
//!
//! 本模块为 `nyar.library_invoke` JSON section 提供**结构化、可序列化**的
//! ABI schema 枚举。
//!
//! 设计约束（架构师硬性要求）：
//! - 禁止按容器名称硬编码 ABI（`is_list_i64`、`is_arraylist` 一律禁止）
//! - 禁止按函数名判断导出入口
//! - 禁止缺 layout 时默认布局
//! - 每个 NyarType 必须生成语义 identity；不认识的变体显式返回 `Unsupported`
//!
//! 下游消费：
//! - `library_invoke.rs`：序列化为 `nyar.library_invoke` JSON section
//! - `wasm_js_glue`：按结构化 schema 生成 JS marshal/demarshal 代码
//! - 其他宿主（CLR / JVM / Native）：按同一份 schema 生成各自绑定

use std::collections::BTreeMap;

use nyar_types::{AggregateLayout, AggregateLayoutPlan, LayoutId, NyarType, combined_content_hash};
use serde::{Deserialize, Serialize};

/// 结构化 Wasm ABI schema。
///
/// 这是**唯一**跨后端的库导出 ABI 合同。每个枚举变体对应一类可以
/// 出现在 Wasm 导出函数参数/返回值位置的语义类型，携带足够让下游
/// 宿主生成 marshal 代码的信息。
///
/// # 与 `NyarType` 的关系
///
/// `NyarType` 是语言平台层类型，承载完整语义信息；`WasmAbiType` 是
/// **ABI 位置筛选**后的子集——有些 `NyarType` 变体（如 `Function`、
/// `TraitObject`）根本不允许出现在库导出的参数/返回值上，会被映射
/// 成 `Unsupported`。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum WasmAbiType {
    /// Nothing / void-as-value。Wasm 无 carrier，JS glue 侧 `undefined`。
    Unit,

    /// 32-bit integer（有符号或无符号统一映射为 I32 carrier）。
    I32,

    /// 64-bit integer（有符号或无符号统一映射为 I64 carrier）。
    I64,

    /// 32-bit float。Wasm physical carrier: f32。
    ///
    /// Wasm 原生支持 `f32` 参数和返回值（有独立的 `valtype F32` 和
    /// 完整的 f32 指令集）。当前某些底层发射库可能只暴露了 F64 变体，
    /// 那是实现细节；ABI schema 层必须按 Wasm 原生能力声明 carrier。
    F32,

    /// 64-bit float。
    F64,

    /// Boolean，Wasm carrier 为 I32（0 = false，非零 = true）。
    Bool,

    /// Unicode scalar character，Wasm carrier 为 I32。
    Char,

    /// UTF-8 string。
    ///
    /// Node/JS-glue 轨宿主字符串走 i32 句柄（`hostIntern`），
    /// WASI 轨可能走 ptr+len——schema 只声明语义身份，不声明物理布局。
    Utf8,

    /// UTF-16 string（当前 JS glue 轨尚未支持，显式标为 Unsupported 更安全——
    /// 下游宿主看到 Utf16 可以拒绝而不是默默按 Utf8 处理）。
    #[serde(skip)]
    Utf16,

    /// Heap array `[T]`。
    ///
    /// 在 Wasm GC backend 里对应 `arraytype`（若元素是 reference）
    /// 或线性内存 buffer（若元素是纯 value）。
    ///
    /// 该变体同时用作**序列视图**：布局形状是「一个 heap array 字段 + 一个整数
    /// 计数字段」的 nominal 聚合（例如 `ArrayList<T>` 的 `_items` + `_length`）
    /// 在 ABI 位置投影为 `Array<T>`，并在 `container` 里携带把 JS 数组装配回
    /// VM 聚合所需的构造合同。投影只由冻结布局形状派生，不看类型名。
    Array {
        /// 元素类型的 ABI schema。
        element: Box<WasmAbiType>,
        /// 物理数组读写操作（wasm 导出名，按元素类型确定性派生）。
        ops: WasmAbiArrayOps,
        /// 序列视图的容器构造合同；普通 `[T]` 为 `None`。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        container: Option<Box<WasmAbiSequence>>,
    },

    /// Fixed-length stack array `[T; N]`。
    FixedArray {
        /// 元素类型的 ABI schema。
        element: Box<WasmAbiType>,
        /// 元素数量。
        length: usize,
    },

    /// Option / nullable `T?`。
    ///
    /// Nyar 源层写作 `Nullable(T)`；schema 层统一命名为 Option
    /// （与 Valkyrie 表面语法 `T?`、`Option<T>` 对齐）。
    ///
    /// Wasm 物理降级是 **niche**：`None` = `ref.null`，`Some(引用)` 直接是该引用；
    /// `Some(标量)` 则需要把标量装进 `[i32]` / `[i64]` box 后再放进 anyref 槽。
    /// `box_contract` 只在标量载荷需要装箱时出现；引用载荷（容器 / 具名聚合 /
    /// 嵌套 Option）没有合同，宿主直接透传。宿主不得按 kind 猜哪一侧。
    Option {
        /// 内部非空值类型。
        inner: Box<WasmAbiType>,
        /// 标量载荷的 box/unbox 导出合同；引用载荷为 `None`。
        #[serde(default, skip_serializing_if = "Option::is_none", rename = "box")]
        box_contract: Option<Box<WasmAbiOptionBox>>,
    },

    /// Tuple / product aggregate `(T1, T2, ...)`。
    ///
    /// 位置与位置之间有结构含义（不同于匿名 union）。
    Tuple {
        /// 按声明顺序排列的字段类型。
        fields: Vec<WasmAbiType>,
    },

    /// 带语义身份的 nominal / generic aggregate。
    ///
    /// 用于承载 `Apply(Named("ArrayList"), [Integer64])`、用户自定义 struct、
    /// 或任何 NyarType 层无法内联展开的命名类型。
    ///
    /// # 稳定 nominal identity
    ///
    /// `name` 只是简单名；`module_path` 提供完整命名空间（例如
    /// `"std.collections"`）。下游宿主（JS glue / CLR / JVM）在 marshal
    /// 注册表中**必须同时检查两者**，才能区分同名但不同来源包的 aggregate。
    ///
    /// # 禁止硬编码
    ///
    /// 本变体**不**在 schema 生成阶段判断 `name` 是否等于 `"ArrayList"` /
    /// `"HashMap"`——那是下游宿主的 marshal 注册表要做的事。本模块只负责
    /// 忠实地传递语义身份。
    NamedAggregate {
        /// 名义类型的简单名（不含模块路径）。
        name: String,
        /// 类型所属的完整命名空间路径（例如 `"std::collections"`）。
        ///
        /// # 上游拼写来源
        ///
        /// `rusty-valkyrie` 的 canonical 阶段在**声明** TypeId 上登记
        /// `MirStruct::qualified_name()`（`namespace.name` 规范为 `::`）。泛型实例
        /// `ArrayList<i64>` 在 `lower_type` 中回落到声明 TypeId 取基类型拼写，因此
        /// `Apply` 的 `module_path` 与声明一致。
        ///
        /// 仍为空字符串的场景只是**如实**的缺口：sum 声明目前没有 namespace 事实，
        /// 只有简单名；无名空间（顶层）声明本身也没有路径可拆。
        module_path: String,
        /// 实例化的泛型参数（无泛型时为空 vec）。
        generic_args: Vec<WasmAbiType>,
        /// 冻结布局派生的字段访问合同（无布局身份时为空）。
        ///
        /// 只有 [`abi_schema_for_type_with_layouts`] 会填充：每个字段携带字段名、
        /// 字段类型 schema 与从聚合句柄读出该字段的 wasm accessor 导出名。
        /// JS glue 等宿主据此按字段读回聚合；空列表表示只有身份、没有字段桥，
        /// 宿主必须失败闭合而不是按名字猜测字段。
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        fields: Vec<WasmAbiField>,
        /// 构造该聚合的 wasm 导出名（由布局身份确定性派生）；`None` 表示该布局
        /// 没有 GC struct 构造桥，宿主必须失败闭合而不是自行分配内存。
        #[serde(default, skip_serializing_if = "Option::is_none")]
        constructor: Option<String>,
    },

    /// `class` 等引用存储名义实例的**不透明宿主句柄**。
    ///
    /// 冻结布局的存储类是 [`nyar_types::StorageKind::Reference`] 且不是序列视图时，
    /// 该 nominal 在跨 ABI 位置是 GC 引用本身：语义是同一实例的共享身份，
    /// 不是字段快照。宿主必须原样保留该引用（JS glue 包装成不透明句柄对象），
    /// 不能按字段逐一复制；否则 `mut self` 方法会改在副本上，跨调用不可见。
    ///
    /// 实例由语言侧构造函数创建（例如 `[export]` 的 `new`），宿主不得按字段
    /// 现场重建；`name` / `module_path` 只作身份核对，字段合同不在本变体中。
    InstanceHandle {
        /// 名义类型的简单名（不含模块路径）。
        name: String,
        /// 类型所属的完整命名空间路径。
        module_path: String,
    },

    /// 零 payload unite（`is_unite` sum）的变体身份合同。
    ///
    /// unite 在 Wasm 侧是 `[tag, payload]` GC struct；宿主拿到引用后经
    /// `tag_accessor` 读出判别值，再按 `variants` 的 tag→名字映射回读变体身份。
    /// 只有冻结布局证明 `is_unite` 且全部变体无 payload 时才生成此变体；
    /// 带 payload 的 sum 仍保留 [`WasmAbiType::NamedAggregate`] 身份，宿主在
    /// 边界失败闭合，而不是静默丢弃 payload。
    Sum {
        /// unite 的简单名（不含模块路径；canonical 目前没有 sum 的 namespace 事实）。
        name: String,
        /// 按声明顺序排列的变体身份。
        variants: Vec<WasmAbiSumVariant>,
        /// `(unite anyref) -> i32`：读出判别 tag 的 wasm 导出名。
        tag_accessor: String,
    },

    /// 不支持出现在库导出 ABI 位置的类型。
    ///
    /// # 何时产生
    ///
    /// - `NyarType::Function`：函数类型不能跨 ABI 边界
    /// - `NyarType::TraitObject`：动态分发 witness table 无稳定 Wasm carrier
    /// - `NyarType::Union(_)`：匿名 tagged union 的 ABI 合同尚未定义
    /// - `NyarType::Integer128 { .. }`：Wasm 无原生 i128，需要 explicit 拆分合同
    /// - `NyarType::Bottom` / `void`：底部类型不应出现在参数位置
    Unsupported {
        /// 可读原因，供 ABI 合同校验失败时展示。
        reason: String,
    },
}

/// 具名聚合的字段访问合同：布局派生的 demarshal 元数据。
///
/// 字段顺序即声明顺序；`accessor` 是从布局身份确定性派生的 wasm 导出名，
/// 宿主按该名字从聚合句柄读出单个字段的物理载体，再按 `ty` 反编组。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmAbiField {
    /// 字段名（来自冻结布局，禁止按类型名或 metadata 猜测）。
    pub name: String,
    /// 字段类型的 ABI schema。
    #[serde(rename = "type")]
    pub ty: WasmAbiType,
    /// 字段 accessor 的 wasm 导出名（布局身份确定性派生）。
    pub accessor: String,
}

/// unite 变体身份：判别 tag 与声明名逐一对应。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmAbiSumVariant {
    /// 变体名（冻结 sum 布局，禁止按下标或拼写猜测）。
    pub name: String,
    /// 判别 tag（与 Wasm GC struct 的 tag 字段同源）。
    pub tag: u32,
}

/// 物理 heap array 的读写操作导出名。
///
/// 名字按**元素类型**确定性派生，与 `gc_array_type_indices` 的注册键同源；
/// 宿主只按 schema 里的名字调度，不按容器名推断。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmAbiArrayOps {
    /// `(array anyref) -> i32`：`array.len`。
    pub length: String,
    /// `(array anyref, i32) -> element`：`array.get`。
    pub get: String,
    /// `(i32 len) -> array anyref`：`array.new_default`。
    #[serde(rename = "new")]
    pub new: String,
    /// `(array anyref, i32, element) -> ()`：`array.set`。
    pub set: String,
}

/// 序列视图的容器构造合同。
///
/// 派生条件（全部来自冻结布局，不看类型名）：
/// - 布局存储类是 `Reference`（跨 ABI 位置是 GC 句柄）；
/// - 恰好两个字段：一个 heap `Array<T>` 字段，一个整数字段；
/// - 有已注册的 GC struct 类型，因此能发射 `struct.new` 构造函数。
///
/// `items_field_index` / `length_field_index` 是构造参数顺序事实；宿主按
/// `field_count` 组参数，不按字段名猜测。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmAbiSequence {
    /// `struct.new` 包装导出名（布局身份确定性派生）。
    pub constructor: String,
    /// backing array 字段在构造参数里的下标。
    pub items_field_index: usize,
    /// 计数字段在构造参数里的下标。
    pub length_field_index: usize,
    /// backing array accessor 导出名（从容器句柄读出数组句柄）。
    pub items_accessor: String,
    /// 计数字段 accessor 导出名。
    pub length_accessor: String,
    /// 构造参数总数（当前恒为 2）。
    pub field_count: usize,
    /// 计数字段的 ABI schema（`i32` 或 `i64`）。
    pub length_type: Box<WasmAbiType>,
}

/// `Option<标量>` 的装箱合同。
///
/// 派生自 Wasm 物理降级事实：标量载荷在 anyref 槽里的唯一合法表示是后端已注册的
/// `[i32]` / `[i64]` box struct。`new` / `get` 是模块导出的 `struct.new` /
/// `ref.cast` + `struct.get` 包装；名字由载荷载体确定性派生，与
/// [`WasmAbiSequence::constructor`] 同源（布局/载体身份，不看类型名）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmAbiOptionBox {
    /// `(payload 载体) -> anyref`：`struct.new` 包装导出名。
    pub new: String,
    /// `(anyref) -> payload 载体`：`ref.cast` + `struct.get` 包装导出名。
    pub get: String,
    /// 载荷物理载体（`i32` / `i64`），同时是 box struct 的身份事实。
    pub payload: String,
}

impl WasmAbiType {
    /// 返回值是否有 Wasm 物理 carrier。
    ///
    /// `Unit` 和 `Unsupported` 都不产生 Wasm 参数/返回槽——
    /// 区别在于 `Unsupported` 应该被**拒绝**而不是静默忽略。
    pub fn has_wasm_carrier(&self) -> bool {
        !matches!(self, WasmAbiType::Unit | WasmAbiType::Unsupported { .. })
    }

    /// 从 schema 提取 `Unsupported::reason`（若存在）。
    pub fn unsupported_reason(&self) -> Option<&str> {
        match self {
            WasmAbiType::Unsupported { reason } => Some(reason),
            _ => None,
        }
    }

    /// 降级为 JSON section 使用的简化字符串名（仅限标量和 Unit）。
    ///
    /// 供 `WasmAbiType::from_legacy_str` 解析旧版 `{"params": ["i64"]}` JSON
    /// 时做 round-trip 测试。**新 ABI schema 不消费本方法**。
    #[doc(hidden)]
    pub fn legacy_scalar_name(&self) -> Option<&'static str> {
        match self {
            WasmAbiType::Unit => Some("unit"),
            WasmAbiType::I32 => Some("i32"),
            WasmAbiType::I64 => Some("i64"),
            WasmAbiType::F32 => Some("f32"),
            WasmAbiType::F64 => Some("f64"),
            WasmAbiType::Bool => Some("bool"),
            WasmAbiType::Char => Some("char"),
            WasmAbiType::Utf8 => Some("utf8"),
            _ => None,
        }
    }
}

/// 将 `NyarType` 映射为结构化的 `WasmAbiType`。
///
/// # 映射规则
///
/// | NyarType                     | WasmAbiType                                    | 备注                                           |
/// |------------------------------|------------------------------------------------|------------------------------------------------|
/// | `Unit` / `Bottom`            | `Unit` / `Unsupported`                         | Bottom 不应出现在导出参数                      |
/// | `Integer8/16/32 {..}`        | `I32`                                          | Wasm 参数统一 i32 carrier                      |
/// | `Integer64 {..}`             | `I64`                                          |                                                |
/// | `Integer128 {..}`            | `Unsupported`                                  | 需显式拆分合同                                 |
/// | `Float32` / `Float64`        | `F32` / `F64`                                  |                                                |
/// | `Boolean`                    | `Bool`                                         |                                                |
/// | `Character`                  | `Char`                                         |                                                |
/// | `Utf8`                       | `Utf8`                                         |                                                |
/// | `Utf16`                      | `Utf16`（当前视为 Unsupported）                | JS glue 轨暂无支持                             |
/// | `Array(T)`                   | `Array { element: schema(T) }`                 |                                                |
/// | `FixedArray { element, n }`  | `FixedArray { element: schema(element), n }`   |                                                |
/// | `Nullable(T)`                | `Option { inner: schema(T), box?: ... }`        | 标量载荷带装箱合同 |
/// | `Tuple([...])`               | `Tuple { fields: [...] }`                      |                                                |
/// | `Named(id)`                  | `NamedAggregate { name, module_path, generic_args: [] }` |                       |
/// | `Apply(Base, args)`          | `NamedAggregate { name, module_path, schema(args)}`|                                        |
/// | `Function` / `TraitObject`   | `Unsupported`                                  | 不能跨 ABI 边界                                |
/// | `Union([...])`               | `Unsupported`                                  | ABI 合同未定义                                 |
pub fn abi_schema_for_type(ty: &NyarType) -> WasmAbiType {
    // 结构推导与 ovm 共用 `crate::abi_structure`，本函数只做 wasm 物理投影。
    project_structure(&crate::abi_structure::abi_structure_for_type(ty), None)
}

/// wasm 物理投影：把目标中立结构投影成 `WasmAbiType`。
///
/// 目标差异只在这里体现：字段 accessor / `struct.new` / 数组操作导出名由冻结
/// 布局身份派生；结构本身（字段、槽位、变体 tag）来自共享结构层。
fn project_structure(structure: &crate::abi_structure::AbiStructure, layouts: Option<&AggregateLayoutPlan>) -> WasmAbiType {
    use crate::abi_structure::AbiStructure as S;
    match structure {
        S::Unit => WasmAbiType::Unit,
        S::I32 => WasmAbiType::I32,
        S::I64 => WasmAbiType::I64,
        S::F32 => WasmAbiType::F32,
        S::F64 => WasmAbiType::F64,
        S::Bool => WasmAbiType::Bool,
        S::Char => WasmAbiType::Char,
        S::Utf8 => WasmAbiType::Utf8,
        S::Array { element, element_key } => WasmAbiType::Array {
            element: Box::new(project_structure(element, layouts)),
            ops: array_ops_export_names_from_key(element_key),
            container: None,
        },
        S::FixedArray { element, length } => {
            WasmAbiType::FixedArray { element: Box::new(project_structure(element, layouts)), length: *length }
        }
        S::Option { inner } => {
            let inner = Box::new(project_structure(inner, layouts));
            let box_contract = option_box_contract(&inner).map(Box::new);
            WasmAbiType::Option { inner, box_contract }
        }
        S::Tuple { fields } => {
            WasmAbiType::Tuple { fields: fields.iter().map(|field| project_structure(field, layouts)).collect() }
        }
        S::Aggregate(aggregate) => match aggregate.layout {
            // 身份模式（无布局事实）：只给身份，不给字段桥与构造合同，
            // 宿主必须失败闭合而不是按名字猜字段。
            None => WasmAbiType::NamedAggregate {
                name: aggregate.name.clone(),
                module_path: aggregate.module_path.clone(),
                generic_args: aggregate.generic_args.iter().map(|arg| project_structure(arg, layouts)).collect(),
                fields: Vec::new(),
                constructor: None,
            },
            Some(layout_id) => {
                let Some(aggregate_layout) =
                    layouts.and_then(|plan| plan.layouts.iter().find(|item| item.id == layout_id))
                else {
                    return WasmAbiType::Unsupported { reason: format!("layout `{layout_id}` 未注册，无法派生字段访问合同") };
                };
                let mut fields = Vec::with_capacity(aggregate.fields.len());
                for (index, field) in aggregate.fields.iter().enumerate() {
                    let field_schema = project_structure(&field.ty, layouts);
                    if let Some(reason) = field_schema.unsupported_reason() {
                        return WasmAbiType::Unsupported {
                            reason: format!(
                                "聚合 `{}::{}` 字段 `{}` 没有 Wasm ABI 合同: {reason}",
                                aggregate_layout.namespace, aggregate_layout.name, field.name
                            ),
                        };
                    }
                    fields.push(WasmAbiField {
                        name: field.name.clone(),
                        ty: field_schema,
                        accessor: field_accessor_export_name(aggregate_layout, index),
                    });
                }
                WasmAbiType::NamedAggregate {
                    name: aggregate.name.clone(),
                    module_path: aggregate.module_path.clone(),
                    generic_args: Vec::new(),
                    fields,
                    constructor: Some(aggregate_constructor_export_name(aggregate_layout)),
                }
            }
        },
        S::Sequence(sequence) => {
            let Some(aggregate_layout) =
                layouts.and_then(|plan| plan.layouts.iter().find(|item| item.id == sequence.layout))
            else {
                return WasmAbiType::Unsupported {
                    reason: format!("layout `{}` 未注册，无法派生序列构造合同", sequence.layout),
                };
            };
            let element = project_structure(&sequence.element, layouts);
            if let Some(reason) = element.unsupported_reason() {
                return WasmAbiType::Unsupported {
                    reason: format!(
                        "序列容器 `{}::{}` 的元素类型没有 Wasm ABI 合同: {reason}",
                        aggregate_layout.namespace, aggregate_layout.name
                    ),
                };
            }
            let length_type = project_structure(&sequence.length_ty, layouts);
            if let Some(reason) = length_type.unsupported_reason() {
                return WasmAbiType::Unsupported {
                    reason: format!(
                        "序列容器 `{}::{}` 的计数字段没有 Wasm ABI 合同: {reason}",
                        aggregate_layout.namespace, aggregate_layout.name
                    ),
                };
            }
            WasmAbiType::Array {
                element: Box::new(element),
                ops: array_ops_export_names_from_key(&sequence.element_key),
                container: Some(Box::new(WasmAbiSequence {
                    constructor: aggregate_constructor_export_name(aggregate_layout),
                    items_field_index: sequence.items_slot,
                    length_field_index: sequence.length_slot,
                    items_accessor: field_accessor_export_name(aggregate_layout, sequence.items_slot),
                    length_accessor: field_accessor_export_name(aggregate_layout, sequence.length_slot),
                    field_count: sequence.field_count,
                    length_type: Box::new(length_type),
                })),
            }
        }
        S::InstanceHandle { name, module_path, .. } => {
            WasmAbiType::InstanceHandle { name: name.clone(), module_path: module_path.clone() }
        }
        S::Unite { name, variants, .. } => WasmAbiType::Sum {
            name: name.clone(),
            variants: variants.iter().map(|variant| WasmAbiSumVariant { name: variant.name.clone(), tag: variant.tag }).collect(),
            tag_accessor: sum_tag_accessor_name(name, true, variants),
        },
        S::Unsupported { reason } => WasmAbiType::Unsupported { reason: reason.clone() },
    }
}

/// 携带冻结布局事实的 `NyarType` → `WasmAbiType` 映射：`Nominal` 从已注册布局
/// 派生字段访问合同（字段名、字段类型 schema、accessor 导出名）。
///
/// 字段 schema 递归派生；任一字段没有 Wasm ABI 合同时整个聚合降级为
/// `Unsupported`（结构化失败，不静默跳过字段）。`Nominal` 携带的布局未注册
/// 时同样直接失败——预发射身份门禁已经拒绝那种形态，这里不从无身份事实派生桥。
pub fn abi_schema_for_type_with_layouts(
    ty: &NyarType,
    layouts: &AggregateLayoutPlan,
    layout_by_type: &BTreeMap<NyarType, LayoutId>,
    sums: &BTreeMap<NyarType, nyar_types::SumTypeLayout>,
) -> WasmAbiType {
    project_structure(
        &crate::abi_structure::abi_structure_for_type_with_layouts(ty, layouts, layout_by_type, sums),
        Some(layouts),
    )
}

/// `struct.new` 包装导出的确定性名字：由冻结布局身份派生。
pub fn aggregate_constructor_export_name(layout: &AggregateLayout) -> String {
    let digest = combined_content_hash(&[&layout_identity_facts(layout)]);
    let suffix = digest.get(..16).unwrap_or(&digest);
    let sanitize = |text: &str| text.chars().map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' }).collect::<String>();
    format!("nyar$new${}${}", sanitize(&layout.name), suffix)
}

/// `Option<标量>` 的 grain 合同派生：只按**载荷物理载体**决定，不看类型名。
///
/// 与后端已注册的 box struct 一一对应（`entry.rs` 的 `[i32]` / `[i64]` / `[f64]`）：
/// - `i32` 载体：`I32` / `Bool` / `Char` / `Utf8`（JS glue 里 UTF-8 是宿主 i32 句柄）；
/// - `i64` 载体：`I64`；
/// - `f64` 载体：`F64`。
///
/// 引用载体（`Array` / `FixedArray` / `Tuple` / `NamedAggregate` / `Sum` / 嵌套 `Option`）
/// 本身就是可空 anyref 槽里的值，不需要合同；其余标量载体（`F32`）没有目标合同，
/// 后端也没有注册对应 box struct，返回 `None`，由宿主结构化失败而不是现场造表示。
pub fn option_box_contract(inner: &WasmAbiType) -> Option<WasmAbiOptionBox> {
    let payload = match inner {
        WasmAbiType::I32 | WasmAbiType::Bool | WasmAbiType::Char | WasmAbiType::Utf8 => "i32",
        WasmAbiType::I64 => "i64",
        WasmAbiType::F64 => "f64",
        _ => return None,
    };
    let digest = combined_content_hash(&[&format!("option-box:{payload}")]);
    let suffix = digest.get(..16).unwrap_or(&digest).to_string();
    Some(WasmAbiOptionBox {
        new: format!("nyar$option$new${payload}${suffix}"),
        get: format!("nyar$option$get${payload}${suffix}"),
        payload: payload.to_string(),
    })
}

/// 物理数组操作导出的确定性名字：由元素类型的结构化拼写派生。
pub fn array_ops_export_names(element: &NyarType) -> WasmAbiArrayOps {
    array_ops_export_names_from_key(&crate::abi_structure::canonical_type_spelling(element))
}

/// 同 [`array_ops_export_names`]，但使用共享结构层的 canonical 元素拼写种子。
fn array_ops_export_names_from_key(key: &str) -> WasmAbiArrayOps {
    let digest = combined_content_hash(&[key]);
    let suffix = digest.get(..16).unwrap_or(&digest).to_string();
    WasmAbiArrayOps {
        length: format!("nyar$array$length${suffix}"),
        get: format!("nyar$array$get${suffix}"),
        new: format!("nyar$array$new${suffix}"),
        set: format!("nyar$array$set${suffix}"),
    }
}

fn layout_identity_facts(layout: &AggregateLayout) -> String {
    let mut facts = String::new();
    facts.push_str(&layout.namespace);
    facts.push('\n');
    facts.push_str(&layout.name);
    facts.push('\n');
    for item in &layout.fields {
        facts.push_str(&format!("{}:{:?}@{}+{}\n", item.name, item.ty, item.offset, item.size));
    }
    facts
}

/// 字段 accessor 的确定性导出名：只由冻结布局身份（命名空间、类型名、
/// 全部字段的名/型/偏移/尺寸）派生，不按类型名识别，不查询任何注解。
///
/// 同一布局在任意导出位置派生同一名字；字段形状不同的布局永不撞名。
pub fn field_accessor_export_name(layout: &AggregateLayout, field_index: usize) -> String {
    let field = &layout.fields[field_index];
    let mut facts = String::new();
    facts.push_str(&layout.namespace);
    facts.push('\n');
    facts.push_str(&layout.name);
    facts.push('\n');
    for item in &layout.fields {
        facts.push_str(&format!("{}:{:?}@{}+{}\n", item.name, item.ty, item.offset, item.size));
    }
    let digest = combined_content_hash(&[&facts]);
    let suffix = digest.get(..16).unwrap_or(&digest);
    let sanitize = |text: &str| text.chars().map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' }).collect::<String>();
    format!("nyar$field${}${}${}", sanitize(&layout.name), sanitize(&field.name), suffix)
}

/// unite 变体 tag accessor 的确定性导出名：只由冻结 sum 身份（类型名、
/// `is_unite`、全部变体名与判别 tag）派生，不按类型名或下标猜测。
///
/// 同一 sum 在任意导出位置派生同一名字；变体形状不同的 unite 永不撞名。
pub fn sum_tag_accessor_export_name(sum: &nyar_types::SumTypeLayout) -> String {
    sum_tag_accessor_name(&sum.name, sum.is_unite, &crate::abi_structure::sum_variants_of(sum))
}

/// 由共享结构身份派生 unite tag accessor 名字；wasm 是唯一需要这个物理名字的目标。
fn sum_tag_accessor_name(name: &str, is_unite: bool, variants: &[crate::abi_structure::AbiVariant]) -> String {
    let facts = crate::abi_structure::sum_identity_facts(name, is_unite, variants);
    let digest = combined_content_hash(&[&facts]);
    let suffix = digest.get(..16).unwrap_or(&digest);
    let sanitize = |text: &str| text.chars().map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' }).collect::<String>();
    format!("nyar$sum$tag${}${}", sanitize(name), suffix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyar_types::{FieldLayout, Identifier};

    fn nyar(s: &str) -> NyarType {
        NyarType::from_surface_primitive(s).expect("surface primitive")
    }

    #[test]
    fn scalar_round_trip() {
        assert_eq!(abi_schema_for_type(&NyarType::Unit), WasmAbiType::Unit);
        assert_eq!(abi_schema_for_type(&nyar("bool")), WasmAbiType::Bool);
        assert_eq!(abi_schema_for_type(&nyar("i32")), WasmAbiType::I32);
        assert_eq!(abi_schema_for_type(&nyar("i64")), WasmAbiType::I64);
        assert_eq!(abi_schema_for_type(&nyar("u64")), WasmAbiType::I64);
        assert_eq!(abi_schema_for_type(&nyar("f64")), WasmAbiType::F64);
        assert_eq!(abi_schema_for_type(&nyar("f32")), WasmAbiType::F32);
        assert_eq!(abi_schema_for_type(&nyar("utf8")), WasmAbiType::Utf8);
    }

    #[test]
    fn array_element_preserved() {
        let element = nyar("i64");
        let arr = NyarType::Array(Box::new(element.clone()));
        assert_eq!(
            abi_schema_for_type(&arr),
            WasmAbiType::Array { element: Box::new(WasmAbiType::I64), ops: array_ops_export_names(&element), container: None }
        );
    }

    #[test]
    fn nullable_becomes_option() {
        let opt = NyarType::Nullable(Box::new(nyar("i32")));
        let WasmAbiType::Option { inner, box_contract } = abi_schema_for_type(&opt) else {
            panic!("Nullable(i32) 必须投影为 Option")
        };
        assert_eq!(*inner, WasmAbiType::I32);
        let contract = box_contract.expect("标量载荷必须带装箱合同");
        assert_eq!(contract.payload, "i32");
        assert!(contract.new.starts_with("nyar$option$new$i32$"));
        assert!(contract.get.starts_with("nyar$option$get$i32$"));
    }

    #[test]
    fn nullable_handle_payload_has_no_box_contract() {
        let opt = NyarType::Nullable(Box::new(NyarType::Array(Box::new(nyar("i64")))));
        let WasmAbiType::Option { box_contract, .. } = abi_schema_for_type(&opt) else {
            panic!("Nullable([i64]) 必须投影为 Option")
        };
        assert!(box_contract.is_none(), "引用载荷直接复用可空槽引用，不得要求装箱");
    }

    #[test]
    fn nullable_i64_payload_uses_i64_box_contract() {
        let contract = option_box_contract(&WasmAbiType::I64).expect("i64 载荷可装箱");
        assert_eq!(contract.payload, "i64");
        assert!(contract.new.starts_with("nyar$option$new$i64$"));
    }

    #[test]
    fn nullable_f64_payload_uses_f64_box_contract() {
        let contract = option_box_contract(&WasmAbiType::F64).expect("f64 载荷有 `[f64]` box 合同");
        assert_eq!(contract.payload, "f64");
        assert!(contract.new.starts_with("nyar$option$new$f64$"));
        assert!(contract.get.starts_with("nyar$option$get$f64$"));
    }

    #[test]
    fn nullable_f32_payload_has_no_box_contract() {
        assert!(option_box_contract(&WasmAbiType::F32).is_none(), "后端未注册 [f32] box，不得发明合同");
    }

    #[test]
    fn two_sum_generic_application_uses_declaration_qualified_base() {
        // two-sum 参数 ArrayList<i64>，i64 返回 ArrayList<i64>。
        // ArrayList<i64> 在 NyarType 层是 Apply(Named(...), [Integer64])。
        // 上游 `type_surface_spellings` 只在**声明** TypeId 上登记拼写；实例 TypeId 自身
        // 没有单独 spelling，`lower_type` 回落到声明取 qualified 基类型，因此这里必须
        // 用真实拼写 `std::collections::ArrayList`，不能再用裸简单名。
        let arraylist_i64 = NyarType::Apply(Box::new(NyarType::Named(Identifier::new("std::collections::ArrayList"))), vec![nyar("i64")]);
        let schema = abi_schema_for_type(&arraylist_i64);
        match schema {
            WasmAbiType::NamedAggregate { name, module_path, generic_args, .. } => {
                assert_eq!(name, "ArrayList");
                assert_eq!(module_path, "std::collections");
                assert_eq!(generic_args, vec![WasmAbiType::I64]);
            }
            other => panic!("Expected NamedAggregate, got {other:?}"),
        }
    }

    #[test]
    fn same_named_aggregates_in_different_namespaces_stay_distinct() {
        // 同名、同泛型实参但不同 namespace 的两个 aggregate 必须产生不同
        // NamedAggregate：宿主注册表按 module_path + name + generic_args 区分，
        // module_path 相同会把两者错误地命中同一条 marshal 合同。
        let alpha = abi_schema_for_type(&NyarType::Apply(Box::new(NyarType::Named(Identifier::new("alpha::Box"))), vec![nyar("i64")]));
        let beta = abi_schema_for_type(&NyarType::Apply(Box::new(NyarType::Named(Identifier::new("beta::Box"))), vec![nyar("i64")]));
        match (&alpha, &beta) {
            (
                WasmAbiType::NamedAggregate { name: left_name, module_path: left_path, generic_args: left_args, .. },
                WasmAbiType::NamedAggregate { name: right_name, module_path: right_path, generic_args: right_args, .. },
            ) => {
                assert_eq!(left_name, right_name);
                assert_eq!(left_args, right_args);
                assert_eq!(left_path, "alpha");
                assert_eq!(right_path, "beta");
                assert_ne!(left_path, right_path);
            }
            other => panic!("Expected two NamedAggregate, got {other:?}"),
        }
        assert_ne!(alpha, beta);
    }

    #[test]
    fn module_path_extracted_from_qualified_identifier() {
        // 如果上游给了完整路径 "std::collections::ArrayList"
        let qualified = NyarType::Apply(Box::new(NyarType::Named(Identifier::new("std::collections::ArrayList"))), vec![nyar("i64")]);
        let schema = abi_schema_for_type(&qualified);
        match schema {
            WasmAbiType::NamedAggregate { name, module_path, .. } => {
                assert_eq!(name, "ArrayList");
                assert_eq!(module_path, "std::collections");
            }
            other => panic!("Expected NamedAggregate, got {other:?}"),
        }
    }

    #[test]
    fn unsupported_variants_rejected() {
        let void = abi_schema_for_type(&NyarType::Bottom);
        assert!(void.unsupported_reason().is_some());

        let fn_ty = abi_schema_for_type(&NyarType::Function(Box::new(nyar_fn())));
        assert!(fn_ty.unsupported_reason().is_some());
    }

    #[test]
    fn serde_roundtrip_scalar() {
        let schema = WasmAbiType::I64;
        let json = serde_json::to_string(&schema).unwrap();
        assert!(json.contains("\"kind\":\"I64\""), "got: {json}");
        let back: WasmAbiType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, schema);
    }

    #[test]
    fn serde_roundtrip_array() {
        let schema = WasmAbiType::Array {
            element: Box::new(WasmAbiType::I64),
            ops: array_ops_export_names(&nyar("i64")),
            container: None,
        };
        let json = serde_json::to_string(&schema).unwrap();
        let back: WasmAbiType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, schema);
    }

    #[test]
    fn serde_roundtrip_named_aggregate() {
        let schema = WasmAbiType::NamedAggregate {
            name: "ArrayList".to_string(),
            module_path: "std::collections".to_string(),
            generic_args: vec![WasmAbiType::I64],
            fields: Vec::new(),
            constructor: None,
        };
        let json = serde_json::to_string(&schema).unwrap();
        assert!(json.contains("ArrayList"), "got: {json}");
        // 空字段合同不进入 JSON：旧形状消费者不受影响。
        assert!(!json.contains("fields"), "got: {json}");
        let back: WasmAbiType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, schema);
    }

    fn config_layout() -> AggregateLayout {
        AggregateLayout {
            id: 7,
            name: "ConfigArtifact".to_string(),
            namespace: "app".to_string(),
            key: None,
            storage: nyar_types::StorageKind::Value,
            size: 8,
            align: 4,
            fields: vec![
                FieldLayout { name: "target".to_string(), ty: nyar("i32"), offset: 0, size: 4, align: 4 },
                FieldLayout { name: "entry".to_string(), ty: nyar("i32"), offset: 4, size: 4, align: 4 },
            ],
        }
    }

    #[test]
    fn nominal_with_registered_layout_derives_field_accessors() {
        let layout = config_layout();
        let mut plan = AggregateLayoutPlan::default();
        plan.layouts.push(layout);
        let ty = NyarType::Nominal { layout: 7, spelling: Identifier::new("app::ConfigArtifact") };
        let schema = abi_schema_for_type_with_layouts(&ty, &plan, &BTreeMap::new(), &BTreeMap::new());
        match schema {
            WasmAbiType::NamedAggregate { name, module_path, fields, .. } => {
                assert_eq!(name, "ConfigArtifact");
                assert_eq!(module_path, "app");
                assert_eq!(fields.len(), 2);
                assert_eq!(fields[0].name, "target");
                assert_eq!(fields[0].ty, WasmAbiType::I32);
                assert_eq!(fields[1].name, "entry");
                assert!(fields[0].accessor.starts_with("nyar$field$ConfigArtifact$target$"), "got: {}", fields[0].accessor);
                assert_ne!(fields[0].accessor, fields[1].accessor);
            }
            other => panic!("Expected NamedAggregate, got {other:?}"),
        }
    }

    #[test]
    fn reference_nominal_with_registered_layout_derives_instance_handle() {
        // `class` 是引用存储：宿主拿到的是同一 GC 引用，不是字段快照。
        // schema 只保留名义身份，不导出字段访问合同；字段级复制会让
        // `mut self` 方法改在副本上，跨调用丢状态。
        let mut layout = config_layout();
        layout.name = "Counter".to_string();
        layout.namespace = "demo".to_string();
        layout.storage = nyar_types::StorageKind::Reference;
        let mut plan = AggregateLayoutPlan::default();
        plan.layouts.push(layout);
        let ty = NyarType::Nominal { layout: 7, spelling: Identifier::new("demo::Counter") };
        let schema = abi_schema_for_type_with_layouts(&ty, &plan, &BTreeMap::new(), &BTreeMap::new());
        assert_eq!(
            schema,
            WasmAbiType::InstanceHandle { name: "Counter".to_string(), module_path: "demo".to_string() },
            "引用存储名义实例必须投影为不透明实例句柄"
        );
        let json = serde_json::to_string(&schema).unwrap();
        assert!(json.contains("\"kind\":\"InstanceHandle\""), "got: {json}");
        assert!(!json.contains("fields"), "实例句柄不得携带字段合同: {json}");
    }

    #[test]
    fn field_accessor_names_are_deterministic_and_layout_bound() {
        let layout = config_layout();
        let first = field_accessor_export_name(&layout, 0);
        let second = field_accessor_export_name(&config_layout(), 0);
        assert_eq!(first, second, "同一布局身份必须派生同一 accessor 名");
        let mut shifted = config_layout();
        shifted.fields[1].offset += 4;
        assert_ne!(first, field_accessor_export_name(&shifted, 0), "布局事实变化必须改变身份摘要");
    }

    #[test]
    fn nominal_with_unregistered_layout_fails_closed() {
        let plan = AggregateLayoutPlan::default();
        let ty = NyarType::Nominal { layout: 99, spelling: Identifier::new("app::Ghost") };
        let schema = abi_schema_for_type_with_layouts(&ty, &plan, &BTreeMap::new(), &BTreeMap::new());
        assert!(schema.unsupported_reason().is_some(), "未注册 layout 不得派生字段桥");
    }

    #[test]
    fn aggregate_field_without_abi_contract_fails_the_whole_aggregate() {
        // i128 字段没有 Wasm 载体：整个聚合 schema 降级为 Unsupported，
        // 结构化失败，不静默跳过字段。
        let layout = AggregateLayout {
            id: 8,
            name: "Heavy".to_string(),
            namespace: "app".to_string(),
            key: None,
            storage: nyar_types::StorageKind::Value,
            size: 16,
            align: 8,
            fields: vec![FieldLayout { name: "big".to_string(), ty: nyar("i128"), offset: 0, size: 16, align: 8 }],
        };
        let mut plan = AggregateLayoutPlan::default();
        plan.layouts.push(layout);
        let ty = NyarType::Nominal { layout: 8, spelling: Identifier::new("app::Heavy") };
        let schema = abi_schema_for_type_with_layouts(&ty, &plan, &BTreeMap::new(), &BTreeMap::new());
        let reason = schema.unsupported_reason().expect("i128 字段必须使整个聚合失败闭合");
        assert!(reason.contains("big"), "got: {reason}");
    }

    fn nyar_fn() -> nyar_types::NyarFunctionType {
        nyar_types::NyarFunctionType { params: vec![nyar("i32")], return_type: nyar("i32") }
    }
}

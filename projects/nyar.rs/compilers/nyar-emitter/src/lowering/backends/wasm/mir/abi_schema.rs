//! 阶段 B：唯一的 Valkyrie → Wasm ABI 合同。
//!
//! 本模块为 `nyar.library_invoke` JSON section 提供**结构化、可序列化**的
//! ABI schema 枚举，替代 `library_glue.rs` 里硬编码的 `abi_kind_for_type -> Option<&'static str>`。
//!
//! 设计约束（架构师硬性要求）：
//! - 禁止按容器名称硬编码 ABI（`is_list_i64`、`is_arraylist` 一律禁止）
//! - 禁止按函数名判断导出入口
//! - 禁止缺 layout 时默认布局
//! - 每个 NyarType 必须生成语义 identity；不认识的变体显式返回 `Unsupported`
//!
//! 下游消费：
//! - `library_glue.rs`：序列化为 `nyar.library_invoke` JSON section
//! - `wasm_js_glue`：按结构化 schema 生成 JS marshal/demarshal 代码
//! - 其他宿主（CLR / JVM / Native）：按同一份 schema 生成各自绑定

use nyar_types::{Identifier, NyarType};
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
    Array {
        /// 元素类型的 ABI schema。
        element: Box<WasmAbiType>,
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
    Option {
        /// 内部非空值类型。
        inner: Box<WasmAbiType>,
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
        /// 可读原因，供 `library_glue` 报错时展示。
        reason: String,
    },
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
/// | `Nullable(T)`                | `Option { inner: schema(T) }`                  |                                                |
/// | `Tuple([...])`               | `Tuple { fields: [...] }`                      |                                                |
/// | `Named(id)`                  | `NamedAggregate { name, module_path, generic_args: [] }` |                       |
/// | `Apply(Base, args)`          | `NamedAggregate { name, module_path, schema(args)}`|                                        |
/// | `Function` / `TraitObject`   | `Unsupported`                                  | 不能跨 ABI 边界                                |
/// | `Union([...])`               | `Unsupported`                                  | ABI 合同未定义                                 |
pub fn abi_schema_for_type(ty: &NyarType) -> WasmAbiType {
    match ty {
        NyarType::Bottom => WasmAbiType::Unsupported { reason: "`void` / bottom type 不能出现在库导出 ABI 位置".to_string() },
        NyarType::Unit => WasmAbiType::Unit,

        NyarType::Boolean => WasmAbiType::Bool,
        NyarType::Character => WasmAbiType::Char,

        // 所有 <= 32-bit 整数统一映射到 I32 carrier；signedness 在 Wasm 层通过
        // 有符号/无符号指令区分，ABI carrier 本身无符号属性。
        NyarType::Integer8 { .. } | NyarType::Integer16 { .. } | NyarType::Integer32 { .. } => WasmAbiType::I32,
        NyarType::Integer64 { .. } => WasmAbiType::I64,
        NyarType::Integer128 { .. } => {
            WasmAbiType::Unsupported { reason: "Integer128 没有原生 Wasm carrier，需要显式拆分合同".to_string() }
        }

        NyarType::Float32 => WasmAbiType::F32,
        NyarType::Float64 => WasmAbiType::F64,

        NyarType::Utf8 => WasmAbiType::Utf8,
        NyarType::Utf16 => WasmAbiType::Unsupported { reason: "Utf16 在当前 JS glue 轨没有稳定的宿主 ABI 合同".to_string() },

        NyarType::Array(element) => WasmAbiType::Array { element: Box::new(abi_schema_for_type(element)) },
        NyarType::FixedArray { element, length } => {
            WasmAbiType::FixedArray { element: Box::new(abi_schema_for_type(element)), length: *length }
        }

        NyarType::Nullable(inner) => WasmAbiType::Option { inner: Box::new(abi_schema_for_type(inner)) },

        NyarType::Tuple(fields) => WasmAbiType::Tuple { fields: fields.iter().map(abi_schema_for_type).collect() },

        // 名义类型：把 Identifier 展开成简单名字 + 模块路径；generic_args 为空。
        NyarType::Named(id) => {
            WasmAbiType::NamedAggregate { name: simple_name_of(id), module_path: module_path_of(id), generic_args: Vec::new() }
        }

        // 泛型应用：`Apply(Base, [T1, T2, ...])`
        // 把 Base（通常是 Named）的语义名字和模块路径提取出来，递归映射泛型参数。
        NyarType::Apply(base, args) => {
            let (name, module_path, base_generic_args) = match base.as_ref() {
                NyarType::Named(id) => (simple_name_of(id), module_path_of(id), Vec::new()),
                NyarType::Apply(_, _) => {
                    // 嵌套 Apply：展平到最顶层 Named
                    flatten_apply_identity(base.as_ref())
                }
                other => {
                    return WasmAbiType::Unsupported {
                        reason: format!("Apply 的 Base 必须是 Named 或 Apply，实际是 {other:?}——无法提取语义 identity"),
                    };
                }
            };
            let mut generic_args: Vec<WasmAbiType> = base_generic_args.iter().map(|t| abi_schema_for_type(t)).collect();
            generic_args.extend(args.iter().map(abi_schema_for_type));
            WasmAbiType::NamedAggregate { name, module_path, generic_args }
        }

        // 函数类型不能跨 ABI 边界（宿主和 Wasm 的函数指针合同尚未定义）。
        NyarType::Function(_) => WasmAbiType::Unsupported { reason: "函数类型不能出现在库导出 ABI 位置".to_string() },

        // 动态分发的 trait object 没有稳定的 Wasm carrier。
        NyarType::TraitObject(_) => {
            WasmAbiType::Unsupported { reason: "TraitObject（dynamic dispatch）没有稳定的 Wasm carrier".to_string() }
        }

        // 匿名 tagged union 的 ABI 合同（tag 放在哪？payload 如何对齐？）未定义。
        NyarType::Union(_) => WasmAbiType::Unsupported { reason: "匿名 union 的 Wasm ABI 合同尚未定义".to_string() },
    }
}

/// 从 `Identifier` 提取简单名（丢掉模块路径前缀）。
fn simple_name_of(id: &Identifier) -> String {
    let full = id.as_str();
    full.rsplit_once("::")
        .map(|(_, last)| last.to_string())
        .or_else(|| full.rsplit_once('.').map(|(_, last)| last.to_string()))
        .unwrap_or_else(|| full.to_string())
}

/// 从 `Identifier` 提取模块路径（丢掉最终简单名）。
///
/// - `"std::collections::ArrayList"` → `"std::collections"`
/// - `"std.collections.ArrayList"` → `"std.collections"`
/// - `"ArrayList"` → `""`（简单名无路径前缀）
///
/// 空返回值只在**上游如实无路径事实**时出现：canonical 已为每个 struct 声明
/// 登记 `MirStruct::qualified_name()`；没有路径的只剩无 namespace 的 sum 声明
/// 和顶层（无名空间）声明。不要用简单名拼 package path 来填补空值。
fn module_path_of(id: &Identifier) -> String {
    let full = id.as_str();
    if let Some((prefix, _)) = full.rsplit_once("::") {
        prefix.to_string()
    }
    else if let Some((prefix, _)) = full.rsplit_once('.') {
        prefix.to_string()
    }
    else {
        String::new()
    }
}

/// 把嵌套的 `Apply(Apply(Named("A"), ...), ...)` 展平，提取最顶层
/// 语义名、模块路径和最底层的 generic_args 残留（极罕见场景）。
fn flatten_apply_identity(mut ty: &NyarType) -> (String, String, Vec<NyarType>) {
    let mut accumulated: Vec<NyarType> = Vec::new();
    loop {
        match ty {
            NyarType::Named(id) => return (simple_name_of(id), module_path_of(id), accumulated),
            NyarType::Apply(base, args) => {
                accumulated.extend(args.iter().cloned());
                ty = base.as_ref();
            }
            other => {
                return (format!("<unexpected:{other:?}>"), String::new(), accumulated);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let arr = NyarType::Array(Box::new(nyar("i64")));
        assert_eq!(abi_schema_for_type(&arr), WasmAbiType::Array { element: Box::new(WasmAbiType::I64) });
    }

    #[test]
    fn nullable_becomes_option() {
        let opt = NyarType::Nullable(Box::new(nyar("i32")));
        assert_eq!(abi_schema_for_type(&opt), WasmAbiType::Option { inner: Box::new(WasmAbiType::I32) });
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
            WasmAbiType::NamedAggregate { name, module_path, generic_args } => {
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
                WasmAbiType::NamedAggregate { name: left_name, module_path: left_path, generic_args: left_args },
                WasmAbiType::NamedAggregate { name: right_name, module_path: right_path, generic_args: right_args },
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
        let schema = WasmAbiType::Array { element: Box::new(WasmAbiType::I64) };
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
        };
        let json = serde_json::to_string(&schema).unwrap();
        assert!(json.contains("ArrayList"), "got: {json}");
        let back: WasmAbiType = serde_json::from_str(&json).unwrap();
        assert_eq!(back, schema);
    }

    fn nyar_fn() -> nyar_types::NyarFunctionType {
        nyar_types::NyarFunctionType { params: vec![nyar("i32")], return_type: nyar("i32") }
    }
}

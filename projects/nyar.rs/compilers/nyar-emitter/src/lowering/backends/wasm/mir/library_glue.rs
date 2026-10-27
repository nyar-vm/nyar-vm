//! 库模式 JSON ABI：消费 `WasmAbiType` 结构化合同，序列化到
//! `nyar.library_invoke` 自定义段。
//!
//! # 历史
//!
//! v1（本阶段之前）：`abi_kind_for_type` 只认 `Integer64 → Some("i64")`，
//! 其余全部 None。任何带 `ArrayList<T>` / `Option<T>` / `utf8` 的导出
//! 都会被拒绝。
//!
//! v2：消费 `super::abi_schema::WasmAbiType`，完整序列化
//! NyarType 变体到 JSON。禁止硬编码、禁止缺 layout 时默认布局。
//!
//! v3：增加 `glue_entries` 数组，为下游 JS glue consumer
//! 提供 aggregate 的语义身份 skeleton。`export_name` 暂留占位，
//! 等 Stream C 确认 valkyrie.v 的具体 Wasm 导出后补全。
//!
//! v5：`nyar.library_invoke` 顶层新增 `abi_digest`——对 `exports` 与
//! `glue_entries` 的规范化序列化（对象键排序）做 SHA-256；同一 ABI 事实恒得
//! 同一 digest。sequence helper 合成不再按类型名特判，改为消费 plan 冻结的
//! aggregate layout 结构事实（数组字段 + 元素 schema）。
//!
//! v6（当前阶段）：顶层新增 `version`（当前为 `1`）供跨仓消费者校验协议版本，
//! 并把 `version` 纳入 `abi_digest` 的规范化输入；`construct` 的物理签名改为
//! 容量/长度语义（`["i32"] → "i32"`），与 JS glue 的 `construct(arr.length)`
//! 一致；`validate_glue_entry_exports` 要求所有 operation 的 `export_name`
//! （含 `backend_synthetic`）都真实存在于 Wasm export section。
//!
//! # JSON 格式
//!
//! ```json
//! {
//!   "version": 1,
//!   "exports": {
//!     "solve": {
//!       "params": [
//!         {"kind": "NamedAggregate", "name": "ArrayList", "module_path": "std::collections", "generic_args": [{"kind": "I64"}]},
//!         {"kind": "I64"}
//!       ],
//!       "returns": {"kind": "NamedAggregate", "name": "ArrayList", "module_path": "std::collections", "generic_args": [{"kind": "I64"}]}
//!     }
//!   },
//!   "glue_entries": [
//!     {
//!       "kind": "NamedAggregate",
//!       "name": "ArrayList",
//!       "module_path": "std::collections",
//!       "generic_args": [{"kind": "I64"}],
//!       "operations": [],
//!       "source": "declaration_identity",
//!       "status": "pending_export_name"
//!     }
//!   ],
//!   "abi_digest": "sha256-<hex>"
//! }
//! ```
//!
//! # glue_entries 约束
//!
//! - **只**声明 `NamedAggregate` 的语义身份（name + module_path + generic_args）
//! - **不**硬编码任何特定 aggregate 有哪些 operations（那是 valkyrie.v stdlib 的事）
//! - 结构事实成立时由 backend 合成 sequence helper operations，`export_source`
//!   区分 `declared`（plan 已声明导出）与 `backend_synthetic`（backend 合成名）；
//!   两者都必须在 artifact 校验阶段真实存在于 Wasm export section，缺失即失败

use std::collections::BTreeSet;

use nyar_types::{AggregateLayoutPlan, NyarType, combined_content_hash};

use crate::{
    FragmentSubmission, Result,
    nyar_backend_wasi::{WasmBinaryModule, WasmPackageKind, WasmSection},
};

use super::abi_schema::{WasmAbiType, abi_schema_for_type};

/// `nyar.library_invoke` JSON schema 版本。
///
/// 跨仓消费者（如 `leetcode.v` 的 `abi.ts`）据此校验协议版本；版本变化同时会
/// 改变 [`abi_contract_digest`]。
const LIBRARY_INVOKE_SCHEMA_VERSION: u64 = 1;

/// 把函数的参数/返回类型提升为 `WasmAbiType` schema。
///
/// 任何 `Unsupported` 变体会立刻返回可读错误，**禁止**静默降级。
/// `instance_label` 仅用于错误信息（可以是 instance ID 的 Display 或公开名）。
fn schematize_function_abi(
    instance_label: &str,
    function: &crate::backend_plan_views::ExecutableFunction,
) -> Result<(Vec<WasmAbiType>, WasmAbiType)> {
    let mut params = Vec::with_capacity(function.param_types.len());
    for ty in &function.param_types {
        let schema = abi_schema_for_type(ty);
        if let Some(reason) = schema.unsupported_reason() {
            return Err(format!("库导出 `{instance_label}` 参数类型 `{ty}` 没有 Wasm ABI 合同: {reason}"));
        }
        params.push(schema);
    }

    let ret_schema = abi_schema_for_type(&function.return_type);
    if let Some(reason) = ret_schema.unsupported_reason() {
        return Err(format!("库导出 `{instance_label}` 返回类型 `{}` 没有 Wasm ABI 合同: {reason}", function.return_type));
    }
    Ok((params, ret_schema))
}

/// 递归扫描一个 `WasmAbiType` schema，收集所有（可能嵌套的）`NamedAggregate` 的
/// 完整 identity 字符串，用于去重生成 glue_entries。
///
/// identity key 格式：`{module_path}|{name}|{json_of_generic_args}`——
/// 同一个 name 在不同 module_path 下视为不同 aggregate；
/// 同一个 aggregate 但不同 generic_args 也视为不同 instance。
fn collect_named_aggregate_keys(schema: &WasmAbiType, out: &mut BTreeSet<String>) {
    match schema {
        WasmAbiType::NamedAggregate { name, module_path, generic_args } => {
            // 先递归 generic_args，收集嵌套 aggregate
            for arg in generic_args {
                collect_named_aggregate_keys(arg, out);
            }
            let generic_json = serde_json::to_string(generic_args).unwrap_or_default();
            let key = format!("{module_path}|{name}|{generic_json}");
            out.insert(key);
        }
        WasmAbiType::Array { element } => collect_named_aggregate_keys(element, out),
        WasmAbiType::FixedArray { element, .. } => collect_named_aggregate_keys(element, out),
        WasmAbiType::Option { inner } => collect_named_aggregate_keys(inner, out),
        WasmAbiType::Tuple { fields } => {
            for field in fields {
                collect_named_aggregate_keys(field, out);
            }
        }
        // 标量 / Unit / Unsupported 都不可能承载 NamedAggregate
        _ => {}
    }
}

/// 从 identity key 字符串还原 `(module_path, name, generic_args_json)`。
///
/// key 格式是 `{module_path}|{name}|{json}`——但 module_path 和 name
/// 本身可能不含 `|`（合法标识字符），所以按 `|` 拆成三段。
fn split_aggregate_key(key: &str) -> Option<(&str, &str, &str)> {
    let mut parts = key.splitn(3, '|');
    let module_path = parts.next()?;
    let name = parts.next()?;
    let generic_json = parts.next()?;
    Some((module_path, name, generic_json))
}

/// 把 `WasmAbiType` 标量映射到 Wasm carrier 名称字符串（lowercase）。
///
/// 用于合成 ArrayList helper export 的 param_carriers / return_carrier 字段。
/// 只有标量类型有稳定的 Wasm carrier；聚合 / 容器类型（Array / Option / Tuple / NamedAggregate）
/// 不参与 ArrayList element type 推断。
fn scalar_carrier_name(ty: &WasmAbiType) -> Option<&'static str> {
    match ty {
        WasmAbiType::I32 => Some("i32"),
        WasmAbiType::I64 => Some("i64"),
        WasmAbiType::F32 => Some("f32"),
        WasmAbiType::F64 => Some("f64"),
        // Bool / Char 在 Wasm 层统一用 i32 carrier
        WasmAbiType::Bool | WasmAbiType::Char => Some("i32"),
        // Utf8 在 JS-glue 轨用 i32 句柄
        WasmAbiType::Utf8 => Some("i32"),
        _ => None,
    }
}

/// 把 `WasmAbiType` 标量映射到 export_name 短名（lowercase，用于合成函数名）。
///
/// 与 `scalar_carrier_name` 不同——Bool/Char/Utf8 虽然 carrier 是 i32，但
/// export_name 的类型标识保留语义身份（bool / char / utf8），让下游消费者
/// 能区分同 carrier 不同语义的 element type。
fn scalar_export_tag(ty: &WasmAbiType) -> Option<&'static str> {
    match ty {
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

/// glue operation 的 export 来源标记。
///
/// - `declared`：`export_name` 由 `TargetExecutionPlan::wasm_export_names()`
///   显式声明，emit 阶段必须真实出现在 Wasm export section。
/// - `backend_synthetic`：backend 合成的 helper 名，plan 未声明。
const EXPORT_SOURCE_DECLARED: &str = "declared";
/// backend 合成、plan 未声明的 glue operation 来源标记。
const EXPORT_SOURCE_BACKEND_SYNTHETIC: &str = "backend_synthetic";

/// 规范化模块路径，抹平 `::` 与 `.` 的分隔差异。
fn normalize_module_path(path: &str) -> String {
    path.replace("::", ".")
}

/// 从 plan 冻结的 aggregate layout 里查找承载指定 ABI 元素类型的动态数组字段。
///
/// 这是「序列型聚合」的唯一结构事实来源：布局字段表里存在 `[T]` 字段，且 `T`
/// 的 ABI schema 等于当前实例的首个泛型实参。不按类型名特判。
///
/// 身份匹配优先用 `name` + `namespace`（`::` / `.` 归一）；当 ABI 侧没有
/// module_path 且多个同名布局都满足结构事实（歧义）时返回 `None`，不做猜测。
fn array_field_element<'a>(layouts: &'a AggregateLayoutPlan, name: &str, module_path: &str, element: &WasmAbiType) -> Option<&'a NyarType> {
    let normalized_module = normalize_module_path(module_path);
    let mut exact = None;
    let mut loose = Vec::new();
    for layout in &layouts.layouts {
        if layout.name != name {
            continue;
        }
        for field in &layout.fields {
            let NyarType::Array(field_element) = &field.ty
            else {
                continue;
            };
            if abi_schema_for_type(field_element) != *element {
                continue;
            }
            if !normalized_module.is_empty() && normalize_module_path(&layout.namespace) == normalized_module {
                exact = Some(field_element.as_ref());
                break;
            }
            loose.push(field_element.as_ref());
        }
        if exact.is_some() {
            break;
        }
    }
    exact.or_else(|| if loose.len() == 1 { Some(loose[0]) } else { None })
}

/// 合成 sequence helper export 名称的固定前缀。
const SEQUENCE_HELPER_PREFIX: &str = "$seq_";

/// 由 aggregate 的 module_path + name + element tag 派生稳定的 helper 名 stem。
///
/// 名称是 aggregate 实例的身份投影：不同 namespace 或不同元素类型不会得到同一个
/// helper 名，也不会让任意序列型结构自称 `ArrayList`。只保留字母数字与 `_`，其余
/// 字符（`::`、`.` 等）归一为 `_`。
fn sequence_helper_stem(module_path: &str, name: &str, tag: &str) -> String {
    let mut stem = String::from(SEQUENCE_HELPER_PREFIX);
    let normalized_module = normalize_module_path(module_path);
    if !normalized_module.is_empty() {
        push_identifier_fragment(&mut stem, &normalized_module);
        stem.push('_');
    }
    push_identifier_fragment(&mut stem, name);
    stem.push('_');
    push_identifier_fragment(&mut stem, tag);
    stem
}

/// 把输入中的非标识符字符替换为 `_`，追加到 `out`。
fn push_identifier_fragment(out: &mut String, input: &str) {
    out.extend(input.chars().map(|ch| if ch.is_alphanumeric() || ch == '_' { ch } else { '_' }));
}

/// 为结构事实成立的序列型 NamedAggregate 合成 intrinsic lowering 的 helper
/// export operations。
///
/// # 约束
///
/// - 是否合成只由 [`array_field_element`] 的结构事实决定：plan 的 aggregate
///   layout 必须声明 `[T]` 字段且 `T` 等于 `generic_args[0]` 的 ABI schema
/// - `generic_args[0]` 必须是标量（有 `scalar_carrier_name`）；容器类型跳过
/// - 按 Wasm backend lowering intrinsic 的 pattern 合成：construct / push / length / at
///
/// # 合成 pattern
///
/// 设 element type 的 carrier 为 `E`（如 `"i64"`）、export tag 为 `T`（如 `"i64"`），
/// `{id}` 由 aggregate 的 module_path + name + `T` 派生（见 [`sequence_helper_stem`]）：
///
/// | operation | export_name         | param_carriers    | return_carrier |
/// |-----------|---------------------|-------------------|----------------|
/// | construct | `$seq_{id}_new`     | `["i32"]`         | `i32`          |
/// | push      | `$seq_{id}_push`    | `["i32", E]`      | `void`         |
/// | length    | `$seq_{id}_len`     | `["i32"]`         | `i32`          |
/// | at        | `$seq_{id}_at`      | `["i32", "i32"]`  | `E`            |
///
/// construct 的第一个参数是容量/长度（与 JS glue 的 `construct(arr.length)` 一致），
/// 返回 `i32` 句柄；push 的第一个参数是句柄，第二个参数是元素。
///
/// `export_source` 按 plan 已声明导出派生：命中 `declared_exports` 记为
/// `declared`，否则记为 `backend_synthetic`。
fn synthesize_sequence_helper_operations(
    layouts: &AggregateLayoutPlan,
    name: &str,
    module_path: &str,
    generic_args: &[WasmAbiType],
    declared_exports: &BTreeSet<String>,
) -> Option<Vec<serde_json::Value>> {
    let element = generic_args.first()?;
    array_field_element(layouts, name, module_path, element)?;
    let carrier = scalar_carrier_name(element)?;
    let tag = scalar_export_tag(element)?;

    let export_source = |export_name: &str| {
        if declared_exports.contains(export_name) { EXPORT_SOURCE_DECLARED } else { EXPORT_SOURCE_BACKEND_SYNTHETIC }
    };

    let stem = sequence_helper_stem(module_path, name, tag);
    let new_export = format!("{stem}_new");
    let push_export = format!("{stem}_push");
    let len_export = format!("{stem}_len");
    let at_export = format!("{stem}_at");

    Some(vec![
        serde_json::json!({
            "operation": "construct",
            "export_name": new_export,
            "param_carriers": ["i32"],
            "return_carrier": "i32",
            "export_source": export_source(&new_export)
        }),
        serde_json::json!({
            "operation": "push",
            "export_name": push_export,
            "param_carriers": ["i32", carrier],
            "return_carrier": "void",
            "export_source": export_source(&push_export)
        }),
        serde_json::json!({
            "operation": "length",
            "export_name": len_export,
            "param_carriers": ["i32"],
            "return_carrier": "i32",
            "export_source": export_source(&len_export)
        }),
        serde_json::json!({
            "operation": "at",
            "export_name": at_export,
            "param_carriers": ["i32", "i32"],
            "return_carrier": carrier,
            "export_source": export_source(&at_export)
        }),
    ])
}

/// 把收集到的 NamedAggregate identity keys 渲染为 `glue_entries` JSON 数组。
///
/// # v3 → v4 变化
///
/// v3 版本 `operations` 永远是空数组，source/status 标记为
/// `"declaration_identity"` / `"pending_export_name"`——等 Stream C 回填。
///
/// v4：由于 intrinsic lowering 直接降低成 Wasm GC 指令（不是 Wasm 顶层 export），
/// wasm backend 自己为序列型 aggregate 合成 helper export operations：
/// - `source: "backend_synthetic"` 标记 operations 来自 backend lowering 合成
/// - `status: "ready"` 标记 export_name 已可被 JS glue 消费
/// - 结构事实不成立或 element 非标量的 aggregate 保持 v3 旧行为（operations 空）
///
/// v5：合成判定改为消费 plan 的 aggregate layout 结构事实，不再按类型名特判。
fn render_glue_entries(keys: &BTreeSet<String>, layouts: &AggregateLayoutPlan, declared_exports: &BTreeSet<String>) -> Vec<serde_json::Value> {
    let mut entries = Vec::with_capacity(keys.len());
    for key in keys {
        let Some((module_path, name, generic_json)) = split_aggregate_key(key)
        else {
            continue;
        };
        let generic_args_json: serde_json::Value = serde_json::from_str(generic_json).unwrap_or_else(|_| serde_json::json!([]));
        // 同时解析结构化的 generic_args 供合成函数使用
        let generic_args_typed: Vec<WasmAbiType> = serde_json::from_str(generic_json).unwrap_or_default();

        if let Some(operations) = synthesize_sequence_helper_operations(layouts, name, module_path, &generic_args_typed, declared_exports) {
            entries.push(serde_json::json!({
                "kind": "NamedAggregate",
                "name": name,
                "module_path": module_path,
                "generic_args": generic_args_json,
                "operations": operations,
                "source": "backend_synthetic",
                "status": "ready"
            }));
        }
        else {
            entries.push(serde_json::json!({
                "kind": "NamedAggregate",
                "name": name,
                "module_path": module_path,
                "generic_args": generic_args_json,
                "operations": [],
                "source": "declaration_identity",
                "status": "pending_export_name"
            }));
        }
    }
    entries
}

pub(super) fn append_library_mode_glue(
    wasm_package_kind: WasmPackageKind,
    submission: &FragmentSubmission,
    module: &mut WasmBinaryModule,
) -> Result<()> {
    if wasm_package_kind != WasmPackageKind::Library {
        return Ok(());
    }
    let mut invoke_exports = serde_json::Map::new();
    let mut aggregate_keys: BTreeSet<String> = BTreeSet::new();

    for (instance, public_name) in submission.backend_plan.wasm_export_names() {
        let function =
            submission.backend_plan.get_function(instance).ok_or_else(|| format!("库导出 `{instance}` 缺少 Compiler 函数体"))?.function;

        let instance_label = format!("{instance}");
        let (params_schema, returns_schema) = schematize_function_abi(&instance_label, &function)?;

        // 收集所有 param/return schema 里出现的 NamedAggregate identity
        for param in &params_schema {
            collect_named_aggregate_keys(param, &mut aggregate_keys);
        }
        collect_named_aggregate_keys(&returns_schema, &mut aggregate_keys);

        // 注意：WasmAbiType 的 serde(tag = "kind", rename_all = "PascalCase")
        // 会自动生成 {"kind": "I64"} / {"kind": "Array", "element": {...}} 格式。
        if invoke_exports
            .insert(
                public_name.clone(),
                serde_json::json!({
                    "params": params_schema,
                    "returns": returns_schema,
                }),
            )
            .is_some()
        {
            return Err(format!("库导出公开名 `{public_name}` 重复"));
        }
    }
    if invoke_exports.is_empty() {
        return Err(format!("库 Wasm 产物没有可表达的导出合同"));
    }

    let declared_exports: BTreeSet<String> = submission.backend_plan.wasm_export_names().values().cloned().collect();
    let layouts = submission.backend_plan.aggregate_layouts();
    let glue_entries = render_glue_entries(&aggregate_keys, layouts, &declared_exports);
    let abi_digest = abi_contract_digest(LIBRARY_INVOKE_SCHEMA_VERSION, &serde_json::Value::Object(invoke_exports.clone()), &glue_entries);
    let payload = serde_json::json!({
        "version": LIBRARY_INVOKE_SCHEMA_VERSION,
        "exports": invoke_exports,
        "glue_entries": glue_entries,
        "abi_digest": abi_digest,
    });

    module.sections.push(WasmSection { id: 0, name: Some("nyar.library_invoke".to_owned()), bytes: payload.to_string().into_bytes() });
    Ok(())
}

/// 规范化 JSON 序列化：对象键按字节序排序，数组保持原序，标量沿用 JSON 文本。
///
/// `serde_json::Map` 默认是 `BTreeMap`，但 map 顺序可能随 feature 变化；这里显式
/// 排序，保证 digest 只由 ABI 事实决定。
fn canonical_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(&String, &serde_json::Value)> = map.iter().collect();
            entries.sort_by(|left, right| left.0.cmp(right.0));
            let body = entries
                .iter()
                .map(|(key, value)| format!("{}:{}", serde_json::Value::String((*key).clone()), canonical_json(value)))
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{body}}}")
        }
        serde_json::Value::Array(items) => {
            format!("[{}]", items.iter().map(canonical_json).collect::<Vec<_>>().join(","))
        }
        other => other.to_string(),
    }
}

/// 计算 `nyar.library_invoke` 的稳定 ABI digest。
///
/// 对 `version`、`exports` 与 `glue_entries` 的规范化 JSON 做 SHA-256：同一 ABI
/// 事实恒得同一 digest，协议版本、任一导出签名或 glue operation 变化都会改变
/// digest。
fn abi_contract_digest(version: u64, exports: &serde_json::Value, glue_entries: &[serde_json::Value]) -> String {
    let facts = serde_json::json!({
        "version": version,
        "exports": exports,
        "glue_entries": glue_entries,
    });
    format!("sha256-{}", combined_content_hash(&[&canonical_json(&facts)]))
}

/// 把 glue entry 渲染成稳定的 aggregate identity 标签（用于错误消息）。
fn aggregate_identity_label(entry: &serde_json::Value) -> String {
    let kind = entry.get("kind").and_then(|value| value.as_str()).unwrap_or("<unknown>");
    let module_path = entry.get("module_path").and_then(|value| value.as_str()).unwrap_or("");
    let name = entry.get("name").and_then(|value| value.as_str()).unwrap_or("<unknown>");
    let generic_kinds = entry
        .get("generic_args")
        .and_then(|value| value.as_array())
        .map(|args| args.iter().filter_map(|arg| arg.get("kind").and_then(|kind| kind.as_str())).collect::<Vec<_>>().join(","))
        .unwrap_or_default();
    format!("{kind}:{module_path}.{name}<{generic_kinds}>")
}

/// 校验 `glue_entries` 引用的 Wasm export 是否真实存在于本次模块的 export section。
///
/// 校验覆盖**所有** operation，不区分 `export_source`：`declared` 名来自
/// `TargetExecutionPlan::wasm_export_names()`，`backend_synthetic` 名由 backend
/// 合成，二者都必须在 artifact 校验阶段真实导出。缺失时返回含 aggregate identity、
/// operation、export_name 的结构化错误。
///
/// 因此 `backend_synthetic` helper 若没有真实 producer，库构建会在此处诚实失败，
/// 而不是拖到 JS glue 运行时才失败。
pub(crate) fn validate_glue_entry_exports(glue_entries: &[serde_json::Value], wasm_exports: &BTreeSet<String>) -> Result<()> {
    for entry in glue_entries {
        let identity = aggregate_identity_label(entry);
        let Some(operations) = entry.get("operations").and_then(|value| value.as_array())
        else {
            continue;
        };
        for operation in operations {
            let operation_name = operation.get("operation").and_then(|value| value.as_str()).unwrap_or("<unknown>");
            let export_name = operation.get("export_name").and_then(|value| value.as_str()).unwrap_or("");
            if export_name.is_empty() {
                return Err(format!("glue 合同的 operation 缺少 export_name: aggregate={identity} operation={operation_name}"));
            }
            if !wasm_exports.contains(export_name) {
                return Err(format!(
                    "glue 合同引用的 Wasm export 不存在: aggregate={identity} operation={operation_name} export_name={export_name} \
                     available={wasm_exports:?}"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyar_types::layout::{AggregateLayout, FieldLayout, StorageKind};

    fn nyar(s: &str) -> NyarType {
        NyarType::from_surface_primitive(s).expect("surface primitive")
    }

    /// 构造「带一个 `[T]` 字段 + 一个容量字段」的聚合布局，模拟真实
    /// `ArrayList<T>`（`_items: [T]` / `_capacity: usize`）。
    fn sequence_layout(name: &str, namespace: &str, element: NyarType) -> AggregateLayoutPlan {
        AggregateLayoutPlan {
            layouts: vec![AggregateLayout {
                id: 0,
                name: name.to_string(),
                namespace: namespace.to_string(),
                key: None,
                storage: StorageKind::Reference,
                size: 16,
                align: 8,
                fields: vec![
                    FieldLayout { name: "_items".to_string(), ty: NyarType::Array(Box::new(element)), offset: 0, size: 8, align: 8 },
                    FieldLayout { name: "_capacity".to_string(), ty: nyar("i32"), offset: 8, size: 4, align: 4 },
                ],
            }],
            ..AggregateLayoutPlan::default()
        }
    }

    #[test]
    fn collect_named_aggregates_from_params_and_return() {
        // 模拟 two-sum: ArrayList<i64>, i64 → ArrayList<i64>
        let list_i64 = WasmAbiType::NamedAggregate {
            name: "ArrayList".to_string(),
            module_path: "std::collections".to_string(),
            generic_args: vec![WasmAbiType::I64],
        };
        let mut keys = BTreeSet::new();
        collect_named_aggregate_keys(&list_i64, &mut keys);
        collect_named_aggregate_keys(&WasmAbiType::I64, &mut keys);

        let layouts = sequence_layout("ArrayList", "std.collections", nyar("i64"));
        // 两个 list_i64 应该被去重成一个 key
        let entries = render_glue_entries(&keys, &layouts, &BTreeSet::new());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["name"], "ArrayList");
        assert_eq!(entries[0]["module_path"], "std::collections");
        // 结构事实成立且 element 是标量 → backend synthetic 合成 operations
        assert_eq!(entries[0]["status"], "ready");
        assert_eq!(entries[0]["source"], "backend_synthetic");
        let ops = entries[0]["operations"].as_array().expect("operations array");
        assert_eq!(ops.len(), 4, "ArrayList<i64> 应有 construct/push/length/at 四个 operation");
        assert_eq!(ops[0]["operation"], "construct");
        assert_eq!(ops[0]["export_name"], "$seq_std_collections_ArrayList_i64_new");
        assert_eq!(ops[0]["param_carriers"], serde_json::json!(["i32"]));
        assert_eq!(ops[0]["return_carrier"], "i32");
        assert_eq!(ops[0]["export_source"], "backend_synthetic");
        assert_eq!(ops[1]["operation"], "push");
        assert_eq!(ops[1]["export_name"], "$seq_std_collections_ArrayList_i64_push");
        assert_eq!(ops[1]["param_carriers"], serde_json::json!(["i32", "i64"]));
        assert_eq!(ops[1]["return_carrier"], "void");
        assert_eq!(ops[2]["operation"], "length");
        assert_eq!(ops[2]["export_name"], "$seq_std_collections_ArrayList_i64_len");
        assert_eq!(ops[2]["param_carriers"], serde_json::json!(["i32"]));
        assert_eq!(ops[2]["return_carrier"], "i32");
        assert_eq!(ops[3]["operation"], "at");
        assert_eq!(ops[3]["export_name"], "$seq_std_collections_ArrayList_i64_at");
        assert_eq!(ops[3]["param_carriers"], serde_json::json!(["i32", "i32"]));
        assert_eq!(ops[3]["return_carrier"], "i64");
    }

    #[test]
    fn collect_nested_aggregate_in_option() {
        // Option<ArrayList<i64>> 应该被递归收集，内部 ArrayList<i64> 应被合成
        let opt = WasmAbiType::Option {
            inner: Box::new(WasmAbiType::NamedAggregate {
                name: "ArrayList".to_string(),
                module_path: String::new(),
                generic_args: vec![WasmAbiType::I64],
            }),
        };
        let mut keys = BTreeSet::new();
        collect_named_aggregate_keys(&opt, &mut keys);
        let layouts = sequence_layout("ArrayList", "", nyar("i64"));
        let entries = render_glue_entries(&keys, &layouts, &BTreeSet::new());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["status"], "ready");
        assert_eq!(entries[0]["source"], "backend_synthetic");
        assert_eq!(entries[0]["operations"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn collect_distinguishes_by_module_path() {
        // 两个同名但不同 module_path 的 aggregate 应视为不同
        // 注意：没有 generic_args 的 ArrayList 不参与合成（无 element 类型），保持旧行为
        let a =
            WasmAbiType::NamedAggregate { name: "ArrayList".to_string(), module_path: "std::collections".to_string(), generic_args: vec![] };
        let b = WasmAbiType::NamedAggregate { name: "ArrayList".to_string(), module_path: "user::lib".to_string(), generic_args: vec![] };
        let mut keys = BTreeSet::new();
        collect_named_aggregate_keys(&a, &mut keys);
        collect_named_aggregate_keys(&b, &mut keys);
        let entries = render_glue_entries(&keys, &AggregateLayoutPlan::default(), &BTreeSet::new());
        assert_eq!(entries.len(), 2);
        // 无 generic_args → 合成失败 → 旧行为
        for entry in &entries {
            assert_eq!(entry["status"], "pending_export_name");
            assert_eq!(entry["source"], "declaration_identity");
            assert!(entry["operations"].as_array().is_some_and(|a| a.is_empty()));
        }
    }

    // ---- 结构事实判定（不再按类型名特判） ----

    #[test]
    fn synthesize_uses_structure_not_type_name() {
        // 名字不是 ArrayList，但布局声明了 `[i32]` 字段 → 仍按结构事实合成
        let layouts = sequence_layout("Deque", "", nyar("i32"));
        let ops = synthesize_sequence_helper_operations(&layouts, "Deque", "", &[WasmAbiType::I32], &BTreeSet::new())
            .expect("structure fact should synthesize sequence helpers");
        assert_eq!(ops.len(), 4);
        assert_eq!(ops[0]["export_name"], "$seq_Deque_i32_new");
    }

    #[test]
    fn synthesize_array_list_i32() {
        let layouts = sequence_layout("ArrayList", "", nyar("i32"));
        let ops = synthesize_sequence_helper_operations(&layouts, "ArrayList", "", &[WasmAbiType::I32], &BTreeSet::new())
            .expect("ArrayList<i32> should synthesize");
        assert_eq!(ops.len(), 4);
        assert_eq!(ops[0]["export_name"], "$seq_ArrayList_i32_new");
        assert_eq!(ops[0]["param_carriers"], serde_json::json!(["i32"]));
        assert_eq!(ops[1]["export_name"], "$seq_ArrayList_i32_push");
        assert_eq!(ops[3]["return_carrier"], "i32");
    }

    #[test]
    fn synthesize_array_list_f64() {
        let layouts = sequence_layout("ArrayList", "", nyar("f64"));
        let ops = synthesize_sequence_helper_operations(&layouts, "ArrayList", "", &[WasmAbiType::F64], &BTreeSet::new())
            .expect("ArrayList<f64> should synthesize");
        assert_eq!(ops[0]["export_name"], "$seq_ArrayList_f64_new");
        assert_eq!(ops[1]["param_carriers"], serde_json::json!(["i32", "f64"]));
        assert_eq!(ops[3]["return_carrier"], "f64");
    }

    #[test]
    fn synthesize_array_list_bool_uses_i32_carrier_but_bool_tag() {
        let layouts = sequence_layout("ArrayList", "", nyar("bool"));
        let ops = synthesize_sequence_helper_operations(&layouts, "ArrayList", "", &[WasmAbiType::Bool], &BTreeSet::new())
            .expect("ArrayList<bool> should synthesize");
        // carrier 是 i32（Wasm 层），但 export tag 保留 bool
        assert_eq!(ops[0]["export_name"], "$seq_ArrayList_bool_new");
        assert_eq!(ops[0]["param_carriers"], serde_json::json!(["i32"]));
        assert_eq!(ops[3]["return_carrier"], "i32");
    }

    #[test]
    fn sequence_helper_names_follow_aggregate_identity() {
        let list_i64 = sequence_helper_stem("std::collections", "ArrayList", "i64");
        let list_f64 = sequence_helper_stem("std::collections", "ArrayList", "f64");
        let other_namespace = sequence_helper_stem("user::lib", "ArrayList", "i64");
        assert_eq!(list_i64, "$seq_std_collections_ArrayList_i64");
        assert_ne!(list_i64, list_f64, "不同元素类型必须得到不同 helper 名");
        assert_ne!(list_i64, other_namespace, "不同 namespace 必须得到不同 helper 名");
    }

    #[test]
    fn synthesize_skips_without_layout_structure_fact() {
        // plan 没有该 aggregate 的布局 → 不伪造 operations
        assert!(
            synthesize_sequence_helper_operations(&AggregateLayoutPlan::default(), "ArrayList", "", &[WasmAbiType::I64], &BTreeSet::new())
                .is_none()
        );
    }

    #[test]
    fn synthesize_skips_when_element_differs_from_generic_arg() {
        // 布局的 `[f64]` 字段与实例实参 I64 不一致 → 不是该实例的序列结构
        let layouts = sequence_layout("ArrayList", "", nyar("f64"));
        assert!(synthesize_sequence_helper_operations(&layouts, "ArrayList", "", &[WasmAbiType::I64], &BTreeSet::new()).is_none());
    }

    #[test]
    fn synthesize_skips_non_scalar_element() {
        // ArrayList<ArrayList<i64>>：结构事实成立，但 element 是容器 → 跳过
        let nested = NyarType::Array(Box::new(nyar("i64")));
        let layouts = sequence_layout("ArrayList", "", nested);
        let schema = WasmAbiType::Array { element: Box::new(WasmAbiType::I64) };
        assert!(synthesize_sequence_helper_operations(&layouts, "ArrayList", "", &[schema], &BTreeSet::new()).is_none());
    }

    #[test]
    fn synthesize_skips_no_generic_args() {
        let layouts = sequence_layout("ArrayList", "", nyar("i64"));
        assert!(synthesize_sequence_helper_operations(&layouts, "ArrayList", "", &[], &BTreeSet::new()).is_none());
    }

    #[test]
    fn declared_export_marks_operation_source_declared() {
        let layouts = sequence_layout("ArrayList", "", nyar("i64"));
        let mut declared = BTreeSet::new();
        declared.insert("$seq_ArrayList_i64_new".to_string());
        let ops = synthesize_sequence_helper_operations(&layouts, "ArrayList", "", &[WasmAbiType::I64], &declared)
            .expect("structure fact should synthesize");
        assert_eq!(ops[0]["export_source"], "declared");
        assert_eq!(ops[1]["export_source"], "backend_synthetic");
    }

    // ---- ABI digest ----

    #[test]
    fn abi_digest_is_independent_of_object_key_order() {
        let exports_a: serde_json::Value = serde_json::from_str(
            r#"{"solve":{"params":[{"kind":"I64"}],"returns":{"kind":"Unit"}},"build":{"params":[],"returns":{"kind":"I32"}}}"#,
        )
        .unwrap();
        let exports_b: serde_json::Value = serde_json::from_str(
            r#"{"build":{"returns":{"kind":"I32"},"params":[]},"solve":{"returns":{"kind":"Unit"},"params":[{"kind":"I64"}]}}"#,
        )
        .unwrap();
        let digest_a = abi_contract_digest(LIBRARY_INVOKE_SCHEMA_VERSION, &exports_a, &[]);
        let digest_b = abi_contract_digest(LIBRARY_INVOKE_SCHEMA_VERSION, &exports_b, &[]);
        assert!(digest_a.starts_with("sha256-"), "digest should be prefixed: {digest_a}");
        assert_eq!(digest_a, digest_b, "同一 ABI 事实必须得到同一 digest");
    }

    #[test]
    fn abi_digest_changes_with_glue_operations() {
        let exports: serde_json::Value = serde_json::from_str(r#"{"solve":{"params":[],"returns":{"kind":"I64"}}}"#).unwrap();
        let empty_digest = abi_contract_digest(LIBRARY_INVOKE_SCHEMA_VERSION, &exports, &[]);
        let with_glue = vec![serde_json::json!({
            "kind": "NamedAggregate",
            "name": "ArrayList",
            "module_path": "",
            "generic_args": [{"kind": "I64"}],
            "operations": [{"operation": "construct", "export_name": "$seq_ArrayList_i64_new"}],
            "source": "backend_synthetic",
            "status": "ready"
        })];
        let glue_digest = abi_contract_digest(LIBRARY_INVOKE_SCHEMA_VERSION, &exports, &with_glue);
        assert_ne!(empty_digest, glue_digest, "glue 合同变化必须改变 digest");
    }

    #[test]
    fn abi_digest_changes_with_schema_version() {
        let exports: serde_json::Value = serde_json::from_str(r#"{"solve":{"params":[],"returns":{"kind":"I64"}}}"#).unwrap();
        let version_one = abi_contract_digest(1, &exports, &[]);
        let version_two = abi_contract_digest(2, &exports, &[]);
        assert_ne!(version_one, version_two, "schema 版本必须参与 digest 规范化输入");
    }

    #[test]
    fn canonical_json_sorts_object_keys() {
        let left: serde_json::Value = serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap();
        let right: serde_json::Value = serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap();
        assert_eq!(canonical_json(&left), canonical_json(&right));
        assert_eq!(canonical_json(&left), r#"{"a":1,"b":2}"#);
    }

    // ---- glue export 真实性校验 ----

    fn glue_entry_with_operation(export_source: &str) -> serde_json::Value {
        serde_json::json!({
            "kind": "NamedAggregate",
            "name": "ArrayList",
            "module_path": "std::collections",
            "generic_args": [{"kind": "I64"}],
            "operations": [{
                "operation": "construct",
                "export_name": "solve",
                "param_carriers": ["i32"],
                "return_carrier": "i32",
                "export_source": export_source
            }],
            "source": "backend_synthetic",
            "status": "ready"
        })
    }

    #[test]
    fn validate_glue_entry_exports_rejects_missing_declared_export() {
        let entries = vec![glue_entry_with_operation(EXPORT_SOURCE_DECLARED)];
        let wasm_exports: BTreeSet<String> = ["memory".to_string()].into_iter().collect();
        let error = validate_glue_entry_exports(&entries, &wasm_exports).expect_err("missing export must fail");
        assert!(error.contains("export_name=solve"), "error must name the missing export: {error}");
        assert!(error.contains("ArrayList<I64>"), "error must carry aggregate identity: {error}");
        assert!(error.contains("construct"), "error must carry the operation: {error}");
    }

    #[test]
    fn validate_glue_entry_exports_accepts_present_declared_export() {
        let entries = vec![glue_entry_with_operation(EXPORT_SOURCE_DECLARED)];
        let wasm_exports: BTreeSet<String> = ["solve".to_string()].into_iter().collect();
        validate_glue_entry_exports(&entries, &wasm_exports).expect("present export must pass");
    }

    #[test]
    fn validate_glue_entry_exports_rejects_backend_synthetic_without_producer() {
        // 合成 helper 没有真实 producer 时必须在 artifact 校验阶段失败，
        // 不能留到 JS glue 运行时才报错。
        let mut entry = glue_entry_with_operation(EXPORT_SOURCE_BACKEND_SYNTHETIC);
        entry["operations"][0]["export_name"] = serde_json::json!("$seq_std_collections_ArrayList_i64_new");
        let error = validate_glue_entry_exports(&[entry], &BTreeSet::new()).expect_err("missing synthetic export must fail");
        assert!(error.contains("export_name=$seq_std_collections_ArrayList_i64_new"), "error must name the missing export: {error}");
        assert!(error.contains("ArrayList<I64>"), "error must carry aggregate identity: {error}");
        assert!(error.contains("construct"), "error must carry the operation: {error}");
    }

    #[test]
    fn validate_glue_entry_exports_accepts_backend_synthetic_with_real_export() {
        let mut entry = glue_entry_with_operation(EXPORT_SOURCE_BACKEND_SYNTHETIC);
        entry["operations"][0]["export_name"] = serde_json::json!("$seq_std_collections_ArrayList_i64_new");
        let wasm_exports: BTreeSet<String> = ["$seq_std_collections_ArrayList_i64_new".to_string()].into_iter().collect();
        validate_glue_entry_exports(&[entry], &wasm_exports).expect("present synthetic export must pass");
    }

    #[test]
    fn validate_glue_entry_exports_rejects_empty_declared_export_name() {
        let mut entry = glue_entry_with_operation(EXPORT_SOURCE_DECLARED);
        entry["operations"][0]["export_name"] = serde_json::json!("");
        let error = validate_glue_entry_exports(&[entry], &BTreeSet::new()).expect_err("empty export_name must fail");
        assert!(error.contains("缺少 export_name"), "error must explain the missing name: {error}");
    }
}

//! 库模式 JSON ABI：消费 `WasmAbiType` 结构化合同，序列化到
//! `nyar.library_invoke` 自定义段。
//!
//! 合同版本固定为 `0`。本模块只描述当前合同，不保留版本迁移叙述，也不提供
//! 旧形状的兼容路径。
//!
//! `exports[*].params` 是**参数身份对象数组**：`index`（声明顺序，0 起连续）、
//! `name`（来自 compiler schema 的 canonical 参数名，禁止按 metadata / 类型名
//! 猜测）、`type`（`WasmAbiType` JSON）与 `carrier`（物理载体：`i32` / `i64` /
//! `f32` / `f64` / `void` / `handle`）。参数名缺失、重复，或参数身份数量与类型
//! 数量不一致都会在此结构化失败。
//!
//! # JSON 格式
//!
//! ```json
//! {
//!   "version": 0,
//!   "exports": {
//!     "twoSum": {
//!       "params": [
//!         {
//!           "index": 0,
//!           "name": "nums",
//!           "type": {"kind": "NamedAggregate", "name": "ArrayList", "module_path": "std::collections", "generic_args": [{"kind": "I64"}]},
//!           "carrier": "handle"
//!         },
//!         {"index": 1, "name": "target", "type": {"kind": "I64"}, "carrier": "i64"}
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
//! - 结构事实成立时，`operations` 逐项来自 `TargetExecutionPlan` 冻结的
//!   [`AggregateOperationPlan`]；`export_source` 恒为 `declared`（plan 已声明导出）
//! - plan 缺少结构事实要求的 operation 时，库构建在此阶段结构化失败；缺失导出
//!   同样在 artifact 校验阶段失败

use std::collections::BTreeSet;

use nyar_types::{AggregateLayout, AggregateLayoutPlan, NyarType, ValueOrigin, combined_content_hash};

use crate::{
    AggregateOperation, AggregateOperationPlan, FragmentSubmission, LibraryExportAbi, Result,
    backend_plan_views::ExecutableFunction,
    nyar_backend_wasi::{WasmBinaryModule, WasmPackageKind, WasmSection},
};

use super::abi_schema::{WasmAbiType, abi_schema_for_type};

/// `nyar.library_invoke` JSON schema 版本（`/v0`）。
///
/// 跨仓消费者（如 `leetcode.v` 的 `abi.ts`）据此校验协议版本；版本变化同时会
/// 改变 [`abi_contract_digest`]。参数身份为 `{index, name, type, carrier}`，
/// 不提供旧形状的兼容路径。
const LIBRARY_INVOKE_SCHEMA_VERSION: u64 = 0;

/// 从 `WasmAbiType` schema 推导物理载体名称。
///
/// 本函数**只用于与 plan 冻结载体交叉核对**，不是载体事实源；`carrier` 字段
/// 只能取 [`LibraryExportAbi`] 的冻结值。标量按 Wasm 原生槽位，容器（数组 /
/// Option / Tuple / 具名聚合）统一走 `handle`。`Unsupported` 不应出现在参数位，
/// 出现即失败。
fn abi_carrier_name(schema: &WasmAbiType) -> Result<&'static str> {
    Ok(match schema {
        WasmAbiType::I32 => "i32",
        WasmAbiType::I64 => "i64",
        WasmAbiType::F32 => "f32",
        WasmAbiType::F64 => "f64",
        // Bool / Char 在 Wasm 层统一用 i32 槽位。
        WasmAbiType::Bool | WasmAbiType::Char => "i32",
        // Utf8 在 JS glue 轨是 host-intern i32 句柄。
        WasmAbiType::Utf8 => "i32",
        WasmAbiType::Unit => "void",
        WasmAbiType::Array { .. }
        | WasmAbiType::FixedArray { .. }
        | WasmAbiType::Option { .. }
        | WasmAbiType::Tuple { .. }
        | WasmAbiType::NamedAggregate { .. } => "handle",
        WasmAbiType::Utf16 => return Err("Utf16 没有稳定的宿主 ABI 载体".to_string()),
        WasmAbiType::Unsupported { reason } => return Err(format!("类型没有 Wasm ABI 载体: {reason}")),
    })
}

/// 从函数 SSA 值提取入口参数身份 `(声明下标, canonical 参数名)`。
///
/// 身份必须来自 `ValueOrigin::Parameter`，数量必须与 `param_types` 一致，
/// 下标必须连续且从 0 开始，参数名必须非空且不重复；任何偏离都在此结构化失败。
fn parameter_identities<'a>(instance_label: &str, function: &'a ExecutableFunction) -> Result<Vec<(usize, &'a str)>> {
    let mut identities = function
        .values
        .iter()
        .filter_map(|value| match &value.origin {
            ValueOrigin::Parameter { index, name } => Some((*index, name.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();
    if identities.len() != function.param_types.len() {
        return Err(format!(
            "库导出 `{instance_label}` 的入口参数身份数量与参数类型数量不一致: parameter_origins={} param_types={}",
            identities.len(),
            function.param_types.len()
        ));
    }
    identities.sort_by_key(|(index, _)| *index);
    let mut names = BTreeSet::new();
    for (position, (index, name)) in identities.iter().enumerate() {
        if *index != position {
            return Err(format!("库导出 `{instance_label}` 参数下标不连续: position={position} index={index}"));
        }
        if name.trim().is_empty() {
            return Err(format!("库导出 `{instance_label}` 参数 {index} 缺少 canonical 参数名"));
        }
        if !names.insert(*name) {
            return Err(format!("库导出 `{instance_label}` 参数 {index} 的参数名 `{name}` 重复"));
        }
    }
    Ok(identities)
}

/// 把函数的参数/返回类型提升为 `WasmAbiType` schema，并同时产出带参数身份的
/// `params` JSON 条目。
///
/// `carrier` 字段只取 `abi` 中 plan 冻结的载体值；`abi_carrier_name` 仅用来做
/// 交叉核对，冻结值与 schema 不一致即结构化失败。任何 `Unsupported` 变体会
/// 立刻返回可读错误，**禁止**静默降级。
/// `instance_label` 仅用于错误信息（可以是 instance ID 的 Display 或公开名）。
///
/// 返回 `(参数 schema 列表, params JSON 条目, 返回 schema)`：前者供聚合 identity
/// 收集复用，后者是新版 `exports[*].params` 合同。
fn schematize_function_abi(
    instance_label: &str,
    function: &ExecutableFunction,
    abi: &LibraryExportAbi,
) -> Result<(Vec<WasmAbiType>, Vec<serde_json::Value>, WasmAbiType)> {
    let identities = parameter_identities(instance_label, function)?;
    if abi.params.len() != function.param_types.len() {
        return Err(format!(
            "库导出 `{instance_label}` 的冻结 ABI 参数载体数量与参数类型数量不一致: plan_params={} param_types={}",
            abi.params.len(),
            function.param_types.len()
        ));
    }
    let mut params = Vec::with_capacity(function.param_types.len());
    let mut params_json = Vec::with_capacity(function.param_types.len());
    for (position, ty) in function.param_types.iter().enumerate() {
        let schema = abi_schema_for_type(ty);
        if let Some(reason) = schema.unsupported_reason() {
            return Err(format!("库导出 `{instance_label}` 参数 {position} 类型 `{ty}` 没有 Wasm ABI 合同: {reason}"));
        }
        let (index, name) = identities[position];
        let carrier = abi.params[position];
        let schema_carrier =
            abi_carrier_name(&schema).map_err(|reason| format!("库导出 `{instance_label}` 参数 {index} `{name}` 的 {reason}"))?;
        if schema_carrier != carrier.as_str() {
            return Err(format!(
                "库导出 `{instance_label}` 参数 {index} `{name}` 的冻结载体与 Wasm ABI schema 不一致: \
                 plan_carrier={} schema_carrier={schema_carrier}",
                carrier.as_str()
            ));
        }
        params_json.push(serde_json::json!({
            "index": index,
            "name": name,
            "type": schema,
            "carrier": carrier.as_str(),
        }));
        params.push(schema);
    }

    let ret_schema = abi_schema_for_type(&function.return_type);
    if let Some(reason) = ret_schema.unsupported_reason() {
        return Err(format!("库导出 `{instance_label}` 返回类型 `{}` 没有 Wasm ABI 合同: {reason}", function.return_type));
    }
    let schema_carrier =
        abi_carrier_name(&ret_schema).map_err(|reason| format!("库导出 `{instance_label}` 返回类型 `{}` 的 {reason}", function.return_type))?;
    if schema_carrier != abi.returns.as_str() {
        return Err(format!(
            "库导出 `{instance_label}` 返回值的冻结载体与 Wasm ABI schema 不一致: plan_carrier={} schema_carrier={schema_carrier}",
            abi.returns.as_str()
        ));
    }
    Ok((params, params_json, ret_schema))
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

/// glue operation 的 export 来源标记。
///
/// operation 只能来自 `TargetExecutionPlan::aggregate_operations()`；其
/// `export_name` 由 plan 显式声明，必须在 artifact 校验阶段真实出现在 Wasm
/// export section。
const EXPORT_SOURCE_DECLARED: &str = "declared";

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
fn array_field_element<'a>(
    layouts: &'a AggregateLayoutPlan,
    name: &str,
    module_path: &str,
    element: &WasmAbiType,
) -> Option<(&'a AggregateLayout, &'a NyarType)> {
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
                exact = Some((layout, field_element.as_ref()));
                break;
            }
            loose.push((layout, field_element.as_ref()));
        }
        if exact.is_some() {
            break;
        }
    }
    exact.or_else(|| if loose.len() == 1 { loose.pop() } else { None })
}

/// 结构事实要求、但必须由 plan 提供 producer 的 glue operation。
struct RequiredGlueOperation {
    /// operation 种类。
    operation: AggregateOperation,
    /// 结构事实要求的参数载体名称。
    param_carriers: Vec<&'static str>,
    /// 结构事实要求的返回载体名称。
    return_carrier: &'static str,
}

/// 从 aggregate 布局结构事实推导该实例需要的 glue operation。
///
/// 只推导「需要哪些 operation」与所需 carriers，**不**生成任何 export 名；
/// 真实 export 与 producer 必须来自 plan。generic args 缺失、无匹配 `[T]`
/// 字段或元素非标量时返回 `None`（该实例不被判定为序列型）。
fn required_sequence_operations<'a>(
    layouts: &'a AggregateLayoutPlan,
    name: &str,
    module_path: &str,
    generic_args: &[WasmAbiType],
) -> Option<(&'a AggregateLayout, Vec<RequiredGlueOperation>)> {
    let element = generic_args.first()?;
    let (layout, _) = array_field_element(layouts, name, module_path, element)?;
    let carrier = scalar_carrier_name(element)?;
    Some((
        layout,
        vec![
            RequiredGlueOperation { operation: AggregateOperation::Construct, param_carriers: vec!["i32"], return_carrier: "i32" },
            RequiredGlueOperation { operation: AggregateOperation::Push, param_carriers: vec!["i32", carrier], return_carrier: "void" },
            RequiredGlueOperation { operation: AggregateOperation::Length, param_carriers: vec!["i32"], return_carrier: "i32" },
            RequiredGlueOperation { operation: AggregateOperation::At, param_carriers: vec!["i32", "i32"], return_carrier: carrier },
        ],
    ))
}

/// 把收集到的 NamedAggregate identity keys 渲染为 `glue_entries` JSON 数组。
///
/// operations 逐项来自 plan 冻结的 [`AggregateOperationPlan`]。结构事实要求的
/// operation 在 plan 中缺失时返回结构化错误（aggregate identity、缺失 operation、
/// 所需 carriers），不再由 backend 合成 export 名。
fn render_glue_entries(
    keys: &BTreeSet<String>,
    layouts: &AggregateLayoutPlan,
    operations: &[AggregateOperationPlan],
) -> Result<Vec<serde_json::Value>> {
    let mut entries = Vec::with_capacity(keys.len());
    for key in keys {
        let Some((module_path, name, generic_json)) = split_aggregate_key(key)
        else {
            continue;
        };
        let generic_args_json: serde_json::Value = serde_json::from_str(generic_json).unwrap_or_else(|_| serde_json::json!([]));
        let generic_args_typed: Vec<WasmAbiType> = serde_json::from_str(generic_json).unwrap_or_default();
        let identity = format!("NamedAggregate:{module_path}.{name}<{generic_json}>");

        let rendered = match required_sequence_operations(layouts, name, module_path, &generic_args_typed) {
            Some((layout, required)) => {
                let available = operations.iter().filter(|operation| operation.layout_id == layout.id).collect::<Vec<_>>();
                let mut rendered = Vec::with_capacity(required.len());
                for required in required {
                    let Some(operation) = available.iter().find(|operation| operation.operation == required.operation)
                    else {
                        return Err(format!(
                            "库导出 aggregate 缺少 glue operation producer: aggregate={identity} layout_id={} layout={}.{} \
                             operation={} required_param_carriers={:?} required_return_carrier={} available_operations={:?}; \
                             TargetExecutionPlan 未声明该 operation，backend 不得合成",
                            layout.id,
                            layout.namespace,
                            layout.name,
                            required.operation.as_str(),
                            required.param_carriers,
                            required.return_carrier,
                            available.iter().map(|operation| operation.operation.as_str()).collect::<Vec<_>>(),
                        ));
                    };
                    let actual_param_carriers = operation.param_carriers.iter().map(|carrier| carrier.as_str()).collect::<Vec<_>>();
                    if actual_param_carriers != required.param_carriers || operation.return_carrier.as_str() != required.return_carrier {
                        return Err(format!(
                            "glue operation 载体与 aggregate 结构事实不符: aggregate={identity} layout_id={} layout={}.{} \
                             operation={} expected_param_carriers={:?} actual_param_carriers={:?} expected_return_carrier={} \
                             actual_return_carrier={}",
                            layout.id,
                            layout.namespace,
                            layout.name,
                            required.operation.as_str(),
                            required.param_carriers,
                            actual_param_carriers,
                            required.return_carrier,
                            operation.return_carrier.as_str(),
                        ));
                    }
                    rendered.push(serde_json::json!({
                        "operation": required.operation.as_str(),
                        "export_name": operation.export_name,
                        "param_carriers": actual_param_carriers,
                        "return_carrier": operation.return_carrier.as_str(),
                        "export_source": EXPORT_SOURCE_DECLARED
                    }));
                }
                rendered
            }
            None => Vec::new(),
        };

        let ready = !rendered.is_empty();
        entries.push(serde_json::json!({
            "kind": "NamedAggregate",
            "name": name,
            "module_path": module_path,
            "generic_args": generic_args_json,
            "operations": rendered,
            "source": if ready { "plan" } else { "declaration_identity" },
            "status": if ready { "ready" } else { "pending_export_name" }
        }));
    }
    Ok(entries)
}

pub(super) fn append_library_mode_glue(
    wasm_package_kind: WasmPackageKind,
    submission: &FragmentSubmission,
    module: &mut WasmBinaryModule,
) -> Result<()> {
    if wasm_package_kind != WasmPackageKind::Library {
        return Ok(());
    }
    let plan = &submission.backend_plan;
    plan.validate_aggregate_operations()?;
    let aggregate_operation_producers: BTreeSet<_> =
        plan.aggregate_operations().iter().map(|operation| operation.producer.instance()).collect();
    let mut invoke_exports = serde_json::Map::new();
    let mut aggregate_keys: BTreeSet<String> = BTreeSet::new();

    for (instance, public_name) in plan.wasm_export_names() {
        // aggregate helper 的 producer 是 glue 导出，不是语言层公开导出。
        if aggregate_operation_producers.contains(instance) {
            continue;
        }
        let function = plan.get_function(instance).ok_or_else(|| format!("库导出 `{instance}` 缺少 Compiler 函数体"))?.function;
        let abi = plan
            .library_export_abi(instance)
            .ok_or_else(|| format!("库导出 `{instance}`（公开名 `{public_name}`）没有冻结的 ABI 载体合同；backend 不得自行推导 carrier"))?;

        let instance_label = format!("{instance}");
        let (params_schema, params_json, returns_schema) = schematize_function_abi(&instance_label, &function, abi)?;

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
                    "params": params_json,
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

    let glue_entries = render_glue_entries(&aggregate_keys, plan.aggregate_layouts(), plan.aggregate_operations())?;
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
/// `glue_entries` 的 operation 均来自 `TargetExecutionPlan::aggregate_operations()`，
/// 其 `export_name` 必须由 plan 声明且真实导出。缺失时返回含 aggregate identity、
/// operation、export_name 的结构化错误。
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
    use crate::{AbiCarrier, AggregateOperationCarrier, AggregateOperationProducer, TargetExecutionPlan};
    use nyar_types::{
        ItemInstanceId,
        layout::{FieldLayout, StorageKind},
    };

    fn nyar(s: &str) -> NyarType {
        NyarType::from_surface_primitive(s).expect("surface primitive")
    }

    /// 构造「带一个 `[T]` 字段 + 一个容量字段」的聚合布局，模拟真实
    /// 序列型聚合（`_items: [T]` / `_capacity: usize`）。
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

    fn plan_operation(
        layout_id: nyar_types::LayoutId,
        operation: AggregateOperation,
        param_carriers: &[AggregateOperationCarrier],
        return_carrier: AggregateOperationCarrier,
        export_name: &str,
    ) -> AggregateOperationPlan {
        AggregateOperationPlan {
            layout_id,
            operation,
            param_carriers: param_carriers.to_vec(),
            return_carrier,
            export_name: export_name.to_string(),
            producer: AggregateOperationProducer::FunctionBody(ItemInstanceId::from_index(0).expect("测试实例")),
        }
    }

    /// 由 plan 提供的完整 sequence operation 集合（export 名由 plan 决定）。
    fn sequence_operations(
        layout_id: nyar_types::LayoutId,
        carrier: AggregateOperationCarrier,
        export_prefix: &str,
    ) -> Vec<AggregateOperationPlan> {
        use AggregateOperationCarrier::{I32, Void};
        vec![
            plan_operation(layout_id, AggregateOperation::Construct, &[I32], I32, &format!("{export_prefix}_new")),
            plan_operation(layout_id, AggregateOperation::Push, &[I32, carrier], Void, &format!("{export_prefix}_push")),
            plan_operation(layout_id, AggregateOperation::Length, &[I32], I32, &format!("{export_prefix}_len")),
            plan_operation(layout_id, AggregateOperation::At, &[I32, I32], carrier, &format!("{export_prefix}_at")),
        ]
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
        let operations = sequence_operations(0, AggregateOperationCarrier::I64, "$seq_std_collections_ArrayList_i64");
        // 两个 list_i64 应该被去重成一个 key
        let entries = render_glue_entries(&keys, &layouts, &operations).expect("plan 提供完整 operation");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["name"], "ArrayList");
        assert_eq!(entries[0]["module_path"], "std::collections");
        assert_eq!(entries[0]["status"], "ready");
        assert_eq!(entries[0]["source"], "plan");
        let ops = entries[0]["operations"].as_array().expect("operations array");
        assert_eq!(ops.len(), 4, "sequence 聚合应有 construct/push/length/at 四个 operation");
        assert_eq!(ops[0]["operation"], "construct");
        assert_eq!(ops[0]["export_name"], "$seq_std_collections_ArrayList_i64_new");
        assert_eq!(ops[0]["param_carriers"], serde_json::json!(["i32"]));
        assert_eq!(ops[0]["return_carrier"], "i32");
        assert_eq!(ops[0]["export_source"], "declared");
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
        // Option<ArrayList<i64>> 应该被递归收集
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
        let operations = sequence_operations(0, AggregateOperationCarrier::I64, "$seq_ArrayList_i64");
        let entries = render_glue_entries(&keys, &layouts, &operations).expect("plan 提供完整 operation");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["status"], "ready");
        assert_eq!(entries[0]["source"], "plan");
        assert_eq!(entries[0]["operations"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn collect_distinguishes_by_module_path() {
        // 两个同名但不同 module_path 的 aggregate 应视为不同
        let a =
            WasmAbiType::NamedAggregate { name: "ArrayList".to_string(), module_path: "std::collections".to_string(), generic_args: vec![] };
        let b = WasmAbiType::NamedAggregate { name: "ArrayList".to_string(), module_path: "user::lib".to_string(), generic_args: vec![] };
        let mut keys = BTreeSet::new();
        collect_named_aggregate_keys(&a, &mut keys);
        collect_named_aggregate_keys(&b, &mut keys);
        let entries = render_glue_entries(&keys, &AggregateLayoutPlan::default(), &[]).expect("无 generic_args 不要求 operation");
        assert_eq!(entries.len(), 2);
        for entry in &entries {
            assert_eq!(entry["status"], "pending_export_name");
            assert_eq!(entry["source"], "declaration_identity");
            assert!(entry["operations"].as_array().is_some_and(|a| a.is_empty()));
        }
    }

    // ---- 结构事实只决定「需要哪些 operation」，不生成 export 名 ----

    #[test]
    fn render_uses_structure_not_type_name() {
        // 名字不是 ArrayList，但布局声明了 `[i32]` 字段 → 仍按结构事实要求 operation
        let layouts = sequence_layout("Deque", "", nyar("i32"));
        let operations = sequence_operations(0, AggregateOperationCarrier::I32, "$seq_Deque_i32");
        let mut keys = BTreeSet::new();
        collect_named_aggregate_keys(
            &WasmAbiType::NamedAggregate { name: "Deque".to_string(), module_path: String::new(), generic_args: vec![WasmAbiType::I32] },
            &mut keys,
        );
        let entries = render_glue_entries(&keys, &layouts, &operations).expect("结构事实成立且 plan 齐全");
        assert_eq!(entries[0]["operations"].as_array().unwrap().len(), 4);
        assert_eq!(entries[0]["operations"][0]["export_name"], "$seq_Deque_i32_new");
    }

    #[test]
    fn required_operation_carriers_follow_element_schema() {
        let f64_layouts = sequence_layout("ArrayList", "", nyar("f64"));
        let (_, required) = required_sequence_operations(&f64_layouts, "ArrayList", "", &[WasmAbiType::F64]).expect("f64 sequence fact");
        assert_eq!(required[1].operation, AggregateOperation::Push);
        assert_eq!(required[1].param_carriers, vec!["i32", "f64"]);
        assert_eq!(required[3].operation, AggregateOperation::At);
        assert_eq!(required[3].return_carrier, "f64");

        let bool_layouts = sequence_layout("ArrayList", "", nyar("bool"));
        let (_, required) = required_sequence_operations(&bool_layouts, "ArrayList", "", &[WasmAbiType::Bool]).expect("bool sequence fact");
        // Bool 在 Wasm 层用 i32 carrier；export 名由 plan 决定。
        assert_eq!(required[1].param_carriers, vec!["i32", "i32"]);
        assert_eq!(required[3].return_carrier, "i32");
    }

    #[test]
    fn render_reports_missing_sequence_operation() {
        let layouts = sequence_layout("ArrayList", "std.collections", nyar("i64"));
        let mut operations = sequence_operations(0, AggregateOperationCarrier::I64, "$seq_std_collections_ArrayList_i64");
        operations.retain(|operation| operation.operation != AggregateOperation::At);
        let mut keys = BTreeSet::new();
        collect_named_aggregate_keys(
            &WasmAbiType::NamedAggregate {
                name: "ArrayList".to_string(),
                module_path: "std::collections".to_string(),
                generic_args: vec![WasmAbiType::I64],
            },
            &mut keys,
        );
        let error = render_glue_entries(&keys, &layouts, &operations).expect_err("缺少 producer 必须失败");
        assert!(error.contains("缺少 glue operation producer"), "error must explain the missing producer: {error}");
        assert!(error.contains("ArrayList"), "error must carry aggregate identity: {error}");
        assert!(error.contains("operation=at"), "error must name the missing operation: {error}");
        assert!(error.contains("required_param_carriers=[\"i32\", \"i32\"]"), "error must list required carriers: {error}");
    }

    #[test]
    fn render_reports_operation_carrier_mismatch() {
        let layouts = sequence_layout("ArrayList", "", nyar("i64"));
        let mut operations = sequence_operations(0, AggregateOperationCarrier::I64, "$seq_ArrayList_i64");
        operations[3].return_carrier = AggregateOperationCarrier::I32;
        let mut keys = BTreeSet::new();
        collect_named_aggregate_keys(
            &WasmAbiType::NamedAggregate { name: "ArrayList".to_string(), module_path: String::new(), generic_args: vec![WasmAbiType::I64] },
            &mut keys,
        );
        let error = render_glue_entries(&keys, &layouts, &operations).expect_err("载体不符必须失败");
        assert!(error.contains("载体与 aggregate 结构事实不符"), "error must explain carrier mismatch: {error}");
        assert!(error.contains("operation=at"), "error must name the mismatched operation: {error}");
    }

    #[test]
    fn render_skips_without_layout_structure_fact() {
        // plan 没有该 aggregate 的布局 → 不被判定为序列型，不伪造 operations
        let mut keys = BTreeSet::new();
        collect_named_aggregate_keys(
            &WasmAbiType::NamedAggregate { name: "ArrayList".to_string(), module_path: String::new(), generic_args: vec![WasmAbiType::I64] },
            &mut keys,
        );
        let entries = render_glue_entries(&keys, &AggregateLayoutPlan::default(), &[]).expect("无结构事实不要求 operation");
        assert!(entries[0]["operations"].as_array().is_some_and(|operations| operations.is_empty()));
        assert_eq!(entries[0]["status"], "pending_export_name");
    }

    #[test]
    fn render_skips_when_element_differs_from_generic_arg() {
        // 布局的 `[f64]` 字段与实例实参 I64 不一致 → 不是该实例的序列结构
        let layouts = sequence_layout("ArrayList", "", nyar("f64"));
        let mut keys = BTreeSet::new();
        collect_named_aggregate_keys(
            &WasmAbiType::NamedAggregate { name: "ArrayList".to_string(), module_path: String::new(), generic_args: vec![WasmAbiType::I64] },
            &mut keys,
        );
        let entries = render_glue_entries(&keys, &layouts, &[]).expect("结构不匹配不要求 operation");
        assert!(entries[0]["operations"].as_array().is_some_and(|operations| operations.is_empty()));
    }

    #[test]
    fn render_skips_non_scalar_element() {
        // ArrayList<ArrayList<i64>>：结构事实成立，但 element 是容器 → 跳过
        let nested = NyarType::Array(Box::new(nyar("i64")));
        let layouts = sequence_layout("ArrayList", "", nested);
        let schema = WasmAbiType::Array { element: Box::new(WasmAbiType::I64) };
        let mut keys = BTreeSet::new();
        collect_named_aggregate_keys(
            &WasmAbiType::NamedAggregate { name: "ArrayList".to_string(), module_path: String::new(), generic_args: vec![schema] },
            &mut keys,
        );
        let entries = render_glue_entries(&keys, &layouts, &[]).expect("容器元素不要求 operation");
        assert!(entries[0]["operations"].as_array().is_some_and(|operations| operations.is_empty()));
    }

    #[test]
    fn render_skips_no_generic_args() {
        let layouts = sequence_layout("ArrayList", "", nyar("i64"));
        let mut keys = BTreeSet::new();
        collect_named_aggregate_keys(
            &WasmAbiType::NamedAggregate { name: "ArrayList".to_string(), module_path: String::new(), generic_args: vec![] },
            &mut keys,
        );
        let entries = render_glue_entries(&keys, &layouts, &[]).expect("无泛型实参不要求 operation");
        assert!(entries[0]["operations"].as_array().is_some_and(|operations| operations.is_empty()));
    }

    #[test]
    fn plan_operations_render_as_declared_exports() {
        let layouts = sequence_layout("ArrayList", "", nyar("i64"));
        let operations = sequence_operations(0, AggregateOperationCarrier::I64, "$seq_ArrayList_i64");
        let mut keys = BTreeSet::new();
        collect_named_aggregate_keys(
            &WasmAbiType::NamedAggregate { name: "ArrayList".to_string(), module_path: String::new(), generic_args: vec![WasmAbiType::I64] },
            &mut keys,
        );
        let entries = render_glue_entries(&keys, &layouts, &operations).expect("plan 提供完整 operation");
        for operation in entries[0]["operations"].as_array().unwrap() {
            assert_eq!(operation["export_source"], "declared");
        }
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
            "source": "plan",
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

    fn glue_entry_with_operation() -> serde_json::Value {
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
                "export_source": EXPORT_SOURCE_DECLARED
            }],
            "source": "plan",
            "status": "ready"
        })
    }

    #[test]
    fn validate_glue_entry_exports_rejects_missing_declared_export() {
        let entries = vec![glue_entry_with_operation()];
        let wasm_exports: BTreeSet<String> = ["memory".to_string()].into_iter().collect();
        let error = validate_glue_entry_exports(&entries, &wasm_exports).expect_err("missing export must fail");
        assert!(error.contains("export_name=solve"), "error must name the missing export: {error}");
        assert!(error.contains("ArrayList<I64>"), "error must carry aggregate identity: {error}");
        assert!(error.contains("construct"), "error must carry the operation: {error}");
    }

    #[test]
    fn validate_glue_entry_exports_accepts_present_declared_export() {
        let entries = vec![glue_entry_with_operation()];
        let wasm_exports: BTreeSet<String> = ["solve".to_string()].into_iter().collect();
        validate_glue_entry_exports(&entries, &wasm_exports).expect("present export must pass");
    }

    #[test]
    fn validate_glue_entry_exports_rejects_plan_operation_without_export() {
        // plan operation 没有真实 export 时必须在 artifact 校验阶段失败，
        // 不能留到 JS glue 运行时才报错。
        let mut entry = glue_entry_with_operation();
        entry["operations"][0]["export_name"] = serde_json::json!("$seq_std_collections_ArrayList_i64_new");
        let error = validate_glue_entry_exports(&[entry], &BTreeSet::new()).expect_err("missing plan export must fail");
        assert!(error.contains("export_name=$seq_std_collections_ArrayList_i64_new"), "error must name the missing export: {error}");
        assert!(error.contains("ArrayList<I64>"), "error must carry aggregate identity: {error}");
        assert!(error.contains("construct"), "error must carry the operation: {error}");
    }

    #[test]
    fn validate_glue_entry_exports_accepts_plan_operation_with_real_export() {
        let mut entry = glue_entry_with_operation();
        entry["operations"][0]["export_name"] = serde_json::json!("$seq_std_collections_ArrayList_i64_new");
        let wasm_exports: BTreeSet<String> = ["$seq_std_collections_ArrayList_i64_new".to_string()].into_iter().collect();
        validate_glue_entry_exports(&[entry], &wasm_exports).expect("present plan export must pass");
    }

    #[test]
    fn validate_glue_entry_exports_rejects_empty_declared_export_name() {
        let mut entry = glue_entry_with_operation();
        entry["operations"][0]["export_name"] = serde_json::json!("");
        let error = validate_glue_entry_exports(&[entry], &BTreeSet::new()).expect_err("empty export_name must fail");
        assert!(error.contains("缺少 export_name"), "error must explain the missing name: {error}");
    }

    // ---- 参数身份合同 ----

    /// 构造带 `ValueOrigin::Parameter` 身份的测试函数。
    fn abi_function(parameters: &[(usize, &str, NyarType)], return_type: NyarType) -> ExecutableFunction {
        ExecutableFunction {
            symbol: "identity".to_string(),
            return_type,
            param_types: parameters.iter().map(|(_, _, ty)| ty.clone()).collect(),
            value_types: std::collections::BTreeMap::new(),
            value_layouts: std::collections::BTreeMap::new(),
            return_layout: None,
            entry: crate::contracts::BlockRef(0),
            values: parameters
                .iter()
                .map(|(index, name, _)| crate::contracts::Value {
                    id: crate::contracts::ValueRef(u32::try_from(*index).expect("测试参数下标")),
                    origin: ValueOrigin::Parameter { index: *index, name: (*name).to_string() },
                })
                .collect(),
            suspend_points: Vec::new(),
            frame_layouts: Vec::new(),
            continuations: Vec::new(),
            case_chains: Vec::new(),
            #[allow(deprecated)]
            state_machine: None,
            suspend_plan: None,
            blocks: vec![crate::contracts::Block {
                id: crate::contracts::BlockRef(0),
                label: "entry".to_string(),
                parameters: Vec::new(),
                instructions: Vec::new(),
                terminator: crate::contracts::Terminator::Return { value: None },
            }],
            diagnostics: Vec::new(),
        }
    }

    fn arraylist_i64() -> NyarType {
        NyarType::Apply(Box::new(NyarType::Named(nyar_types::Identifier::new("std::collections::ArrayList"))), vec![nyar("i64")])
    }

    fn frozen_abi(params: Vec<AbiCarrier>, returns: AbiCarrier) -> LibraryExportAbi {
        LibraryExportAbi { params, returns }
    }

    #[test]
    fn schematize_renders_parameter_identity_carrier_and_type() {
        let list = arraylist_i64();
        let function = abi_function(&[(0, "nums", list.clone()), (1, "target", nyar("i64"))], list.clone());
        let abi = frozen_abi(vec![AbiCarrier::Handle, AbiCarrier::I64], AbiCarrier::Handle);
        let (schemas, params_json, returns) = schematize_function_abi("twoSum", &function, &abi).expect("参数身份完整");
        assert_eq!(schemas, vec![abi_schema_for_type(&list), WasmAbiType::I64]);
        assert_eq!(
            params_json,
            vec![
                serde_json::json!({
                    "index": 0,
                    "name": "nums",
                    "type": {"kind": "NamedAggregate", "name": "ArrayList", "module_path": "std::collections", "generic_args": [{"kind": "I64"}]},
                    "carrier": "handle"
                }),
                serde_json::json!({"index": 1, "name": "target", "type": {"kind": "I64"}, "carrier": "i64"}),
            ]
        );
        assert_eq!(returns, abi_schema_for_type(&list));
    }

    #[test]
    fn schematize_rejects_duplicate_parameter_names() {
        let function = abi_function(&[(0, "value", nyar("i64")), (1, "value", nyar("i64"))], nyar("i64"));
        let abi = frozen_abi(vec![AbiCarrier::I64, AbiCarrier::I64], AbiCarrier::I64);
        let error = schematize_function_abi("identity", &function, &abi).expect_err("参数名重复必须失败");
        assert!(error.contains("identity"), "error must carry the export name: {error}");
        assert!(error.contains("index=1") || error.contains("参数 1"), "error must carry the parameter index: {error}");
        assert!(error.contains("重复"), "error must explain the duplicate name: {error}");
    }

    #[test]
    fn schematize_rejects_missing_parameter_name() {
        let function = abi_function(&[(0, "", nyar("i64"))], nyar("i64"));
        let abi = frozen_abi(vec![AbiCarrier::I64], AbiCarrier::I64);
        let error = schematize_function_abi("identity", &function, &abi).expect_err("参数名缺失必须失败");
        assert!(error.contains("identity"), "error must carry the export name: {error}");
        assert!(error.contains("缺少 canonical 参数名"), "error must explain the missing name: {error}");
    }

    #[test]
    fn schematize_rejects_identity_count_mismatch() {
        let mut function = abi_function(&[(0, "value", nyar("i64"))], nyar("i64"));
        function.param_types.push(nyar("i32"));
        let abi = frozen_abi(vec![AbiCarrier::I64, AbiCarrier::I32], AbiCarrier::I64);
        let error = schematize_function_abi("identity", &function, &abi).expect_err("身份数量不一致必须失败");
        assert!(error.contains("数量不一致"), "error must explain the count mismatch: {error}");
    }

    #[test]
    fn schematize_rejects_carrier_disagreement_with_plan() {
        // plan 冻结参数为 Handle，但实际类型是标量 i32：交叉核对必须失败，
        // backend 不得以 schema 推导结果覆盖 plan 事实。
        let function = abi_function(&[(0, "value", nyar("i32"))], nyar("i32"));
        let abi = frozen_abi(vec![AbiCarrier::Handle], AbiCarrier::I32);
        let error = schematize_function_abi("identity", &function, &abi).expect_err("冻结核对不一致必须失败");
        assert!(error.contains("identity"), "error must carry the instance label: {error}");
        assert!(error.contains("参数 0"), "error must carry the parameter index: {error}");
        assert!(error.contains("plan_carrier=handle"), "error must carry the plan carrier: {error}");
        assert!(error.contains("schema_carrier=i32"), "error must carry the schema carrier: {error}");
    }

    #[test]
    fn plan_freezes_library_export_abi_carriers() {
        let list = arraylist_i64();
        let function = abi_function(&[(0, "nums", list.clone()), (1, "target", nyar("i64"))], list);
        let name = nyar::QualifiedName::new(vec![nyar::Identifier::new("twoSum")]);
        let mut plan = TargetExecutionPlan::from_functions(std::collections::BTreeMap::from([(name, function)]));
        let instance = *plan.instances().first().expect("测试函数实例");
        plan.testing_wasm_export_names_mut().insert(instance, "twoSum".to_string());
        plan.testing_freeze_library_export_abis().expect("冻结 ABI 必须成功");
        let abi = plan.library_export_abi(&instance).expect("导出实例必须有冻结 ABI");
        assert_eq!(abi.params, vec![AbiCarrier::Handle, AbiCarrier::I64]);
        assert_eq!(abi.returns, AbiCarrier::Handle);
    }

    #[test]
    fn carrier_mapping_covers_scalars_and_handles() {
        assert_eq!(abi_carrier_name(&WasmAbiType::I32).expect("i32"), "i32");
        assert_eq!(abi_carrier_name(&WasmAbiType::I64).expect("i64"), "i64");
        assert_eq!(abi_carrier_name(&WasmAbiType::F32).expect("f32"), "f32");
        assert_eq!(abi_carrier_name(&WasmAbiType::F64).expect("f64"), "f64");
        assert_eq!(abi_carrier_name(&WasmAbiType::Bool).expect("bool"), "i32");
        assert_eq!(abi_carrier_name(&WasmAbiType::Char).expect("char"), "i32");
        assert_eq!(abi_carrier_name(&WasmAbiType::Utf8).expect("utf8"), "i32");
        assert_eq!(abi_carrier_name(&WasmAbiType::Unit).expect("unit"), "void");
        for schema in [
            WasmAbiType::Array { element: Box::new(WasmAbiType::I64) },
            WasmAbiType::FixedArray { element: Box::new(WasmAbiType::I64), length: 2 },
            WasmAbiType::Option { inner: Box::new(WasmAbiType::I64) },
            WasmAbiType::Tuple { fields: vec![WasmAbiType::I64] },
            WasmAbiType::NamedAggregate { name: "ArrayList".to_string(), module_path: String::new(), generic_args: vec![WasmAbiType::I64] },
        ] {
            assert_eq!(abi_carrier_name(&schema).expect("handle"), "handle");
        }
    }
}

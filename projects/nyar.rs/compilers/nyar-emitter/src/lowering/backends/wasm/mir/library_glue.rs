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
//! v3（当前阶段）：增加 `glue_entries` 数组，为下游 JS glue consumer
//! 提供 aggregate 的语义身份 skeleton。`export_name` 暂留占位，
//! 等 Stream C 确认 valkyrie.v 的具体 Wasm 导出后补全。
//!
//! # JSON 格式
//!
//! ```json
//! {
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
//!   ]
//! }
//! ```
//!
//! # glue_entries 约束
//!
//! - **只**声明 `NamedAggregate` 的语义身份（name + module_path + generic_args）
//! - **不**硬编码任何特定 aggregate 有哪些 operations（那是 valkyrie.v stdlib 的事）
//! - `operations` 数组在本阶段永远为空数组——下游 Stream C 补 export_name 后再填

use std::collections::BTreeSet;

use crate::{
    FragmentSubmission, Result,
    nyar_backend_wasi::{WasmBinaryModule, WasmPackageKind, WasmSection},
};

use super::abi_schema::{abi_schema_for_type, WasmAbiType};

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
            return Err(format!(
                "库导出 `{instance_label}` 参数类型 `{ty}` 没有 Wasm ABI 合同: {reason}"
            ));
        }
        params.push(schema);
    }

    let ret_schema = abi_schema_for_type(&function.return_type);
    if let Some(reason) = ret_schema.unsupported_reason() {
        return Err(format!(
            "库导出 `{instance_label}` 返回类型 `{}` 没有 Wasm ABI 合同: {reason}",
            function.return_type
        ));
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

/// 把收集到的 NamedAggregate identity keys 渲染为 `glue_entries` JSON 数组。
///
/// 本阶段只输出语义 skeleton：
/// - `operations` 永远是空数组
/// - `export_name` 留等 Stream C 补 valkyrie.v 的具体导出
/// - `source: "declaration_identity"` 标记身份来自编译器 schema
/// - `status: "pending_export_name"` 标记 export_name 占位待填
fn render_glue_entries(keys: &BTreeSet<String>) -> Vec<serde_json::Value> {
    let mut entries = Vec::with_capacity(keys.len());
    for key in keys {
        let Some((module_path, name, generic_json)) = split_aggregate_key(key) else {
            continue;
        };
        let generic_args: serde_json::Value =
            serde_json::from_str(generic_json).unwrap_or_else(|_| serde_json::json!([]));
        entries.push(serde_json::json!({
            "kind": "NamedAggregate",
            "name": name,
            "module_path": module_path,
            "generic_args": generic_args,
            "operations": [],
            "source": "declaration_identity",
            "status": "pending_export_name"
        }));
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
        let function = submission
            .backend_plan
            .get_function(instance)
            .ok_or_else(|| format!("库导出 `{instance}` 缺少 Compiler 函数体"))?
            .function;

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

    let glue_entries = render_glue_entries(&aggregate_keys);
    let payload = serde_json::json!({
        "exports": invoke_exports,
        "glue_entries": glue_entries,
    });

    module.sections.push(WasmSection {
        id: 0,
        name: Some("nyar.library_invoke".to_owned()),
        bytes: payload.to_string().into_bytes(),
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nyar_types::{Identifier, NyarType};

    fn nyar(s: &str) -> NyarType {
        NyarType::from_surface_primitive(s).expect("surface primitive")
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

        // 两个 list_i64 应该被去重成一个 key
        let entries = render_glue_entries(&keys);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["name"], "ArrayList");
        assert_eq!(entries[0]["module_path"], "std::collections");
        assert_eq!(entries[0]["status"], "pending_export_name");
        assert_eq!(entries[0]["source"], "declaration_identity");
        assert!(entries[0]["operations"].as_array().is_some_and(|a| a.is_empty()));
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
        assert_eq!(keys.len(), 1);
    }

    #[test]
    fn collect_distinguishes_by_module_path() {
        // 两个同名但不同 module_path 的 aggregate 应视为不同
        let a = WasmAbiType::NamedAggregate {
            name: "ArrayList".to_string(),
            module_path: "std::collections".to_string(),
            generic_args: vec![],
        };
        let b = WasmAbiType::NamedAggregate {
            name: "ArrayList".to_string(),
            module_path: "user::lib".to_string(),
            generic_args: vec![],
        };
        let mut keys = BTreeSet::new();
        collect_named_aggregate_keys(&a, &mut keys);
        collect_named_aggregate_keys(&b, &mut keys);
        assert_eq!(keys.len(), 2);
    }
}

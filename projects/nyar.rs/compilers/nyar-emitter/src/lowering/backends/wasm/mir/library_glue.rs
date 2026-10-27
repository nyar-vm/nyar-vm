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

/// 为 `name == "ArrayList"` 的 NamedAggregate 合成 intrinsic lowering 的
/// helper export operations。
///
/// # 约束
///
/// - 只处理 `name == "ArrayList"` 的场景；HashMap / Option / Tuple 等跳过
/// - 严格要求 `generic_args[0]` 是标量（有 `scalar_carrier_name`）；容器类型跳过
/// - 按 Wasm backend lowering intrinsic 的 pattern 合成：construct / push / length / at
///
/// # 合成 pattern
///
/// 设 element type 的 carrier 为 `E`（如 `"i64"`）、export tag 为 `T`（如 `"i64"`）：
///
/// | operation | export_name            | param_carriers    | return_carrier |
/// |-----------|------------------------|-------------------|----------------|
/// | construct | `$ArrayList_{T}_new`   | `[E]`             | `i32`          |
/// | push      | `$ArrayList_{T}_push`  | `["i32", E]`      | `void`         |
/// | length    | `$ArrayList_{T}_len`   | `["i32"]`         | `i32`          |
/// | at        | `$ArrayList_{T}_at`    | `["i32", "i32"]`  | `E`            |
///
/// construct 的第一个参数是 element type carrier——当前 intrinsic lowering 里
/// ArrayList 构造时会按 element size 预分配，这个 carrier 用于标识具体哪个
/// 合成函数。后续如果 construct 签名有变化，只改本表即可。
fn synthesize_array_list_operations(
    name: &str,
    generic_args: &[WasmAbiType],
) -> Option<Vec<serde_json::Value>> {
    if name != "ArrayList" {
        return None;
    }
    let element = generic_args.first()?;
    let carrier = scalar_carrier_name(element)?;
    let tag = scalar_export_tag(element)?;

    let new_export = format!("$ArrayList_{tag}_new");
    let push_export = format!("$ArrayList_{tag}_push");
    let len_export = format!("$ArrayList_{tag}_len");
    let at_export = format!("$ArrayList_{tag}_at");

    Some(vec![
        serde_json::json!({
            "operation": "construct",
            "export_name": new_export,
            "param_carriers": [carrier],
            "return_carrier": "i32"
        }),
        serde_json::json!({
            "operation": "push",
            "export_name": push_export,
            "param_carriers": ["i32", carrier],
            "return_carrier": "void"
        }),
        serde_json::json!({
            "operation": "length",
            "export_name": len_export,
            "param_carriers": ["i32"],
            "return_carrier": "i32"
        }),
        serde_json::json!({
            "operation": "at",
            "export_name": at_export,
            "param_carriers": ["i32", "i32"],
            "return_carrier": carrier
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
/// wasm backend 自己为 ArrayList 合成 helper export operations：
/// - `source: "backend_synthetic"` 标记 operations 来自 backend lowering 合成
/// - `status: "ready"` 标记 export_name 已可被 JS glue 消费
/// - 非 ArrayList 或 element 非标量的 aggregate 保持 v3 旧行为（operations 空）
fn render_glue_entries(keys: &BTreeSet<String>) -> Vec<serde_json::Value> {
    let mut entries = Vec::with_capacity(keys.len());
    for key in keys {
        let Some((module_path, name, generic_json)) = split_aggregate_key(key) else {
            continue;
        };
        let generic_args_json: serde_json::Value =
            serde_json::from_str(generic_json).unwrap_or_else(|_| serde_json::json!([]));
        // 同时解析结构化的 generic_args 供合成函数使用
        let generic_args_typed: Vec<WasmAbiType> = serde_json::from_str(generic_json)
            .unwrap_or_default();

        if let Some(operations) = synthesize_array_list_operations(name, &generic_args_typed) {
            entries.push(serde_json::json!({
                "kind": "NamedAggregate",
                "name": name,
                "module_path": module_path,
                "generic_args": generic_args_json,
                "operations": operations,
                "source": "backend_synthetic",
                "status": "ready"
            }));
        } else {
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
        // v4: ArrayList<i64> element 是标量，应被 backend synthetic 合成 operations
        assert_eq!(entries[0]["status"], "ready");
        assert_eq!(entries[0]["source"], "backend_synthetic");
        let ops = entries[0]["operations"].as_array().expect("operations array");
        assert_eq!(ops.len(), 4, "ArrayList<i64> 应有 construct/push/length/at 四个 operation");
        assert_eq!(ops[0]["operation"], "construct");
        assert_eq!(ops[0]["export_name"], "$ArrayList_i64_new");
        assert_eq!(ops[0]["param_carriers"], serde_json::json!(["i64"]));
        assert_eq!(ops[0]["return_carrier"], "i32");
        assert_eq!(ops[1]["operation"], "push");
        assert_eq!(ops[1]["export_name"], "$ArrayList_i64_push");
        assert_eq!(ops[1]["param_carriers"], serde_json::json!(["i32", "i64"]));
        assert_eq!(ops[1]["return_carrier"], "void");
        assert_eq!(ops[2]["operation"], "length");
        assert_eq!(ops[2]["export_name"], "$ArrayList_i64_len");
        assert_eq!(ops[2]["param_carriers"], serde_json::json!(["i32"]));
        assert_eq!(ops[2]["return_carrier"], "i32");
        assert_eq!(ops[3]["operation"], "at");
        assert_eq!(ops[3]["export_name"], "$ArrayList_i64_at");
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
        let entries = render_glue_entries(&keys);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["status"], "ready");
        assert_eq!(entries[0]["source"], "backend_synthetic");
        assert_eq!(entries[0]["operations"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn collect_distinguishes_by_module_path() {
        // 两个同名但不同 module_path 的 aggregate 应视为不同
        // 注意：没有 generic_args 的 ArrayList 不参与合成（无 element 类型），保持旧行为
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
        let entries = render_glue_entries(&keys);
        assert_eq!(entries.len(), 2);
        // 无 generic_args → 合成失败 → 旧行为
        for entry in &entries {
            assert_eq!(entry["status"], "pending_export_name");
            assert_eq!(entry["source"], "declaration_identity");
            assert!(entry["operations"].as_array().is_some_and(|a| a.is_empty()));
        }
    }

    // ---- v4 new: synthesize_array_list_operations unit tests ----

    #[test]
    fn synthesize_array_list_i32() {
        let ops = synthesize_array_list_operations(
            "ArrayList",
            &[WasmAbiType::I32],
        ).expect("ArrayList<i32> should synthesize");
        assert_eq!(ops.len(), 4);
        assert_eq!(ops[0]["export_name"], "$ArrayList_i32_new");
        assert_eq!(ops[0]["param_carriers"], serde_json::json!(["i32"]));
        assert_eq!(ops[1]["export_name"], "$ArrayList_i32_push");
        assert_eq!(ops[3]["return_carrier"], "i32");
    }

    #[test]
    fn synthesize_array_list_f64() {
        let ops = synthesize_array_list_operations(
            "ArrayList",
            &[WasmAbiType::F64],
        ).expect("ArrayList<f64> should synthesize");
        assert_eq!(ops[0]["export_name"], "$ArrayList_f64_new");
        assert_eq!(ops[1]["param_carriers"], serde_json::json!(["i32", "f64"]));
        assert_eq!(ops[3]["return_carrier"], "f64");
    }

    #[test]
    fn synthesize_array_list_bool_uses_i32_carrier_but_bool_tag() {
        let ops = synthesize_array_list_operations(
            "ArrayList",
            &[WasmAbiType::Bool],
        ).expect("ArrayList<bool> should synthesize");
        // carrier 是 i32（Wasm 层），但 export tag 保留 bool
        assert_eq!(ops[0]["export_name"], "$ArrayList_bool_new");
        assert_eq!(ops[0]["param_carriers"], serde_json::json!(["i32"]));
        assert_eq!(ops[3]["return_carrier"], "i32");
    }

    #[test]
    fn synthesize_array_list_skips_non_arraylist() {
        // HashMap → None
        assert!(synthesize_array_list_operations(
            "HashMap",
            &[WasmAbiType::I64, WasmAbiType::I64],
        ).is_none());
    }

    #[test]
    fn synthesize_array_list_skips_non_scalar_element() {
        // ArrayList<ArrayList<i64>> → element 是 NamedAggregate → None
        let nested = WasmAbiType::NamedAggregate {
            name: "ArrayList".to_string(),
            module_path: String::new(),
            generic_args: vec![WasmAbiType::I64],
        };
        assert!(synthesize_array_list_operations("ArrayList", &[nested]).is_none());
    }

    #[test]
    fn synthesize_array_list_skips_no_generic_args() {
        // 无 generic_args → None
        assert!(synthesize_array_list_operations("ArrayList", &[]).is_none());
    }
}

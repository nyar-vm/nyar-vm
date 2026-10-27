//! 库模式 JSON ABI：消费 `WasmAbiType` 结构化合同，序列化到
//! `nyar.library_invoke` 自定义段。
//!
//! # 历史
//!
//! v1（本阶段之前）：`abi_kind_for_type` 只认 `Integer64 → Some("i64")`，
//! 其余全部 None。任何带 `ArrayList<T>` / `Option<T>` / `utf8` 的导出
//! 都会被拒绝。
//!
//! v2（本阶段）：消费 `super::abi_schema::WasmAbiType`，完整序列化
//! NyarType 变体到 JSON。禁止硬编码、禁止缺 layout 时默认布局。
//!
//! # JSON 格式
//!
//! ```json
//! {
//!   "exports": {
//!     "solve": {
//!       "params": [
//!         {"kind": "NamedAggregate", "name": "ArrayList", "generic_args": [{"kind": "I64"}]},
//!         {"kind": "I64"}
//!       ],
//!       "returns": {"kind": "NamedAggregate", "name": "ArrayList", "generic_args": [{"kind": "I64"}]}
//!     }
//!   },
//!   "glue": {}
//! }
//! ```

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

pub(super) fn append_library_mode_glue(
    wasm_package_kind: WasmPackageKind,
    submission: &FragmentSubmission,
    module: &mut WasmBinaryModule,
) -> Result<()> {
    if wasm_package_kind != WasmPackageKind::Library {
        return Ok(());
    }
    let mut invoke_exports = serde_json::Map::new();
    for (instance, public_name) in submission.backend_plan.wasm_export_names() {
        let function = submission
            .backend_plan
            .get_function(instance)
            .ok_or_else(|| format!("库导出 `{instance}` 缺少 Compiler 函数体"))?
            .function;

        let instance_label = format!("{instance}");
        let (params_schema, returns_schema) = schematize_function_abi(&instance_label, &function)?;

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
    let payload = serde_json::json!({ "exports": invoke_exports, "glue": {} });
    module.sections.push(WasmSection {
        id: 0,
        name: Some("nyar.library_invoke".to_owned()),
        bytes: payload.to_string().into_bytes(),
    });
    Ok(())
}

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
//!         {"index": 0, "name": "nums", "type": {"kind": "I64"}, "carrier": "i64"}
//!       ],
//!       "returns": {"kind": "I64"}
//!     }
//!   },
//!   "abi_digest": "sha256-<hex>"
//! }
//! ```

use nyar_types::combined_content_hash;

use crate::{
    FragmentSubmission, LibraryExportAbi, Result,
    abi_structure::canonical_parameter_identities,
    backend_plan_views::ExecutableFunction,
    nyar_backend_wasi::{WasmBinaryModule, WasmPackageKind, WasmSection},
};

use super::abi_schema::{abi_schema_for_type, abi_schema_for_type_with_layouts};

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
/// Option / Tuple / 具名聚合）与引用存储名义实例（`InstanceHandle`，共享 GC
/// 引用）统一走 `handle`。`Unsupported` 不应出现在参数位，出现即失败。
fn abi_carrier_name(schema: &super::abi_schema::WasmAbiType) -> Result<&'static str> {
    use super::abi_schema::WasmAbiType;
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
        WasmAbiType::InstanceHandle { .. } => "handle",
        WasmAbiType::Array { .. }
        | WasmAbiType::FixedArray { .. }
        | WasmAbiType::Option { .. }
        | WasmAbiType::Tuple { .. }
        | WasmAbiType::NamedAggregate { .. }
        | WasmAbiType::Sum { .. } => "handle",
        WasmAbiType::Utf16 => {
            return Err(crate::lowering::features::physical_contract::UTF16_UNRESOLVED_TEXT_ABI.to_string());
        }
        WasmAbiType::Unsupported { reason } => return Err(format!("类型没有 Wasm ABI 载体: {reason}")),
    })
}

/// 把函数的参数/返回类型提升为 `WasmAbiType` schema，并同时产出带参数身份的
/// `params` JSON 条目。
///
/// `carrier` 字段只取 `abi` 中 plan 冻结的载体值；`abi_carrier_name` 仅用来做
/// 交叉核对，冻结值与 schema 不一致即结构化失败。任何 `Unsupported` 变体会
/// 立刻返回可读错误，**禁止**静默降级。
/// `instance_label` 仅用于错误信息（可以是 instance ID 的 Display 或公开名）。
fn schematize_function_abi(
    instance_label: &str,
    function: &ExecutableFunction,
    abi: &LibraryExportAbi,
    layouts: &nyar_types::AggregateLayoutPlan,
    layout_by_type: &std::collections::BTreeMap<nyar_types::NyarType, nyar_types::LayoutId>,
    sums: &std::collections::BTreeMap<nyar_types::NyarType, nyar_types::SumTypeLayout>,
) -> Result<(Vec<serde_json::Value>, super::abi_schema::WasmAbiType)> {
    let identities = canonical_parameter_identities(instance_label, function)?;
    if abi.params.len() != function.param_types.len() {
        return Err(format!(
            "库导出 `{instance_label}` 的冻结 ABI 参数载体数量与参数类型数量不一致: plan_params={} param_types={}",
            abi.params.len(),
            function.param_types.len()
        ));
    }
    let mut params_json = Vec::with_capacity(function.param_types.len());
    for (position, ty) in function.param_types.iter().enumerate() {
        let schema = abi_schema_for_type_with_layouts(ty, layouts, layout_by_type, sums);
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
    }

    let ret_schema = abi_schema_for_type_with_layouts(&function.return_type, layouts, layout_by_type, sums);
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
    Ok((params_json, ret_schema))
}

/// 把库模式导出 ABI 合同序列化进 `nyar.library_invoke` 自定义段。
pub(super) fn append_library_invoke_section(
    wasm_package_kind: WasmPackageKind,
    submission: &FragmentSubmission,
    module: &mut WasmBinaryModule,
) -> Result<()> {
    if wasm_package_kind != WasmPackageKind::Library {
        return Ok(());
    }
    let plan = &submission.backend_plan;
    let ctx = crate::lowering::shared::executable::ExecutableLoweringContext::new(submission);
    let mut invoke_exports = serde_json::Map::new();

    for (instance, public_name) in plan.wasm_export_names() {
        let function = plan.get_function(instance).ok_or_else(|| format!("库导出 `{instance}` 缺少 Compiler 函数体"))?.function;
        let abi = plan
            .library_export_abi(instance)
            .ok_or_else(|| format!("库导出 `{instance}`（公开名 `{public_name}`）没有冻结的 ABI 载体合同；backend 不得自行推导 carrier"))?;

        let instance_label = format!("{instance}");
        let (params_json, returns_schema) = schematize_function_abi(
            &instance_label,
            &function,
            abi,
            ctx.layouts,
            plan.aggregate_layout_by_nyar_type(),
            plan.sum_layout_by_nyar_type(),
        )?;

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
        return Err("库 Wasm 产物没有可表达的导出合同".to_string());
    }

    let abi_digest = abi_contract_digest(LIBRARY_INVOKE_SCHEMA_VERSION, &serde_json::Value::Object(invoke_exports.clone()));
    let payload = serde_json::json!({
        "version": LIBRARY_INVOKE_SCHEMA_VERSION,
        "exports": invoke_exports,
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
/// 对 `version` 与 `exports` 的规范化 JSON 做 SHA-256：同一 ABI 事实恒得同一
/// digest，协议版本或任一导出签名变化都会改变 digest。
fn abi_contract_digest(version: u64, exports: &serde_json::Value) -> String {
    let facts = serde_json::json!({
        "version": version,
        "exports": exports,
    });
    format!("sha256-{}", combined_content_hash(&[&canonical_json(&facts)]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AbiCarrier, LibraryExportAbi, TargetExecutionPlan};
    use nyar_types::{NyarType, ValueOrigin};

    fn nyar(s: &str) -> NyarType {
        NyarType::from_surface_primitive(s).expect("surface primitive")
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
        let digest_a = abi_contract_digest(LIBRARY_INVOKE_SCHEMA_VERSION, &exports_a);
        let digest_b = abi_contract_digest(LIBRARY_INVOKE_SCHEMA_VERSION, &exports_b);
        assert!(digest_a.starts_with("sha256-"), "digest should be prefixed: {digest_a}");
        assert_eq!(digest_a, digest_b, "同一 ABI 事实必须得到同一 digest");
    }

    #[test]
    fn abi_digest_changes_with_schema_version() {
        let exports: serde_json::Value = serde_json::from_str(r#"{"solve":{"params":[],"returns":{"kind":"I64"}}}"#).unwrap();
        let version_one = abi_contract_digest(1, &exports);
        let version_two = abi_contract_digest(2, &exports);
        assert_ne!(version_one, version_two, "schema 版本必须参与 digest 规范化输入");
    }

    #[test]
    fn canonical_json_sorts_object_keys() {
        let left: serde_json::Value = serde_json::from_str(r#"{"b":2,"a":1}"#).unwrap();
        let right: serde_json::Value = serde_json::from_str(r#"{"a":1,"b":2}"#).unwrap();
        assert_eq!(canonical_json(&left), canonical_json(&right));
        assert_eq!(canonical_json(&left), r#"{"a":1,"b":2}"#);
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
        let (params_json, returns) =
            schematize_function_abi(
                "twoSum",
                &function,
                &abi,
                &nyar_types::AggregateLayoutPlan::default(),
                &std::collections::BTreeMap::new(),
                &std::collections::BTreeMap::new(),
            )
            .expect("参数身份完整");
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
        let error =
            schematize_function_abi(
                "identity",
                &function,
                &abi,
                &nyar_types::AggregateLayoutPlan::default(),
                &std::collections::BTreeMap::new(),
                &std::collections::BTreeMap::new(),
            )
            .expect_err("参数名重复必须失败");
        assert!(error.contains("identity"), "error must carry the export name: {error}");
        assert!(error.contains("index=1") || error.contains("参数 1"), "error must carry the parameter index: {error}");
        assert!(error.contains("重复"), "error must explain the duplicate name: {error}");
    }

    #[test]
    fn schematize_rejects_missing_parameter_name() {
        let function = abi_function(&[(0, "", nyar("i64"))], nyar("i64"));
        let abi = frozen_abi(vec![AbiCarrier::I64], AbiCarrier::I64);
        let error =
            schematize_function_abi(
                "identity",
                &function,
                &abi,
                &nyar_types::AggregateLayoutPlan::default(),
                &std::collections::BTreeMap::new(),
                &std::collections::BTreeMap::new(),
            )
            .expect_err("参数名缺失必须失败");
        assert!(error.contains("identity"), "error must carry the export name: {error}");
        assert!(error.contains("缺少 canonical 参数名"), "error must explain the missing name: {error}");
    }

    #[test]
    fn schematize_rejects_identity_count_mismatch() {
        let mut function = abi_function(&[(0, "value", nyar("i64"))], nyar("i64"));
        function.param_types.push(nyar("i32"));
        let abi = frozen_abi(vec![AbiCarrier::I64, AbiCarrier::I32], AbiCarrier::I64);
        let error = schematize_function_abi(
            "identity",
            &function,
            &abi,
            &nyar_types::AggregateLayoutPlan::default(),
            &std::collections::BTreeMap::new(),
            &std::collections::BTreeMap::new(),
        )
        .expect_err("身份数量不一致必须失败");
        assert!(error.contains("数量不一致"), "error must explain the count mismatch: {error}");
    }

    #[test]
    fn schematize_rejects_carrier_disagreement_with_plan() {
        // plan 冻结参数为 Handle，但实际类型是标量 i32：交叉核对必须失败，
        // backend 不得以 schema 推导结果覆盖 plan 事实。
        let function = abi_function(&[(0, "value", nyar("i32"))], nyar("i32"));
        let abi = frozen_abi(vec![AbiCarrier::Handle], AbiCarrier::I32);
        let error = schematize_function_abi(
            "identity",
            &function,
            &abi,
            &nyar_types::AggregateLayoutPlan::default(),
            &std::collections::BTreeMap::new(),
            &std::collections::BTreeMap::new(),
        )
        .expect_err("冻结核对不一致必须失败");
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
        use crate::lowering::backends::wasm::mir::abi_schema::WasmAbiType;
        assert_eq!(abi_carrier_name(&WasmAbiType::I32).expect("i32"), "i32");
        assert_eq!(abi_carrier_name(&WasmAbiType::I64).expect("i64"), "i64");
        assert_eq!(abi_carrier_name(&WasmAbiType::F32).expect("f32"), "f32");
        assert_eq!(abi_carrier_name(&WasmAbiType::F64).expect("f64"), "f64");
        assert_eq!(abi_carrier_name(&WasmAbiType::Bool).expect("bool"), "i32");
        assert_eq!(abi_carrier_name(&WasmAbiType::Char).expect("char"), "i32");
        assert_eq!(abi_carrier_name(&WasmAbiType::Utf8).expect("utf8"), "i32");
        assert_eq!(abi_carrier_name(&WasmAbiType::Unit).expect("unit"), "void");
        for schema in [
            WasmAbiType::Array {
                element: Box::new(WasmAbiType::I64),
                ops: crate::lowering::backends::wasm::mir::abi_schema::WasmAbiArrayOps {
                    length: String::new(),
                    get: String::new(),
                    new: String::new(),
                    set: String::new(),
                },
                container: None,
            },
            WasmAbiType::FixedArray { element: Box::new(WasmAbiType::I64), length: 2 },
            WasmAbiType::Option { inner: Box::new(WasmAbiType::I64), box_contract: None },
            WasmAbiType::Tuple { fields: vec![WasmAbiType::I64] },
            WasmAbiType::NamedAggregate {
                name: "ArrayList".to_string(),
                module_path: String::new(),
                generic_args: vec![WasmAbiType::I64],
                fields: Vec::new(),
                constructor: None,
            },
        ] {
            assert_eq!(abi_carrier_name(&schema).expect("handle"), "handle");
        }
    }
}

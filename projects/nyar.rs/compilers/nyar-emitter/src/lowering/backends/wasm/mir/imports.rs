//! Host import collection / validation / index-building for Node + WASI.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    BackendImport, FragmentSubmission,
    backend_plan_views::{
        ExecutableConstant as MirConstant, ExecutableInstruction as MirInstruction, ExecutableInstructionKind as MirInstructionKind,
    },
    nyar_backend_wasi::WasiPreview,
    target_operator::TargetOperatorBinding,
};
use acorn_wasm::{VALTYPE_I32, VALTYPE_I64};
use nyar_types::{ItemInstanceId, NyarType};

use super::{
    super::{
        super::{
            executable::ExecutableLoweringContext,
            interop::{wasi_host_import_target, wasm_host_import_target, wasm_operator_target, wasm_or_js_glue_host_import_target},
        },
        sections::wasm_function_type,
    },
    representation::{wasm_js_glue_abi_value_type, wasm_param_value_type},
};

pub(super) fn collect_wasm_host_imports(submission: &FragmentSubmission, synthesize_const_utf8: bool) -> Vec<(String, String)> {
    let mut imports = Vec::new();
    let mut seen = BTreeSet::new();
    // 计划声明的导入是权威集合：`external_import_links` 与 `imports` 在完整流水线里
    // 应当同源，但测试夹具可能只登记其一；两边都收集，按 `(module, field)` 去重。
    let declared_links =
        submission.backend_plan.external_import_links().values().chain(submission.backend_plan.imports().values().map(|import| &import.link));
    for link in declared_links {
        let Some(target) = wasm_or_js_glue_host_import_target(link)
        else {
            continue;
        };
        let key = (target.module.to_string(), target.field.to_string());
        if seen.insert(key.clone()) {
            imports.push(key);
        }
    }
    if synthesize_const_utf8 {
        let key = ("env".to_string(), "const_utf8".to_string());
        if seen.insert(key.clone()) {
            imports.push(key);
        }
    }
    for field in crate::nyar_backend_wasm_js_glue::SYNTHETIC_NODE_IMPORT_FIELDS {
        let key = ("env".to_string(), (*field).to_string());
        if seen.insert(key.clone()) {
            imports.push(key);
        }
    }
    imports.sort();
    imports
}

pub(super) fn collect_wasi_host_imports(submission: &FragmentSubmission, preview: WasiPreview) -> Vec<(String, String)> {
    let mut imports = Vec::new();
    let mut seen = BTreeSet::new();
    for link in submission.backend_plan.external_import_links().values() {
        let Some(target) = wasi_host_import_target(link)
        else {
            continue;
        };
        let Some((module, field)) = crate::nyar_backend_wasi::wasi_adapt_import_for_preview(target.module, target.function, preview)
        else {
            continue;
        };
        let key = (crate::nyar_backend_wasi::wasi_versioned_import_module_for(&module, preview), field);
        if seen.insert(key.clone()) {
            imports.push(key);
        }
    }
    for link in submission.backend_plan.external_import_links().values() {
        let Some(target) = wasm_host_import_target(link)
        else {
            continue;
        };
        let key = (target.module.to_string(), target.field.to_string());
        if seen.insert(key.clone()) {
            imports.push(key);
        }
    }
    imports.sort();
    crate::nyar_backend_wasi::expand_wasi_cli_stream_imports(&mut imports);
    imports
}

pub(super) fn collect_mir_string_literals(submission: &FragmentSubmission, operations: &[ItemInstanceId]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut literals = Vec::new();
    let exec = &submission.backend_plan;
    for operation in operations {
        let Some(view) = exec.get_function(operation)
        else {
            continue;
        };
        for block in &view.function.blocks {
            for instruction in &block.instructions {
                if let MirInstructionKind::LoadConstant { constant: MirConstant::Utf8(text), .. } = &instruction.kind {
                    if seen.insert(text.clone()) {
                        literals.push(text.clone());
                    }
                }
            }
        }
    }
    literals
}

pub(super) fn const_utf8_import_index(imports: &[(String, String)]) -> Option<u32> {
    imports.iter().position(|(module, field)| module == "env" && field == "const_utf8").map(|index| index as u32)
}

pub(super) fn validate_string_literal_payloads(
    submission: &FragmentSubmission,
    operations: &[ItemInstanceId],
    string_literal_index: &BTreeMap<String, u32>,
    string_literal_offset: &BTreeMap<String, u32>,
    const_utf8_import: Option<u32>,
    wasi_mode: bool,
) -> crate::Result<()> {
    for operation in operations {
        let Some(view) = submission.backend_plan.get_function(operation)
        else {
            continue;
        };
        for block in &view.function.blocks {
            for (index, instruction) in block.instructions.iter().enumerate() {
                let MirInstructionKind::LoadConstant { constant: MirConstant::Utf8(text), .. } = &instruction.kind
                else {
                    continue;
                };
                let registered = if wasi_mode {
                    string_literal_offset.contains_key(text)
                }
                else {
                    const_utf8_import.is_some() && string_literal_index.contains_key(text)
                };
                if !registered {
                    return Err(format!(
                        "WASM pre-emission verifier: `{}` block {} instruction {index} 的 utf8 字面量缺少宿主载体（wasi={wasi_mode}, const_utf8={const_utf8_import:?}）；不得回落 `i32 0` 句柄",
                        view.function.symbol, block.id.0
                    ));
                }
            }
        }
    }
    Ok(())
}

pub(super) fn build_string_literal_index(literals: &[String]) -> BTreeMap<String, u32> {
    literals.iter().enumerate().map(|(index, text)| (text.clone(), index as u32)).collect()
}

/// `env` 合成宿主导入的物理签名。
///
/// 签名事实由 JS glue 协议所有者（[`crate::nyar_backend_wasm_js_glue`]）声明；
/// 本层只做 wasm 指令类型编码，不自行维护字段名匹配表。
pub(super) fn wasm_import_type_for_field(field: &str) -> Option<Vec<u8>> {
    crate::nyar_backend_wasm_js_glue::glue_import_signature(field).map(|(params, results)| wasm_function_type(params, results))
}

/// WASI core 导入的物理签名，事实由 WASI 协议所有者（[`crate::nyar_backend_wasi`]）声明。
pub(super) fn wasi_import_type_for_field(module: &str, field: &str) -> Option<Vec<u8>> {
    crate::nyar_backend_wasi::wasi_import_signature(module, field).map(|(params, results)| wasm_function_type(params, results))
}

/// `(module, field)` → 已在 `TargetExecutionPlan` 绑定的导入签名（`[js]` / `[wasm]` 等）。
pub(super) fn host_import_signature_index(submission: &FragmentSubmission) -> BTreeMap<(String, String), BackendImport> {
    let mut index = BTreeMap::new();
    for import in submission.backend_plan.imports().values() {
        let target = wasm_or_js_glue_host_import_target(&import.link).or_else(|| wasm_host_import_target(&import.link));
        let Some(target) = target
        else {
            continue;
        };
        let key = (target.module.to_string(), target.field.to_string());
        index.entry(key).or_insert_with(|| import.clone());
    }
    index
}

pub(super) fn wasm_import_type_from_backend_import(
    ctx: &ExecutableLoweringContext,
    import: &BackendImport,
    wasi_mode: bool,
    js_glue_utf8_as_anyref: bool,
) -> crate::Result<Vec<u8>> {
    // `WasiComponent` 的宿主导入没有冻结的 `utf16` 载体，仍走严格表；
    // `WasmJsGlue` 边界才按 `declared_text_projection` 把 `utf16` 降为 i32 宿主句柄。
    // 导入签名不在 `validate_text_encoding_projection` 的覆盖范围内，所以这里必须
    // 自己区分边界，不能依赖 verifier 兜底。
    let abi_value_type = |ty: &NyarType| {
        if wasi_mode {
            wasm_param_value_type(ctx, ty, js_glue_utf8_as_anyref)
        }
        else {
            wasm_js_glue_abi_value_type(ctx, ty, js_glue_utf8_as_anyref)
        }
    };
    let params = import.parameter_types.iter().map(abi_value_type).collect::<crate::Result<Vec<_>>>()?;
    let results = match &import.return_type {
        NyarType::Unit | NyarType::Bottom => Vec::new(),
        ty => vec![abi_value_type(ty)?],
    };
    Ok(wasm_function_type(&params, &results))
}

pub(super) fn wasm_import_type_bytes_for_host(
    submission: &FragmentSubmission,
    ctx: &ExecutableLoweringContext,
    wasi_mode: bool,
    module: &str,
    field: &str,
    js_glue_utf8_as_anyref: bool,
    signature_index: &BTreeMap<(String, String), BackendImport>,
) -> crate::Result<Vec<u8>> {
    // 单一事实源：`TargetExecutionPlan` 已冻结的 `BackendImport` 声明优先。
    // 字段名表只是历史快照，不得越过 plan 改写已声明的导入签名。
    let key = (module.to_string(), field.to_string());
    if let Some(import) = signature_index.get(&key) {
        return wasm_import_type_from_backend_import(ctx, import, wasi_mode, js_glue_utf8_as_anyref);
    }
    // fallback 只服务 plan 不存在的合成宿主导入（`env::const_utf8`、`NODE_HOST_IMPORT_FIELDS`
    // 里的 utf8_* 辅助、WASI 预演化的 stream 内建等）；它不得覆盖 plan 已声明的事实。
    let fallback = if wasi_mode { wasi_import_type_for_field(module, field) } else { wasm_import_type_for_field(field) };
    fallback.ok_or_else(|| format!("host import `{module}::{field}` has no frozen ABI signature"))
}

pub(super) fn wasm_import_type_bytes(wasi_mode: bool, module: &str, field: &str) -> crate::Result<Vec<u8>> {
    let resolved = if wasi_mode { wasi_import_type_for_field(module, field) } else { wasm_import_type_for_field(field) };
    resolved.ok_or_else(|| format!("host import `{module}::{field}` has no frozen ABI signature"))
}

pub(super) fn wasi_core_import_name<'a>(module: &'a str, field: &'a str, preview: WasiPreview) -> (&'a str, &'a str) {
    if preview == WasiPreview::Preview2 {
        let bare = module.split('@').next().unwrap_or(module);
        let canonical = match bare {
            "wasi:clocks/monotonic-clock" => Some("cm32p2|wasi:clocks/monotonic-clock@0.2"),
            "wasi:io/streams" => Some("cm32p2|wasi:io/streams@0.2"),
            _ => None,
        };
        if let Some(canonical) = canonical {
            return (canonical, field);
        }
    }
    (module, field)
}

pub(super) fn build_callee_import_index(submission: &FragmentSubmission, imports: &[(String, String)]) -> BTreeMap<ItemInstanceId, u32> {
    let import_index_by_key: BTreeMap<(String, String), u32> =
        imports.iter().enumerate().map(|(index, (module, field))| ((module.clone(), field.clone()), index as u32)).collect();
    let mut callee_import_index = BTreeMap::new();
    for (callee, import) in submission.backend_plan.imports() {
        let link = &import.link;
        let Some(target) = wasm_or_js_glue_host_import_target(link)
        else {
            continue;
        };
        let Some(&index) = import_index_by_key.get(&(target.module.to_string(), target.field.to_string()))
        else {
            continue;
        };
        callee_import_index.insert(*callee, index);
    }
    callee_import_index
}

pub(super) fn build_callee_wasm_operator_index(submission: &FragmentSubmission) -> BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)> {
    let mut callee_wasm_operator = BTreeMap::new();
    for (callee, import) in submission.backend_plan.imports() {
        let Some(mnemonic) = wasm_operator_target(&import.link)
        else {
            continue;
        };
        let binding = import.target_operator.unwrap_or(TargetOperatorBinding::Unfrozen);
        callee_wasm_operator.insert(*callee, (mnemonic.to_string(), binding));
    }
    callee_wasm_operator
}

pub(super) fn build_wasi_callee_import_index(
    submission: &FragmentSubmission,
    imports: &[(String, String)],
    preview: WasiPreview,
) -> BTreeMap<ItemInstanceId, u32> {
    let import_index_by_key: BTreeMap<(String, String), u32> =
        imports.iter().enumerate().map(|(index, (module, field))| ((module.clone(), field.clone()), index as u32)).collect();
    let mut callee_import_index = BTreeMap::new();
    for (callee, import) in submission.backend_plan.imports() {
        let link = &import.link;
        let Some(target) = wasi_host_import_target(link)
        else {
            continue;
        };
        let Some((module, field)) = crate::nyar_backend_wasi::wasi_adapt_import_for_preview(target.module, target.function, preview)
        else {
            continue;
        };
        let module = crate::nyar_backend_wasi::wasi_versioned_import_module_for(&module, preview);
        let Some(&index) = import_index_by_key.get(&(module, field))
        else {
            continue;
        };
        callee_import_index.insert(*callee, index);
    }
    for (callee, import) in submission.backend_plan.imports() {
        let link = &import.link;
        let Some(target) = wasm_host_import_target(link)
        else {
            continue;
        };
        let Some(&index) = import_index_by_key.get(&(target.module.to_string(), target.field.to_string()))
        else {
            continue;
        };
        callee_import_index.insert(*callee, index);
    }
    callee_import_index
}

#[cfg(test)]
mod host_import_signature_tests {
    use nyar::{ExternalImportLink, Identifier};

    use super::*;
    use crate::{FragmentSubmission, TargetExecutionPlan};
    use nyar_types::ImportIndex;

    #[test]
    fn wasm_import_type_uses_backend_import_when_field_not_frozen() {
        let import_id = ItemInstanceId::from_index(1).expect("import");
        let mut plan = TargetExecutionPlan::default();
        plan.testing_register_import(
            import_id,
            BackendImport {
                index: ImportIndex::from_index(0).expect("import index"),
                link: ExternalImportLink::host(Some(Identifier::new("js_builtin")), vec!["fetch".into()]),
                parameter_types: vec![NyarType::Integer32 { signed: true }, NyarType::Integer32 { signed: true }],
                return_type: NyarType::Integer32 { signed: true },
                target_operator: None,
            },
        );
        let submission = FragmentSubmission::from_backend_plan(plan);
        let ctx = ExecutableLoweringContext::new(&submission);
        let signatures = host_import_signature_index(&submission);
        let type_bytes = wasm_import_type_bytes_for_host(&submission, &ctx, false, "env", "fetch", false, &signatures)
            .expect("js_builtin fetch must derive wasm signature from BackendImport");
        let params = super::super::strings::wasm_function_type_param_bytes(&type_bytes);
        assert_eq!(params, vec![VALTYPE_I32, VALTYPE_I32]);
        assert_eq!(super::super::strings::wasm_function_type_result_byte(&type_bytes), Some(VALTYPE_I32));
    }

    #[test]
    fn wasm_import_type_prefers_backend_import_over_field_name_table() {
        let import_id = ItemInstanceId::from_index(1).expect("import");
        let mut plan = TargetExecutionPlan::default();
        plan.testing_register_import(
            import_id,
            BackendImport {
                index: ImportIndex::from_index(0).expect("import index"),
                // `env::utf8_length` 同时存在于字段名表（`[i32] -> [i32]`），
                // 这里由 plan 声明一个不同的签名，验证 plan 胜出。
                link: ExternalImportLink::host(Some(Identifier::new("js_builtin")), vec!["utf8_length".into()]),
                parameter_types: vec![NyarType::Integer32 { signed: true }, NyarType::Integer32 { signed: true }],
                return_type: NyarType::Integer64 { signed: true },
                target_operator: None,
            },
        );
        let submission = FragmentSubmission::from_backend_plan(plan);
        let ctx = ExecutableLoweringContext::new(&submission);
        let signatures = host_import_signature_index(&submission);
        assert!(signatures.contains_key(&("env".to_string(), "utf8_length".to_string())), "plan import 必须解析到 env::utf8_length");
        assert_eq!(
            wasm_import_type_for_field("utf8_length"),
            Some(wasm_function_type(&[VALTYPE_I32], &[VALTYPE_I32])),
            "字段名表快照应保持 `[i32] -> [i32]`"
        );
        let type_bytes = wasm_import_type_bytes_for_host(&submission, &ctx, false, "env", "utf8_length", false, &signatures)
            .expect("plan 声明的签名必须胜出");
        assert_eq!(super::super::strings::wasm_function_type_param_bytes(&type_bytes), vec![VALTYPE_I32, VALTYPE_I32]);
        assert_eq!(super::super::strings::wasm_function_type_result_byte(&type_bytes), Some(VALTYPE_I64));
    }
}

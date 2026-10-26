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

use super::representation::wasm_param_value_type;
use super::super::{
    super::executable::ExecutableLoweringContext,
    super::interop::{wasi_host_import_target, wasm_host_import_target, wasm_operator_target, wasm_or_js_glue_host_import_target},
    sections::wasm_function_type,
};

pub(super) fn collect_wasm_host_imports(submission: &FragmentSubmission, synthesize_const_utf8: bool) -> Vec<(String, String)> {
    let mut imports = Vec::new();
    let mut seen = BTreeSet::new();
    for link in submission.backend_plan.external_import_links().values() {
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
    for field in NODE_UTF8_HOST_IMPORT_FIELDS {
        let key = ("env".to_string(), (*field).to_string());
        if seen.insert(key.clone()) {
            imports.push(key);
        }
    }
    imports.sort();
    imports
}

pub(super) const NODE_UTF8_HOST_IMPORT_FIELDS: &[&str] = &[
    "utf8_trim",
    "utf8_length",
    "utf8_concat",
    "utf8_starts_with",
    "utf8_ends_with",
    "utf8_contains",
    "utf8_equals",
    "utf8_replace",
    "utf8_index_of",
    "utf8_slice",
    "utf8_to_lower",
    "utf8_to_upper",
];

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
    expand_wasi_cli_stream_intrinsics(&mut imports);
    imports
}

pub(super) fn expand_wasi_cli_stream_intrinsics(imports: &mut Vec<(String, String)>) {
    let modules: Vec<String> = imports
        .iter()
        .filter(|(module, field)| field == "write-via-stream" && (module.contains("stdout") || module.contains("stderr")))
        .map(|(module, _)| module.clone())
        .collect();
    for module in modules {
        for field in [
            "[stream-new-0]write-via-stream",
            "[stream-write-0]write-via-stream",
            "[stream-drop-writable-0]write-via-stream",
            "[future-drop-readable-1]write-via-stream",
        ] {
            let key = (module.clone(), field.to_string());
            if !imports.iter().any(|item| item == &key) {
                imports.push(key);
            }
        }
    }
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

pub(super) fn wasm_import_type_for_field(field: &str) -> Option<Vec<u8>> {
    let signature = match field {
        "cli_get_verbose" => wasm_function_type(&[], &[VALTYPE_I32]),
        "const_utf8" => wasm_function_type(&[VALTYPE_I32], &[VALTYPE_I32]),
        "cli_get_project" | "cli_get_target" | "cli_get_output" => wasm_function_type(&[], &[VALTYPE_I32]),
        "get_current_directory" | "get_input" | "read_source_byte" => wasm_function_type(&[], &[VALTYPE_I32]),
        "read_file_text" => wasm_function_type(&[VALTYPE_I32], &[VALTYPE_I32]),
        "file_exists" | "directory_exists" | "create_directory" => wasm_function_type(&[VALTYPE_I32], &[VALTYPE_I32]),
        "write_file_text" => wasm_function_type(&[VALTYPE_I32, VALTYPE_I32], &[VALTYPE_I32]),
        "get_files" => wasm_function_type(&[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32], &[VALTYPE_I32]),
        "utf8_length" => wasm_function_type(&[VALTYPE_I32], &[VALTYPE_I32]),
        "utf8_index_of" => wasm_function_type(&[VALTYPE_I32, VALTYPE_I32], &[VALTYPE_I32]),
        "utf8_trim" | "utf8_to_lower" | "utf8_to_upper" => wasm_function_type(&[VALTYPE_I32], &[VALTYPE_I32]),
        "utf8_concat" => wasm_function_type(&[VALTYPE_I32, VALTYPE_I32], &[VALTYPE_I32]),
        "utf8_starts_with" | "utf8_ends_with" | "utf8_contains" | "utf8_equals" => {
            wasm_function_type(&[VALTYPE_I32, VALTYPE_I32], &[VALTYPE_I32])
        }
        "utf8_replace" => wasm_function_type(&[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32], &[VALTYPE_I32]),
        "utf8_slice" => wasm_function_type(&[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32], &[VALTYPE_I32]),
        "emit_byte" => wasm_function_type(&[VALTYPE_I32], &[]),
        "System.Console" | "System.Diagnostics.Process" => wasm_function_type(&[VALTYPE_I32], &[]),
        _ => return None,
    };
    Some(signature)
}

pub(super) fn wasi_import_type_for_field(_module: &str, field: &str) -> Option<Vec<u8>> {
    let signature = match field {
        "get-arguments" | "get-environment" => wasm_function_type(&[VALTYPE_I32], &[]),
        "exit" => wasm_function_type(&[VALTYPE_I32], &[]),
        "now" | "resolution" | "get-resolution" if _module.contains("monotonic-clock") => wasm_function_type(&[], &[VALTYPE_I64]),
        "now" | "resolution" | "get-resolution" if _module.contains("wall-clock") || _module.contains("system-clock") => {
            wasm_function_type(&[], &[VALTYPE_I64, VALTYPE_I32])
        }
        "get-random-u64" | "get-insecure-random-u64" => wasm_function_type(&[], &[VALTYPE_I64]),
        "get-random-bytes" | "get-insecure-random-bytes" => wasm_function_type(&[VALTYPE_I32], &[VALTYPE_I32, VALTYPE_I32]),
        "insecure-seed" if _module.contains("wasi:random/insecure-seed") => wasm_function_type(&[], &[VALTYPE_I64, VALTYPE_I64]),
        "write-via-stream" if _module.contains("wasi:cli/stdout") || _module.contains("wasi:cli/stderr") => {
            wasm_function_type(&[VALTYPE_I32], &[VALTYPE_I32])
        }
        field if field.starts_with("[stream-new-") => wasm_function_type(&[], &[VALTYPE_I64]),
        field if field.starts_with("[stream-write-") || field.starts_with("[async][stream-write-") => {
            wasm_function_type(&[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32], &[VALTYPE_I32])
        }
        field if field.starts_with("[stream-drop-writable-") => wasm_function_type(&[VALTYPE_I32], &[]),
        field if field.starts_with("[future-drop-readable-") => wasm_function_type(&[VALTYPE_I32], &[]),
        "read-via-stream" if _module.contains("wasi:cli/stdin") => wasm_function_type(&[VALTYPE_I32], &[VALTYPE_I32]),
        "blocking-write-and-flush" | "blocking-read-and-skip" => wasm_function_type(&[VALTYPE_I32, VALTYPE_I32], &[VALTYPE_I32]),
        _ => return None,
    };
    Some(signature)
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
    js_glue_utf8_as_anyref: bool,
) -> Vec<u8> {
    let params = import
        .parameter_types
        .iter()
        .map(|ty| wasm_param_value_type(ctx, ty, js_glue_utf8_as_anyref))
        .collect::<Vec<_>>();
    let results = match &import.return_type {
        NyarType::Unit | NyarType::Bottom => Vec::new(),
        ty => vec![wasm_param_value_type(ctx, ty, js_glue_utf8_as_anyref)],
    };
    wasm_function_type(&params, &results)
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
    if let Some(frozen) = if wasi_mode {
        wasi_import_type_for_field(module, field)
    }
    else {
        wasm_import_type_for_field(field)
    } {
        return Ok(frozen);
    }
    let key = (module.to_string(), field.to_string());
    if let Some(import) = signature_index.get(&key) {
        return Ok(wasm_import_type_from_backend_import(ctx, import, js_glue_utf8_as_anyref));
    }
    Err(format!("host import `{module}::{field}` has no frozen ABI signature"))
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
                link: ExternalImportLink::host(Some(Identifier::new("js")), vec!["fetch".into()]),
                parameter_types: vec![NyarType::Integer32 { signed: true }, NyarType::Integer32 { signed: true }],
                return_type: NyarType::Integer32 { signed: true },
                target_operator: None,
            },
        );
        let submission = FragmentSubmission::from_backend_plan(plan);
        let ctx = ExecutableLoweringContext::new(&submission);
        let signatures = host_import_signature_index(&submission);
        let type_bytes = wasm_import_type_bytes_for_host(&submission, &ctx, false, "env", "fetch", false, &signatures)
            .expect("js fetch must derive wasm signature from BackendImport");
        let params = super::super::strings::wasm_function_type_param_bytes(&type_bytes);
        assert_eq!(params, vec![VALTYPE_I32, VALTYPE_I32]);
        assert_eq!(
            super::super::strings::wasm_function_type_result_byte(&type_bytes),
            Some(VALTYPE_I32)
        );
    }
}

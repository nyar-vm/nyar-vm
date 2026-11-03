//! Module-level entry functions.

use super::{
    super::{
        super::executable::ExecutableLoweringContext,
        cabi::{
            CABI_HEAP_GLOBAL_INDEX, cabi_heap_base_after_data, cabi_heap_global_section, memory_min_pages_for_heap_base,
            wasm_cabi_realloc_bump_body,
        },
        gc::{WASM_GC_ANYREF, wasm_gc_array_type, wasm_gc_struct_type},
        sections::{
            code_section_bytes, data_section_bytes, decode_uleb128, elem_section_bytes, encode_sleb128_i32, encode_sleb128_i64, encode_uleb128,
            export_section_bytes, function_section_bytes, import_section_bytes, memory_section_bytes, table_section_bytes, type_section_bytes,
            wasm_function_body, wasm_function_type,
        },
    },
    HostHandleToManagedUtf8Plan, ManagedUtf8ToHostHandlePlan, MirInstructionKind, MirOperand, StorageKind, TargetOperatorBinding,
    Utf8ArrayElementMaterializePlan, WASM_GC_EXTERNREF, WasmGcArrayOperation,
    imports::{
        build_callee_import_index, build_callee_wasm_operator_index, build_string_literal_index, build_wasi_callee_import_index,
        collect_mir_string_literals, collect_wasi_host_imports, collect_wasm_host_imports, const_utf8_import_index,
        host_import_signature_index, validate_string_literal_payloads, wasi_core_import_name, wasm_import_type_bytes_for_host,
    },
    library_invoke,
    lowerer::lower_mir_function_to_wasm_bytes,
    representation::{
        is_js_glue_host_string_type, layout_needs_gc_struct, type_is_inline_aggregate, type_is_wasm_gc_heap_reference,
        wasm_gc_field_type_byte_for_glue, wasm_param_types, wasm_return_value_type,
    },
    strings::{
        build_wasi_string_data_section, build_wasi_string_literal_offsets, wasi_string_data_section_size, wasm_function_type_param_bytes,
        wasm_function_type_result_byte,
    },
    type_registry::{register_gc_array_types, register_gc_struct_types, register_gc_sum_types, wasm_array_element_type_key},
};
use crate::{
    FragmentSubmission,
    backend_plan_views::NyarType,
    contracts::instruction_primary_result,
    nyar_backend_wasi::{WasiPreview, WasmBinaryModule, WasmPackageKind, WasmSection},
};
use acorn_wasm::{
    BLOCKTYPE_EMPTY, VALTYPE_ANYREF, VALTYPE_EXTERNREF, VALTYPE_F64, VALTYPE_I32, VALTYPE_I64, VALTYPE_REF_NULL, WasmExternalKind, WasmOpcode,
    encode_array_copy, encode_array_get, encode_array_len, encode_array_new_default, encode_array_new_fixed, encode_array_set,
    encode_block_empty, encode_br, encode_br_if, encode_call, encode_drop, encode_f64_const, encode_f64_load, encode_f64_store,
    encode_global_get, encode_global_set, encode_i32_add, encode_i32_and, encode_i32_const, encode_i32_load, encode_i32_store,
    encode_i64_const, encode_i64_load, encode_i64_store, encode_local_get, encode_local_set, encode_local_tee, encode_loop_empty,
    encode_memarg, encode_memory_copy, encode_memory_fill, encode_memory_grow, encode_memory_size, encode_ref_cast_array,
    encode_ref_cast_type_index, encode_ref_null_anyref, encode_ref_null_externref, encode_struct_get, encode_struct_new_default,
    encode_struct_set, encode_unreachable,
};

use nyar_types::{AggregateLayout, FieldLayout, ItemInstanceId, LayoutId};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn build_param_types_by_instance(
    ctx: &ExecutableLoweringContext,
    submission: &FragmentSubmission,
    gc_struct_type_indices: &BTreeMap<LayoutId, u32>,
    js_glue_utf8_as_anyref: bool,
) -> BTreeMap<ItemInstanceId, Vec<u8>> {
    submission
        .backend_plan
        .instances()
        .into_iter()
        .map(|instance| {
            let view = submission.backend_plan.get_function(&instance).expect("Compiler 实例必须有函数体");
            (instance, wasm_param_types(ctx, &view.function, gc_struct_type_indices, js_glue_utf8_as_anyref))
        })
        .collect()
}

pub(super) fn build_return_types_by_instance(
    ctx: &ExecutableLoweringContext,
    submission: &FragmentSubmission,
    gc_struct_type_indices: &BTreeMap<LayoutId, u32>,
    js_glue_utf8_as_anyref: bool,
) -> BTreeMap<ItemInstanceId, Option<u8>> {
    submission
        .backend_plan
        .instances()
        .into_iter()
        .map(|instance| {
            let view = submission.backend_plan.get_function(&instance).expect("Compiler 实例必须有函数体");
            (instance, wasm_return_value_type(ctx, &view.function, gc_struct_type_indices, js_glue_utf8_as_anyref))
        })
        .collect()
}

pub(crate) fn lower_fragment_mir_to_wasm_module(
    submission: &FragmentSubmission,
    export_name: &str,
) -> crate::Result<(WasmBinaryModule, Vec<(String, String)>)> {
    lower_fragment_mir_to_wasm_module_for(
        submission,
        export_name,
        crate::nyar_backend_wasi::WasiPreview::Preview2,
        crate::nyar_backend_wasi::WasmPackageKind::Binary,
    )
}

pub(crate) fn lower_fragment_mir_to_wasm_module_for(
    submission: &FragmentSubmission,
    export_name: &str,
    wasi_preview: crate::nyar_backend_wasi::WasiPreview,
    wasm_package_kind: crate::nyar_backend_wasi::WasmPackageKind,
) -> crate::Result<(WasmBinaryModule, Vec<(String, String)>)> {
    // 存储类选择的唯一闸门：编码前拒绝无身份的名义类型残差。
    // 形态判断发生在 `storage_for_type` / `wasm_param_value_type` / GC 字段字节之前。
    crate::lowering::shared::executable::validate_storage_identities(submission).map_err(|error| {
        let owner = if error.function.is_empty() { error.site.clone() } else { format!("{} at {}", error.function, error.site) };
        format!("WASM pre-emission verifier: {owner} 携带无身份名义类型 `{}`；declaration / `TypeId` 缺失时不得按短名解析存储类", error.ty)
    })?;
    let wasi_mode = export_name == "_start";
    let operations = submission.backend_plan.instances();
    // 字符串字面量属于被发射模块本身，与导出名是 `main` 还是库导出无关；
    // 只有常量载体（`const_utf8` / WASI 偏移）在两种宿主形态下不同。
    let string_literals = collect_mir_string_literals(submission, &operations);
    let mut host_imports = if wasi_mode {
        collect_wasi_host_imports(submission, wasi_preview)
    }
    else {
        // 入口模块与库导出统一消费 fragment/plan 冻结身份声明的宿主导入表；
        // 确有字符串字面量时再合成 `const_utf8`。宿主导入 callable 不能因为
        // 导出名不是 `main` 就失去 `callee_import_index` 注册路径。
        collect_wasm_host_imports(submission, !string_literals.is_empty())
    };
    // `import_count` 在 managed→host 句柄转换导入按需合成之后再取（见下方冻结步）。
    let callee_import_index = if wasi_mode {
        build_wasi_callee_import_index(submission, &host_imports, wasi_preview)
    }
    else {
        build_callee_import_index(submission, &host_imports)
    };
    let callee_wasm_operator = build_callee_wasm_operator_index(submission);
    // Node JS-glue：宿主导入与 `.mjs` 启动壳统一使用 i32 句柄（`hostIntern` / `const_utf8`），
    // ?`wasm_import_type_for_field` ?i32 签名对齐。若?utf8 标成 anyref，call 实参?
    // 被降?`ref.null`，丢?`cli_get_*` 返回的句柄（自举 `exports.build` 会静默失败）?
    // WASI 轨同样以线性内?i32 偏移传字符串?
    let js_glue_utf8_as_anyref = false;
    let ctx = ExecutableLoweringContext::new(submission);
    let host_import_signatures = host_import_signature_index(submission);
    let mut module = WasmBinaryModule::new();
    // main_type ?type 段的?0 ?() -> i32，供 `build` 桩函数使用?
    // ?MIR 函数的类型从?1 项开始追加，各自携带独立的返回类型?
    let main_type = wasm_function_type(&[], &[VALTYPE_I32]);
    let mut type_indices = vec![main_type];
    let mut import_type_entries = Vec::new();
    for (module, field) in &host_imports {
        let import_type_index = u32::try_from(type_indices.len()).expect("type index overflow");
        let type_bytes =
            wasm_import_type_bytes_for_host(submission, &ctx, wasi_mode, module, field, js_glue_utf8_as_anyref, &host_import_signatures)?;
        type_indices.push(type_bytes);
        import_type_entries.push(import_type_index);
    }
    let mut function_indices = Vec::new();
    let mut code_bodies = Vec::new();
    let mut exports = Vec::new();
    // 语言规范：wasm/wasi 强制 wasm-gc。始终为引用 layout 注册 structtype?
    let gc_struct_type_indices = register_gc_struct_types(&ctx, &mut type_indices, js_glue_utf8_as_anyref);
    // ?heap array element_type 注册 wasm-gc arraytype（Node/V8 ?WASI/wasmtime 均支持）?
    // 必须?glue 感知路径：`[utf8]` →?i32 元，禁止 Named→anyref 误登记?
    let mut gc_array_type_indices = register_gc_array_types(&ctx, &mut type_indices, js_glue_utf8_as_anyref);
    // sum 的物理布局（tag + anyref payload）是后端私有决定，必须按 nominal identity 单独注册，
    // 让 `SumNew` 构造的对象与 `SumPayloadGet` / `SumVariantIs` 的 `ref.cast` 指向同一条目。
    let gc_sum_type_indices = register_gc_sum_types(&ctx, &mut type_indices);
    // Scalar unite payloads such as utf8/bool/i32 handles need boxing into an
    // `[i32]` struct before entering the anyref payload slot.
    let gc_i32_box_type_index = {
        let type_index = u32::try_from(type_indices.len()).expect("type index overflow");
        type_indices.push(wasm_gc_struct_type(&[VALTYPE_I32]));
        type_index
    };
    let gc_i64_box_type_index = {
        let type_index = u32::try_from(type_indices.len()).expect("type index overflow");
        type_indices.push(wasm_gc_struct_type(&[VALTYPE_I64]));
        type_index
    };
    // 合同 §5：managed 字符串 → 宿主 i32 句柄的转换在目标计划阶段冻结，编码期只消费配方表。
    // `NyarType::Utf8` SSA 值携带已登记 gc struct layout 时，宿主导入边界必须按配方转换；
    // 合成导入 `env::host_intern_utf8` 仅在模块确有 managed 字符串时按需追加，因此
    // `import_count` 在冻结步之后再取。
    let managed_utf8_to_host_handle = if wasi_mode {
        BTreeMap::new()
    }
    else {
        freeze_managed_utf8_to_host_handle_plans(
            submission,
            &ctx,
            &gc_struct_type_indices,
            &mut gc_array_type_indices,
            &mut type_indices,
            js_glue_utf8_as_anyref,
            &mut host_imports,
            &mut import_type_entries,
            &host_import_signatures,
        )?
    };
    // 合同 §5：反方向边界。库导出形参的 SSA 值绑定已登记 gc struct layout 时物理槽是
    // anyref，而库 ABI 载体冻结为 i32 宿主句柄；宿主句柄直送 anyref 槽会让函数体内首个
    // `ref.cast` 命中包装句柄而 `illegal cast`。导出边界必须按配方把句柄物化为 managed
    // 字符串，配方表同样在目标计划阶段冻结；合成导入 `env::host_utf8_extract` 按需追加，
    // `import_count` 因此在本冻结步之后再取。
    let host_handle_to_managed_utf8 = if wasi_mode {
        BTreeMap::new()
    }
    else {
        freeze_host_handle_to_managed_utf8_plans(
            submission,
            &ctx,
            &gc_struct_type_indices,
            &mut gc_array_type_indices,
            &mut type_indices,
            js_glue_utf8_as_anyref,
            &mut host_imports,
            &mut import_type_entries,
            &host_import_signatures,
        )?
    };
    // 合同 §5：`[utf8]` 数组元素边界。`array.get` 结果 SSA 值绑定已登记 gc struct
    // layout 时物理槽是 anyref，而元素字节按冻结合同是 i32 宿主句柄；编码期必须按
    // 配方把句柄物化为 managed 字符串，配方表同样在目标计划阶段冻结；合成导入
    // `env::host_utf8_extract` 按需追加，`import_count` 因此在本冻结步之后再取。
    let utf8_array_element_materialize = if wasi_mode {
        BTreeMap::new()
    }
    else {
        freeze_utf8_array_element_materialize_plans(
            submission,
            &ctx,
            &callee_wasm_operator,
            &gc_struct_type_indices,
            &mut gc_array_type_indices,
            &mut type_indices,
            js_glue_utf8_as_anyref,
            &mut host_imports,
            &mut import_type_entries,
            &host_import_signatures,
        )?
    };
    let import_count = u32::try_from(host_imports.len()).expect("import count overflow");
    // 函数类型?structtype/arraytype/sumtype 之后开始追加?
    // type_indices 当前?[main_type, structtype_1, ..., structtype_N]?
    // 第一个函数类型应放在 N+1 处，对应 type_index = type_indices.len()?
    // 若仍用硬编码 `index + 1`，函数声明会指向 structtype 条目?
    // 导致 V8 ?`no signature at index 1 (N types)`?
    let function_type_base = u32::try_from(type_indices.len()).expect("type index overflow");
    let string_literal_index = build_string_literal_index(&string_literals);
    // WASI 轨：构建字符串字面量到线性内存偏移量的映射，存入 data 段?
    let string_literal_offset = if wasi_mode { build_wasi_string_literal_offsets(&string_literals) } else { BTreeMap::new() };
    let const_utf8_import = const_utf8_import_index(&host_imports);
    validate_string_literal_payloads(submission, &operations, &string_literal_index, &string_literal_offset, const_utf8_import, wasi_mode)?;
    // 仅对确有 MIR 体的 operation 分配稠密函数下标，避免「operations 枚举下标」与
    // `code_bodies.len()` 错位：错位时 call 会打到别人的 (param i64) 却按本函?anyref 签名?ref.null?
    let mir_operations: Vec<(nyar_types::ItemInstanceId, _)> = operations
        .iter()
        .filter_map(|operation| submission.backend_plan.get_function(operation).map(|view| (*operation, view.function)))
        .collect();
    let base_function_index = import_count;
    let mut function_index_by_instance = BTreeMap::new();
    let mut type_index_by_instance = BTreeMap::new();
    let mut param_types_by_function_index: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    let mut return_types_by_function_index: BTreeMap<u32, Option<u8>> = BTreeMap::new();
    for (dense, (instance, mir_fn)) in mir_operations.iter().enumerate() {
        let wasm_idx = base_function_index + dense as u32;
        let type_idx = function_type_base + dense as u32;
        let params = wasm_param_types(&ctx, mir_fn, &gc_struct_type_indices, js_glue_utf8_as_anyref);
        let ret = wasm_return_value_type(&ctx, mir_fn, &gc_struct_type_indices, js_glue_utf8_as_anyref);
        param_types_by_function_index.insert(wasm_idx, params);
        return_types_by_function_index.insert(wasm_idx, ret);
        function_index_by_instance.insert(*instance, wasm_idx);
        type_index_by_instance.insert(*instance, type_idx);
    }
    // 具名函数作值：预扫描全部可执行体，收集 `LoadFunctionRef` 引用的实例并按身份
    // 顺序登记 funcref table 槽位；只有被作值引用的函数才进 table。引用的实例没有
    // 函数体（导入或未生成）属于计划事实缺失，编码前结构化失败，禁止现场补造槽位。
    let mut function_value_instances: BTreeSet<ItemInstanceId> = BTreeSet::new();
    for (_, mir_fn) in mir_operations.iter() {
        for block in &mir_fn.blocks {
            for instruction in &block.instructions {
                if let MirInstructionKind::LoadFunctionRef { instance } = &instruction.kind {
                    function_value_instances.insert(*instance);
                }
            }
        }
    }
    let function_value_table_slots: BTreeMap<ItemInstanceId, u32> =
        function_value_instances.iter().copied().enumerate().map(|(slot, instance)| (instance, slot as u32)).collect();
    let function_value_table_functions: Vec<u32> = function_value_instances
        .iter()
        .map(|instance| {
            function_index_by_instance.get(instance).copied().ok_or_else(|| format!("WASM 函数值 `{instance:?}` 引用的实例没有可执行函数体"))
        })
        .collect::<crate::Result<Vec<_>>>()?;
    // 间接调用的 typeidx 只能反查模块已注册的 functype：按签名字节建表，
    // 禁止为间接调用新造类型条目或按参数个数猜签名。
    let mut type_index_by_signature: BTreeMap<(Vec<u8>, Option<u8>), u32> = BTreeMap::new();
    for (dense, (_, mir_fn)) in mir_operations.iter().enumerate() {
        let params = wasm_param_types(&ctx, mir_fn, &gc_struct_type_indices, js_glue_utf8_as_anyref);
        let ret = wasm_return_value_type(&ctx, mir_fn, &gc_struct_type_indices, js_glue_utf8_as_anyref);
        type_index_by_signature.entry((params, ret)).or_insert(function_type_base + dense as u32);
    }
    // Node JS-glue：宿主字符串?anyref 传递；?`wasm_import_type_for_field` ?anyref 签名对齐?
    let param_types_by_instance = build_param_types_by_instance(&ctx, submission, &gc_struct_type_indices, js_glue_utf8_as_anyref);
    let return_types_by_instance = build_return_types_by_instance(&ctx, submission, &gc_struct_type_indices, js_glue_utf8_as_anyref);
    let import_param_types: Vec<Vec<u8>> = host_imports
        .iter()
        .map(|(module, field)| -> crate::Result<Vec<u8>> {
            let type_bytes =
                wasm_import_type_bytes_for_host(submission, &ctx, wasi_mode, module, field, js_glue_utf8_as_anyref, &host_import_signatures)?;
            Ok(wasm_function_type_param_bytes(&type_bytes))
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let import_return_types: Vec<Option<u8>> = host_imports
        .iter()
        .map(|(module, field)| -> crate::Result<Option<u8>> {
            let type_bytes =
                wasm_import_type_bytes_for_host(submission, &ctx, wasi_mode, module, field, js_glue_utf8_as_anyref, &host_import_signatures)?;
            Ok(wasm_function_type_result_byte(&type_bytes))
        })
        .collect::<crate::Result<Vec<_>>>()?;
    let mut mir_wasm_functions: Vec<(u32, String)> = Vec::new();

    for (dense, (instance, mir_fn)) in mir_operations.iter().enumerate() {
        let full = submission.backend_plan.abi_name_for_instance(*instance).ok_or_else(|| format!("函数实例缺少 ABI 标签"))?.to_string();
        let param_types = wasm_param_types(&ctx, mir_fn, &gc_struct_type_indices, js_glue_utf8_as_anyref);
        // 每个函数独立计算返回类型,取代旧的模块?`returns_i32`?
        // 引用返回类型 →?anyref (VALTYPE_ANYREF)；i32 →?VALTYPE_I32；void →?空?
        let return_value_type = wasm_return_value_type(&ctx, mir_fn, &gc_struct_type_indices, js_glue_utf8_as_anyref);
        let result_types: &[u8] = match &return_value_type {
            Some(byte) => std::slice::from_ref(byte),
            None => &[],
        };
        let fn_type = wasm_function_type(&param_types, result_types);
        type_indices.push(fn_type);
        let type_index = function_type_base + dense as u32;
        function_indices.push(type_index);
        let wasm_index = import_count + u32::try_from(code_bodies.len()).expect("code body index overflow");
        code_bodies.push(wasm_function_body(lower_mir_function_to_wasm_bytes(
            submission,
            mir_fn,
            return_value_type,
            js_glue_utf8_as_anyref,
            wasi_mode,
            &function_index_by_instance,
            &type_index_by_instance,
            &param_types_by_instance,
            &return_types_by_instance,
            &param_types_by_function_index,
            &return_types_by_function_index,
            &import_param_types,
            &import_return_types,
            &gc_struct_type_indices,
            &gc_array_type_indices,
            &gc_sum_type_indices,
            gc_i32_box_type_index,
            gc_i64_box_type_index,
            &callee_import_index,
            &callee_wasm_operator,
            &host_imports,
            &string_literal_index,
            &string_literal_offset,
            const_utf8_import,
            &function_value_table_slots,
            &type_index_by_signature,
            &managed_utf8_to_host_handle,
            &utf8_array_element_materialize,
        )?));
        mir_wasm_functions.push((wasm_index, full.clone()));
        if let Some(public_name) = submission.backend_plan.wasm_export_names().get(instance) {
            exports.push((public_name.as_str(), WasmExternalKind::Func.as_u8(), wasm_index));
        }
        else if submission.backend_plan.entry_operation() == Some(*instance) {
            exports.push((export_name, WasmExternalKind::Func.as_u8(), wasm_index));
        }
    }

    if exports.is_empty() {
        return Err(format!("Wasm 缺少已绑定的入口或公开导出，禁止重发函数体或选择首函数"));
    }

    // WASI command world: wasmtime looks for `wasi:cli/run@-?run` returning result (i32).
    // Keep `_start` / short `run` for raw core runners; synthesize a result wrapper when needed.
    let wasi_cli_run_export = crate::nyar_backend_wasi::wasi_cli_run_export_name_for(wasi_preview);
    if wasi_mode {
        if let Some((_, _, index)) =
            exports.iter().find(|(name, kind, _)| *name == "_start" && *kind == WasmExternalKind::Func.as_u8()).copied()
        {
            // Entry may be `[] -> []` / `[] -> i32`, or still carry guest argv as GC array
            // (e.g. a guest entry taking `[utf8]` and returning unit). WIT `run: func() -> result` and core
            // `_start` are zero-arg. Synthesize nullary wrappers that push typed defaults
            // (empty `[utf8]` via `array.new_default`, not `ref.null` -?null traps on
            // `ref.cast`/`array.len`), call entry, drop any return, then Ok(0) for CLI run.
            // (Nonzero guest status belongs on `wasi:cli/exit`, not this empty `result`.)
            // Full argv via `get-arguments` cabi is follow-up; empty argv still exercises help.
            let entry_instance = submission.backend_plan.entry_operation().expect("WASI 入口必须保留 Compiler 实例身份");
            let entry_return = return_types_by_instance.get(&entry_instance).copied();
            let entry_mir_params: Vec<NyarType> = submission
                .backend_plan
                .entry_operation()
                .as_ref()
                .and_then(|instance| submission.backend_plan.get_function(instance))
                .map(|view| view.function.param_types.clone())
                .unwrap_or_default();
            let entry_wasm_params = param_types_by_function_index.get(&index).cloned().expect("WASI 入口缺少物理签名");

            let get_arguments_import = host_imports.iter().position(|(_, field)| field == "get-arguments").map(|index| index as u32);
            let argv_array_ty = gc_array_type_indices.get("Utf8").copied();
            let entry_needs_argv = entry_mir_params.iter().any(
                |ty| matches!(ty, NyarType::Array(element) | NyarType::FixedArray { element, .. } if is_js_glue_host_string_type(element.as_ref())),
            ) || (entry_mir_params.is_empty()
                && entry_wasm_params.iter().any(|ty| *ty == VALTYPE_ANYREF || *ty == WASM_GC_ANYREF));

            let build_entry_wrapper_body = |with_ok_i32: bool| -> Vec<u8> {
                let use_get_arguments = get_arguments_import.zip(argv_array_ty).filter(|_| entry_needs_argv);
                let mut body = Vec::new();
                if let Some((_imp, arr_ty)) = use_get_arguments {
                    // locals: 7×i32 + 1×(ref null $arr_ty)
                    encode_uleb128(2, &mut body);
                    encode_uleb128(7, &mut body);
                    body.push(VALTYPE_I32);
                    encode_uleb128(1, &mut body);
                    body.push(VALTYPE_REF_NULL);
                    encode_sleb128_i32(arr_ty as i32, &mut body);
                }
                else {
                    encode_uleb128(0, &mut body);
                }
                if let Some((import_index, array_ty)) = use_get_arguments {
                    emit_wasi_argv_from_get_arguments(import_index, array_ty, &mut body);
                    // Remaining non-argv params (rare): still push defaults after argv.
                    if entry_mir_params.len() > 1 {
                        for ty in entry_mir_params.iter().skip(1) {
                            emit_wasi_entry_default_arg(ty, &gc_array_type_indices, &mut body);
                        }
                    }
                }
                else if !entry_mir_params.is_empty() {
                    for ty in &entry_mir_params {
                        emit_wasi_entry_default_arg(ty, &gc_array_type_indices, &mut body);
                    }
                }
                else {
                    for &param_ty in &entry_wasm_params {
                        match param_ty {
                            VALTYPE_I64 => encode_i64_const(0, &mut body),
                            VALTYPE_F64 => encode_f64_const(0.0, &mut body),
                            VALTYPE_ANYREF | WASM_GC_ANYREF => {
                                if let Some(array_ty) = argv_array_ty {
                                    encode_i32_const(0, &mut body);
                                    encode_array_new_default(array_ty, &mut body);
                                }
                                else {
                                    encode_ref_null_anyref(&mut body);
                                }
                            }
                            VALTYPE_EXTERNREF | WASM_GC_EXTERNREF => encode_ref_null_externref(&mut body),
                            _ => encode_i32_const(0, &mut body),
                        }
                    }
                }
                encode_call(index, &mut body);
                if entry_return.flatten().is_some() {
                    encode_drop(&mut body);
                }
                if with_ok_i32 {
                    encode_i32_const(0, &mut body);
                }
                WasmOpcode::End.encode(&mut body);
                body
            };

            // Nullary `_start` / `run` when entry still carries params (WASI command world).
            if !entry_mir_params.is_empty() || !entry_wasm_params.is_empty() {
                let start_type_index = u32::try_from(type_indices.len()).expect("type index overflow");
                type_indices.push(wasm_function_type(&[], &[]));
                function_indices.push(start_type_index);
                let start_wrapper = build_entry_wrapper_body(false);
                let start_func_index = import_count + u32::try_from(code_bodies.len()).expect("code body overflow");
                code_bodies.push(wasm_function_body(start_wrapper));
                for export in &mut exports {
                    if (export.0 == "_start" || export.0 == "run") && export.1 == WasmExternalKind::Func.as_u8() {
                        export.2 = start_func_index;
                    }
                }
                if !exports.iter().any(|(name, _, _)| *name == "run") {
                    exports.push(("run", WasmExternalKind::Func.as_u8(), start_func_index));
                }
            }
            else if !exports.iter().any(|(name, _, _)| *name == "run") {
                exports.push(("run", WasmExternalKind::Func.as_u8(), index));
            }

            if !exports.iter().any(|(name, _, _)| *name == wasi_cli_run_export.as_str()) {
                let wrapper_type_index = u32::try_from(type_indices.len()).expect("type index overflow");
                type_indices.push(wasm_function_type(&[], &[VALTYPE_I32]));
                function_indices.push(wrapper_type_index);
                let wrapper = build_entry_wrapper_body(true);
                let wrapper_func_index = import_count + u32::try_from(code_bodies.len()).expect("code body overflow");
                code_bodies.push(wasm_function_body(wrapper));
                exports.push((wasi_cli_run_export.as_str(), WasmExternalKind::Func.as_u8(), wrapper_func_index));
            }
        }
    }

    // 库导出边界物化 wrapper：为每个携带 host 句柄→managed 物化配方的实例合成一个
    // 物理签名把配方涉及形参的 anyref 槽替换为 i32 句柄（与冻结库 ABI 载体一致）的
    // 包装函数；体内按配方把句柄字节拷进 managed struct 后调用内部函数，导出重定向到
    // wrapper。其余形参与返回原样透传，未命中配方的实例不生成 wrapper。
    if !wasi_mode {
        for (instance, plans) in &host_handle_to_managed_utf8 {
            if plans.is_empty() {
                continue;
            }
            let internal_index = function_index_by_instance
                .get(instance)
                .copied()
                .ok_or_else(|| format!("库导出 `{instance}` 缺少已分配函数下标，无法合成宿主句柄物化 wrapper"))?;
            let internal_params = param_types_by_function_index
                .get(&internal_index)
                .cloned()
                .ok_or_else(|| format!("库导出 `{instance}` 缺少物理签名，无法合成宿主句柄物化 wrapper"))?;
            for plan in plans {
                let Some(&slot) = internal_params.get(plan.param_index as usize)
                else {
                    return Err(format!(
                        "库导出 `{instance}` 物化配方形参下标 {} 超出物理签名参数数 {}",
                        plan.param_index,
                        internal_params.len()
                    ));
                };
                if slot != WASM_GC_ANYREF {
                    return Err(format!(
                        "库导出 `{instance}` 物化配方形参 {index} 物理槽不是 anyref（实际 0x{slot:02X}）；配方与注册表事实分叉，禁止编码",
                        index = plan.param_index
                    ));
                }
            }
            let mut wrapper_params = internal_params.clone();
            for plan in plans {
                wrapper_params[plan.param_index as usize] = VALTYPE_I32;
            }
            let wrapper_result: Vec<u8> = return_types_by_function_index.get(&internal_index).copied().flatten().into_iter().collect();
            let wrapper_type_index = u32::try_from(type_indices.len()).expect("type index overflow");
            type_indices.push(wasm_function_type(&wrapper_params, &wrapper_result));
            function_indices.push(wrapper_type_index);
            let mut wrapper_body = Vec::new();
            emit_host_handle_to_managed_utf8_wrapper_body(
                plans,
                internal_index,
                u32::try_from(internal_params.len()).expect("param count overflow"),
                &mut wrapper_body,
            );
            let wrapper_func_index = import_count + u32::try_from(code_bodies.len()).expect("code body overflow");
            code_bodies.push(wasm_function_body(wrapper_body));
            let public_name = submission
                .backend_plan
                .wasm_export_names()
                .get(instance)
                .cloned()
                .ok_or_else(|| format!("库导出 `{instance}` 物化配方缺少公开导出名"))?;
            let mut redirected = false;
            for export in &mut exports {
                if export.0 == public_name.as_str() && export.1 == WasmExternalKind::Func.as_u8() {
                    export.2 = wrapper_func_index;
                    redirected = true;
                }
            }
            if !redirected {
                return Err(format!("库导出 `{instance}`（公开名 `{public_name}`）缺少 Func 导出条目，无法重定向到宿主句柄物化 wrapper"));
            }
        }
    }

    // 库导出聚合的字段 accessor：从冻结布局派生逐字段读取函数并导出，供宿主
    // 按 `nyar.library_invoke` 的字段合同读回聚合返回值。只有库导出返回签名
    // 可达的布局参与；accessor 名与 schema 同源确定性派生，不按名字识别。
    let mut accessor_export_names: Vec<String> = Vec::new();
    if wasm_package_kind == WasmPackageKind::Library {
        emit_library_field_accessors(
            submission,
            &ctx,
            &gc_struct_type_indices,
            &gc_array_type_indices,
            js_glue_utf8_as_anyref,
            &mut type_indices,
            &mut function_indices,
            &mut code_bodies,
            &mut exports,
            import_count,
            &mut accessor_export_names,
        )?;
    }
    library_invoke::append_library_invoke_section(wasm_package_kind, submission, &mut module)?;

    module.sections.push(type_section_bytes(type_indices));
    if !host_imports.is_empty() {
        let import_entries = host_imports
            .iter()
            .zip(import_type_entries.iter())
            .map(|((module, field), type_index)| {
                let (core_module, core_field) = wasi_core_import_name(module, field, wasi_preview);
                (core_module, core_field, *type_index)
            })
            .collect::<Vec<_>>();
        module.sections.push(import_section_bytes(&import_entries));
    }

    let data_end = if wasi_mode && !string_literals.is_empty() { wasi_string_data_section_size(&string_literals) } else { 0 };
    let heap_base = cabi_heap_base_after_data(data_end as usize);

    // Append cabi_realloc functype + function shared with MIR linear bump.
    let realloc_type_index = {
        let realloc_ty = wasm_function_type(&[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32, VALTYPE_I32], &[VALTYPE_I32]);
        let type_section = module.sections.iter_mut().find(|item| item.id == 1).expect("type section");
        let mut pos = 0;
        let count = decode_uleb128(&type_section.bytes, &mut pos);
        let rest = type_section.bytes[pos..].to_vec();
        let mut bytes = Vec::new();
        encode_uleb128(count + 1, &mut bytes);
        bytes.extend_from_slice(&rest);
        bytes.extend_from_slice(&realloc_ty);
        type_section.bytes = bytes;
        count
    };

    let realloc_func_index = import_count + u32::try_from(function_indices.len()).unwrap();
    function_indices.push(realloc_type_index);
    code_bodies.push(wasm_function_body(wasm_cabi_realloc_bump_body()));

    module.sections.push(function_section_bytes(&function_indices));
    // funcref table：只为被作值引用的函数建表（id 4，必须排在 function(3) 与 memory(5) 之间）。
    if !function_value_table_functions.is_empty() {
        module.sections.push(table_section_bytes(u32::try_from(function_value_table_functions.len()).expect("funcref table 大小溢出")));
    }
    module.sections.push(memory_section_bytes(memory_min_pages_for_heap_base(heap_base)));
    module.sections.push(cabi_heap_global_section(heap_base));

    if !exports.iter().any(|(name, _, _)| *name == "memory") {
        exports.push(("memory", WasmExternalKind::Memory.as_u8(), 0));
    }
    if !exports.iter().any(|(name, _, _)| *name == "cabi_realloc") {
        exports.push(("cabi_realloc", WasmExternalKind::Func.as_u8(), realloc_func_index));
    }

    module.sections.push(export_section_bytes(&exports));
    // active elem 段：table 槽位 i 初始化为对应函数的下标（id 9，必须排在 export(7) 与 code(10) 之间）。
    if !function_value_table_functions.is_empty() {
        module.sections.push(elem_section_bytes(0, &function_value_table_functions));
    }
    module.sections.push(code_section_bytes(&code_bodies));
    if wasi_mode {
        // WASI 轨：字符串字面量存入 data 段，运行时通过偏移量访问?
        if !string_literals.is_empty() {
            let data = build_wasi_string_data_section(&string_literals);
            module.sections.push(data_section_bytes(0, &data));
        }
    }
    else if !string_literals.is_empty() {
        // Node 轨：字符串字面量存入自定义段 `nyar.strings`，由 `.mjs` 启动壳加载?
        let payload = serde_json::json!({ "strings": string_literals });
        module.sections.push(WasmSection { id: 0, name: Some("nyar.strings".to_string()), bytes: payload.to_string().into_bytes() });
    }
    append_wasm_spy_metadata_sections(
        &mut module,
        &ctx,
        submission,
        import_count,
        &mir_wasm_functions,
        &gc_struct_type_indices,
        &gc_array_type_indices,
    );
    Ok((module, host_imports))
}

/// 目标计划阶段冻结「managed 字符串 → 宿主 i32 句柄」转换配方（合同 §5）。
///
/// 扫描所有函数携带 gc struct layout 的 `NyarType::Utf8` SSA 值：这些值在宿主导入
/// 边界（ABI 声明 i32 句柄）必须由编码期按配方转换，转换事实不允许在编码阶段
/// 按类型名或布局形状重新推导。字节字段槽来自 layout 的整数字节数组字段，
/// arraytype 缺失时按注册表同一构造器补登记；合成导入 `env::host_intern_utf8`
/// 仅在确有 managed 字符串时追加到导入表尾部。
#[allow(clippy::too_many_arguments)]
fn freeze_managed_utf8_to_host_handle_plans(
    submission: &FragmentSubmission,
    ctx: &ExecutableLoweringContext,
    gc_struct_type_indices: &BTreeMap<LayoutId, u32>,
    gc_array_type_indices: &mut BTreeMap<String, u32>,
    type_indices: &mut Vec<Vec<u8>>,
    js_glue_utf8_as_anyref: bool,
    host_imports: &mut Vec<(String, String)>,
    import_type_entries: &mut Vec<u32>,
    host_import_signatures: &BTreeMap<(String, String), crate::BackendImport>,
) -> crate::Result<BTreeMap<LayoutId, ManagedUtf8ToHostHandlePlan>> {
    let mut managed_string_layouts = BTreeSet::new();
    for instance in submission.backend_plan.instances() {
        let Some(view) = submission.backend_plan.get_function(&instance)
        else {
            continue;
        };
        for (value, ty) in &view.function.value_types {
            if !is_js_glue_host_string_type(ty) {
                continue;
            }
            if let Some(layout_id) = view.function.value_layouts.get(value).copied() {
                managed_string_layouts.insert(layout_id);
            }
        }
    }
    if managed_string_layouts.is_empty() {
        return Ok(BTreeMap::new());
    }
    // 合成导入按需追加：下标即 wasm 函数索引（导入排在函数索引空间最前）。
    let intern_import_index = match host_imports.iter().position(|(module, field)| module == "env" && field == "host_intern_utf8") {
        Some(index) => u32::try_from(index).expect("import index overflow"),
        None => {
            let type_index = u32::try_from(type_indices.len()).expect("type index overflow");
            type_indices.push(wasm_import_type_bytes_for_host(
                submission,
                ctx,
                false,
                "env",
                "host_intern_utf8",
                js_glue_utf8_as_anyref,
                host_import_signatures,
            )?);
            import_type_entries.push(type_index);
            host_imports.push(("env".to_string(), "host_intern_utf8".to_string()));
            u32::try_from(host_imports.len() - 1).expect("import index overflow")
        }
    };
    let mut plans = BTreeMap::new();
    for layout_id in managed_string_layouts {
        // 未登记 gc struct 的 layout 槽位本就是 i32 句柄，不存在转换需求。
        let Some(struct_type_index) = gc_struct_type_indices.get(&layout_id).copied()
        else {
            continue;
        };
        let layout =
            ctx.layout_by_id(layout_id).ok_or_else(|| format!("WASM managed 字符串转换冻结失败: layout_id={layout_id:?} 缺少聚合布局身份"))?;
        let (bytes_field_slot, element_type) = layout
            .fields
            .iter()
            .enumerate()
            .find_map(|(index, field)| match &field.ty {
                NyarType::Array(element) | NyarType::FixedArray { element, .. } => {
                    matches!(element.as_ref(), NyarType::Integer8 { .. }).then(|| (index as u32, element.as_ref().clone()))
                }
                _ => None,
            })
            .ok_or_else(|| format!("WASM managed 字符串转换冻结失败: layout `{}` 缺少整数字节数组字段，无法生成 host 句柄配方", layout.name))?;
        let key = wasm_array_element_type_key(&element_type);
        let bytes_array_type_index = match gc_array_type_indices.get(&key).copied() {
            Some(type_index) => type_index,
            None => {
                let type_index = u32::try_from(type_indices.len()).expect("type index overflow");
                type_indices.push(wasm_gc_array_type(wasm_gc_field_type_byte_for_glue(&element_type, js_glue_utf8_as_anyref)));
                gc_array_type_indices.insert(key, type_index);
                type_index
            }
        };
        plans.insert(
            layout_id,
            ManagedUtf8ToHostHandlePlan { struct_type_index, bytes_field_slot, bytes_array_type_index, intern_import_index },
        );
    }
    Ok(plans)
}

/// 目标计划阶段冻结「宿主 i32 句柄 → managed 字符串」库导出边界物化配方（合同 §5）。
///
/// 扫描已登记公开导出名的实例：entry 块形参 SSA 值绑定已登记 gc struct layout 的
/// `NyarType::Utf8` 形参，其物理槽是 anyref，而宿主按冻结库 ABI 传 i32 句柄，导出
/// 边界必须物化。物化事实与冻结 ABI 载体交叉核对：载体不是 `i32` 说明合同已坏，
/// 结构化失败，禁止静默回落。字节字段槽与 arraytype 推导与 managed→host 冻结同法；
/// 合成导入 `env::host_utf8_extract` 仅在确有需物化形参时追加到导入表尾部。
#[allow(clippy::too_many_arguments)]
fn freeze_host_handle_to_managed_utf8_plans(
    submission: &FragmentSubmission,
    ctx: &ExecutableLoweringContext,
    gc_struct_type_indices: &BTreeMap<LayoutId, u32>,
    gc_array_type_indices: &mut BTreeMap<String, u32>,
    type_indices: &mut Vec<Vec<u8>>,
    js_glue_utf8_as_anyref: bool,
    host_imports: &mut Vec<(String, String)>,
    import_type_entries: &mut Vec<u32>,
    host_import_signatures: &BTreeMap<(String, String), crate::BackendImport>,
) -> crate::Result<BTreeMap<ItemInstanceId, Vec<HostHandleToManagedUtf8Plan>>> {
    let mut plans_by_instance: BTreeMap<ItemInstanceId, Vec<HostHandleToManagedUtf8Plan>> = BTreeMap::new();
    for (instance, public_name) in submission.backend_plan.wasm_export_names() {
        let Some(view) = submission.backend_plan.get_function(instance)
        else {
            continue;
        };
        let mir_fn = &view.function;
        let Some(entry_block) = mir_fn.blocks.iter().find(|block| block.id == mir_fn.entry)
        else {
            continue;
        };
        for (index, ty) in mir_fn.param_types.iter().enumerate() {
            if !is_js_glue_host_string_type(ty) {
                continue;
            }
            let Some(layout_id) = entry_block.parameters.get(index).and_then(|value| mir_fn.value_layouts.get(value)).copied()
            else {
                continue;
            };
            // 未登记 gc struct 的形参槽本就是 i32 句柄，宿主句柄直通，不存在物化需求。
            let Some(struct_type_index) = gc_struct_type_indices.get(&layout_id).copied()
            else {
                continue;
            };
            let abi = submission.backend_plan.library_export_abi(instance).ok_or_else(|| {
                format!("库导出 `{instance}`（export_name={public_name}）缺少冻结 ABI 载体合同，无法核对宿主句柄→managed 物化配方")
            })?;
            let Some(carrier) = abi.params.get(index)
            else {
                return Err(format!(
                    "库导出 `{instance}`（export_name={public_name}）形参 {index} 缺少冻结 ABI 载体，无法核对宿主句柄→managed 物化配方"
                ));
            };
            if carrier.as_str() != "i32" {
                return Err(format!(
                    "库导出 `{instance}`（export_name={public_name}）形参 {index} 携带 managed 字符串 layout 但冻结 ABI 载体为 `{}`；宿主句柄→managed 物化合同已坏",
                    carrier.as_str()
                ));
            }
            let layout =
                ctx.layout_by_id(layout_id).ok_or_else(|| format!("WASM 宿主句柄物化冻结失败: layout_id={layout_id:?} 缺少聚合布局身份"))?;
            let (bytes_field_slot, element_type) = layout
                .fields
                .iter()
                .enumerate()
                .find_map(|(field_index, field)| match &field.ty {
                    NyarType::Array(element) | NyarType::FixedArray { element, .. } => {
                        matches!(element.as_ref(), NyarType::Integer8 { .. }).then(|| (field_index as u32, element.as_ref().clone()))
                    }
                    _ => None,
                })
                .ok_or_else(|| format!("WASM 宿主句柄物化冻结失败: layout `{}` 缺少整数字节数组字段，无法生成物化配方", layout.name))?;
            let key = wasm_array_element_type_key(&element_type);
            let bytes_array_type_index = match gc_array_type_indices.get(&key).copied() {
                Some(type_index) => type_index,
                None => {
                    let type_index = u32::try_from(type_indices.len()).expect("type index overflow");
                    type_indices.push(wasm_gc_array_type(wasm_gc_field_type_byte_for_glue(&element_type, js_glue_utf8_as_anyref)));
                    gc_array_type_indices.insert(key, type_index);
                    type_index
                }
            };
            // extract 导入按需追加：下标即 wasm 函数索引（导入排在函数索引空间最前）。
            let extract_import_index = match host_imports.iter().position(|(module, field)| module == "env" && field == "host_utf8_extract") {
                Some(existing) => u32::try_from(existing).expect("import index overflow"),
                None => {
                    let type_index = u32::try_from(type_indices.len()).expect("type index overflow");
                    type_indices.push(wasm_import_type_bytes_for_host(
                        submission,
                        ctx,
                        false,
                        "env",
                        "host_utf8_extract",
                        js_glue_utf8_as_anyref,
                        host_import_signatures,
                    )?);
                    import_type_entries.push(type_index);
                    host_imports.push(("env".to_string(), "host_utf8_extract".to_string()));
                    u32::try_from(host_imports.len() - 1).expect("import index overflow")
                }
            };
            plans_by_instance.entry(*instance).or_default().push(HostHandleToManagedUtf8Plan {
                param_index: index as u32,
                struct_type_index,
                bytes_field_slot,
                bytes_array_type_index,
                extract_import_index,
            });
        }
    }
    Ok(plans_by_instance)
}

/// 目标计划阶段冻结「宿主 i32 句柄 → managed 字符串」数组元素边界物化配方（合同 §5）。
///
/// `[utf8]` 数组按冻结合同登记为 `arraytype [i32]`（元素是宿主句柄），而 `array.get`
/// 结果 SSA 值携带已登记 gc struct layout 时物理槽是 anyref；编码期必须按这张配方表
/// 把取出的 i32 句柄物化为 managed 字符串 struct，禁止按符号名特判，也不得把计划槽
/// 降级为 i32。扫描命中边界的 layout：MIR `ArrayGet` 臂与 `[wasm_operator("array.get")]`
/// 冻结绑定调用，结果值为宿主字符串类型且 layout 已登记 gc struct。字节字段槽与
/// arraytype 推导与库导出物化冻结同法；合成导入 `env::host_utf8_extract` 仅在确有
/// 命中边界的 layout 时追加到导入表尾部。WASI 轨没有 `host_utf8_extract` 宿主能力，
/// 装配侧不调用本函数（空表），编码期命中边界即结构化失败，与 WASI 既有 managed
/// 字符串边界同口径。
#[allow(clippy::too_many_arguments)]
fn freeze_utf8_array_element_materialize_plans(
    submission: &FragmentSubmission,
    ctx: &ExecutableLoweringContext,
    callee_wasm_operator: &BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)>,
    gc_struct_type_indices: &BTreeMap<LayoutId, u32>,
    gc_array_type_indices: &mut BTreeMap<String, u32>,
    type_indices: &mut Vec<Vec<u8>>,
    js_glue_utf8_as_anyref: bool,
    host_imports: &mut Vec<(String, String)>,
    import_type_entries: &mut Vec<u32>,
    host_import_signatures: &BTreeMap<(String, String), crate::BackendImport>,
) -> crate::Result<BTreeMap<LayoutId, Utf8ArrayElementMaterializePlan>> {
    let mut boundary_layouts = BTreeSet::new();
    for instance in submission.backend_plan.instances() {
        let Some(view) = submission.backend_plan.get_function(&instance)
        else {
            continue;
        };
        let mir_fn = &view.function;
        for block in &mir_fn.blocks {
            for instruction in &block.instructions {
                let is_array_get = match &instruction.kind {
                    MirInstructionKind::ArrayGet { .. } => true,
                    MirInstructionKind::Call { callee: MirOperand::Item(callee), .. } => matches!(
                        callee_wasm_operator.get(callee),
                        Some((_, TargetOperatorBinding::FrozenWasmGcArray(plan))) if plan.operation == WasmGcArrayOperation::Get
                    ),
                    _ => false,
                };
                if !is_array_get {
                    continue;
                }
                for result in &instruction.results {
                    let Some(ty) = mir_fn.value_types.get(result)
                    else {
                        continue;
                    };
                    if !is_js_glue_host_string_type(ty) {
                        continue;
                    }
                    let Some(layout_id) = mir_fn.value_layouts.get(result).copied()
                    else {
                        continue;
                    };
                    if gc_struct_type_indices.contains_key(&layout_id) {
                        boundary_layouts.insert(layout_id);
                    }
                }
            }
        }
    }
    let mut plans = BTreeMap::new();
    for layout_id in boundary_layouts {
        let Some(struct_type_index) = gc_struct_type_indices.get(&layout_id).copied()
        else {
            continue;
        };
        let layout =
            ctx.layout_by_id(layout_id).ok_or_else(|| format!("WASM 数组元素物化冻结失败: layout_id={layout_id:?} 缺少聚合布局身份"))?;
        let (bytes_field_slot, element_type) = layout
            .fields
            .iter()
            .enumerate()
            .find_map(|(field_index, field)| match &field.ty {
                NyarType::Array(element) | NyarType::FixedArray { element, .. } => {
                    matches!(element.as_ref(), NyarType::Integer8 { .. }).then(|| (field_index as u32, element.as_ref().clone()))
                }
                _ => None,
            })
            .ok_or_else(|| format!("WASM 数组元素物化冻结失败: layout `{}` 缺少整数字节数组字段，无法生成物化配方", layout.name))?;
        let key = wasm_array_element_type_key(&element_type);
        let bytes_array_type_index = match gc_array_type_indices.get(&key).copied() {
            Some(type_index) => type_index,
            None => {
                let type_index = u32::try_from(type_indices.len()).expect("type index overflow");
                type_indices.push(wasm_gc_array_type(wasm_gc_field_type_byte_for_glue(&element_type, js_glue_utf8_as_anyref)));
                gc_array_type_indices.insert(key, type_index);
                type_index
            }
        };
        // extract 导入按需追加：下标即 wasm 函数索引（导入排在函数索引空间最前）。
        let extract_import_index = match host_imports.iter().position(|(module, field)| module == "env" && field == "host_utf8_extract") {
            Some(existing) => u32::try_from(existing).expect("import index overflow"),
            None => {
                let type_index = u32::try_from(type_indices.len()).expect("type index overflow");
                type_indices.push(wasm_import_type_bytes_for_host(
                    submission,
                    ctx,
                    false,
                    "env",
                    "host_utf8_extract",
                    js_glue_utf8_as_anyref,
                    host_import_signatures,
                )?);
                import_type_entries.push(type_index);
                host_imports.push(("env".to_string(), "host_utf8_extract".to_string()));
                u32::try_from(host_imports.len() - 1).expect("import index overflow")
            }
        };
        plans.insert(
            layout_id,
            Utf8ArrayElementMaterializePlan { struct_type_index, bytes_field_slot, bytes_array_type_index, extract_import_index },
        );
    }
    Ok(plans)
}

/// 发射库导出物化 wrapper 函数体：按形参顺序，命中配方的形参把宿主 i32 句柄物化为
/// managed 字符串 struct（`host_utf8_extract` 取回 `(ptr, len)` → `array.new_default`
/// → 逐字节 `i32.load8_u` + `array.set` → `struct.new_default` + `struct.set`），
/// 其余形参原样透传，最后 `call` 内部函数并保留返回。
///
/// scratch local 布局（形参之后）：`+0` ptr、`+1` len、`+2` idx（i32）、`+3` arr、`+4` obj（anyref）。
fn emit_host_handle_to_managed_utf8_wrapper_body(
    plans: &[HostHandleToManagedUtf8Plan],
    internal_function_index: u32,
    param_count: u32,
    body: &mut Vec<u8>,
) {
    encode_uleb128(2, body);
    encode_uleb128(3, body);
    body.push(VALTYPE_I32);
    encode_uleb128(2, body);
    body.push(WASM_GC_ANYREF);
    let ptr_local = param_count;
    let len_local = param_count + 1;
    let idx_local = param_count + 2;
    let arr_local = param_count + 3;
    let obj_local = param_count + 4;
    for index in 0..param_count {
        let Some(plan) = plans.iter().find(|plan| plan.param_index == index)
        else {
            encode_local_get(index, body);
            continue;
        };
        // 句柄 → (ptr, len)：多值返回按结果序压栈，逆序落 local。
        encode_local_get(index, body);
        encode_call(plan.extract_import_index, body);
        encode_local_set(len_local, body);
        encode_local_set(ptr_local, body);
        // bytes = array.new_default(len)
        encode_local_get(len_local, body);
        encode_array_new_default(plan.bytes_array_type_index, body);
        encode_local_set(arr_local, body);
        // 逐字节拷贝：i32.load8_u(ptr + i) → array.set(bytes, i, …)
        encode_i32_const(0, body);
        encode_local_set(idx_local, body);
        encode_block_empty(body);
        encode_loop_empty(body);
        encode_local_get(idx_local, body);
        encode_local_get(len_local, body);
        WasmOpcode::I32GeU.encode(body);
        encode_br_if(1, body);
        encode_local_get(arr_local, body);
        encode_ref_cast_type_index(plan.bytes_array_type_index, body);
        encode_local_get(idx_local, body);
        encode_local_get(ptr_local, body);
        encode_local_get(idx_local, body);
        encode_i32_add(body);
        encode_memarg(WasmOpcode::I32Load8U, 0, 0, body);
        encode_array_set(plan.bytes_array_type_index, body);
        encode_local_get(idx_local, body);
        encode_i32_const(1, body);
        encode_i32_add(body);
        encode_local_set(idx_local, body);
        encode_br(0, body);
        WasmOpcode::End.encode(body); // loop
        WasmOpcode::End.encode(body); // block
        // managed struct：_repr 字段装入字节数组（与 `emit_box_i64_payload` 同款 local 往返）。
        encode_struct_new_default(plan.struct_type_index, body);
        encode_local_set(obj_local, body);
        encode_local_get(obj_local, body);
        encode_ref_cast_type_index(plan.struct_type_index, body);
        encode_local_get(arr_local, body);
        encode_struct_set(plan.struct_type_index, plan.bytes_field_slot, body);
        encode_local_get(obj_local, body);
    }
    encode_call(internal_function_index, body);
    WasmOpcode::End.encode(body);
}

pub(super) fn emit_wasi_entry_default_arg(ty: &NyarType, gc_array_type_indices: &BTreeMap<String, u32>, body: &mut Vec<u8>) {
    match ty {
        NyarType::Array(element) | NyarType::FixedArray { element, .. } => {
            let key = wasm_array_element_type_key(element);
            let type_index = gc_array_type_indices.get(&key).copied();
            if let Some(type_index) = type_index {
                encode_i32_const(0, body);
                encode_array_new_default(type_index, body);
            }
            else {
                panic!("WASI entry array parameter `{key}` 缺少已注册的 array type");
            }
        }
        NyarType::Float64 | NyarType::Float32 => encode_f64_const(0.0, body),
        NyarType::Integer64 { .. } | NyarType::Integer128 { .. } => encode_i64_const(0, body),
        NyarType::Unit => encode_ref_null_anyref(body),
        NyarType::Bottom => encode_i32_const(0, body),
        other if is_js_glue_host_string_type(other) => encode_i32_const(0, body),
        other if type_is_wasm_gc_heap_reference(other) => {
            panic!("WASI entry heap 参数 `{other:?}` 缺少 array type 合同")
        }
        _ => encode_i32_const(0, body),
    }
}

/// Out-pointer slot in the reserved `[0, WASI_STRING_DATA_OFFSET)` region for `get-arguments`.
pub(super) const WASI_GET_ARGUMENTS_OUT_PTR: i32 = 16;

/// Build guest `[utf8]` argv from `wasi:cli/environment#get-arguments` (cabi out-pointer).
///
/// Host writes `{list_ptr, list_len}` at `out`; each element is Canonical ABI `{str_ptr, str_len}`
/// (raw utf8 bytes). Guest handles use `[len:u32 LE][utf8…]` in the cabi bump heap (global 0).
///
/// Locals (declared by caller): `0..6` i32, `7` (ref null $array_ty) (argv).
/// Stack effect: pushes argv anyref.
pub(super) fn emit_wasi_argv_from_get_arguments(get_arguments_import: u32, array_ty: u32, body: &mut Vec<u8>) {
    // get-arguments(out=16)
    encode_i32_const(WASI_GET_ARGUMENTS_OUT_PTR, body);
    encode_call(get_arguments_import, body);
    // list_ptr = i32.load(16)
    encode_i32_const(WASI_GET_ARGUMENTS_OUT_PTR, body);
    encode_i32_load(2, 0, body);
    encode_local_set(0, body);
    // list_len = i32.load(20)
    encode_i32_const(WASI_GET_ARGUMENTS_OUT_PTR + 4, body);
    encode_i32_load(2, 0, body);
    encode_local_set(1, body);
    // argv = array.new_default(list_len)
    encode_local_get(1, body);
    encode_array_new_default(array_ty, body);
    encode_local_set(7, body);
    // i = 0
    encode_i32_const(0, body);
    encode_local_set(2, body);
    // block { loop {
    encode_block_empty(body);
    encode_loop_empty(body);
    // if i >= list_len { br 1 }
    encode_local_get(2, body);
    encode_local_get(1, body);
    WasmOpcode::I32GeU.encode(body);
    encode_br_if(1, body);
    // elem = list_ptr + i*8
    encode_local_get(0, body);
    encode_local_get(2, body);
    encode_i32_const(8, body);
    WasmOpcode::I32Mul.encode(body);
    encode_i32_add(body);
    encode_local_tee(6, body); // elem base in local 6 temporarily
    // str_ptr
    encode_i32_load(2, 0, body);
    encode_local_set(3, body);
    // str_len
    encode_local_get(6, body);
    encode_i32_load(2, 4, body);
    encode_local_set(4, body);
    // size = align4(4 + str_len)
    encode_local_get(4, body);
    encode_i32_const(4, body);
    encode_i32_add(body);
    encode_i32_const(3, body);
    encode_i32_add(body);
    encode_i32_const(!3i32, body); // -4 == !3 for align mask in two's complement-?wait
    // (x + 3) & !3  where !3 as i32 is -4
    WasmOpcode::I32And.encode(body);
    encode_local_set(6, body); // size
    // handle = global.get 0; global.set(handle+size)
    encode_global_get(CABI_HEAP_GLOBAL_INDEX, body);
    encode_local_set(5, body);
    encode_local_get(5, body);
    encode_local_get(6, body);
    encode_i32_add(body);
    encode_global_set(CABI_HEAP_GLOBAL_INDEX, body);
    // store len
    encode_local_get(5, body);
    encode_local_get(4, body);
    encode_i32_store(2, 0, body);
    // memory.copy(handle+4, str_ptr, str_len)
    encode_local_get(5, body);
    encode_i32_const(4, body);
    encode_i32_add(body);
    encode_local_get(3, body);
    encode_local_get(4, body);
    encode_memory_copy(body);
    // array.set(argv, i, handle)
    encode_local_get(7, body);
    encode_local_get(2, body);
    encode_local_get(5, body);
    encode_array_set(array_ty, body);
    // i++
    encode_local_get(2, body);
    encode_i32_const(1, body);
    encode_i32_add(body);
    encode_local_set(2, body);
    encode_br(0, body); // continue loop
    WasmOpcode::End.encode(body); // loop
    WasmOpcode::End.encode(body); // block
    encode_local_get(7, body);
}

pub(super) fn append_wasm_spy_metadata_sections(
    module: &mut WasmBinaryModule,
    ctx: &ExecutableLoweringContext,
    submission: &FragmentSubmission,
    import_count: u32,
    mir_wasm_functions: &[(u32, String)],
    gc_struct_type_indices: &BTreeMap<LayoutId, u32>,
    gc_array_type_indices: &BTreeMap<String, u32>,
) {
    let mut func_payload = format!("import_count={import_count}\n");
    for (wasm_index, symbol) in mir_wasm_functions {
        func_payload.push_str(&format!("{wasm_index}\t{symbol}\n"));
    }
    module.sections.push(WasmSection { id: 0, name: Some("nyar.wasm.functions".to_string()), bytes: func_payload.into_bytes() });

    // 语言规范：wasm/wasi 强制 GC；structtype ?arraytype 均注册?
    let mut gc_payload = String::from("gc_required=true\n");
    for layout in &ctx.layouts.layouts {
        let storage = if layout.storage == StorageKind::Reference { "reference" } else { "value" };
        if let Some(type_index) = gc_struct_type_indices.get(&layout.id) {
            gc_payload.push_str(&format!("struct\t{}\t{}\t{type_index}\tregistered\t{storage}\n", layout.id, layout.name));
        }
        else if layout_needs_gc_struct(ctx, layout) {
            gc_payload.push_str(&format!("struct\t{}\t{}\tmissing\tnot_registered\t{storage}\n", layout.id, layout.name));
        }
        else {
            gc_payload.push_str(&format!("struct\t{}\t{}\tn/a\tlinear_memory\t{storage}\n", layout.id, layout.name));
        }
    }
    for (key, type_index) in gc_array_type_indices {
        gc_payload.push_str(&format!("array\t{key}\t{type_index}\tregistered\n"));
    }
    {
        let exec = &submission.backend_plan;
        for operation in exec.instances() {
            let Some(view) = exec.get_function(&operation)
            else {
                continue;
            };
            let symbol = operation.to_string();
            for block in &view.function.blocks {
                for instruction in &block.instructions {
                    let mut record_missing = |layout_id: LayoutId, type_name: &str, site: &str| {
                        if gc_struct_type_indices.contains_key(&layout_id) {
                            return;
                        }
                        gc_payload.push_str(&format!("mir_use\t{layout_id}\t{type_name}\t{site}\t{symbol}\n"));
                    };
                    match &instruction.kind {
                        MirInstructionKind::StructNew { nominal, .. } => {
                            if let Some(layout) = ctx.layout_by_nominal(*nominal) {
                                if layout.storage == StorageKind::Reference {
                                    record_missing(layout.id, &layout.name, "StructNew");
                                }
                            }
                        }
                        MirInstructionKind::FieldGet { field, .. } | MirInstructionKind::FieldSet { field, .. } => {
                            if let Some((layout, _, _)) = ctx.field_layout_by_id(*field) {
                                if layout.storage == StorageKind::Reference {
                                    record_missing(layout.id, &layout.name, "FieldAccess");
                                }
                            }
                        }
                        MirInstructionKind::AggregateCopy { layout_id, .. } => {
                            if let Some(layout) = ctx.layout_by_id(*layout_id) {
                                if layout.storage == StorageKind::Reference {
                                    record_missing(layout.id, &layout.name, "AggregateCopy");
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    module.sections.push(WasmSection { id: 0, name: Some("nyar.wasm.gc_layouts".to_string()), bytes: gc_payload.into_bytes() });
}

pub(crate) fn augment_wasm_with_value_aggregate_metadata(module: &mut WasmBinaryModule, submission: &FragmentSubmission) {
    let mir_functions_len = submission.backend_plan.instances().len();
    if mir_functions_len == 0 {
        return;
    }
    let value_layout_count =
        submission.backend_plan.aggregate_layouts().layouts.iter().filter(|layout| layout.storage == StorageKind::Value).count();
    let reference_layout_count =
        submission.backend_plan.aggregate_layouts().layouts.iter().filter(|layout| layout.storage == StorageKind::Reference).count();
    let payload =
        format!("mir_functions={};value_layouts={};reference_layouts={}", mir_functions_len, value_layout_count, reference_layout_count);
    module.sections.push(WasmSection { id: 0, name: Some("nyar.value_aggregate".to_string()), bytes: payload.into_bytes() });
}

/// 库导出聚合的字段 accessor 发射（布局派生的 demarshal 读取合同）。
///
/// 从库导出返回签名可达的 nominal 布局（含字段递归）出发，为每个字段发射
/// 一个读取函数并导出：
/// - 已注册 gc struct 的布局：`(param anyref)` → `ref.cast` + `struct.get`，
///   字段槽位字节与冻结注册表一致；
/// - 未注册的 value 布局：`(param i32 线性地址)` → 冻结偏移 + 与 `FieldGet`
///   同配方的类型化 load；内联聚合字段本身即地址，不 load。
///
/// accessor 导出名与 [`super::abi_schema::field_accessor_export_name`] 同源，
/// 由布局身份确定性派生；JS glue 按 `nyar.library_invoke` 字段合同里的名字
/// 查表。缺布局身份直接结构化失败，不做默认回填。
#[allow(clippy::too_many_arguments)]
fn emit_library_field_accessors<'name>(
    submission: &FragmentSubmission,
    ctx: &ExecutableLoweringContext,
    gc_struct_type_indices: &BTreeMap<LayoutId, u32>,
    gc_array_type_indices: &BTreeMap<String, u32>,
    js_glue_utf8_as_anyref: bool,
    type_indices: &mut Vec<Vec<u8>>,
    function_indices: &mut Vec<u32>,
    code_bodies: &mut Vec<Vec<u8>>,
    exports: &mut Vec<(&'name str, u8, u32)>,
    import_count: u32,
    name_store: &'name mut Vec<String>,
) -> crate::Result<()> {
    /// 从导出位置（参数或返回值）收集桥接可达节点。
    ///
    /// - `Nominal` 自带布局；`Apply` 只从计划冻结的 `NyarType → LayoutId`
    ///   索引解析，索引缺失直接跳过（schema 侧同样没有字段合同，宿主失败闭合）。
    /// - heap `Array<T>` 收集元素类型，供发射 `array.new_default/get/set/len`。
    /// - 深搜按 `reachable` 去重，布局环不会死循环。
    fn collect_bridge_nodes(
        ty: &NyarType,
        ctx: &ExecutableLoweringContext,
        reachable: &mut BTreeSet<LayoutId>,
        array_elements: &mut BTreeMap<String, NyarType>,
        worklist: &mut Vec<NyarType>,
    ) {
        match ty {
            NyarType::Array(element) => {
                array_elements
                    .entry(wasm_array_element_type_key(element))
                    .or_insert_with(|| element.as_ref().clone());
                worklist.push(element.as_ref().clone());
            }
            NyarType::FixedArray { element, .. } | NyarType::Nullable(element) => {
                worklist.push(element.as_ref().clone());
            }
            NyarType::Tuple(elements) | NyarType::Union(elements) => {
                worklist.extend(elements.iter().cloned());
            }
            NyarType::Nominal { layout, .. } => {
                if reachable.insert(*layout) {
                    if let Some(item) = ctx.layout_by_id(*layout) {
                        worklist.extend(item.fields.iter().map(|field| field.ty.clone()));
                    }
                }
            }
            NyarType::Apply(..) => {
                if let Some(item) = ctx.layout_for_nyar_type(ty) {
                    if reachable.insert(item.id) {
                        worklist.extend(item.fields.iter().map(|field| field.ty.clone()));
                    }
                }
                else if let NyarType::Apply(base, args) = ty {
                    worklist.push(base.as_ref().clone());
                    worklist.extend(args.iter().cloned());
                }
            }
            _ => {}
        }
    }

    let mut worklist: Vec<NyarType> = Vec::new();
    for (instance, _) in submission.backend_plan.wasm_export_names() {
        let Some(view) = submission.backend_plan.get_function(instance)
        else {
            continue;
        };
        worklist.extend(view.function.param_types.iter().cloned());
        worklist.push(view.function.return_type.clone());
    }
    let mut reachable: BTreeSet<LayoutId> = BTreeSet::new();
    let mut array_elements: BTreeMap<String, NyarType> = BTreeMap::new();
    while let Some(ty) = worklist.pop() {
        collect_bridge_nodes(&ty, ctx, &mut reachable, &mut array_elements, &mut worklist);
    }

    // 阶段 1：按布局/字段顺序确定性派生全部 accessor 身份；名字统一进
    // 调用方作用域的 name_store，导出条目从中稳定借用。
    #[derive(Clone, Copy)]
    struct AccessorPlan {
        layout_id: LayoutId,
        field_index: usize,
        gc_struct_type_index: Option<u32>,
    }
    let mut plans: Vec<(String, AccessorPlan)> = Vec::new();
    let mut emitted_names: BTreeSet<String> = BTreeSet::new();
    for layout_id in &reachable {
        let layout = ctx.layout_by_id(*layout_id).expect("可达布局已通过身份核对");
        if layout.fields.is_empty() {
            continue;
        }
        let gc_struct_type_index = gc_struct_type_indices.get(layout_id).copied();
        for (field_index, _) in layout.fields.iter().enumerate() {
            let name = super::abi_schema::field_accessor_export_name(layout, field_index);
            if !emitted_names.insert(name.clone()) {
                continue;
            }
            plans.push((name, AccessorPlan { layout_id: *layout_id, field_index, gc_struct_type_index }));
        }
    }
    let accessor_name_start = name_store.len();
    name_store.extend(plans.iter().map(|(name, _)| name.clone()));

    // 阶段 2：聚合构造桥计划。布局有 GC struct 且跨 ABI 位置是 reference 时，
    // 发射 `struct.new` 包装：参数按字段声明顺序取物理载体，结果 anyref。
    // 这是宿主把 JS 数组装配成 VM 容器（例如 ArrayList 的 backing array +
    // 计数）的唯一合法入口，不在宿主侧复刻布局。
    #[derive(Clone, Copy)]
    struct ConstructorPlan {
        layout_id: LayoutId,
        struct_type_index: u32,
    }
    let mut constructor_plans: Vec<(String, ConstructorPlan)> = Vec::new();
    for layout_id in &reachable {
        let layout = ctx.layout_by_id(*layout_id).expect("可达布局已通过身份核对");
        let Some(struct_type_index) = gc_struct_type_indices.get(layout_id).copied()
        else {
            continue;
        };
        if layout.storage != StorageKind::Reference {
            continue;
        }
        let name = super::abi_schema::aggregate_constructor_export_name(layout);
        if !emitted_names.insert(name.clone()) {
            continue;
        }
        constructor_plans.push((name, ConstructorPlan { layout_id: *layout_id, struct_type_index }));
    }
    let constructor_name_start = name_store.len();
    name_store.extend(constructor_plans.iter().map(|(name, _)| name.clone()));

    // 阶段 3：heap array 物理读写桥计划。按元素类型确定性派生四个导出：
    // `length` / `get` / `new` / `set`。宿主只用 schema 里的名字调度。
    struct ArrayOpPlan {
        name: String,
        array_type_index: u32,
        element_type: u8,
        kind: u8,
    }
    let mut array_plans: Vec<ArrayOpPlan> = Vec::new();
    for (key, element) in &array_elements {
        let Some(array_type_index) = gc_array_type_indices.get(key).copied()
        else {
            return Err(format!("WASM 容器数组桥发射失败: 元素类型 `{element:?}` 未注册 gc arraytype（key={key}）"));
        };
        let element_type = wasm_gc_field_type_byte_for_glue(element, js_glue_utf8_as_anyref);
        let ops = super::abi_schema::array_ops_export_names(element);
        array_plans.push(ArrayOpPlan { name: ops.length, array_type_index, element_type, kind: 0 });
        array_plans.push(ArrayOpPlan { name: ops.get, array_type_index, element_type, kind: 1 });
        array_plans.push(ArrayOpPlan { name: ops.new, array_type_index, element_type, kind: 2 });
        array_plans.push(ArrayOpPlan { name: ops.set, array_type_index, element_type, kind: 3 });
    }
    let array_name_start = name_store.len();
    name_store.extend(array_plans.iter().map(|plan| plan.name.clone()));

    // 名字全部登记完成后才建立不可变切片；导出条目借用该切片。
    let names: &[String] = name_store.as_slice();

    // 阶段 4：发射 accessor 函数体并登记导出。
    for (position, (_, plan)) in plans.iter().enumerate() {
        let layout = ctx.layout_by_id(plan.layout_id).expect("可达布局已通过身份核对");
        let field = &layout.fields[plan.field_index];
        let (param_types, result_type, body) = match plan.gc_struct_type_index {
            Some(struct_type_index) => {
                // gc struct：(param anyref) → ref.cast + struct.get。
                let result_type = wasm_gc_field_type_byte_for_glue(&field.ty, js_glue_utf8_as_anyref);
                let mut body = Vec::new();
                encode_uleb128(0, &mut body);
                encode_local_get(0, &mut body);
                encode_ref_cast_type_index(struct_type_index, &mut body);
                encode_struct_get(struct_type_index, plan.field_index as u32, &mut body);
                WasmOpcode::End.encode(&mut body);
                (vec![WASM_GC_ANYREF], result_type, body)
            }
            None => {
                // 线性 value 布局：(param i32 地址) → 冻结偏移 + 类型化 load，
                // load 配方与 `FieldGet` 的线性路径完全一致。
                let inline_aggregate = type_is_inline_aggregate(ctx, &field.ty);
                let result_type = if inline_aggregate {
                    VALTYPE_I32
                }
                else {
                    match field.ty {
                        NyarType::Float64 => VALTYPE_F64,
                        NyarType::Integer64 { .. } => VALTYPE_I64,
                        _ => VALTYPE_I32,
                    }
                };
                let mut body = Vec::new();
                encode_uleb128(0, &mut body);
                encode_local_get(0, &mut body);
                encode_i32_const(field.offset as i32, &mut body);
                encode_i32_add(&mut body);
                if !inline_aggregate {
                    match field.ty {
                        NyarType::Float64 => encode_f64_load(3, 0, &mut body),
                        NyarType::Integer64 { .. } => encode_i64_load(3, 0, &mut body),
                        _ => encode_i32_load(2, 0, &mut body),
                    }
                }
                WasmOpcode::End.encode(&mut body);
                (vec![VALTYPE_I32], result_type, body)
            }
        };
        let type_index = u32::try_from(type_indices.len()).expect("type index overflow");
        type_indices.push(wasm_function_type(&param_types, &[result_type]));
        function_indices.push(type_index);
        let function_index = import_count + u32::try_from(code_bodies.len()).expect("code body overflow");
        code_bodies.push(wasm_function_body(body));
        exports.push((names[accessor_name_start + position].as_str(), WasmExternalKind::Func.as_u8(), function_index));
    }

    // 阶段 5：发射聚合构造函数体。
    for (position, (_, plan)) in constructor_plans.iter().enumerate() {
        let layout = ctx.layout_by_id(plan.layout_id).expect("可达布局已通过身份核对");
        let param_types: Vec<u8> =
            layout.fields.iter().map(|field| wasm_gc_field_type_byte_for_glue(&field.ty, js_glue_utf8_as_anyref)).collect();
        let mut body = Vec::new();
        encode_uleb128(0, &mut body);
        for field_index in 0..layout.fields.len() {
            encode_local_get(field_index as u32, &mut body);
        }
        acorn_wasm::WasmGcOpcode::StructNew.encode(&mut body);
        encode_uleb128(plan.struct_type_index, &mut body);
        WasmOpcode::End.encode(&mut body);
        let type_index = u32::try_from(type_indices.len()).expect("type index overflow");
        type_indices.push(wasm_function_type(&param_types, &[WASM_GC_ANYREF]));
        function_indices.push(type_index);
        let function_index = import_count + u32::try_from(code_bodies.len()).expect("code body overflow");
        code_bodies.push(wasm_function_body(body));
        exports.push((names[constructor_name_start + position].as_str(), WasmExternalKind::Func.as_u8(), function_index));
    }

    // 阶段 6：发射物理数组读写函数体。
    for (position, plan) in array_plans.iter().enumerate() {
        let array_type_index = plan.array_type_index;
        let element_type = plan.element_type;
        let (param_types, result_types, body) = match plan.kind {
            0 => {
                let mut body = Vec::new();
                encode_uleb128(0, &mut body);
                encode_local_get(0, &mut body);
                encode_ref_cast_type_index(array_type_index, &mut body);
                encode_array_len(&mut body);
                WasmOpcode::End.encode(&mut body);
                (vec![WASM_GC_ANYREF], vec![VALTYPE_I32], body)
            }
            1 => {
                let mut body = Vec::new();
                encode_uleb128(0, &mut body);
                encode_local_get(0, &mut body);
                encode_ref_cast_type_index(array_type_index, &mut body);
                encode_local_get(1, &mut body);
                encode_array_get(array_type_index, &mut body);
                WasmOpcode::End.encode(&mut body);
                (vec![WASM_GC_ANYREF, VALTYPE_I32], vec![element_type], body)
            }
            2 => {
                let mut body = Vec::new();
                encode_uleb128(0, &mut body);
                encode_local_get(0, &mut body);
                encode_array_new_default(array_type_index, &mut body);
                WasmOpcode::End.encode(&mut body);
                (vec![VALTYPE_I32], vec![WASM_GC_ANYREF], body)
            }
            _ => {
                let mut body = Vec::new();
                encode_uleb128(0, &mut body);
                // array.set 无局部变量，body 里不声明 locals。
                encode_local_get(0, &mut body);
                encode_ref_cast_type_index(array_type_index, &mut body);
                encode_local_get(1, &mut body);
                encode_local_get(2, &mut body);
                encode_array_set(array_type_index, &mut body);
                WasmOpcode::End.encode(&mut body);
                (vec![WASM_GC_ANYREF, VALTYPE_I32, element_type], Vec::new(), body)
            }
        };
        let type_index = u32::try_from(type_indices.len()).expect("type index overflow");
        type_indices.push(wasm_function_type(&param_types, &result_types));
        function_indices.push(type_index);
        let function_index = import_count + u32::try_from(code_bodies.len()).expect("code body overflow");
        code_bodies.push(wasm_function_body(body));
        exports.push((names[array_name_start + position].as_str(), WasmExternalKind::Func.as_u8(), function_index));
    }
    Ok(())
}

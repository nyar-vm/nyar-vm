//! Per-function MIR -> WASM bytecode lowerer.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    super::{
        super::{
            executable::{ExecutableLoweringContext, collect_reachable_blocks},
            interop::wasm_operator_target,
            pattern_matching_contract::validate_pattern_matching_invariants,
        },
        cabi::CABI_HEAP_GLOBAL_INDEX,
        gc::WASM_GC_ANYREF,
        sections::{decode_uleb128, encode_uleb128},
    },
    NominalInstanceId,
    StorageKind,
    representation::{
        is_js_glue_host_string_type, mir_storage_for_type, wasm_gc_field_type_byte_for_glue, wasm_param_types, wasm_param_value_type,
        wasm_param_value_type_for,
    },
    type_registry::wasm_array_element_type_key,
};
use crate::{
    FragmentSubmission,
    backend_plan_views::{
        ExecutableBlock as MirBlock, ExecutableBlockRef as MirBlockRef, ExecutableConstant as MirConstant,
        ExecutableDispatchKind as MirDispatchKind, ExecutableFunction as MirFunction, ExecutableInstruction as MirInstruction,
        ExecutableInstructionKind as MirInstructionKind, ExecutableOperand as MirOperand, ExecutableReceiverPassingKind as ReceiverPassingKind,
        ExecutableStorageKind as MirStorageKind, ExecutableTerminator as MirTerminator, ExecutableValueRef as MirValueRef, NyarType,
    },
    contracts::{ArrayInitialization, instruction_primary_result},
    target_operator::TargetOperatorBinding,
};
use acorn_wasm::{
    BLOCKTYPE_EMPTY, VALTYPE_ANYREF, VALTYPE_EXTERNREF, VALTYPE_F64, VALTYPE_I32, VALTYPE_I64, VALTYPE_REF_NULL, WasmOpcode, encode_array_copy,
    encode_array_get, encode_array_len, encode_array_new_default, encode_array_new_fixed, encode_array_set, encode_block_empty, encode_br,
    encode_br_if, encode_call, encode_drop, encode_f64_const, encode_f64_load, encode_f64_store, encode_global_get, encode_global_set,
    encode_i32_add, encode_i32_and, encode_i32_const, encode_i32_load, encode_i32_store, encode_i64_const, encode_i64_load, encode_i64_store,
    encode_local_get, encode_local_set, encode_local_tee, encode_loop_empty, encode_memory_copy, encode_memory_fill, encode_memory_grow,
    encode_memory_size, encode_ref_cast_array, encode_ref_cast_type_index, encode_ref_null_anyref, encode_ref_null_externref,
    encode_struct_get, encode_struct_new_default, encode_struct_set, encode_unreachable,
};
use nyar_types::{AggregateLayout, FieldLayout, ItemInstanceId, LayoutId};

pub(in crate::lowering::backends::wasm) fn array_element_type(array_type: &NyarType) -> Option<&NyarType> {
    match array_type {
        NyarType::Array(element) | NyarType::FixedArray { element, .. } => Some(element.as_ref()),
        _ => None,
    }
}
pub(in crate::lowering::backends::wasm) const WASM_GC_EXTERNREF: u8 = VALTYPE_EXTERNREF;

pub(in crate::lowering::backends::wasm) fn lower_mir_function_to_wasm_bytes(
    submission: &FragmentSubmission,
    mir_fn: &MirFunction,
    return_value_type: Option<u8>,
    js_glue_utf8_as_anyref: bool,
    wasi_mode: bool,
    function_index_by_instance: &BTreeMap<ItemInstanceId, u32>,
    type_index_by_instance: &BTreeMap<ItemInstanceId, u32>,
    param_types_by_instance: &BTreeMap<ItemInstanceId, Vec<u8>>,
    return_types_by_instance: &BTreeMap<ItemInstanceId, Option<u8>>,
    param_types_by_function_index: &BTreeMap<u32, Vec<u8>>,
    return_types_by_function_index: &BTreeMap<u32, Option<u8>>,
    import_param_types: &[Vec<u8>],
    import_return_types: &[Option<u8>],
    gc_struct_type_indices: &BTreeMap<LayoutId, u32>,
    gc_array_type_indices: &BTreeMap<String, u32>,
    gc_sum_type_indices: &BTreeMap<NominalInstanceId, u32>,
    gc_i32_box_type_index: u32,
    gc_i64_box_type_index: u32,
    callee_import_index: &BTreeMap<ItemInstanceId, u32>,
    callee_wasm_operator: &BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)>,
    host_imports: &[(String, String)],
    string_literal_index: &BTreeMap<String, u32>,
    string_literal_offset: &BTreeMap<String, u32>,
    const_utf8_import: Option<u32>,
) -> crate::Result<Vec<u8>> {
    if let Err(error) = validate_pattern_matching_invariants(mir_fn) {
        debug_assert!(false, "pattern matching contract violation in function `{}`: {error:?}", mir_fn.symbol);
    }
    let ctx = ExecutableLoweringContext::new(submission);
    let mut lowerer = WasmMirLowerer::new(
        &ctx,
        mir_fn,
        return_value_type,
        js_glue_utf8_as_anyref,
        wasi_mode,
        function_index_by_instance,
        type_index_by_instance,
        param_types_by_instance,
        return_types_by_instance,
        param_types_by_function_index,
        return_types_by_function_index,
        import_param_types,
        import_return_types,
        gc_struct_type_indices,
        gc_array_type_indices,
        gc_sum_type_indices,
        gc_i32_box_type_index,
        gc_i64_box_type_index,
        callee_import_index,
        callee_wasm_operator,
        host_imports,
        string_literal_index,
        string_literal_offset,
        const_utf8_import,
    );
    lowerer.emit_function_body()?;
    Ok(lowerer.finish())
}

pub(in crate::lowering::backends::wasm) struct WasmMirLowerer<'a> {
    pub(in crate::lowering::backends::wasm::mir) ctx: &'a ExecutableLoweringContext<'a>,
    pub(in crate::lowering::backends::wasm::mir) mir_fn: &'a MirFunction,
    /// 当前函数?WASM 返回值类型字节?
    /// `None` = void（无返回值）；`Some(VALTYPE_I32)` = i32；`Some(VALTYPE_ANYREF)` = anyref?
    /// `Some(VALTYPE_F64)` = f64；`Some(VALTYPE_I64)` = i64?
    pub(in crate::lowering::backends::wasm::mir) return_value_type: Option<u8>,
    /// Node JS-glue 路径：utf8/utf16 ?anyref 传递（宿主字符串）?
    pub(in crate::lowering::backends::wasm::mir) js_glue_utf8_as_anyref: bool,
    /// WASI 轨：字符串字面量作为 i32 偏移量传递，存入 data 段?
    pub(in crate::lowering::backends::wasm::mir) wasi_mode: bool,
    pub(in crate::lowering::backends::wasm::mir) code: Vec<u8>,
    /// Scratch: aligned pointer returned by the global bump allocator.
    pub(in crate::lowering::backends::wasm::mir) stack_ptr_local: u32,
    /// Scratch: `new_end` after bump.
    pub(in crate::lowering::backends::wasm::mir) bump_end_local: u32,
    /// Scratch: pages to grow.
    pub(in crate::lowering::backends::wasm::mir) bump_pages_local: u32,
    pub(in crate::lowering::backends::wasm::mir) next_local: u32,
    /// 每个声明 local 的完?valtype 字节（i32=VALTYPE_I32, anyref=VALTYPE_ANYREF, struct ref=VALTYPE_REF+typeidx）?
    pub(in crate::lowering::backends::wasm::mir) local_valtypes: Vec<Vec<u8>>,
    pub(in crate::lowering::backends::wasm::mir) value_locals: BTreeMap<MirValueRef, u32>,
    /// 引用类型 local 的集合。这?local 的值类型是 `anyref` (VALTYPE_ANYREF),而非 i32?
    pub(in crate::lowering::backends::wasm::mir) reference_locals: BTreeMap<MirValueRef, u32>,
    pub(in crate::lowering::backends::wasm::mir) block_order: Vec<MirBlockRef>,
    pub(in crate::lowering::backends::wasm::mir) block_index: BTreeMap<MirBlockRef, usize>,
    pub(in crate::lowering::backends::wasm::mir) function_index_by_instance: &'a BTreeMap<ItemInstanceId, u32>,
    pub(in crate::lowering::backends::wasm::mir) type_index_by_instance: &'a BTreeMap<ItemInstanceId, u32>,
    pub(in crate::lowering::backends::wasm::mir) param_types_by_instance: &'a BTreeMap<ItemInstanceId, Vec<u8>>,
    pub(in crate::lowering::backends::wasm::mir) return_types_by_instance: &'a BTreeMap<ItemInstanceId, Option<u8>>,
    /// ?`function_index_by_instance` 下标对齐?callee 形参 wasm 类型（call 实参 coerce 权威来源）?
    pub(in crate::lowering::backends::wasm::mir) param_types_by_function_index: &'a BTreeMap<u32, Vec<u8>>,
    pub(in crate::lowering::backends::wasm::mir) return_types_by_function_index: &'a BTreeMap<u32, Option<u8>>,
    pub(in crate::lowering::backends::wasm::mir) import_param_types: &'a [Vec<u8>],
    pub(in crate::lowering::backends::wasm::mir) import_return_types: &'a [Option<u8>],
    /// 引用类型 layout_id -> wasm-gc structtype ?type_index?
    pub(in crate::lowering::backends::wasm::mir) gc_struct_type_indices: &'a BTreeMap<LayoutId, u32>,
    /// unite sum_name -> wasm-gc structtype [i32,anyref] ?type_index?
    /// Fine/Fail ?unite 标量 payload（utf8/bool/i32）装箱用 structtype [i32]?
    pub(in crate::lowering::backends::wasm::mir) gc_i32_box_type_index: u32,
    /// 泛型 `T` 数组槽中的 i64 装箱用 structtype `[i64]`。
    pub(in crate::lowering::backends::wasm::mir) gc_i64_box_type_index: u32,
    /// heap array element_type 字符串键 -> wasm-gc arraytype 的 type_index。
    pub(in crate::lowering::backends::wasm::mir) gc_array_type_indices: &'a BTreeMap<String, u32>,
    /// sum nominal identity -> wasm-gc structtype `[i32 tag, anyref payload]` 的 type_index。
    pub(in crate::lowering::backends::wasm::mir) gc_sum_type_indices: &'a BTreeMap<NominalInstanceId, u32>,
    pub(in crate::lowering::backends::wasm::mir) callee_import_index: &'a BTreeMap<ItemInstanceId, u32>,
    /// `[wasm_operator]` 绑定的 provider 实例 → 计划冻结的目标指令配方。
    pub(in crate::lowering::backends::wasm::mir) callee_wasm_operator: &'a BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)>,
    /// WASI 宿主 import 表（?stream 内建），?write-via-stream 协议查找索引?
    pub(in crate::lowering::backends::wasm::mir) host_imports: &'a [(String, String)],
    pub(in crate::lowering::backends::wasm::mir) string_literal_index: &'a BTreeMap<String, u32>,
    /// WASI 轨：字符串字面量到线性内存偏移量的映射?
    pub(in crate::lowering::backends::wasm::mir) string_literal_offset: &'a BTreeMap<String, u32>,
    pub(in crate::lowering::backends::wasm::mir) const_utf8_import: Option<u32>,
    /// CFG 调度中的基本块索引。
    pub(in crate::lowering::backends::wasm::mir) pc_local: u32,
}

impl<'a> WasmMirLowerer<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        ctx: &'a ExecutableLoweringContext<'a>,
        mir_fn: &'a MirFunction,
        return_value_type: Option<u8>,
        js_glue_utf8_as_anyref: bool,
        wasi_mode: bool,
        function_index_by_instance: &'a BTreeMap<ItemInstanceId, u32>,
        type_index_by_instance: &'a BTreeMap<ItemInstanceId, u32>,
        param_types_by_instance: &'a BTreeMap<ItemInstanceId, Vec<u8>>,
        return_types_by_instance: &'a BTreeMap<ItemInstanceId, Option<u8>>,
        param_types_by_function_index: &'a BTreeMap<u32, Vec<u8>>,
        return_types_by_function_index: &'a BTreeMap<u32, Option<u8>>,
        import_param_types: &'a [Vec<u8>],
        import_return_types: &'a [Option<u8>],
        gc_struct_type_indices: &'a BTreeMap<LayoutId, u32>,
        gc_array_type_indices: &'a BTreeMap<String, u32>,
        gc_sum_type_indices: &'a BTreeMap<NominalInstanceId, u32>,
        gc_i32_box_type_index: u32,
        gc_i64_box_type_index: u32,
        callee_import_index: &'a BTreeMap<ItemInstanceId, u32>,
        callee_wasm_operator: &'a BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)>,
        host_imports: &'a [(String, String)],
        string_literal_index: &'a BTreeMap<String, u32>,
        string_literal_offset: &'a BTreeMap<String, u32>,
        const_utf8_import: Option<u32>,
    ) -> Self {
        let block_order = collect_reachable_blocks(mir_fn);
        let block_index = block_order.iter().enumerate().map(|(index, block_id)| (*block_id, index)).collect();
        // wasm 函数参数占据 local 0..param_count-1，声明局部从 param_count 开始?
        // stack_ptr 是第一个声明的局部，位于 local param_count?
        // 若函数有 anyref 参数，旧代码?stack_ptr 放在 local 0 会与参数 0 冲突?
        // 导致 V8 ?`local.set[0] expected type anyref, found i32.const`?
        let param_count = mir_fn.param_types.len() as u32;
        let mut lowerer = Self {
            ctx,
            mir_fn,
            return_value_type,
            js_glue_utf8_as_anyref,
            wasi_mode,
            code: Vec::new(),
            stack_ptr_local: param_count,
            bump_end_local: param_count + 1,
            bump_pages_local: param_count + 2,
            // Branch flag locals 按嵌套深度按需 alloc_i32_local，不预留单一 slot?
            next_local: param_count + 3,
            local_valtypes: vec![vec![VALTYPE_I32], vec![VALTYPE_I32], vec![VALTYPE_I32]],
            value_locals: BTreeMap::new(),
            reference_locals: BTreeMap::new(),
            block_order,
            block_index,
            function_index_by_instance,
            type_index_by_instance,
            param_types_by_instance,
            return_types_by_instance,
            param_types_by_function_index,
            return_types_by_function_index,
            import_param_types,
            import_return_types,
            gc_struct_type_indices,
            gc_array_type_indices,
            gc_sum_type_indices,
            gc_i32_box_type_index,
            gc_i64_box_type_index,
            callee_import_index,
            callee_wasm_operator,
            host_imports,
            string_literal_index,
            string_literal_offset,
            const_utf8_import,
            pc_local: 0,
        };
        lowerer.pc_local = lowerer.alloc_i32_local();
        let entry = mir_fn.blocks.iter().find(|block| block.id == mir_fn.entry).expect("WASM 函数缺少入口块");
        assert_eq!(entry.parameters.len(), mir_fn.param_types.len(), "WASM 入口参数与函数合同不一致");
        for block in &mir_fn.blocks {
            for (param_slot, param) in block.parameters.iter().enumerate() {
                let ty = mir_fn.value_types.get(param).expect("WASM 块参数缺少已确定类型");
                let stack_type = wasm_param_value_type_for(
                    ctx,
                    ty,
                    mir_fn.value_layouts.get(param).copied(),
                    gc_struct_type_indices,
                    js_glue_utf8_as_anyref,
                );
                let local = if block.id == mir_fn.entry {
                    assert_eq!(ty, &mir_fn.param_types[param_slot], "WASM 入口 SSA 类型与函数参数不一致");
                    u32::try_from(param_slot).expect("WASM 参数索引溢出")
                }
                else {
                    lowerer.alloc_value_local(stack_type)
                };
                assert!(!lowerer.value_locals.contains_key(param) && !lowerer.reference_locals.contains_key(param), "WASM 块参数重复定义");
                if matches!(stack_type, WASM_GC_ANYREF | WASM_GC_EXTERNREF) {
                    lowerer.reference_locals.insert(*param, local);
                }
                else {
                    lowerer.value_locals.insert(*param, local);
                }
            }
        }
        for block in &mir_fn.blocks {
            for instruction in &block.instructions {
                lowerer.plan_instruction(instruction);
            }
        }
        lowerer
    }

    pub(super) fn storage_for_type(&self, ty: &NyarType) -> MirStorageKind {
        mir_storage_for_type(self.ctx, ty, self.js_glue_utf8_as_anyref)
    }

    pub(super) fn finish(self) -> Vec<u8> {
        // ?local index 顺序扫描,将相同类型的连续 local 合并为一组?
        // wasm 规范要求 local 声明组的总数等于实际 local ?顺序?local 索引?
        let mut locals: Vec<(u32, Vec<u8>)> = Vec::new();
        let mut index = 0u32;
        while index < self.local_valtypes.len() as u32 {
            let ty = &self.local_valtypes[index as usize];
            let mut count = 1u32;
            while (index + count) < self.local_valtypes.len() as u32 && self.local_valtypes[(index + count) as usize] == *ty {
                count += 1;
            }
            locals.push((count, ty.clone()));
            index += count;
        }
        let mut body = Vec::new();
        encode_uleb128(locals.len() as u32, &mut body);
        for (count, ty) in locals {
            encode_uleb128(count, &mut body);
            body.extend_from_slice(&ty);
        }
        body.extend_from_slice(&self.code);
        WasmOpcode::End.encode(&mut body);
        body
    }

    pub(super) fn alloc_i32_local(&mut self) -> u32 {
        let local = self.next_local;
        self.next_local += 1;
        self.local_valtypes.push(vec![VALTYPE_I32]);
        local
    }

    pub(super) fn alloc_i64_local(&mut self) -> u32 {
        let local = self.next_local;
        self.next_local += 1;
        self.local_valtypes.push(vec![VALTYPE_I64]);
        local
    }

    pub(super) fn alloc_f64_local(&mut self) -> u32 {
        let local = self.next_local;
        self.next_local += 1;
        self.local_valtypes.push(vec![VALTYPE_F64]);
        local
    }

    /// 按栈上标?valtype 分配 local（i64/f64/i32）。引用类型请?`alloc_anyref_local`?
    pub(super) fn alloc_scalar_local_for_stack_type(&mut self, stack_ty: u8) -> u32 {
        match stack_ty {
            VALTYPE_I64 => self.alloc_i64_local(),
            VALTYPE_F64 => self.alloc_f64_local(),
            _ => self.alloc_i32_local(),
        }
    }

    /// 分配一?anyref local。该 local ?`finish()` 中按?local index 自动归入 anyref 组?
    pub(super) fn alloc_anyref_local(&mut self) -> u32 {
        let local = self.next_local;
        self.next_local += 1;
        self.local_valtypes.push(vec![WASM_GC_ANYREF]);
        local
    }

    /// ?wasm local 绝对索引映射为值类型字节?
    ///
    /// `local_valtypes` 仅覆盖声明局部（?`stack_ptr_local` 起）?
    /// 函数参数类型来自 `mir_fn.param_types`?
    pub(super) fn wasm_local_value_type(&self, local_index: u32) -> u8 {
        if local_index < self.stack_ptr_local {
            let ty = &self.mir_fn.param_types[local_index as usize];
            let entry = self.mir_fn.blocks.iter().find(|block| block.id == self.mir_fn.entry).expect("Wasm 函数缺少入口合同");
            let parameter = entry.parameters.get(local_index as usize).expect("Wasm 参数 local 缺少 SSA 身份");
            return wasm_param_value_type_for(
                self.ctx,
                ty,
                self.mir_fn.value_layouts.get(parameter).copied(),
                self.gc_struct_type_indices,
                self.js_glue_utf8_as_anyref,
            );
        }
        let declared = usize::try_from(local_index - self.stack_ptr_local).expect("WASM local 索引溢出");
        let valtype = self.local_valtypes.get(declared).expect("WASM local 未预规划");
        assert_eq!(valtype.len(), 1, "WASM local 类型合同不完整");
        valtype[0]
    }

    pub(super) fn plan_instruction(&mut self, instruction: &MirInstruction) {
        // StoreVar 是纯附加操作（let 绑定），wasm 后端不消费其 name/value
        // 但它的 results 仍要规划 local —— 该 SSA 值会被 Jump/Return 等 terminator 引用。
        for output in &instruction.results {
            let stack_type = self.plan_result_stack_type(instruction, output);
            assert!(!self.value_locals.contains_key(output) && !self.reference_locals.contains_key(output), "WASM SSA 结果重复定义");
            let local = self.alloc_value_local(stack_type);
            if matches!(stack_type, WASM_GC_ANYREF | WASM_GC_EXTERNREF) {
                self.reference_locals.insert(*output, local);
            }
            else {
                self.value_locals.insert(*output, local);
            }
        }
    }

    /// 为一条指令的某个 output 决定规划槽位的 wasm 栈类型。
    /// 对 Copy 特判：直接使用 source operand 已规划的物理槽 valtype，
    /// 确保 plan 与 emit 阶段 `operand_wasm_stack_type(source)` 的权威结果一致。
    fn plan_result_stack_type(&self, instruction: &MirInstruction, output: &MirValueRef) -> u8 {
        if let MirInstructionKind::Copy { source } = &instruction.kind {
            if let MirOperand::Value(vref) = source {
                if let Some(local) = self.reference_locals.get(vref).or_else(|| self.value_locals.get(vref)).copied() {
                    return self.wasm_local_value_type(local);
                }
            }
        }
        let ty = self.mir_fn.value_types.get(output).expect("WASM SSA 结果缺少已确定类型");
        wasm_param_value_type_for(
            self.ctx,
            ty,
            self.mir_fn.value_layouts.get(output).copied(),
            self.gc_struct_type_indices,
            self.js_glue_utf8_as_anyref,
        )
    }

    pub(super) fn alloc_value_local(&mut self, stack_type: u8) -> u32 {
        let local = self.next_local;
        self.next_local = self.next_local.checked_add(1).expect("WASM local 索引溢出");
        assert!(matches!(stack_type, VALTYPE_I32 | VALTYPE_I64 | VALTYPE_F64 | WASM_GC_ANYREF | WASM_GC_EXTERNREF), "WASM 不支持此槽类型");
        self.local_valtypes.push(vec![stack_type]);
        local
    }

    pub(super) fn emit_f64_operand(&mut self, operand: &MirOperand) {
        self.emit_contract_operand(operand, VALTYPE_F64);
    }

    pub(super) fn emit_i32_operand(&mut self, operand: &MirOperand) {
        self.emit_contract_operand(operand, VALTYPE_I32);
    }

    /// 引用 local →?i32 布尔：`ref.is_null` ?`i32.eqz`?=非空, 0=null）?
    pub(super) fn emit_anyref_local_as_i32_bool(&mut self, local: u32) {
        self.emit_local_get(local);
        WasmOpcode::RefIsNull.encode(&mut self.code);
        WasmOpcode::I32Eqz.encode(&mut self.code);
    }

    pub(super) fn emit_call_arguments(&mut self, arguments: &[MirOperand], param_types: &[u8]) {
        assert_eq!(arguments.len(), param_types.len(), "WASM 调用实参与已解析签名不一致");
        for (argument, expected) in arguments.iter().zip(param_types) {
            self.emit_operand_coerced(argument, *expected);
        }
    }

    /// Receiver array-ness is Semantic MIR metadata, never a nominal name or
    /// backend-local storage class.
    pub(super) fn operand_is_heap_array(&self, operand: &MirOperand) -> bool {
        match operand {
            MirOperand::Value(vref) => match self.mir_fn.value_types.get(vref) {
                Some(ty) => matches!(ty, NyarType::Array(_) | NyarType::FixedArray { .. }),
                None => false,
            },
            _ => false,
        }
    }

    /// `length` 需宿主字符串；`+` / `==` 需两端均为宿主字符串（否则?i32 原语）?
    pub(super) fn utf8_method_receiver_ok(&self, method: &str, arguments: &[MirOperand]) -> bool {
        match method {
            "length" => self.operand_is_utf8_host_string(arguments.first()),
            "concat" | "infix +" | "equals" | "infix ==" => {
                self.operand_is_utf8_host_string(arguments.first()) && self.operand_is_utf8_host_string(arguments.get(1))
            }
            _ => true,
        }
    }

    pub(super) fn operand_is_utf8_host_string(&self, operand: Option<&MirOperand>) -> bool {
        let Some(operand) = operand
        else {
            return false;
        };
        match operand {
            MirOperand::Constant(MirConstant::Utf8(_)) => !self.wasi_mode,
            MirOperand::Value(vref) => {
                if let Some(ty) = self.mir_fn.value_types.get(vref) {
                    return is_js_glue_host_string_type(ty);
                }
                false
            }
            MirOperand::Symbol(_) => false,
            _ => false,
        }
    }

    /// 普通调用只按 Compiler 实例身份查询物理函数下标。
    pub(super) fn resolve_callee_function_index(&self, callee: &MirOperand) -> Option<u32> {
        let MirOperand::Item(instance) = callee
        else {
            return None;
        };
        self.function_index_by_instance.get(instance).copied()
    }

    /// 类型化调用必须具备对应实例的物理签名，禁止选择第一个函数类型。
    pub(super) fn resolve_callee_type_index(&self, callee: &MirOperand) -> u32 {
        let MirOperand::Item(instance) = callee
        else {
            panic!("WASM 调用缺少 Compiler 实例身份");
        };
        *self.type_index_by_instance.get(instance).expect("WASM 调用缺少实例签名")
    }

    /// Emits the WASM `call` instruction (opcode 0x10).
    pub(super) fn emit_call(&mut self, function_index: u32) {
        WasmOpcode::Call.encode(&mut self.code);
        encode_uleb128(function_index, &mut self.code);
    }

    /// Emits the WASM `call_indirect` instruction (opcode 0x11).
    pub(super) fn emit_call_indirect(&mut self, type_index: u32, table_index: u32) {
        WasmOpcode::CallIndirect.encode(&mut self.code);
        encode_uleb128(type_index, &mut self.code);
        encode_uleb128(table_index, &mut self.code);
    }

    /// 该 layout 是否以 wasm-gc struct 承载（物理表示的唯一权威）。
    ///
    /// 只有 `register_gc_struct_types` 的注册结果有资格回答这个问题。注册条件已覆盖
    /// `storage == Reference`、含引用字段、以及 MIR 以引用方式使用该 layout 三种来源；
    /// 创建点（`StructNew`/`TupleNew`）、读写点（`FieldGet`/`FieldSet`）与 ABI 槽位
    /// （`type_uses_gc_struct_param`）都必须只读这张注册表。
    ///
    /// 反面教训：曾在此处按 `layout.storage` / 字段类型二次推导，导致同一 layout 在
    /// 构造侧走线性内存、消费侧走 gc struct，运行时直接 `illegal cast`。
    pub(super) fn layout_uses_gc_struct(&self, layout: &AggregateLayout) -> bool {
        self.gc_struct_type_indices.contains_key(&layout.id)
    }

    pub(super) fn resolve_layout(&self, nominal: nyar_types::NominalInstanceId) -> AggregateLayout {
        self.ctx
            .layout_by_nominal(nominal)
            .cloned()
            .unwrap_or_else(|| panic!("WASM semantic MIR contract violation: missing aggregate layout identity in `{}`", self.mir_fn.symbol))
    }

    pub(super) fn emit_operand(&mut self, operand: &MirOperand) {
        match operand {
            MirOperand::Value(value) => {
                let local = self.planned_value_local(*value);
                self.emit_local_get(local);
            }
            MirOperand::Constant(constant) => self.emit_load_constant(constant),
            MirOperand::Symbol(_) => panic!("WASM 值操作数缺少 SSA 身份"),
            MirOperand::Item(_) => {
                panic!("WASM callable identity cannot be used as a value operand")
            }
        }
    }

    pub(super) fn output_scalar_slot_type(&self, output: MirValueRef) -> u8 {
        self.wasm_local_value_type(self.planned_value_local(output))
    }

    /// 按目标槽 valtype 发射常量（Int→i64/i32/f64；Unit→anyref；String 仍按 wasi/js 路径）?
    pub(super) fn emit_load_constant_for_slot(&mut self, constant: &MirConstant, slot_ty: u8) {
        match constant {
            MirConstant::Int(value) => match slot_ty {
                VALTYPE_I64 => self.emit_i64_const(*value),
                VALTYPE_F64 => self.emit_f64_const(*value as f64),
                VALTYPE_I32 => self.emit_i32_const(*value as i32),
                _ => panic!("WASM 整数常量与已规划槽不一致"),
            },
            MirConstant::Bool(value) => match slot_ty {
                VALTYPE_I32 => self.emit_i32_const(if *value { 1 } else { 0 }),
                _ => panic!("WASM 布尔常量与已规划槽不一致"),
            },
            MirConstant::Float64(value) => match slot_ty {
                VALTYPE_F64 => self.emit_f64_const(value.into_inner()),
                _ => panic!("WASM 浮点常量与已规划槽不一致"),
            },
            MirConstant::Utf8(_) => {
                assert_eq!(slot_ty, VALTYPE_I32, "WASM 文本句柄与已规划槽不一致");
                self.emit_load_constant(constant);
            }
            MirConstant::Unit => {
                assert_eq!(slot_ty, WASM_GC_ANYREF, "WASM 单元值与已规划槽不一致");
                self.emit_load_constant(constant);
            }
            // 可空空值就是 `T?` 的空引用；槽必须已按 Nullable 规划成 anyref。
            MirConstant::Null => {
                assert_eq!(slot_ty, WASM_GC_ANYREF, "WASM 空值与已规划的 nullable 槽不一致");
                self.emit_ref_null_anyref();
            }
            MirConstant::Utf16(_) => {
                panic!("WASM lowering requires an explicit UTF-16 ABI contract")
            }
        }
    }

    pub(super) fn emit_load_constant(&mut self, constant: &MirConstant) {
        match constant {
            MirConstant::Int(value) => self.emit_i32_const(*value as i32),
            MirConstant::Float64(value) => self.emit_f64_const(value.into_inner()),
            MirConstant::Bool(value) => self.emit_i32_const(if *value { 1 } else { 0 }),
            MirConstant::Utf8(text) => {
                // WASI 轨：字符串字面量作为线性内存偏移量（i32）传递，
                // 字符串内容已通过 data 段在模块初始化时写入线性内存?
                // Canonical ABI 的「字符串」对?**utf8 字节序列**（非语言?string）?
                if self.wasi_mode {
                    if let Some(&offset) = self.string_literal_offset.get(text) {
                        self.emit_i32_const(offset as i32);
                        return;
                    }
                }
                else if let Some(import_index) = self.const_utf8_import {
                    if let Some(&literal_index) = self.string_literal_index.get(text) {
                        self.emit_i32_const(literal_index as i32);
                        self.emit_call(import_index);
                        return;
                    }
                }
                // 未登记宿主载体的字面量不得伪造 `i32 0` 句柄；
                // 预编码校验 `validate_string_literal_payloads` 已拒绝该形状。
                eprintln!(
                    "[wasm::mir] missing host carrier for utf8 literal at `{}` (wasi={}, const_utf8={:?})",
                    self.mir_fn.symbol, self.wasi_mode, self.const_utf8_import
                );
                encode_unreachable(&mut self.code);
            }
            MirConstant::Utf16(_) => {
                panic!("WASM lowering requires an explicit UTF-16 ABI contract")
            }
            // Unit ADT=1（恰有一个值）；用 ref.null any ?GC 占位，勿?void(ADT=0) 混淆?
            MirConstant::Unit => self.emit_ref_null_anyref(),
            // `T?` 的空引用：直接编码空引用，不构造名义 Option 载体。
            MirConstant::Null => self.emit_ref_null_anyref(),
        }
    }

    /// 将栈顶值存?output 对应?local?
    ///
    /// 按与 `Call` 输出存储相同的优先级查找 local?
    /// `reference_locals`（anyref）→ `scalar_locals`（i32）→ `value_locals`（i32）?
    ///
    /// 旧实现仅检?`scalar_locals`，当 `plan_instruction` ?output 分配?
    /// `reference_locals`（引用类型）时找不到 local，导致栈顶值未存储?
    /// 或被其他指令错误消费。修复后 `Copy`/`Identity` 等调?`store_scalar` 的指?
    /// 能正确将 anyref 结果存入 anyref local，避?`local.set expected i32, found anyref`?
    pub(super) fn store_scalar(&mut self, value: MirValueRef) {
        self.assign_output_local(value);
    }

    pub(super) fn operand_address_local(&self, operand: &MirOperand) -> Option<u32> {
        match operand {
            MirOperand::Value(value) => self.value_locals.get(value).copied(),
            _ => None,
        }
    }

    pub(super) fn field_store_stack_type(&self, field: &FieldLayout) -> u8 {
        if self.storage_for_type(&field.ty) == StorageKind::Reference {
            // 值类型聚合在线性内存中不存?GC 引用,?i32 占位?
            VALTYPE_I32
        }
        else if self.storage_for_type(&field.ty) == StorageKind::Value {
            VALTYPE_I32
        }
        else {
            wasm_param_value_type(self.ctx, &field.ty, self.js_glue_utf8_as_anyref)
        }
    }

    /// wasm-gc `struct.set` 字段值栈类型（与 `register_gc_struct_types` / `wasm_gc_field_type_byte` 一致）?
    pub(super) fn gc_struct_field_stack_type(&self, field: &FieldLayout) -> u8 {
        wasm_gc_field_type_byte_for_glue(&field.ty, self.js_glue_utf8_as_anyref)
    }

    /// Emits a store for a single field.
    ///
    /// For scalar fields, emits the matching `i32.store` / `i64.store` / `f64.store`.
    /// For nested value-type fields, emits `memory.copy` to copy the inline contents
    /// from the source address (already on the stack) into the parent's field region.
    /// Stack at entry: `[dest_addr, source_value_or_addr]`.
    pub(super) fn emit_store_at_field(&mut self, field: &FieldLayout) {
        let is_value_type = self.storage_for_type(&field.ty) == StorageKind::Value;
        if is_value_type {
            // Stack: [dest_addr, source_addr] →?memory.copy expects [dest, source, len]
            self.emit_i32_const(field.size as i32);
            self.emit_memory_copy();
            return;
        }
        match field.ty {
            NyarType::Float64 => encode_f64_store(3, 0, &mut self.code),
            NyarType::Integer64 { .. } => encode_i64_store(3, 0, &mut self.code),
            _ => encode_i32_store(2, 0, &mut self.code),
        }
    }

    pub(super) fn emit_load_at_field(&mut self, field: &FieldLayout) {
        match field.ty {
            NyarType::Float64 => encode_f64_load(3, 0, &mut self.code),
            NyarType::Integer64 { .. } => encode_i64_load(3, 0, &mut self.code),
            _ => encode_i32_load(2, 0, &mut self.code),
        }
    }
}

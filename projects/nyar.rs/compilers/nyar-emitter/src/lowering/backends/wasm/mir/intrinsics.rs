//! Split from `lowerer.rs`.
//!
//! wasm GC 数组操作的共享发射器：MIR `ArrayNew`/`ArrayGet`/`ArraySet`/`ArrayLength`
//! 臂与 `[wasm_operator("array.*")]` 冻结绑定共用这一组方法。arraytype 索引一律在
//! 编码期从 RepresentationPlan 注册表（`gc_array_type_indices`）按元素类型解析，
//! 查不到即计划事实缺失，结构化失败，禁止 runtime trap 占位。
use super::*;

impl<'a> WasmMirLowerer<'a> {
    /// 发射 `array.new_default <arraytype>`：按元素类型解析 arraytype，分配默认值数组。
    /// 栈：`[length: i32]` → `[arrayref]`，结果写入 `output` 的引用槽。
    pub(super) fn emit_array_new_default_ops(
        &mut self,
        element_type: &NyarType,
        length: &MirOperand,
        output: MirValueRef,
    ) -> crate::Result<()> {
        let Some(type_index) = self.resolve_gc_array_type_index(element_type)
        else {
            return Err(format!(
                "WASM ArrayNew missing gc arraytype in `{}` (element={element_type:?}); \
                 RepresentationPlan must register arraytype — refusing runtime trap placeholders",
                self.mir_fn.symbol
            ));
        };
        self.emit_contract_operand(length, VALTYPE_I32)?;
        self.emit_array_new_default(type_index);
        self.validate_output_slot_type(output, WASM_GC_ANYREF);
        self.assign_output_local(output);
        Ok(())
    }

    /// 发射 `ref.cast array` + `array.len`：栈 `[anyref]` → `[i32]`，结果写入 `output`。
    pub(super) fn emit_array_length_ops(&mut self, array: &MirOperand, output: Option<MirValueRef>) -> crate::Result<()> {
        self.emit_operand_coerced(array, WASM_GC_ANYREF)?;
        self.emit_ref_cast_array();
        self.emit_array_len();
        if let Some(output) = output {
            self.store_scalar(output);
        }
        Ok(())
    }

    /// 发射 `ref.cast (ref arraytype)` + `array.get`：栈 `[arrayref, i32]` → `[element]`。
    pub(super) fn emit_array_get_ops(&mut self, arguments: &[MirOperand], output: Option<MirValueRef>) -> crate::Result<()> {
        if arguments.len() < 2 {
            return Ok(());
        }
        let element_type =
            self.infer_array_element_type(&arguments[0]).or_else(|| output.and_then(|value| self.mir_fn.value_types.get(&value).cloned()));
        let type_index = element_type.as_ref().and_then(|ty| self.resolve_gc_array_type_index(ty));
        let mut element_stack_ty =
            element_type.as_ref().map(|ty| wasm_gc_field_type_byte_for_glue(ty, self.js_glue_utf8_as_anyref)).unwrap_or(VALTYPE_I32);
        // 无 arraytype 时禁止先 `ref.cast` receiver：误分 i32 会触
        // `expected anyref, found i32`（func564 嵌套 ArrayGet）。
        // 消费侧查不到已注册 arraytype 属于计划事实缺失（可执行合同不一致），
        // 必须与 `Cast` 臂同款结构化失败返回 `Err`，禁止 panic 把编译期缺陷升级成崩溃。
        let Some(type_index) = type_index
        else {
            return Err(format!(
                "WASM ArrayGet missing gc arraytype in `{}` (element={element_type:?}); \
                 RepresentationPlan must register arraytype — refusing runtime trap placeholders",
                self.mir_fn.symbol
            ));
        };
        self.emit_operand_coerced(&arguments[0], WASM_GC_ANYREF)?;
        self.emit_ref_cast_array_type(type_index);
        self.emit_i32_operand(&arguments[1])?;
        self.emit_array_get(type_index);
        if let Some(output) = output {
            let wants_i64 = self.mir_fn.value_types.get(&output).is_some_and(|ty| {
                matches!(ty, NyarType::Integer64 { .. } | NyarType::Integer128 { .. })
                    || matches!(ty, NyarType::Named(name) if name.as_str() == "i64")
            });
            if element_stack_ty == WASM_GC_ANYREF && wants_i64 {
                self.emit_unbox_i64_payload();
                element_stack_ty = VALTYPE_I64;
            }
            // `[utf8]` 数组元素边界：元素按冻结合同是 i32 宿主句柄，而结果 SSA 值携带
            // 已登记 gc struct layout（计划槽 anyref）时，必须消费装配期冻结的物化配方
            // 把句柄物化为 managed 字符串 struct（合同 §5）；缺配方即结构化失败。
            let materialize_layout = if element_stack_ty == VALTYPE_I32 { self.utf8_array_element_boundary_layout(output) } else { None };
            if let Some(layout_id) = materialize_layout {
                self.emit_utf8_array_element_materialize(layout_id)?;
                element_stack_ty = WASM_GC_ANYREF;
            }
            // 栈顶类型由 arraytype 元素决定（i32 / anyref），必须与 output 槽 valtype 一致；
            // 禁止 `array.get`→i32 后 `local.set` 进 plan 阶段误分类的 anyref 槽。
            self.validate_output_slot_type(output, element_stack_ty);
            self.assign_output_local(output);
        }
        Ok(())
    }

    /// 发射 `ref.cast (ref arraytype)` + `array.set`：栈 `[arrayref, i32, element]` → `[]`。
    pub(super) fn emit_array_set_ops(&mut self, arguments: &[MirOperand], output: Option<MirValueRef>) -> crate::Result<()> {
        let _ = output;
        if arguments.len() < 3 {
            return Ok(());
        }
        let element_type = self.infer_array_element_type(&arguments[0]);
        let type_index = element_type.as_ref().and_then(|ty| self.resolve_gc_array_type_index(ty));
        // 消费侧查不到已注册 arraytype 属于计划事实缺失（可执行合同不一致），
        // 必须与 `Cast` 臂同款结构化失败返回 `Err`，禁止 panic 把编译期缺陷升级成崩溃。
        let Some(type_index) = type_index
        else {
            return Err(format!(
                "WASM ArraySet missing gc arraytype in `{}` (element={element_type:?}); \
                 RepresentationPlan must register arraytype — refusing runtime trap placeholders",
                self.mir_fn.symbol
            ));
        };
        let element_stack_ty =
            element_type.as_ref().map(|ty| wasm_gc_field_type_byte_for_glue(ty, self.js_glue_utf8_as_anyref)).unwrap_or(VALTYPE_I32);
        self.emit_operand_coerced(&arguments[0], WASM_GC_ANYREF)?;
        self.emit_ref_cast_array_type(type_index);
        self.emit_i32_operand(&arguments[1])?;
        // `[utf8]` 数组元素边界：元素按冻结合同是 i32 宿主句柄，而实参槽是 managed
        // anyref 时，必须消费目标计划冻结的 managed→host 句柄转换配方（合同 §5）；
        // 缺配方即可执行合同不一致，结构化失败，禁止 coerce/占位掩盖 carrier 失配。
        if element_stack_ty == VALTYPE_I32 && self.operand_wasm_stack_type(&arguments[2]) == WASM_GC_ANYREF {
            self.emit_managed_utf8_to_host_handle(&arguments[2])?;
        }
        else {
            self.emit_operand_coerced(&arguments[2], element_stack_ty)?;
        }
        self.emit_array_set(type_index);
        Ok(())
    }

    /// 判定 `array.get` 结果值是否命中 `[utf8]` 数组元素边界：值类型是宿主字符串
    /// 且携带已登记 gc struct layout（计划槽 anyref）。纯类型/布局事实判定，
    /// 不按符号名特判。
    fn utf8_array_element_boundary_layout(&self, value: MirValueRef) -> Option<LayoutId> {
        if !self.mir_fn.value_types.get(&value).is_some_and(is_js_glue_host_string_type) {
            return None;
        }
        self.mir_fn.value_layouts.get(&value).copied()
    }

    /// 按冻结配方把 `array.get` 取出的宿主 i32 句柄物化为 managed 字符串 struct：
    /// `call env::host_utf8_extract` 取回 `(ptr, len)` → `array.new_default` →
    /// 逐字节 `i32.load8_u` + `array.set` → `struct.new_default` + `struct.set`。
    /// 栈效果：`[i32]` → `[anyref]`。与库导出物化 wrapper 同一构造序列。
    fn emit_utf8_array_element_materialize(&mut self, layout_id: LayoutId) -> crate::Result<()> {
        let Some(recipe) = self.utf8_array_element_materialize.get(&layout_id)
        else {
            return Err(format!(
                "WASM 数组元素边界 (layout={layout_id:?}) 缺少目标计划冻结的宿主句柄→managed 物化配方: {}",
                self.mir_fn.symbol
            ));
        };
        let ptr_local = self.alloc_i32_local();
        let len_local = self.alloc_i32_local();
        let index_local = self.alloc_i32_local();
        let array_local = self.alloc_anyref_local();
        let object_local = self.alloc_anyref_local();
        // 句柄 → (ptr, len)：多值返回按结果序压栈，逆序落 local。
        self.emit_call(recipe.extract_import_index);
        self.emit_local_set(len_local);
        self.emit_local_set(ptr_local);
        // bytes = array.new_default(len)
        self.emit_local_get(len_local);
        self.emit_array_new_default(recipe.bytes_array_type_index);
        self.emit_local_set(array_local);
        // 逐字节拷贝：i32.load8_u(ptr + i) → array.set(bytes, i, …)
        self.emit_i32_const(0);
        self.emit_local_set(index_local);
        encode_block_empty(&mut self.code);
        encode_loop_empty(&mut self.code);
        self.emit_local_get(index_local);
        self.emit_local_get(len_local);
        WasmOpcode::I32GeU.encode(&mut self.code);
        encode_br_if(1, &mut self.code);
        self.emit_local_get(array_local);
        self.emit_ref_cast_array_type(recipe.bytes_array_type_index);
        self.emit_local_get(index_local);
        self.emit_local_get(ptr_local);
        self.emit_local_get(index_local);
        self.emit_i32_add();
        encode_memarg(WasmOpcode::I32Load8U, 0, 0, &mut self.code);
        self.emit_array_set(recipe.bytes_array_type_index);
        self.emit_local_get(index_local);
        self.emit_i32_const(1);
        self.emit_i32_add();
        self.emit_local_set(index_local);
        encode_br(0, &mut self.code);
        WasmOpcode::End.encode(&mut self.code); // loop
        WasmOpcode::End.encode(&mut self.code); // block
        // managed struct：_repr 字段装入字节数组（与库导出物化 wrapper 同一构造序列）。
        self.emit_struct_new_default(recipe.struct_type_index);
        self.emit_local_set(object_local);
        self.emit_local_get(object_local);
        self.emit_ref_cast_struct(recipe.struct_type_index);
        self.emit_local_get(array_local);
        self.emit_struct_set(recipe.struct_type_index, recipe.bytes_field_slot);
        self.emit_local_get(object_local);
        Ok(())
    }
}

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
        self.emit_operand(length);
        self.emit_array_new_default(type_index);
        self.validate_output_slot_type(output, WASM_GC_ANYREF);
        self.assign_output_local(output);
        Ok(())
    }

    /// 发射 `ref.cast array` + `array.len`：栈 `[anyref]` → `[i32]`，结果写入 `output`。
    pub(super) fn emit_array_length_ops(&mut self, array: &MirOperand, output: Option<MirValueRef>) -> crate::Result<()> {
        self.emit_operand_coerced(array, WASM_GC_ANYREF);
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
        self.emit_operand_coerced(&arguments[0], WASM_GC_ANYREF);
        self.emit_ref_cast_array_type(type_index);
        self.emit_i32_operand(&arguments[1]);
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
        self.emit_operand_coerced(&arguments[0], WASM_GC_ANYREF);
        self.emit_ref_cast_array_type(type_index);
        self.emit_i32_operand(&arguments[1]);
        self.emit_operand_coerced(&arguments[2], element_stack_ty);
        self.emit_array_set(type_index);
        Ok(())
    }
}

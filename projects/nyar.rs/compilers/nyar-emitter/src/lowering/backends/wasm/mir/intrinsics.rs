//! Split from `lowerer.rs`.
use super::*;

impl<'a> WasmMirLowerer<'a> {
    pub(super) fn emit_intrinsic_array_get(&mut self, arguments: &[MirOperand], output: Option<MirValueRef>) {
        if arguments.len() < 2 {
            return;
        }
        let element_type =
            self.infer_array_element_type(&arguments[0]).or_else(|| output.and_then(|value| self.mir_fn.value_types.get(&value).cloned()));
        if element_type.is_none() {
            eprintln!(
                "[wasm::arrayget-type-trace] symbol={} receiver={:?} receiver_ty={:?} output={:?} output_ty={:?}",
                self.mir_fn.symbol,
                arguments.first(),
                arguments.first().and_then(|operand| match operand {
                    MirOperand::Value(value) => self.mir_fn.value_types.get(value),
                    _ => None,
                }),
                output,
                output.and_then(|value| self.mir_fn.value_types.get(&value)),
            );
        }
        let type_index = element_type.as_ref().and_then(|ty| self.resolve_gc_array_type_index(ty));
        let mut element_stack_ty =
            element_type.as_ref().map(|ty| wasm_gc_field_type_byte_for_glue(ty, self.js_glue_utf8_as_anyref)).unwrap_or(VALTYPE_I32);
        // ?arraytype 时禁止先?receiver ?`ref.cast`：receiver 常为误分?i32?
        // 会触?`expected anyref, found i32`（func564 嵌套 ArrayGet）?
        let Some(type_index) = type_index
        else {
            panic!(
                "WASM ArrayGet missing gc arraytype in `{}` (element={element_type:?}); \
                 RepresentationPlan must register arraytype — refusing runtime trap placeholders",
                self.mir_fn.symbol
            );
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
            // 栈顶类型?arraytype 元素决定（i32 / anyref），必须?output ?valtype 一致；
            // 禁止 `array.get`→i32 ?`local.set` ?plan 阶段误分?anyref 槽?
            self.validate_output_slot_type(output, element_stack_ty);
            self.assign_output_local(output);
        }
    }

    pub(super) fn emit_intrinsic_array_set(&mut self, arguments: &[MirOperand], output: Option<MirValueRef>) {
        let _ = output;
        if arguments.len() < 3 {
            return;
        }
        let element_type = self.infer_array_element_type(&arguments[0]);
        let type_index = element_type.as_ref().and_then(|ty| self.resolve_gc_array_type_index(ty));
        let Some(type_index) = type_index
        else {
            panic!(
                "WASM ArraySet missing gc arraytype in `{}` (element={element_type:?}); \
                 RepresentationPlan must register arraytype — refusing runtime trap placeholders",
                self.mir_fn.symbol
            );
        };
        let element_stack_ty =
            element_type.as_ref().map(|ty| wasm_gc_field_type_byte_for_glue(ty, self.js_glue_utf8_as_anyref)).unwrap_or(VALTYPE_I32);
        self.emit_operand_coerced(&arguments[0], WASM_GC_ANYREF);
        self.emit_ref_cast_array_type(type_index);
        self.emit_i32_operand(&arguments[1]);
        self.emit_operand_coerced(&arguments[2], element_stack_ty);
        self.emit_array_set(type_index);
    }
}

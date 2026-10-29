//! Split from `lowerer.rs`.
use super::*;

impl<'a> WasmMirLowerer<'a> {
    pub(super) fn emit_instruction(&mut self, instruction: &MirInstruction) -> crate::Result<()> {
        match &instruction.kind {
            MirInstructionKind::LoadConstant { constant, .. } => {
                // Int 常量在字节码层默认是 i32.const，但 plan 可能?value_types
                //（Integer64）把 output 分到 i64 local。必须按槽位 valtype 发射?
                // 否则 `i32.const -1; local.set <i64>` →?expected i64, found i32?
                if let Some(output) = instruction_primary_result(instruction) {
                    let slot_ty = self.output_scalar_slot_type(output);
                    self.emit_load_constant_for_slot(constant, slot_ty);
                    self.store_scalar(output);
                }
                else {
                    self.emit_load_constant(constant);
                }
            }
            MirInstructionKind::StoreVar { value, .. } => {
                // `StoreVar` 的 result 是「变量在该程序点的当前值」的 SSA 身份：
                // 后继读取（含块终结符的实参）引用的就是它，而不是 `value` 本身。
                // 因此必须把 `value` 写进 result 的预规划槽；跳过不发射会让该 SSA 值
                // 停留在槽默认值（引用槽是 null、标量槽是 0），后续 `ref.cast` 直接
                // 触发 `illegal cast`。禁止按变量名重建值（那是另一回事），但按 SSA
                // 身份搬运值是这条指令的全部语义。
                let source_stack_ty = self.operand_wasm_stack_type(value);
                self.emit_operand(value);
                if let Some(output) = instruction_primary_result(instruction) {
                    self.validate_output_slot_type(output, source_stack_ty);
                    self.assign_output_local(output);
                }
                else {
                    WasmOpcode::Drop.encode(&mut self.code);
                }
            }
            MirInstructionKind::Copy { source } => {
                // plan/emit 顺序不一致时，output 可能被误分配?i32?
                // ?source **真实栈类?*（含 param 槽位 valtype）校?output 槽?
                // 铁律：`emit_operand` 压栈类型必须与即?`local.set` 的槽 valtype 一致，
                // 否则出现 `local.set expected i32, found anyref`（func ?array 形参 Copy）?
                let source_stack_ty = self.operand_wasm_stack_type(source);
                self.emit_operand(source);
                if let Some(output) = instruction_primary_result(instruction) {
                    self.validate_output_slot_type(output, source_stack_ty);
                    self.assign_output_local(output);
                }
                else {
                    WasmOpcode::Drop.encode(&mut self.code);
                }
            }
            MirInstructionKind::Cast { source, ty } => {
                // 语言层 `as` 是表示转换：转换语义由计划冻结的两端 `NyarType` 决定，
                // 栈类型字节（`i32`/`i64`）区分不了有符号与无符号扩展。
                let source_ty = self.operand_semantic_type(source).ok_or_else(|| {
                    format!("wasm 显式 cast 的源操作数缺少冻结类型: {}", self.mir_fn.symbol)
                })?;
                let conversion = scalar_cast_opcode(source_ty, ty)?;
                let Some(output) = instruction_primary_result(instruction)
                else {
                    return Err(format!("wasm 显式 cast 缺少结果槽: {}", self.mir_fn.symbol));
                };
                self.emit_operand(source);
                if let Some(opcode) = conversion {
                    opcode.encode(&mut self.code);
                }
                self.validate_output_slot_type(output, self.output_scalar_slot_type(output));
                self.assign_output_local(output);
            }
            MirInstructionKind::StructNew { fields, nominal } => {
                let Some(output) = instruction_primary_result(instruction)
                else {
                    panic!("WASM StructNew 缺少结果槽: {}", self.mir_fn.symbol);
                };
                let layout = self.resolve_layout(*nominal);
                if self.layout_uses_gc_struct(&layout) {
                    let type_index = self.resolve_gc_struct_type_index(layout.id, &layout.name);
                    self.validate_output_slot_type(output, WASM_GC_ANYREF);
                    let local = self.planned_value_local(output);
                    self.emit_struct_new_default(type_index);
                    self.emit_local_set(local);
                    for (field_id, value) in fields {
                        let Some((_, field_layout, field_index)) = self.ctx.field_layout_by_id(*field_id)
                        else {
                            continue;
                        };
                        let field = field_layout;
                        self.emit_local_get(local);
                        self.emit_ref_cast_struct(type_index);
                        self.emit_operand_coerced(value, self.gc_struct_field_stack_type(field));
                        self.emit_struct_set(type_index, field_index as u32);
                    }
                }
                else {
                    // 未注册的 layout 只能走线性内存；若计划把它放进了 anyref 槽，
                    // 说明注册条件与 MIR 引用使用不一致，必须失败而不是静默不发射。
                    let &local = self.value_locals.get(&output).unwrap_or_else(|| {
                        panic!(
                            "WASM StructNew 计划不一致: layout `{}` 未注册 gc struct，但结果槽不是线性内存地址 (in `{}`)",
                            layout.name, self.mir_fn.symbol
                        )
                    });
                    self.bump_allocate(layout.size, layout.align);
                    self.emit_local_set(local);
                    for (field_id, value) in fields {
                        let Some((_, field, _)) = self.ctx.field_layout_by_id(*field_id)
                        else {
                            continue;
                        };
                        self.emit_local_get(local);
                        self.emit_i32_const(field.offset as i32);
                        self.emit_i32_add();
                        self.emit_operand_coerced(value, self.field_store_stack_type(field));
                        self.emit_store_at_field(field);
                    }
                }
            }
            MirInstructionKind::TupleNew { layout_id, fields } => {
                let Some(output) = instruction_primary_result(instruction)
                else {
                    panic!("WASM TupleNew 缺少结果槽: {}", self.mir_fn.symbol);
                };
                let Some(layout) = self.ctx.layout_by_id(*layout_id).cloned()
                else {
                    panic!("WASM TupleNew 缺少 layout 身份 `{layout_id:?}` in `{}`", self.mir_fn.symbol);
                };
                if self.layout_uses_gc_struct(&layout) {
                    let type_index = self.resolve_gc_struct_type_index(layout.id, &layout.name);
                    self.validate_output_slot_type(output, WASM_GC_ANYREF);
                    let local = self.planned_value_local(output);
                    self.emit_struct_new_default(type_index);
                    self.emit_local_set(local);
                    for (index, value) in fields.iter().enumerate() {
                        let Some(field) = layout.fields.get(index)
                        else {
                            continue;
                        };
                        self.emit_local_get(local);
                        self.emit_ref_cast_struct(type_index);
                        self.emit_operand_coerced(value, self.gc_struct_field_stack_type(field));
                        self.emit_struct_set(type_index, index as u32);
                    }
                }
                else {
                    let &local = self.value_locals.get(&output).unwrap_or_else(|| {
                        panic!(
                            "WASM TupleNew 计划不一致: layout `{}` 未注册 gc struct，但结果槽不是线性内存地址 (in `{}`)",
                            layout.name, self.mir_fn.symbol
                        )
                    });
                    self.bump_allocate(layout.size, layout.align);
                    self.emit_local_set(local);
                    for (index, value) in fields.iter().enumerate() {
                        let Some(field) = layout.fields.get(index)
                        else {
                            continue;
                        };
                        self.emit_local_get(local);
                        self.emit_i32_const(field.offset as i32);
                        self.emit_i32_add();
                        self.emit_operand_coerced(value, self.field_store_stack_type(field));
                        self.emit_store_at_field(field);
                    }
                }
            }
            MirInstructionKind::ArrayNew { array_type, length, initialization } => {
                let Some(output) = instruction_primary_result(instruction)
                else {
                    return Ok(());
                };
                let Some(&local) = self.reference_locals.get(&output)
                else {
                    return Ok(());
                };
                let Some(element_type) = array_element_type(array_type)
                else {
                    panic!(
                        "WASM ArrayNew expects Array/FixedArray in `{}` (got {array_type:?}); refusing runtime trap placeholders",
                        self.mir_fn.symbol
                    );
                };
                let Some(type_index) = self.resolve_gc_array_type_index(element_type)
                else {
                    panic!(
                        "WASM ArrayNew missing gc arraytype in `{}` (element={element_type:?}); \
                         RepresentationPlan must register arraytype — refusing runtime trap placeholders",
                        self.mir_fn.symbol
                    );
                };
                match initialization {
                    ArrayInitialization::Default => {
                        self.emit_operand(length);
                        self.emit_array_new_default(type_index);
                        self.emit_local_set(local);
                    }
                    ArrayInitialization::Fill(fill) => {
                        // Fail-closed seed path: allocate default then refuse silent fill loops.
                        let _ = fill;
                        self.emit_operand(length);
                        self.emit_array_new_default(type_index);
                        self.emit_local_set(local);
                    }
                }
            }
            MirInstructionKind::ArrayFromElements { layout_id, array_type, elements } => {
                let Some(output) = instruction_primary_result(instruction)
                else {
                    return Ok(());
                };
                let Some(element_type) = array_element_type(array_type)
                else {
                    panic!(
                        "WASM ArrayFromElements expects Array/FixedArray in `{}` (got {array_type:?}); refusing runtime trap placeholders",
                        self.mir_fn.symbol
                    );
                };
                if matches!(array_type, NyarType::FixedArray { .. }) {
                    // Fixed value aggregate: linear layout when available.
                    if let Some(layout) = self.ctx.layout_by_id(*layout_id).cloned() {
                        let Some(&local) = self.value_locals.get(&output)
                        else {
                            return Ok(());
                        };
                        self.bump_allocate(layout.size, layout.align);
                        self.emit_local_set(local);
                        for (index, value) in elements.iter().enumerate() {
                            let Some(field) = layout.fields.get(index)
                            else {
                                continue;
                            };
                            self.emit_local_get(local);
                            self.emit_i32_const(field.offset as i32);
                            self.emit_i32_add();
                            self.emit_operand_coerced(value, self.field_store_stack_type(field));
                            self.emit_store_at_field(field);
                        }
                        return Ok(());
                    }
                }
                let Some(&local) = self.reference_locals.get(&output)
                else {
                    return Ok(());
                };
                let Some(type_index) = self.resolve_gc_array_type_index(element_type)
                else {
                    panic!(
                        "WASM ArrayFromElements missing gc arraytype in `{}` (element={element_type:?}); \
                         RepresentationPlan must register arraytype — refusing runtime trap placeholders",
                        self.mir_fn.symbol
                    );
                };
                let element_stack_ty = wasm_gc_field_type_byte_for_glue(element_type, self.js_glue_utf8_as_anyref);
                for value in elements {
                    self.emit_operand_coerced(value, element_stack_ty);
                }
                self.emit_array_new_fixed(type_index, elements.len() as u32);
                self.emit_local_set(local);
            }
            MirInstructionKind::AggregateCopy { layout_id, source, dest } => {
                let Some(layout) = self.ctx.layout_by_id(*layout_id).cloned()
                else {
                    eprintln!("[wasm::mir] AggregateCopy layout identity is not registered in `{}`", self.mir_fn.symbol);
                    encode_unreachable(&mut self.code);
                    return Ok(());
                };
                // 判定是否走 gc_struct 深拷贝路径。
                // 不能只靠 `gc_struct_type_indices.contains_key`——Value layout 也可能
                // 被 Field*/AggregateCopy 扫描登记，而 Symbol 地址拷贝仍须 memory.copy。
                // 与 StructNew storage=Reference / layout.storage=Value 的不一致对齐：
                // 只要 source/dest 已在 reference_locals（anyref），就必须深拷贝。
                let source_in_ref = if let MirOperand::Value(vref) = source { self.reference_locals.contains_key(vref) } else { false };
                let dest_in_ref = if let MirOperand::Value(vref) = dest { self.reference_locals.contains_key(vref) } else { false };
                let use_gc_struct = source_in_ref || dest_in_ref || layout.storage == StorageKind::Reference;
                if use_gc_struct {
                    // 引用类型聚合:深拷贝。struct.new_with_default + 逐字段 struct.get/set。
                    // 注意:这是语义上的"值拷贝",即产生新对象而非共享引用。
                    let Some(dest_local) = self.operand_reference_local(dest)
                    else {
                        return Ok(());
                    };
                    let Some(source_local) = self.operand_reference_local(source)
                    else {
                        return Ok(());
                    };
                    // Physical unite/Result rule: Value-storage payloads that
                    // already live in anyref slots keep payload identity via
                    // shallow anyref copy. Never ref.cast them into a sibling
                    // aggregate layout (that is the Fine/Fail cross-cast trap).
                    if layout.storage == StorageKind::Value {
                        self.emit_local_get(source_local);
                        self.emit_local_set(dest_local);
                        return Ok(());
                    }
                    let layout_hint = layout.name.clone();
                    let type_index = self.resolve_gc_struct_type_index(layout.id, &layout_hint);
                    // 先构造 dest 对象(字段全默认),再逐字段从 source 拷入。
                    self.emit_struct_new_default(type_index);
                    self.emit_local_set(dest_local);
                    for field_index in 0..layout.fields.len() {
                        // struct.set 期望 [ref, value]：先压 dest_ref，再从 source 读字段值。
                        self.emit_local_get(source_local);
                        self.emit_ref_cast_struct(type_index);
                        self.emit_local_get(dest_local);
                        self.emit_ref_cast_struct(type_index);
                        self.emit_struct_get(type_index, field_index as u32);
                        self.emit_struct_set(type_index, field_index as u32);
                    }
                }
                else {
                    // 值类型聚合：memory.copy 复制线性内存字节。
                    match (self.operand_address_local(source), self.operand_address_local(dest)) {
                        (Some(source_local), Some(dest_local)) => {
                            let size = layout.size;
                            if size > 0 {
                                self.emit_local_get(dest_local);
                                self.emit_local_get(source_local);
                                self.emit_i32_const(size as i32);
                                self.emit_memory_copy();
                            }
                        }
                        _ => {}
                    }
                }
            }
            MirInstructionKind::FieldGet { object, field } => {
                let output = instruction_primary_result(instruction);
                let Some(layout) = self.ctx.field_layout_by_id(*field).map(|(layout, _, _)| layout).cloned()
                else {
                    panic!("WASM FieldGet 缺少字段所属 layout in `{}`: field=`{field}` object={object:?}", self.mir_fn.symbol);
                };
                let use_gc_struct = self.layout_uses_gc_struct(&layout);
                if use_gc_struct {
                    let Some(object_local) = self.operand_reference_local(object)
                    else {
                        panic!(
                            "WASM FieldGet 计划不一致: layout `{}` 已注册 gc struct，但对象槽不是引用 (in `{}`)",
                            layout.name, self.mir_fn.symbol
                        );
                    };
                    let Some((_, _, field_index)) = self.ctx.field_layout_by_id(*field)
                    else {
                        return Ok(());
                    };
                    let type_index = self.resolve_gc_struct_type_index(layout.id, &layout.name);
                    self.emit_local_get(object_local);
                    self.emit_ref_cast_struct(type_index);
                    self.emit_struct_get(type_index, field_index as u32);
                    if let Some(output) = output {
                        let field_ty = &layout.fields[field_index as usize].ty;
                        let stack_ty = wasm_gc_field_type_byte_for_glue(field_ty, self.js_glue_utf8_as_anyref);
                        self.validate_output_slot_type(output, stack_ty);
                        self.assign_output_local(output);
                    }
                    else {
                        WasmOpcode::Drop.encode(&mut self.code);
                    }
                }
                else if let Some(object_local) = self.operand_address_local(object) {
                    let Some((_, field_layout, _)) = self.ctx.field_layout_by_id(*field)
                    else {
                        return Ok(());
                    };
                    self.emit_local_get(object_local);
                    self.emit_i32_const(field_layout.offset as i32);
                    self.emit_i32_add();
                    let field_is_value_type = self.storage_for_type(&field_layout.ty) == StorageKind::Value;
                    if let Some(output) = output {
                        if field_is_value_type {
                            self.validate_output_slot_type(output, VALTYPE_I32);
                            self.assign_output_local(output);
                        }
                        else {
                            self.emit_load_at_field(&field_layout);
                            let stack_type = match field_layout.ty {
                                NyarType::Float64 => VALTYPE_F64,
                                NyarType::Integer64 { .. } => VALTYPE_I64,
                                _ => VALTYPE_I32,
                            };
                            self.validate_output_slot_type(output, stack_type);
                            self.assign_output_local(output);
                        }
                    }
                    else if !field_is_value_type {
                        self.emit_load_at_field(&field_layout);
                        WasmOpcode::Drop.encode(&mut self.code);
                    }
                    else {
                        WasmOpcode::Drop.encode(&mut self.code);
                    }
                }
                else if let Some(object_local) = self.operand_reference_local(object) {
                    let Some((_, _, field_index)) = self.ctx.field_layout_by_id(*field)
                    else {
                        return Ok(());
                    };
                    let layout_hint = layout.name.clone();
                    let type_index = self.resolve_gc_struct_type_index(layout.id, &layout_hint);
                    self.emit_local_get(object_local);
                    self.emit_ref_cast_struct(type_index);
                    self.emit_struct_get(type_index, field_index as u32);
                    if let Some(output) = output {
                        let field_ty = &layout.fields[field_index as usize].ty;
                        let stack_ty = wasm_gc_field_type_byte_for_glue(field_ty, self.js_glue_utf8_as_anyref);
                        self.validate_output_slot_type(output, stack_ty);
                        self.assign_output_local(output);
                    }
                    else {
                        WasmOpcode::Drop.encode(&mut self.code);
                    }
                }
                else {
                    encode_unreachable(&mut self.code);
                }
            }
            MirInstructionKind::FieldSet { object, field, value } => {
                let Some(layout) = self.ctx.field_layout_by_id(*field).map(|(layout, _, _)| layout).cloned()
                else {
                    panic!("WASM FieldSet 缺少 layout in `{}`: field=`{field}`", self.mir_fn.symbol);
                };
                if self.layout_uses_gc_struct(&layout) {
                    let Some(object_local) = self.operand_reference_local(object)
                    else {
                        panic!(
                            "WASM FieldSet 计划不一致: layout `{}` 已注册 gc struct，但对象槽不是引用 (in `{}`)",
                            layout.name, self.mir_fn.symbol
                        );
                    };
                    let Some((_, field_layout, field_index)) = self.ctx.field_layout_by_id(*field)
                    else {
                        return Ok(());
                    };
                    let type_index = self.resolve_gc_struct_type_index(layout.id, &layout.name);
                    self.emit_local_get(object_local);
                    self.emit_ref_cast_struct(type_index);
                    self.emit_operand_coerced(value, self.gc_struct_field_stack_type(&field_layout));
                    self.emit_struct_set(type_index, field_index as u32);
                }
                else if let Some(object_local) = self.operand_address_local(object) {
                    let Some((_, field_layout, _)) = self.ctx.field_layout_by_id(*field)
                    else {
                        return Ok(());
                    };
                    self.emit_local_get(object_local);
                    self.emit_i32_const(field_layout.offset as i32);
                    self.emit_i32_add();
                    self.emit_operand_coerced(value, self.field_store_stack_type(&field_layout));
                    self.emit_store_at_field(&field_layout);
                }
                else {
                    panic!("WASM FieldSet 找不到对象槽 in `{}`: layout=`{}` field=`{field}`", self.mir_fn.symbol, layout.name);
                }
            }
            MirInstructionKind::ArrayGet { array, index } => {
                self.emit_intrinsic_array_get(&[array.clone(), index.clone()], instruction_primary_result(instruction));
            }
            MirInstructionKind::ArraySet { array, index, value } => {
                self.emit_intrinsic_array_set(&[array.clone(), index.clone(), value.clone()], instruction_primary_result(instruction));
            }
            MirInstructionKind::ArrayLength { array } => {
                self.emit_operand_coerced(array, WASM_GC_ANYREF);
                self.emit_ref_cast_array();
                self.emit_array_len();
                if let Some(output) = instruction_primary_result(instruction) {
                    self.store_scalar(output);
                }
            }
            MirInstructionKind::ArrayPush { array, value } => {
                self.emit_intrinsic_array_push(&[array.clone(), value.clone()], instruction_primary_result(instruction));
            }
            MirInstructionKind::NullIsNull { value } => {
                self.emit_operand_coerced(value, WASM_GC_ANYREF);
                WasmOpcode::RefIsNull.encode(&mut self.code);
                if let Some(output) = instruction_primary_result(instruction) {
                    self.store_scalar(output);
                }
            }
            MirInstructionKind::NullUnwrap { value } => {
                // `try?` / pattern lowering already branches on `NullIsNull` before unwrap.
                self.emit_operand_coerced(value, WASM_GC_ANYREF);
                if let Some(output) = instruction_primary_result(instruction) {
                    self.store_scalar(output);
                }
            }
            MirInstructionKind::Call { callee, arguments } => {
                // Slim Call has no dispatch/witness/intrinsic_opcode; route as Static.
                self.emit_call_lowering(callee, arguments, MirDispatchKind::Static, None, None, instruction_primary_result(instruction))?;
            }
            // pattern 无法 lowering：extractor ?resolved 或类型推断失败，
            // 运行?trap——emit wasm `unreachable` (0x00) 立即触发 trap?
            // ── sum（nominal variant）───────────────────────────────────────
            // 物理载体是后端私有的 `[i32 tag, anyref payload]` struct；语义身份
            // （nominal / variant）只来自计划冻结的 `SumRepresentation`，绝不按
            // 字段名、成员短名或字面量 tag 推断。
            MirInstructionKind::SumNew { nominal, variant, payload } => {
                let type_index = self.resolve_gc_sum_type_index(nominal);
                let (tag, payload_type) = self.sum_variant_representation(nominal, variant);
                assert_eq!(
                    payload_type.is_some(),
                    payload.is_some(),
                    "WASM SumNew variant {variant:?} 的操作数与计划 payload 合同不一致 in `{}`",
                    self.mir_fn.symbol
                );
                self.emit_struct_new_default(type_index);
                let object_local = self.alloc_anyref_local();
                self.emit_local_set(object_local);
                self.emit_local_get(object_local);
                self.emit_ref_cast_struct(type_index);
                self.emit_i32_const(i32::try_from(tag).expect("sum tag 超出 i32"));
                self.emit_struct_set(type_index, SUM_TAG_FIELD);
                if let Some(payload) = payload {
                    self.emit_local_get(object_local);
                    self.emit_ref_cast_struct(type_index);
                    self.emit_payload_to_anyref(payload);
                    self.emit_struct_set(type_index, SUM_PAYLOAD_FIELD);
                }
                self.emit_local_get(object_local);
                if let Some(output) = instruction_primary_result(instruction) {
                    self.validate_output_slot_type(output, WASM_GC_ANYREF);
                    self.assign_output_local(output);
                }
                else {
                    WasmOpcode::Drop.encode(&mut self.code);
                }
            }
            MirInstructionKind::SumPayloadGet { nominal, variant, object } => {
                let type_index = self.resolve_gc_sum_type_index(nominal);
                let (_, payload_type) = self.sum_variant_representation(nominal, variant);
                assert!(
                    payload_type.is_some(),
                    "WASM SumPayloadGet 对无 payload 的 variant {variant:?} 取值 in `{}`",
                    self.mir_fn.symbol
                );
                let output = instruction_primary_result(instruction)
                    .unwrap_or_else(|| panic!("WASM SumPayloadGet 缺少结果槽 in `{}`", self.mir_fn.symbol));
                let target_stack_ty = self.wasm_local_value_type(self.planned_value_local(output));
                self.emit_operand_coerced(object, WASM_GC_ANYREF);
                self.emit_ref_cast_struct(type_index);
                self.emit_struct_get(type_index, SUM_PAYLOAD_FIELD);
                self.emit_payload_from_anyref(target_stack_ty);
                self.validate_output_slot_type(output, target_stack_ty);
                self.assign_output_local(output);
            }
            MirInstructionKind::SumVariantIs { nominal, variant, object } => {
                let type_index = self.resolve_gc_sum_type_index(nominal);
                let (tag, _) = self.sum_variant_representation(nominal, variant);
                self.emit_operand_coerced(object, WASM_GC_ANYREF);
                self.emit_ref_cast_struct(type_index);
                self.emit_struct_get(type_index, SUM_TAG_FIELD);
                self.emit_i32_const(i32::try_from(tag).expect("sum tag 超出 i32"));
                WasmOpcode::I32Eq.encode(&mut self.code);
                if let Some(output) = instruction_primary_result(instruction) {
                    self.store_scalar(output);
                }
                else {
                    WasmOpcode::Drop.encode(&mut self.code);
                }
            }
            MirInstructionKind::PatternMatch { value, .. } => {
                eprintln!("[wasm::mir] PatternMatch trap in `{}`: value={:?}", self.mir_fn.symbol, value);
                encode_unreachable(&mut self.code);
            }
            _ => {}
        }
        Ok(())
    }

    /// 把 payload 操作数放进 sum 的 `anyref` 载荷槽。
    ///
    /// 载荷槽统一是 `anyref`，所以标量必须先装箱成 `[i32]` / `[i64]`；直接放标量
    /// 会让 `struct.set` 在 V8 校验阶段失败。引用载荷原样透传。
    fn emit_payload_to_anyref(&mut self, payload: &MirOperand) {
        let stack_ty = self.operand_wasm_stack_type(payload);
        let box_ty = match stack_ty {
            WASM_GC_ANYREF => {
                self.emit_operand(payload);
                return;
            }
            VALTYPE_I32 => self.gc_i32_box_type_index,
            VALTYPE_I64 => self.gc_i64_box_type_index,
            other => panic!(
                "WASM sum payload 不能以 valtype {other} 装箱（symbol=`{}`）；\
                 sum 载荷槽只承诺 anyref 与 `[i32]` / `[i64]` 装箱",
                self.mir_fn.symbol
            ),
        };
        self.emit_struct_new_default(box_ty);
        let tmp = self.alloc_anyref_local();
        self.emit_local_set(tmp);
        self.emit_local_get(tmp);
        self.emit_ref_cast_struct(box_ty);
        self.emit_operand(payload);
        self.emit_struct_set(box_ty, 0);
        self.emit_local_get(tmp);
    }

    /// 把 sum 载荷槽里的 `anyref` 还原成 result 槽要求的物理类型。
    ///
    /// 目标类型来自预规划槽（编译期事实），不是猜的：`anyref` 直接透传，
    /// 标量按对应的装箱 struct 拆箱。
    fn emit_payload_from_anyref(&mut self, target_stack_ty: u8) {
        let box_ty = match target_stack_ty {
            WASM_GC_ANYREF => return,
            VALTYPE_I32 => self.gc_i32_box_type_index,
            VALTYPE_I64 => self.gc_i64_box_type_index,
            other => panic!(
                "WASM sum payload 不能还原为 valtype {other}（symbol=`{}`）",
                self.mir_fn.symbol
            ),
        };
        self.emit_ref_cast_struct(box_ty);
        self.emit_struct_get(box_ty, 0);
    }
}

/// sum 物理载体 `[i32 tag, anyref payload]` 的字段序；必须与
/// `type_registry::register_gc_sum_types` 注册的字段顺序一致。
const SUM_TAG_FIELD: u32 = 0;
const SUM_PAYLOAD_FIELD: u32 = 1;

/// 语言层 `as` 的数值表示转换。
///
/// 语义由计划冻结的两端 `NyarType` 决定：`i32` 与 `u32` 都占 `VALTYPE_I32`，只有
/// `signed` 能区分 `i64.extend_i32_s` 与 `i64.extend_i32_u`。任何缺少精确 opcode 的
/// 组合都必须响亮失败，禁止用近似指令冒充（例如把无符号扩展写成有符号扩展）。
fn scalar_cast_opcode(source: &NyarType, target: &NyarType) -> crate::Result<Option<WasmOpcode>> {
    if source == target {
        return Ok(None);
    }
    Ok(Some(match (source, target) {
        (NyarType::Integer32 { signed: true }, NyarType::Integer64 { .. }) => WasmOpcode::I64ExtendI32S,
        // 位宽收窄：wrap 与符号位无关。
        (NyarType::Integer64 { .. }, NyarType::Integer32 { .. }) => WasmOpcode::I32WrapI64,
        _ => {
            return Err(format!(
                "wasm 后端缺少 {source} → {target} 的精确表示转换编码（acorn_wasm 未提供对应 opcode），不得退化为近似转换"
            ));
        }
    }))
}
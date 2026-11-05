//! Split from `lowerer.rs`.
use super::*;

impl<'a> WasmMirLowerer<'a> {
    pub(super) fn emit_instruction(&mut self, instruction: &MirInstruction) -> crate::Result<()> {
        match &instruction.kind {
            MirInstructionKind::LoadConstant { constant, .. } => {
                // 常量的目标槽 valtype 是唯一权威：i64 槽必须 `i64.const`，
                // i32 槽必须 `i32.const` 且范围可表示。无槽形态没有合法位宽，
                // 不得回退到「猜 i32」。
                let Some(output) = instruction_primary_result(instruction)
                else {
                    return Err(format!("WASM 常量指令缺少结果槽，无法确定目标 valtype: `{}`", self.mir_fn.symbol));
                };
                let slot_ty = self.output_scalar_slot_type(output);
                self.emit_load_constant_for_slot(constant, slot_ty)?;
                self.store_scalar(output);
            }
            MirInstructionKind::StoreVar { value, .. } => {
                // `StoreVar` 的 result 是「变量在该程序点的当前值」的 SSA 身份：
                // 后继读取（含块终结符的实参）引用的就是它，而不是 `value` 本身。
                // 因此必须把 `value` 写进 result 的预规划槽；跳过不发射会让该 SSA 值
                // 停留在槽默认值（引用槽是 null、标量槽是 0），后续 `ref.cast` 直接
                // 触发 `illegal cast`。禁止按变量名重建值（那是另一回事），但按 SSA
                // 身份搬运值是这条指令的全部语义。
                if let Some(output) = instruction_primary_result(instruction) {
                    let slot_ty = self.output_scalar_slot_type(output);
                    self.emit_contract_operand(value, slot_ty)?;
                    self.assign_output_local(output);
                }
                else {
                    self.emit_operand(value)?;
                    WasmOpcode::Drop.encode(&mut self.code);
                }
            }
            MirInstructionKind::Copy { source } => {
                // plan/emit 顺序不一致时，output 可能被误分配?i32?
                // ?source **真实栈类?*（含 param 槽位 valtype）校?output 槽?
                // 铁律：`emit_operand` 压栈类型必须与即?`local.set` 的槽 valtype 一致，
                // 否则出现 `local.set expected i32, found anyref`（func ?array 形参 Copy）?
                if let Some(output) = instruction_primary_result(instruction) {
                    let slot_ty = self.output_scalar_slot_type(output);
                    self.emit_contract_operand(source, slot_ty)?;
                    self.assign_output_local(output);
                }
                else {
                    self.emit_operand(source)?;
                    WasmOpcode::Drop.encode(&mut self.code);
                }
            }
            MirInstructionKind::Cast { source, ty } => {
                // 语言层 `as` 是表示转换：转换语义由计划冻结的两端 `NyarType` 决定，
                // 栈类型字节（`i32`/`i64`）区分不了有符号与无符号扩展。
                let source_ty = self
                    .operand_semantic_type(source)
                    .ok_or_else(|| format!("wasm 显式 cast 的源操作数缺少冻结类型: {}", self.mir_fn.symbol))?;
                let steps = scalar_cast_steps(source_ty, ty)?;
                let Some(output) = instruction_primary_result(instruction)
                else {
                    return Err(format!("wasm 显式 cast 缺少结果槽: {}", self.mir_fn.symbol));
                };
                self.emit_operand(source)?;
                for step in &steps {
                    self.emit_scalar_cast_step(*step);
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
                        self.emit_operand_coerced(value, self.gc_struct_field_stack_type(field))?;
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
                        self.emit_operand_coerced(value, self.field_store_stack_type(field))?;
                        self.emit_store_at_field(field);
                    }
                }
            }
            MirInstructionKind::TupleGet { object, layout_id, index } => {
                let output = instruction_primary_result(instruction);
                let Some(layout) = self.ctx.layout_by_id(*layout_id).cloned()
                else {
                    return Err(format!(
                        "WASM TupleGet 缺少 layout 身份 `{layout_id:?}` in `{}`（object={object:?}, index={index}）",
                        self.mir_fn.symbol
                    ));
                };
                let Some(field) = layout.fields.get(*index as usize).cloned()
                else {
                    return Err(format!(
                        "WASM TupleGet 位置索引 {index} 越界：layout `{}` 长度 {}（in `{}`）",
                        layout.name,
                        layout.fields.len(),
                        self.mir_fn.symbol
                    ));
                };
                // 标签只在类型身份层存在，目标布局按位置排列；这里只按冻结 layout 的
                // 槽位读值，不按标签名或源码顺序重新推导。
                if self.layout_uses_gc_struct(&layout) {
                    let Some(object_local) = self.operand_reference_local(object)
                    else {
                        return Err(format!(
                            "WASM TupleGet 计划不一致: layout `{}` 已注册 gc struct，但对象槽不是引用 (in `{}`)",
                            layout.name, self.mir_fn.symbol
                        ));
                    };
                    let type_index = self.resolve_gc_struct_type_index(layout.id, &layout.name);
                    self.emit_local_get(object_local);
                    self.emit_ref_cast_struct(type_index);
                    self.emit_struct_get(type_index, *index);
                    if let Some(output) = output {
                        let stack_ty = wasm_gc_field_type_byte_for_glue(&field.ty, self.js_glue_utf8_as_anyref);
                        self.validate_output_slot_type(output, stack_ty);
                        self.assign_output_local(output);
                    }
                    else {
                        WasmOpcode::Drop.encode(&mut self.code);
                    }
                }
                else if let Some(object_local) = self.operand_address_local(object) {
                    self.emit_local_get(object_local);
                    self.emit_i32_const(field.offset as i32);
                    self.emit_i32_add();
                    let field_is_inline_aggregate = type_is_inline_aggregate(self.ctx, &field.ty);
                    if let Some(output) = output {
                        if field_is_inline_aggregate {
                            self.validate_output_slot_type(output, VALTYPE_I32);
                            self.assign_output_local(output);
                        }
                        else {
                            self.emit_load_at_field(&field);
                            let stack_type = match field.ty {
                                NyarType::Float64 => VALTYPE_F64,
                                NyarType::Integer64 { .. } => VALTYPE_I64,
                                _ => VALTYPE_I32,
                            };
                            self.validate_output_slot_type(output, stack_type);
                            self.assign_output_local(output);
                        }
                    }
                    else {
                        if !field_is_inline_aggregate {
                            self.emit_load_at_field(&field);
                        }
                        WasmOpcode::Drop.encode(&mut self.code);
                    }
                }
                else if let Some(object_local) = self.operand_reference_local(object) {
                    let type_index = self.resolve_gc_struct_type_index(layout.id, &layout.name);
                    self.emit_local_get(object_local);
                    self.emit_ref_cast_struct(type_index);
                    self.emit_struct_get(type_index, *index);
                    if let Some(output) = output {
                        let stack_ty = wasm_gc_field_type_byte_for_glue(&field.ty, self.js_glue_utf8_as_anyref);
                        self.validate_output_slot_type(output, stack_ty);
                        self.assign_output_local(output);
                    }
                    else {
                        WasmOpcode::Drop.encode(&mut self.code);
                    }
                }
                else {
                    return Err(format!(
                        "WASM TupleGet 计划不一致: layout `{}` 的对象槽既不是地址也不是引用 (in `{}`)",
                        layout.name, self.mir_fn.symbol
                    ));
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
                        self.emit_operand_coerced(value, self.gc_struct_field_stack_type(field))?;
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
                        self.emit_operand_coerced(value, self.field_store_stack_type(field))?;
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
                let _ = local;
                match initialization {
                    ArrayInitialization::Default | ArrayInitialization::Fill(_) => {
                        // Fail-closed seed path: allocate default; Fill 的静默填充循环
                        // 由同一共享发射器拒绝（arraytype 事实缺失即结构化失败）。
                        self.emit_array_new_default_ops(element_type, length, output)?;
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
                            self.emit_operand_coerced(value, self.field_store_stack_type(field))?;
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
                    self.emit_operand_coerced(value, element_stack_ty)?;
                }
                self.emit_array_new_fixed(type_index, elements.len() as u32);
                self.emit_local_set(local);
            }
            MirInstructionKind::AggregateCopy { layout_id, source, dest } => {
                let Some(layout) = self.ctx.layout_by_id(*layout_id).cloned()
                else {
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
                else if layout.fields.is_empty() {
                    // 零字段值布局（如 `[primitive(...)] structure usize { }`）在运行期
                    // 就是标量/空载体：值槽直接持有载体本身，不是线性内存地址。
                    // 对这类赋值必须做标量值拷贝；走 `memory.copy` 会把载体数值当地址用，
                    // 产生静默错误结果（空结构共享同一地址不可观测，同样安全）。
                    let Some(dest_local) = self.operand_address_local(dest)
                    else {
                        return Ok(());
                    };
                    self.emit_contract_operand(source, self.wasm_local_value_type(dest_local))?;
                    self.emit_local_set(dest_local);
                }
                else {
                    // 非空值类型聚合：memory.copy 复制线性内存字节。
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
                        (source_slot, dest_slot) => {
                            // 非空值聚合必须走线性内存字节拷贝；缺任一地址槽就是可执行
                            // 合同不一致，静默跳过会让目标槽保持旧值（运行期错编）。
                            return Err(format!(
                                "WASM AggregateCopy 非地址型聚合缺少地址槽 in `{}`: source={source_slot:?} dest={dest_slot:?}",
                                self.mir_fn.symbol
                            ));
                        }
                    }
                }
            }
            MirInstructionKind::FieldGet { object, field } => {
                let output = instruction_primary_result(instruction);
                // 计划事实缺失（字段无 layout / gc struct 与对象槽表示冲突）属于
                // 可执行合同不一致，必须与 `Cast` 臂同款结构化失败返回 `Err`，
                // 禁止 panic 把编译期缺陷升级成崩溃。
                let Some(layout) = self.ctx.field_layout_by_id(*field).map(|(layout, _, _)| layout).cloned()
                else {
                    return Err(format!("WASM FieldGet 缺少字段所属 layout in `{}`: field=`{field}` object={object:?}", self.mir_fn.symbol));
                };
                let use_gc_struct = self.layout_uses_gc_struct(&layout);
                if use_gc_struct {
                    let Some(object_local) = self.operand_reference_local(object)
                    else {
                        return Err(format!(
                            "WASM FieldGet 计划不一致: layout `{}` 已注册 gc struct，但对象槽不是引用 (in `{}`)",
                            layout.name, self.mir_fn.symbol
                        ));
                    };
                    let Some((_, _, field_index)) = self.ctx.field_layout_by_id(*field)
                    else {
                        // 上面已用同一字段身份解析出 layout，二次查表不可能失败：
                        // 失败即计划自相矛盾，必须报错而不是静默丢弃结果槽。
                        return Err(format!(
                            "WASM FieldGet 字段 layout 二次查表不一致（gc struct 分支）: field=`{field}`（symbol=`{}`）",
                            self.mir_fn.symbol
                        ));
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
                        // 与 gc struct 分支同理：二次查表失败是计划自相矛盾，禁止静默跳过。
                        return Err(format!(
                            "WASM FieldGet 字段 layout 二次查表不一致（地址分支）: field=`{field}`（symbol=`{}`）",
                            self.mir_fn.symbol
                        ));
                    };
                    self.emit_local_get(object_local);
                    self.emit_i32_const(field_layout.offset as i32);
                    self.emit_i32_add();
                    // 只有带字段的内联聚合字段读出的是地址；标量与零字段值布局读出的是载体本身，
                    // 必须走 `*load`，否则会把字段地址当作标量值返回。
                    let field_is_inline_aggregate = type_is_inline_aggregate(self.ctx, &field_layout.ty);
                    if let Some(output) = output {
                        if field_is_inline_aggregate {
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
                    else {
                        if !field_is_inline_aggregate {
                            self.emit_load_at_field(&field_layout);
                        }
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
                // 计划事实缺失（字段无 layout / gc struct 与对象槽表示冲突 / 对象槽无法定位）
                // 属于可执行合同不一致，必须与 `Cast` 臂同款结构化失败返回 `Err`，
                // 禁止 panic 把编译期缺陷升级成崩溃。
                let Some(layout) = self.ctx.field_layout_by_id(*field).map(|(layout, _, _)| layout).cloned()
                else {
                    return Err(format!("WASM FieldSet 缺少 layout in `{}`: field=`{field}`", self.mir_fn.symbol));
                };
                if self.layout_uses_gc_struct(&layout) {
                    let Some(object_local) = self.operand_reference_local(object)
                    else {
                        return Err(format!(
                            "WASM FieldSet 计划不一致: layout `{}` 已注册 gc struct，但对象槽不是引用 (in `{}`)",
                            layout.name, self.mir_fn.symbol
                        ));
                    };
                    let Some((_, field_layout, field_index)) = self.ctx.field_layout_by_id(*field)
                    else {
                        return Ok(());
                    };
                    let type_index = self.resolve_gc_struct_type_index(layout.id, &layout.name);
                    self.emit_local_get(object_local);
                    self.emit_ref_cast_struct(type_index);
                    self.emit_operand_coerced(value, self.gc_struct_field_stack_type(&field_layout))?;
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
                    self.emit_operand_coerced(value, self.field_store_stack_type(&field_layout))?;
                    self.emit_store_at_field(&field_layout);
                }
                else {
                    return Err(format!("WASM FieldSet 找不到对象槽 in `{}`: layout=`{}` field=`{field}`", self.mir_fn.symbol, layout.name));
                }
            }
            MirInstructionKind::ArrayGet { array, index } => {
                self.emit_array_get_ops(&[array.clone(), index.clone()], instruction_primary_result(instruction))?;
            }
            MirInstructionKind::ArraySet { array, index, value } => {
                self.emit_array_set_ops(&[array.clone(), index.clone(), value.clone()], instruction_primary_result(instruction))?;
            }
            MirInstructionKind::ArrayLength { array } => {
                self.emit_array_length_ops(array, instruction_primary_result(instruction))?;
            }
            MirInstructionKind::NullIsNull { value } => {
                self.emit_operand_coerced(value, WASM_GC_ANYREF)?;
                WasmOpcode::RefIsNull.encode(&mut self.code);
                if let Some(output) = instruction_primary_result(instruction) {
                    self.store_scalar(output);
                }
            }
            MirInstructionKind::NullUnwrap { value } => {
                // `try?` / pattern lowering already branches on `NullIsNull` before unwrap.
                self.emit_operand_coerced(value, WASM_GC_ANYREF)?;
                if let Some(output) = instruction_primary_result(instruction) {
                    self.store_scalar(output);
                }
            }
            MirInstructionKind::LoadFunctionRef { instance, environment, captured: _ } => {
                // 函数值 = 闭包对象（合同 §5.3）：gc struct `[code: i32, env: anyref]`。
                // code 槽只来自模块装配预扫描登记的 funcref table 槽位；未登记实例是
                // 计划事实缺失，结构化失败，禁止编码期补造 table 项。无捕获路径同样
                // 构造闭包对象，env 字段为 `ref.null anyref`。
                let Some(slot) = self.function_value_table_slots.get(instance).copied()
                else {
                    return Err(format!("WASM 函数值 `{instance:?}` 缺少已登记的 funcref table 槽位: {}", self.mir_fn.symbol));
                };
                let Some(output) = instruction_primary_result(instruction)
                else {
                    return Err(format!("WASM 函数值 `{instance:?}` 缺少结果槽: {}", self.mir_fn.symbol));
                };
                self.validate_output_slot_type(output, WASM_GC_ANYREF);
                // 闭包对象先落进结果槽，再按「local.get + ref.cast + 字段值 + struct.set」
                // 逐槽写入：`struct.set` 会消费引用，不能让它留在栈上继续写下一槽。
                let local = self.planned_value_local(output);
                self.emit_struct_new_default(self.gc_closure_type_index);
                self.emit_local_set(local);
                self.emit_local_get(local);
                self.emit_ref_cast_struct(self.gc_closure_type_index);
                self.emit_i32_const(i32::try_from(slot).expect("funcref table 槽位溢出"));
                self.emit_struct_set(self.gc_closure_type_index, WASM_CLOSURE_CODE_FIELD);
                self.emit_local_get(local);
                self.emit_ref_cast_struct(self.gc_closure_type_index);
                match environment {
                    Some(operand) => self.emit_operand_coerced(operand, WASM_GC_ANYREF)?,
                    None => self.emit_ref_null_anyref(),
                }
                self.emit_struct_set(self.gc_closure_type_index, WASM_CLOSURE_ENV_FIELD);
            }
            MirInstructionKind::Call { callee, arguments } => {
                // Slim Call 没有 dispatch 字段：路由只由 callee 操作数形态决定——
                // `Item` 是 Compiler 冻结的直接 callable 身份（Static），
                // `Value` 是函数值 SSA（i32 table 槽载体），必须走类型化间接调用。
                let dispatch = match callee {
                    MirOperand::Value(_) => MirDispatchKind::Indirect,
                    _ => MirDispatchKind::Static,
                };
                self.emit_call_lowering(callee, arguments, dispatch, None, None, instruction_primary_result(instruction))?;
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
                    self.emit_payload_to_anyref(payload)?;
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
                assert!(payload_type.is_some(), "WASM SumPayloadGet 对无 payload 的 variant {variant:?} 取值 in `{}`", self.mir_fn.symbol);
                let output = instruction_primary_result(instruction)
                    .unwrap_or_else(|| panic!("WASM SumPayloadGet 缺少结果槽 in `{}`", self.mir_fn.symbol));
                let target_stack_ty = self.wasm_local_value_type(self.planned_value_local(output));
                self.emit_operand_coerced(object, WASM_GC_ANYREF)?;
                self.emit_ref_cast_struct(type_index);
                self.emit_struct_get(type_index, SUM_PAYLOAD_FIELD);
                self.emit_payload_from_anyref(target_stack_ty);
                self.validate_output_slot_type(output, target_stack_ty);
                self.assign_output_local(output);
            }
            MirInstructionKind::SumVariantIs { nominal, variant, object } => {
                let type_index = self.resolve_gc_sum_type_index(nominal);
                let (tag, _) = self.sum_variant_representation(nominal, variant);
                self.emit_operand_coerced(object, WASM_GC_ANYREF)?;
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
            // fail-closed：未实现的指令必须在编码期结构化失败。把 `PatternMatch` 编成
            // 运行期 `unreachable` 会把后端能力缺口伪装成运行期 trap，调用方无法区分
            // 「程序真的不可达」与「后端没实现」。
            MirInstructionKind::PatternMatch { pattern_debug, .. } => {
                return Err(format!("WASM 后端未实现指令 PatternMatch（pattern=`{pattern_debug}`，symbol=`{}`）", self.mir_fn.symbol));
            }
            kind => {
                return Err(format!("WASM 后端未实现指令 {kind:?}（symbol=`{}`）", self.mir_fn.symbol));
            }
        }
        Ok(())
    }

    /// 把 payload 操作数放进 sum 的 `anyref` 载荷槽。
    ///
    /// 载荷槽统一是 `anyref`，所以标量必须先装箱成 `[i32]` / `[i64]`；直接放标量
    /// 会让 `struct.set` 在 V8 校验阶段失败。引用载荷原样透传。
    fn emit_payload_to_anyref(&mut self, payload: &MirOperand) -> crate::Result<()> {
        let stack_ty = self.operand_wasm_stack_type(payload);
        let box_ty = match stack_ty {
            WASM_GC_ANYREF => {
                self.emit_contract_operand(payload, WASM_GC_ANYREF)?;
                return Ok(());
            }
            VALTYPE_I32 => self.gc_i32_box_type_index,
            VALTYPE_I64 => self.gc_i64_box_type_index,
            VALTYPE_F64 => self.gc_f64_box_type_index,
            other => panic!(
                "WASM sum payload 不能以 valtype {other} 装箱（symbol=`{}`）；\
                 sum 载荷槽只承诺 anyref 与 `[i32]` / `[i64]` / `[f64]` 装箱",
                self.mir_fn.symbol
            ),
        };
        self.emit_struct_new_default(box_ty);
        let tmp = self.alloc_anyref_local();
        self.emit_local_set(tmp);
        self.emit_local_get(tmp);
        self.emit_ref_cast_struct(box_ty);
        self.emit_contract_operand(payload, stack_ty)?;
        self.emit_struct_set(box_ty, 0);
        self.emit_local_get(tmp);
        Ok(())
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
            VALTYPE_F64 => self.gc_f64_box_type_index,
            other => panic!("WASM sum payload 不能还原为 valtype {other}（symbol=`{}`）", self.mir_fn.symbol),
        };
        self.emit_ref_cast_struct(box_ty);
        self.emit_struct_get(box_ty, 0);
    }

    /// 发射一步表示转换编码（见 [scalar_cast_steps] 的载体不变量）。
    fn emit_scalar_cast_step(&mut self, step: ScalarCastStep) {
        match step {
            ScalarCastStep::Opcode(opcode) => opcode.encode(&mut self.code),
            ScalarCastStep::Byte(byte) => numeric::emit_opcode(byte, &mut self.code),
            ScalarCastStep::I32Const(value) => self.emit_i32_const(value),
        }
    }
}

/// sum 物理载体 `[i32 tag, anyref payload]` 的字段序；必须与
/// `type_registry::register_gc_sum_types` 注册的字段顺序一致。
const SUM_TAG_FIELD: u32 = 0;
const SUM_PAYLOAD_FIELD: u32 = 1;

/// 语言层 `as` 的数值表示转换的单步编码。
///
/// 截断必须由显式掩码或显式符号扩展完成，扩展必须按源类型的符号性选择指令；
/// 都不得依赖宿主隐式回绕，也不得用近似指令冒充。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScalarCastStep {
    Opcode(WasmOpcode),
    /// acorn_wasm（pin `e7ed84a`）尚未导出的标准 wasm opcode 字节，直接写码流。
    Byte(u8),
    /// 压 `i32.const`，供紧随的 `i32.and` 掩码消费。
    I32Const(i32),
}

/// `i64.extend_i32_u` 的标准 opcode 字节。
const WASM_I64_EXTEND_I32_U: u8 = 0xAD;
/// `i32.extend8_s`。
const WASM_I32_EXTEND8_S: u8 = 0xC0;
/// `i32.extend16_s`。
const WASM_I32_EXTEND16_S: u8 = 0xC1;

/// 语言层 `as` 的数值表示转换配方。
///
/// 语义由计划冻结的两端 `NyarType` 决定：`i32` 与 `u32` 都占 `VALTYPE_I32`，只有
/// `signed` 能区分 `i64.extend_i32_s` 与 `i64.extend_i32_u`；窄整数（`Integer8` /
/// `Integer16`）同样占 `VALTYPE_I32`，因此窄化配方必须显式写出，不能靠载体相同就跳过。
///
/// 载体不变量：窄整数与 `Character` 的 i32 载体值是**规范形态**——无符号是掩码后的
/// 非负值，有符号是符号扩展后的值，`Character` 是码点。于是宽化到 `Integer32` 是恒等，
/// 宽化到 `Integer64` 只需按源符号性扩展一次。
///
/// 任何缺少精确配方的组合都必须响亮失败，禁止用近似指令冒充。
fn scalar_cast_steps(source: &NyarType, target: &NyarType) -> crate::Result<Vec<ScalarCastStep>> {
    if source == target {
        return Ok(Vec::new());
    }
    match (source, target) {
        (NyarType::Integer64 { .. }, NyarType::Integer64 { .. }) => return Ok(Vec::new()),
        _ => {}
    }
    let source_on_i32 = match source {
        NyarType::Integer8 { .. } | NyarType::Integer16 { .. } | NyarType::Integer32 { .. } | NyarType::Character => true,
        NyarType::Integer64 { .. } => false,
        _ => return Err(unsupported_cast(source, target)),
    };
    let mut steps = Vec::new();
    if !source_on_i32 {
        // 64 位源先 wrap 回 i32 载体，再按目标宽度截断。
        steps.push(ScalarCastStep::Opcode(WasmOpcode::I32WrapI64));
    }
    match target {
        NyarType::Integer64 { .. } => steps.push(if source_is_signed(source) {
            ScalarCastStep::Opcode(WasmOpcode::I64ExtendI32S)
        }
        else {
            ScalarCastStep::Byte(WASM_I64_EXTEND_I32_U)
        }),
        // 32 位与 `Character` 就是 i32 载体本身的值。
        NyarType::Integer32 { .. } | NyarType::Character => {}
        NyarType::Integer8 { signed: false } => {
            steps.push(ScalarCastStep::I32Const(0xFF));
            steps.push(ScalarCastStep::Opcode(WasmOpcode::I32And));
        }
        NyarType::Integer8 { signed: true } => steps.push(ScalarCastStep::Byte(WASM_I32_EXTEND8_S)),
        NyarType::Integer16 { signed: false } => {
            steps.push(ScalarCastStep::I32Const(0xFFFF));
            steps.push(ScalarCastStep::Opcode(WasmOpcode::I32And));
        }
        NyarType::Integer16 { signed: true } => steps.push(ScalarCastStep::Byte(WASM_I32_EXTEND16_S)),
        _ => return Err(unsupported_cast(source, target)),
    }
    Ok(steps)
}

/// 源类型在 i32 载体上的值是否可能为负：决定宽化到 64 位时用符号扩展还是零扩展。
fn source_is_signed(source: &NyarType) -> bool {
    match source {
        NyarType::Integer8 { signed } | NyarType::Integer16 { signed } | NyarType::Integer32 { signed } => *signed,
        // `Character` 是 Unicode 码点，恒非负。
        _ => false,
    }
}

fn unsupported_cast(source: &NyarType, target: &NyarType) -> String {
    format!("wasm 后端缺少 {source} → {target} 的精确表示转换编码（acorn_wasm 未提供对应 opcode），不得退化为近似转换")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cast(source: NyarType, target: NyarType) -> Vec<ScalarCastStep> {
        scalar_cast_steps(&source, &target).expect("表示转换应有精确配方")
    }

    #[test]
    fn widening_selects_extension_by_source_signedness() {
        let i32s = NyarType::Integer32 { signed: true };
        let u32v = NyarType::Integer32 { signed: false };
        let i64s = NyarType::Integer64 { signed: true };
        assert_eq!(cast(i32s.clone(), i64s.clone()), vec![ScalarCastStep::Opcode(WasmOpcode::I64ExtendI32S)]);
        assert_eq!(cast(u32v, i64s.clone()), vec![ScalarCastStep::Byte(WASM_I64_EXTEND_I32_U)]);
        // 窄整数在 i32 载体上已是规范形态：宽化到 32 位不需要任何指令。
        assert_eq!(cast(NyarType::Integer8 { signed: false }, NyarType::Integer32 { signed: false }), Vec::new());
        assert_eq!(cast(NyarType::Integer16 { signed: true }, i32s), Vec::new());
        assert_eq!(cast(NyarType::Integer8 { signed: true }, i64s), vec![ScalarCastStep::Opcode(WasmOpcode::I64ExtendI32S)]);
    }

    #[test]
    fn narrowing_masks_unsigned_and_extends_signed_explicitly() {
        assert_eq!(
            cast(NyarType::Integer32 { signed: false }, NyarType::Integer8 { signed: false }),
            vec![ScalarCastStep::I32Const(0xFF), ScalarCastStep::Opcode(WasmOpcode::I32And)]
        );
        assert_eq!(
            cast(NyarType::Integer32 { signed: true }, NyarType::Integer16 { signed: false }),
            vec![ScalarCastStep::I32Const(0xFFFF), ScalarCastStep::Opcode(WasmOpcode::I32And)]
        );
        assert_eq!(
            cast(NyarType::Integer32 { signed: true }, NyarType::Integer8 { signed: true }),
            vec![ScalarCastStep::Byte(WASM_I32_EXTEND8_S)]
        );
        assert_eq!(
            cast(NyarType::Integer64 { signed: false }, NyarType::Integer16 { signed: true }),
            vec![ScalarCastStep::Opcode(WasmOpcode::I32WrapI64), ScalarCastStep::Byte(WASM_I32_EXTEND16_S)]
        );
    }

    #[test]
    fn character_conversions_use_the_code_point_carrier() {
        assert_eq!(cast(NyarType::Character, NyarType::Integer32 { signed: false }), Vec::new());
        // 码点可以超出 16 位，收窄到 `u16` 必须显式掩码。
        assert_eq!(
            cast(NyarType::Character, NyarType::Integer16 { signed: false }),
            vec![ScalarCastStep::I32Const(0xFFFF), ScalarCastStep::Opcode(WasmOpcode::I32And)]
        );
        assert_eq!(cast(NyarType::Character, NyarType::Integer64 { signed: true }), vec![ScalarCastStep::Byte(WASM_I64_EXTEND_I32_U)]);
        assert_eq!(cast(NyarType::Integer16 { signed: false }, NyarType::Character), Vec::new());
        assert_eq!(cast(NyarType::Integer64 { signed: true }, NyarType::Character), vec![ScalarCastStep::Opcode(WasmOpcode::I32WrapI64)]);
    }

    #[test]
    fn unrepresentable_widths_fail_loudly() {
        assert!(scalar_cast_steps(&NyarType::Integer128 { signed: true }, &NyarType::Integer32 { signed: true }).is_err());
        assert!(scalar_cast_steps(&NyarType::Float64, &NyarType::Integer32 { signed: true }).is_err());
    }
}

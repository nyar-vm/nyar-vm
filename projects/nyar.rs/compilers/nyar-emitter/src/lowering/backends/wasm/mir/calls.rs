//! Resolved Semantic MIR calls -> Wasm call forms.
//!
//! Owns direct / import / intrinsic / witness / indirect call emission and the
//! call-shaped helpers currently on the same dispatch path. This is Wasm
//! **call encoding**, not a second MIR. Semantic MIR remains call-resolution
//! authority; this module only maps already-resolved calls to Wasm opcodes.

use super::{numeric, *};
use crate::{
    lowering::backends::wasm::cabi::CABI_HEAP_GLOBAL_INDEX,
    target_operator::{TargetOperandStep, TargetOperandType, TargetOperatorEncoding, TargetOperatorRecipe},
};

/// 计划冻结的目标标量载体 → wasm 栈类型字节。
fn wasm_stack_type(operand_type: TargetOperandType) -> u8 {
    match operand_type {
        TargetOperandType::I32 => VALTYPE_I32,
        TargetOperandType::I64 => VALTYPE_I64,
        TargetOperandType::F64 => VALTYPE_F64,
    }
}

impl<'a> WasmMirLowerer<'a> {
    /// Lowers a non-builtin MIR call to WASM bytecode.
    ///
    /// - Static dispatch emits `call` (0x10) with the resolved function index.
    /// - Witness dispatch emits `call_indirect` (0x11) using the witness operand as the table index.
    /// - `receiver_kind: Some(ByAddress)` means the first argument is a value-type receiver
    ///   whose linear-memory address is already an `i32` local in `value_locals`.
    ///   Because `emit_operand` emits `local.get` for value locals, the receiver's
    ///   linear-memory address is passed directly as the first WASM parameter -?no
    ///   special codegen is needed beyond emitting the argument normally.
    pub(super) fn emit_call_lowering(
        &mut self,
        callee: &MirOperand,
        arguments: &[MirOperand],
        dispatch: MirDispatchKind,
        witness: Option<&MirOperand>,
        receiver_kind: Option<ReceiverPassingKind>,
        output: Option<MirValueRef>,
    ) -> crate::Result<()> {
        if let Some(ReceiverPassingKind::ByAddress) = receiver_kind {
            // Confirm the receiver exists as the first argument; it is emitted below.
        }
        let callee_return;
        match dispatch {
            MirDispatchKind::Static => {
                // `[wasm_operator]` 绑定的 provider 直接在调用点编码为目标指令；
                // 不是 host import，也没有函数体，必须先于 import 解析处理。
                if self.try_emit_wasm_operator_call(callee, arguments, output)? {
                    return Ok(());
                }
                if let Some(import_index) = self.resolve_callee_import_index(callee, arguments) {
                    if self.try_emit_wasi_cli_write_via_stream(arguments, import_index, output)? {
                        return Ok(());
                    }
                    callee_return = self.resolve_callee_return_type(callee, Some(import_index))?;
                    let param_types = self.resolve_callee_param_types(callee, Some(import_index))?;
                    self.emit_host_import_call_arguments(arguments, &param_types)?;
                    self.emit_call(import_index);
                }
                else if let Some(function_index) = self.resolve_callee_function_index(callee) {
                    // 计划事实缺失属可执行合同不一致，禁止 panic（与 `8050500c` 同款结构化失败）。
                    let return_type = *self
                        .return_types_by_function_index
                        .get(&function_index)
                        .ok_or_else(|| format!("WASM 函数下标缺少返回签名: function_index={function_index} in `{}`", self.mir_fn.symbol))?;
                    let param_types = self
                        .param_types_by_function_index
                        .get(&function_index)
                        .ok_or_else(|| format!("WASM 函数下标缺少参数签名: function_index={function_index} in `{}`", self.mir_fn.symbol))?
                        .clone();
                    callee_return = return_type;
                    self.emit_call_arguments(arguments, &param_types)?;
                    self.emit_call(function_index);
                }
                else {
                    // 计划事实缺失属可执行合同不一致，禁止 panic（与 `8050500c` 同款结构化失败）。
                    return Err(format!("WASM 静态调用缺少已解析 callable identity: {} callee={callee:?}", self.mir_fn.symbol));
                }
            }
            MirDispatchKind::Witness => {
                callee_return = self.resolve_callee_return_type(callee, None)?;
                for argument in arguments {
                    self.emit_operand_coerced(argument, self.operand_wasm_stack_type(argument))?;
                }
                // 计划事实缺失属可执行合同不一致，禁止 panic（与 `8050500c` 同款结构化失败）。
                let Some(witness_operand) = witness
                else {
                    return Err(format!("WASM witness 调用缺少显式分派操作数: {}", self.mir_fn.symbol));
                };
                self.emit_operand(witness_operand)?;
                let type_index = self.resolve_callee_type_index(callee);
                self.emit_call_indirect(type_index, 0);
            }
            MirDispatchKind::Indirect => {
                // callee 是函数值 SSA（i32 table 槽载体）。签名事实只来自该 SSA 值
                // 冻结的 `NyarType::Function`：实参 coerce、`call_indirect` 的 typeidx
                // 与返回载体全部由它决定；缺事实即结构化失败，禁止回落静态 callee。
                let (param_types, return_type, type_index) = self.resolve_indirect_callee_signature(callee)?;
                self.emit_call_arguments(arguments, &param_types)?;
                self.emit_operand(callee)?;
                self.emit_call_indirect(type_index, 0);
                callee_return = return_type;
            }
        }
        self.emit_store_call_output(output, callee_return)?;
        Ok(())
    }

    fn emit_store_call_output(&mut self, output: Option<MirValueRef>, callee_return: Option<u8>) -> crate::Result<()> {
        match output {
            Some(output) => {
                // 计划事实缺失属可执行合同不一致，禁止 panic（与 `8050500c` 同款结构化失败）。
                let Some(return_type) = callee_return
                else {
                    return Err(format!("WASM 有 SSA 结果的调用缺少返回合同: {}", self.mir_fn.symbol));
                };
                self.validate_output_slot_type(output, return_type);
                self.assign_output_local(output);
            }
            None if callee_return.is_some() => WasmOpcode::Drop.encode(&mut self.code),
            None => {}
        }
        Ok(())
    }

    /// 宿主导入边界的实参发射：ABI 声明 i32 句柄而实参槽是 managed anyref 时，
    /// 必须消费目标计划冻结的 managed→host 句柄转换配方（合同 §5）；缺配方即
    /// 可执行合同不一致，结构化失败，禁止 coerce/占位掩盖 carrier 失配。
    /// 其余组合保持既有 coerce 语义。
    fn emit_host_import_call_arguments(&mut self, arguments: &[MirOperand], param_types: &[u8]) -> crate::Result<()> {
        assert_eq!(arguments.len(), param_types.len(), "WASM 调用实参与已解析签名不一致");
        for (argument, expected) in arguments.iter().zip(param_types) {
            if *expected == VALTYPE_I32 && self.operand_wasm_stack_type(argument) == WASM_GC_ANYREF {
                self.emit_managed_utf8_to_host_handle(argument)?;
            }
            else {
                self.emit_operand_coerced(argument, *expected)?;
            }
        }
        Ok(())
    }

    /// 按冻结配方把 managed 字符串 gc struct 转为宿主 i32 句柄：
    /// `struct.get` 取字节数组 → bump 分配线性内存 → 逐字节 `array.get` + `i32.store8`
    /// → `call env::host_intern_utf8(ptr, len)`。栈效果：`[anyref] → [i32]`。
    pub(super) fn emit_managed_utf8_to_host_handle(&mut self, operand: &MirOperand) -> crate::Result<()> {
        let MirOperand::Value(value) = operand
        else {
            return Err(format!("WASM 宿主字符串边界操作数缺少 SSA 身份: {}", self.mir_fn.symbol));
        };
        let Some(layout_id) = self.mir_fn.value_layouts.get(value).copied()
        else {
            return Err(format!("WASM 宿主字符串边界 %{} 缺少 layout 身份: {}", value.0, self.mir_fn.symbol));
        };
        let Some(recipe) = self.managed_utf8_to_host_handle.get(&layout_id)
        else {
            return Err(format!(
                "WASM 宿主字符串边界 %{} (layout={layout_id:?}) 缺少目标计划冻结的 managed→host 句柄转换配方: {}",
                value.0, self.mir_fn.symbol
            ));
        };
        // managed struct → 字节数组引用
        self.emit_operand(operand)?;
        self.emit_ref_cast_struct(recipe.struct_type_index);
        self.emit_struct_get(recipe.struct_type_index, recipe.bytes_field_slot);
        self.emit_ref_cast_array_type(recipe.bytes_array_type_index);
        let array_local = self.alloc_anyref_local();
        self.emit_local_set(array_local);
        // 字节长度
        self.emit_local_get(array_local);
        self.emit_ref_cast_array_type(recipe.bytes_array_type_index);
        self.emit_array_len();
        let len_local = self.alloc_i32_local();
        self.emit_local_set(len_local);
        // 动态 bump 分配（global 0 堆指针），增长逻辑与 `bump_allocate` 同构；
        // scratch local 在本次转换内闭合，不与其它发射交叠。
        encode_global_get(CABI_HEAP_GLOBAL_INDEX, &mut self.code);
        self.emit_local_set(self.stack_ptr_local);
        self.emit_local_get(self.stack_ptr_local);
        self.emit_local_get(len_local);
        self.emit_i32_add();
        self.emit_local_set(self.bump_end_local);
        self.emit_local_get(self.bump_end_local);
        self.emit_i32_const(65535);
        self.emit_i32_add();
        self.emit_i32_const(16);
        WasmOpcode::I32ShrU.encode(&mut self.code);
        encode_memory_size(&mut self.code);
        WasmOpcode::I32Sub.encode(&mut self.code);
        self.emit_local_tee(self.bump_pages_local);
        self.emit_i32_const(0);
        WasmOpcode::I32GtS.encode(&mut self.code);
        WasmOpcode::If.encode(&mut self.code);
        self.code.push(BLOCKTYPE_EMPTY);
        self.emit_local_get(self.bump_pages_local);
        encode_memory_grow(&mut self.code);
        self.emit_i32_const(-1);
        WasmOpcode::I32Eq.encode(&mut self.code);
        WasmOpcode::If.encode(&mut self.code);
        self.code.push(BLOCKTYPE_EMPTY);
        encode_unreachable(&mut self.code);
        WasmOpcode::End.encode(&mut self.code);
        WasmOpcode::End.encode(&mut self.code);
        // commit heap cursor：global.set 必须先收到 bump_end 新值（与 `bump_allocate` 一致），
        // 否则 V8 编译期即拒绝（not enough arguments on the stack for global.set）。
        self.emit_local_get(self.bump_end_local);
        encode_global_set(CABI_HEAP_GLOBAL_INDEX, &mut self.code);
        // 逐字节拷贝：array.get(u8→i32) → i32.store8
        let index_local = self.alloc_i32_local();
        self.emit_i32_const(0);
        self.emit_local_set(index_local);
        encode_block_empty(&mut self.code);
        encode_loop_empty(&mut self.code);
        self.emit_local_get(index_local);
        self.emit_local_get(len_local);
        WasmOpcode::I32GeU.encode(&mut self.code);
        encode_br_if(1, &mut self.code);
        self.emit_local_get(self.stack_ptr_local);
        self.emit_local_get(index_local);
        self.emit_i32_add();
        self.emit_local_get(array_local);
        self.emit_ref_cast_array_type(recipe.bytes_array_type_index);
        self.emit_local_get(index_local);
        self.emit_array_get(recipe.bytes_array_type_index);
        encode_memarg(WasmOpcode::I32Store8, 0, 0, &mut self.code);
        self.emit_local_get(index_local);
        self.emit_i32_const(1);
        self.emit_i32_add();
        self.emit_local_set(index_local);
        encode_br(0, &mut self.code);
        WasmOpcode::End.encode(&mut self.code);
        WasmOpcode::End.encode(&mut self.code);
        // 宿主登记：(ptr, len) → i32 handle
        self.emit_local_get(self.stack_ptr_local);
        self.emit_local_get(len_local);
        self.emit_call(recipe.intern_import_index);
        Ok(())
    }

    /// 编码阶段只验证预规划槽，不改变 SSA 表示。
    pub(super) fn validate_output_slot_type(&self, output: MirValueRef, stack_ty: u8) {
        let local = self.planned_value_local(output);
        assert_eq!(self.wasm_local_value_type(local), stack_ty, "WASM 编码结果与预规划槽冲突: %{} in {}", output.0, self.mir_fn.symbol);
    }

    pub(super) fn assign_output_local(&mut self, output: MirValueRef) {
        let local = self.planned_value_local(output);
        self.emit_local_set(local);
    }

    pub(super) fn resolve_callee_param_types(&self, callee: &MirOperand, import_index: Option<u32>) -> crate::Result<Vec<u8>> {
        // 计划事实缺失属可执行合同不一致，禁止 panic（与 `8050500c` 同款结构化失败）。
        if let Some(index) = import_index {
            return self
                .import_param_types
                .get(index as usize)
                .ok_or_else(|| format!("WASM 导入缺少参数签名: import_index={index} in `{}`", self.mir_fn.symbol))
                .cloned();
        }
        let MirOperand::Item(instance) = callee
        else {
            return Err(format!("WASM 普通调用缺少实例身份: {} callee={callee:?}", self.mir_fn.symbol));
        };
        self.param_types_by_instance
            .get(instance)
            .ok_or_else(|| format!("WASM 实例缺少参数签名: callee={callee:?} in `{}`", self.mir_fn.symbol))
            .cloned()
    }

    pub(super) fn resolve_callee_return_type(&self, callee: &MirOperand, import_index: Option<u32>) -> crate::Result<Option<u8>> {
        // 计划事实缺失属可执行合同不一致，禁止 panic（与 `8050500c` 同款结构化失败）。
        if let Some(index) = import_index {
            return self
                .import_return_types
                .get(index as usize)
                .ok_or_else(|| format!("WASM 导入缺少返回签名: import_index={index} in `{}`", self.mir_fn.symbol))
                .copied();
        }
        let MirOperand::Item(instance) = callee
        else {
            return Err(format!("WASM 普通调用缺少实例身份: {} callee={callee:?}", self.mir_fn.symbol));
        };
        self.return_types_by_instance
            .get(instance)
            .ok_or_else(|| format!("WASM 实例缺少返回签名: callee={callee:?} in `{}`", self.mir_fn.symbol))
            .copied()
    }

    /// 间接调用的签名解析：callee SSA 值必须携带冻结的 `NyarType::Function`，
    /// 其物理签名必须命中模块已注册的 functype（按签名字节反查 typeidx）。
    /// 缺类型或缺注册条目都结构化失败，禁止按参数个数或候选顺序猜签名。
    pub(super) fn resolve_indirect_callee_signature(&self, callee: &MirOperand) -> crate::Result<(Vec<u8>, Option<u8>, u32)> {
        let MirOperand::Value(value) = callee
        else {
            return Err(format!("WASM 间接调用缺少函数值操作数: {}", self.mir_fn.symbol));
        };
        let Some(NyarType::Function(signature)) = self.mir_fn.value_types.get(value)
        else {
            return Err(format!("WASM 间接调用 callee %{} 缺少冻结 Function 签名: {}", value.0, self.mir_fn.symbol));
        };
        let param_types: Vec<u8> = signature
            .params
            .iter()
            .map(|ty| wasm_param_value_type_for(self.ctx, ty, None, self.gc_struct_type_indices, self.js_glue_utf8_as_anyref))
            .collect();
        let return_type = match &signature.return_type {
            NyarType::Unit | NyarType::Bottom => None,
            other => Some(wasm_param_value_type_for(self.ctx, other, None, self.gc_struct_type_indices, self.js_glue_utf8_as_anyref)),
        };
        let type_index = *self.type_index_by_signature.get(&(param_types.clone(), return_type)).ok_or_else(|| {
            format!("WASM 间接调用签名缺少已注册 functype（params={param_types:?} return={return_type:?}）: {}", self.mir_fn.symbol)
        })?;
        Ok((param_types, return_type, type_index))
    }

    pub(super) fn operand_wasm_stack_type(&self, operand: &MirOperand) -> u8 {
        match operand {
            MirOperand::Value(value) => self.wasm_local_value_type(self.planned_value_local(*value)),
            MirOperand::Constant(constant) => match constant {
                MirConstant::Utf8(_) => VALTYPE_I32,
                MirConstant::Utf16(_) => {
                    panic!("WASM lowering requires an explicit UTF-16 ABI contract")
                }
                MirConstant::Unit => WASM_GC_ANYREF,
                // 可空空值是空引用，与 nullable 槽的 anyref 载体一致。
                MirConstant::Null => WASM_GC_ANYREF,
                MirConstant::Float64(_) => VALTYPE_F64,
                MirConstant::Int(_) | MirConstant::Bool(_) => VALTYPE_I32,
            },
            MirOperand::Symbol(_) => panic!("WASM 值操作数缺少 SSA 身份"),
            MirOperand::Item(_) => {
                panic!("WASM callable identity cannot be used as a value operand")
            }
        }
    }

    /// 操作数在计划中冻结的语义类型；显式 `as` 的转换语义必须由此决定，
    /// 不得退回到只携带位宽的栈类型字节。
    pub(super) fn operand_semantic_type(&self, operand: &MirOperand) -> Option<&NyarType> {
        match operand {
            MirOperand::Value(value) => self.mir_fn.value_types.get(value),
            _ => None,
        }
    }

    /// 按目标合同发射操作数。
    ///
    /// 常量不携带位宽（`MirConstant::Int` 只有 `i64` 载荷），因此常量一律交给
    /// 目标槽 valtype 决定 `i32.const` / `i64.const` / `f64.const`。任何宽度转换
    /// 只对已规划 SSA 槽生效，避免把 i64 字面量先截断成 i32 再符号扩展。
    pub(super) fn emit_operand_coerced(&mut self, operand: &MirOperand, expected: u8) -> crate::Result<()> {
        if let MirOperand::Constant(constant) = operand {
            return self.emit_load_constant_for_slot(constant, expected);
        }
        let actual = self.operand_wasm_stack_type(operand);
        if actual == expected {
            return self.emit_operand(operand);
        }
        // Reference-local metadata is authoritative for GC aggregates：槽位已是引用，
        // 即便表面分类成 i32 也不得用 ref.null 顶替。
        if expected == WASM_GC_ANYREF && self.operand_reference_local(operand).is_some() {
            return self.emit_operand(operand);
        }
        // 仅允许 TargetExecutionPlan 已承诺的显式宽度转换；禁止用 0 / null 掩盖 carrier 失配。
        if expected == VALTYPE_I64 && (actual == WASM_GC_ANYREF || actual == WASM_GC_EXTERNREF) {
            self.emit_operand(operand)?;
            self.emit_unbox_i64_payload();
            return Ok(());
        }
        if expected == VALTYPE_F64 && (actual == WASM_GC_ANYREF || actual == WASM_GC_EXTERNREF) {
            self.emit_operand(operand)?;
            self.emit_unbox_f64_payload();
            return Ok(());
        }
        // 宽度不同的整数载体只有语义类型能决定扩展/截断方向：i32→i64 需要
        // 有符号/无符号选择，i64→i32 需要显式收窄。这里没有语义类型，任何
        // `extend` / `wrap` 都是靠载体猜，一律结构化失败。
        if (expected == VALTYPE_I64 && actual == VALTYPE_I32) || (expected == VALTYPE_I32 && actual == VALTYPE_I64) {
            return Err(format!(
                "WASM ABI 宽度失配 in `{}`: slot={} actual={} operand={} origin={}; \
                 integer widen/narrow requires an explicit `Cast` driven by both semantic types — refusing to guess sign or truncate",
                self.mir_fn.symbol,
                self.abi_valtype_label(expected),
                self.abi_valtype_label(actual),
                self.describe_operand(operand),
                self.instruction_origin()
            ));
        }
        if expected == WASM_GC_ANYREF && actual == VALTYPE_I64 {
            return self.emit_box_i64_payload(operand);
        }
        if expected == WASM_GC_ANYREF && actual == VALTYPE_F64 {
            return self.emit_box_f64_payload(operand);
        }
        if expected == WASM_GC_EXTERNREF && actual == WASM_GC_ANYREF {
            return self.emit_operand(operand);
        }
        Err(format!(
            "WASM ABI carrier mismatch in `{}`: slot={} actual={} operand={} origin={}; \
             TargetExecutionPlan must provide an explicit conversion — refusing null/0 placeholders",
            self.mir_fn.symbol,
            self.abi_valtype_label(expected),
            self.abi_valtype_label(actual),
            self.describe_operand(operand),
            self.instruction_origin()
        ))
    }

    /// `[wasm_operator("...")]` 绑定：把契约调用直接编码为目标数值指令。
    ///
    /// 只有 callee 携带已绑定 host contract 的实例身份时才命中；未绑定 provider
    /// 的 host contract 与普通函数调用都返回 `false`，走既有 import / local call
    /// 路径。arity、操作数配方、结果载体与 opcode 全部来自计划冻结的
    /// `TargetOperatorPlan`；未冻结的绑定必须返回结构化错误，禁止从助记符猜语义。
    pub(super) fn try_emit_wasm_operator_call(
        &mut self,
        callee: &MirOperand,
        arguments: &[MirOperand],
        output: Option<MirValueRef>,
    ) -> crate::Result<bool> {
        let MirOperand::Item(instance) = callee
        else {
            return Ok(false);
        };
        let Some((mnemonic, binding)) = self.callee_wasm_operator.get(instance)
        else {
            return Ok(false);
        };
        let TargetOperatorBinding::Frozen(plan) = binding
        else {
            if let TargetOperatorBinding::FrozenWasmGcArray(array_plan) = binding {
                return self.try_emit_wasm_gc_array_operator(mnemonic, *array_plan, arguments, output, instance).map(|()| true);
            }
            return Err(format!("WASM `wasm_operator` 绑定缺少可编码助记符 `{mnemonic}`: callee={instance:?} function={}", self.mir_fn.symbol));
        };
        let TargetOperatorEncoding::Wasm(opcode) = plan.encoding
        else {
            return Err(format!(
                "WASM `wasm_operator` 绑定 `{mnemonic}` 冻结了非 wasm 编码: callee={instance:?} function={}",
                self.mir_fn.symbol
            ));
        };
        if arguments.len() != plan.arity {
            return Err(format!(
                "WASM `wasm_operator` 绑定 `{mnemonic}` 实参数量与冻结 arity 不符: callee={instance:?} function={} expected={} actual={}",
                self.mir_fn.symbol,
                plan.arity,
                arguments.len()
            ));
        }
        let operand_type = wasm_stack_type(plan.operand_type);
        match plan.recipe {
            TargetOperatorRecipe::Operands => self.emit_wasm_operator_operands(mnemonic, operand_type, arguments)?,
            TargetOperatorRecipe::Steps(steps) => self.emit_wasm_operator_recipe(mnemonic, operand_type, steps, arguments)?,
        }
        numeric::emit_opcode(opcode, &mut self.code);
        let result_type = wasm_stack_type(plan.result_type);
        match output {
            None => WasmOpcode::Drop.encode(&mut self.code),
            Some(output) => {
                self.validate_output_slot_type(output, result_type);
                self.assign_output_local(output);
            }
        }
        Ok(true)
    }

    /// `[wasm_operator("array.*")]` 冻结绑定：分派到与 MIR `Array*` 臂共享的发射器。
    ///
    /// arity 来自计划冻结事实；arraytype 索引由共享发射器在编码期按元素类型从
    /// RepresentationPlan 注册表解析，与 MIR 臂同一事实源，此处不重新推断。
    fn try_emit_wasm_gc_array_operator(
        &mut self,
        mnemonic: &str,
        plan: WasmGcArrayOperatorPlan,
        arguments: &[MirOperand],
        output: Option<MirValueRef>,
        instance: &nyar_types::ItemInstanceId,
    ) -> crate::Result<()> {
        if arguments.len() != plan.arity {
            return Err(format!(
                "WASM `wasm_operator` 数组绑定 `{mnemonic}` 实参数量与冻结 arity 不符: callee={instance:?} function={} expected={} actual={}",
                self.mir_fn.symbol,
                plan.arity,
                arguments.len()
            ));
        }
        match plan.operation {
            WasmGcArrayOperation::New => {
                let Some(output) = output
                else {
                    return Err(format!(
                        "WASM `wasm_operator` 数组绑定 `{mnemonic}` 缺少结果槽: callee={instance:?} function={}",
                        self.mir_fn.symbol
                    ));
                };
                let Some(element_type) = self.mir_fn.value_types.get(&output).and_then(|ty| array_element_type(ty).cloned())
                else {
                    return Err(format!(
                        "WASM `wasm_operator` 数组绑定 `{mnemonic}` 的结果槽缺少堆数组类型事实: callee={instance:?} function={} output=%{}",
                        self.mir_fn.symbol, output.0
                    ));
                };
                self.emit_array_new_default_ops(&element_type, &arguments[0], output)
            }
            WasmGcArrayOperation::Length => self.emit_array_length_ops(&arguments[0], output),
            WasmGcArrayOperation::Get => self.emit_array_get_ops(arguments, output),
            WasmGcArrayOperation::Set => self.emit_array_set_ops(arguments, output),
        }
    }

    /// 按计划冻结的步骤顺序机械编码常量与实参，再执行冻结 opcode。
    ///
    /// `i32.neg` 的 `0 - x` 配方已在此前的计划构造期展开为常量步骤 + 实参步骤；
    /// backend 不再按助记符判断取负，也不自行决定零常量。
    fn emit_wasm_operator_recipe(
        &mut self,
        mnemonic: &str,
        expected: u8,
        steps: &'static [TargetOperandStep],
        arguments: &[MirOperand],
    ) -> crate::Result<()> {
        for step in steps {
            match step {
                TargetOperandStep::Arguments => self.emit_wasm_operator_operands(mnemonic, expected, arguments)?,
                TargetOperandStep::NarrowTopI64ToI32 => WasmOpcode::I32WrapI64.encode(&mut self.code),
                TargetOperandStep::IntegerConstant { operand_type, value } => {
                    let carrier = wasm_stack_type(*operand_type);
                    if carrier != expected {
                        return Err(format!(
                            "WASM `wasm_operator` 绑定 `{mnemonic}` 的常量配方载体与冻结操作数载体不符: function={} expected={expected:#x} actual={carrier:#x}",
                            self.mir_fn.symbol
                        ));
                    }
                    match operand_type {
                        TargetOperandType::I32 => self.emit_i32_const(*value),
                        TargetOperandType::I64 => self.emit_i64_const(i64::from(*value)),
                        TargetOperandType::F64 => {
                            return Err(format!(
                                "WASM `wasm_operator` 绑定 `{mnemonic}` 冻结了浮点常量配方，但配方只支持整数常量: function={}",
                                self.mir_fn.symbol
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 按目标绑定载体发射实参。SSA 槽的栈类型必须与计划冻结的实参载体一致；
    /// 常量没有独立位宽，交给目标载体在 `emit_load_constant_for_slot` 里选指令。
    fn emit_wasm_operator_operands(&mut self, mnemonic: &str, expected: u8, arguments: &[MirOperand]) -> crate::Result<()> {
        for argument in arguments {
            if let MirOperand::Value(_) = argument {
                let actual = self.operand_wasm_stack_type(argument);
                assert_eq!(
                    actual, expected,
                    "WASM `wasm_operator` 实参栈类型与目标绑定不一致: `{mnemonic}` expects {expected:#x} actual {actual:#x} in {}",
                    self.mir_fn.symbol
                );
            }
            self.emit_contract_operand(argument, expected)?;
        }
        Ok(())
    }

    pub(super) fn resolve_callee_import_index(&self, callee: &MirOperand, _arguments: &[MirOperand]) -> Option<u32> {
        let MirOperand::Item(instance) = callee
        else {
            return None;
        };
        self.callee_import_index.get(instance).copied()
    }

    /// `i64` → wasm-gc struct `[i64]`（anyref），供泛型 `T` 数组槽使用。
    fn emit_box_i64_payload(&mut self, operand: &MirOperand) -> crate::Result<()> {
        let box_ty = self.gc_i64_box_type_index;
        self.emit_struct_new_default(box_ty);
        let tmp = self.alloc_anyref_local();
        self.emit_local_set(tmp);
        self.emit_local_get(tmp);
        self.emit_ref_cast_struct(box_ty);
        self.emit_contract_operand(operand, VALTYPE_I64)?;
        self.emit_struct_set(box_ty, 0);
        self.emit_local_get(tmp);
        Ok(())
    }

    /// 栈顶 anyref（i64 box）→ i64。
    ///
    /// 载荷是 null 时不得回落 `i64 0`：缺值不是有效值。取消 null 分支后，
    /// `ref.cast (ref $i64_box)` 在 null 上执行即 trap，
    /// 由上游 boxing / `ArrayNew` default 合同保证载荷已初始化。
    pub(super) fn emit_unbox_i64_payload(&mut self) {
        let box_ty = self.gc_i64_box_type_index;
        self.emit_ref_cast_struct(box_ty);
        self.emit_struct_get(box_ty, 0);
    }

    /// `f64` → wasm-gc struct `[f64]`（anyref），供 `Option<F64>` 的 niche 载荷槽使用。
    fn emit_box_f64_payload(&mut self, operand: &MirOperand) -> crate::Result<()> {
        let box_ty = self.gc_f64_box_type_index;
        self.emit_struct_new_default(box_ty);
        let tmp = self.alloc_anyref_local();
        self.emit_local_set(tmp);
        self.emit_local_get(tmp);
        self.emit_ref_cast_struct(box_ty);
        self.emit_contract_operand(operand, VALTYPE_F64)?;
        self.emit_struct_set(box_ty, 0);
        self.emit_local_get(tmp);
        Ok(())
    }

    /// 栈顶 anyref（f64 box）→ f64；null 不为 0.0，语义与 i64 拆箱一致。
    pub(super) fn emit_unbox_f64_payload(&mut self) {
        let box_ty = self.gc_f64_box_type_index;
        self.emit_ref_cast_struct(box_ty);
        self.emit_struct_get(box_ty, 0);
    }

    /// p3 `write-via-stream`：utf8 线性句?→?`stream.new` / write / drop →?宿主?
    ///
    /// 顺序必须是：new →??reader 交给 write-via-stream →?write(writer) →?drop-writable →?drop future?
    /// ?write 再交 reader 会在 sync `stream.write` 上永久挂起?
    fn try_emit_wasi_cli_write_via_stream(&mut self, arguments: &[MirOperand], import_index: u32, output: Option<MirValueRef>) -> crate::Result<bool> {
        if !self.wasi_mode || arguments.len() != 1 {
            return Ok(false);
        }
        let Some((module, field)) = self.host_imports.get(import_index as usize)
        else {
            return Ok(false);
        };
        if field != "write-via-stream" {
            return Ok(false);
        }
        if !(module.contains("stdout") || module.contains("stderr")) {
            return Ok(false);
        }
        let Some(stream_new) = self.find_host_import_index(module, "[stream-new-0]write-via-stream")
        else {
            return Ok(false);
        };
        let Some(stream_write) = self.find_host_import_index(module, "[stream-write-0]write-via-stream")
        else {
            return Ok(false);
        };
        let Some(stream_drop_w) = self.find_host_import_index(module, "[stream-drop-writable-0]write-via-stream")
        else {
            return Ok(false);
        };
        let Some(future_drop) = self.find_host_import_index(module, "[future-drop-readable-1]write-via-stream")
        else {
            return Ok(false);
        };
        // utf8 handle: [len:u32 LE][bytes…]
        let handle = self.alloc_i32_local();
        let len = self.alloc_i32_local();
        let bytes_ptr = self.alloc_i32_local();
        let pair = self.alloc_i64_local();
        let writer = self.alloc_i32_local();
        let reader = self.alloc_i32_local();
        let fut = self.alloc_i32_local();

        self.emit_operand_coerced(&arguments[0], VALTYPE_I32)?;
        self.emit_local_set(handle);

        self.emit_local_get(handle);
        encode_i32_load(2, 0, &mut self.code);
        self.emit_local_set(len);

        self.emit_local_get(handle);
        self.emit_i32_const(4);
        self.emit_i32_add();
        self.emit_local_set(bytes_ptr);

        self.emit_call(stream_new);
        self.emit_local_set(pair);

        // reader = low32, writer = high32（与 wit-bindgen raw_stream_new 同构?
        self.emit_local_get(pair);
        WasmOpcode::I32WrapI64.encode(&mut self.code);
        self.emit_local_set(reader);
        self.emit_local_get(pair);
        WasmOpcode::I64Const.encode(&mut self.code);
        encode_sleb128_i64(32, &mut self.code);
        WasmOpcode::I64ShrU.encode(&mut self.code);
        WasmOpcode::I32WrapI64.encode(&mut self.code);
        self.emit_local_set(writer);

        self.emit_local_get(reader);
        self.emit_call(import_index);
        self.emit_local_set(fut);

        self.emit_local_get(writer);
        self.emit_local_get(bytes_ptr);
        self.emit_local_get(len);
        self.emit_call(stream_write);
        WasmOpcode::Drop.encode(&mut self.code);

        self.emit_local_get(writer);
        self.emit_call(stream_drop_w);

        self.emit_local_get(fut);
        self.emit_call(future_drop);

        if let Some(out) = output {
            // console write →?unit；void≠unit，用 anyref null 占位?
            self.validate_output_slot_type(out, VALTYPE_ANYREF);
            self.emit_ref_null_anyref();
            self.assign_output_local(out);
        }
        Ok(true)
    }

    fn find_host_import_index(&self, module: &str, field: &str) -> Option<u32> {
        self.host_imports.iter().position(|(m, f)| m == module && f == field).map(|index| index as u32)
    }
}

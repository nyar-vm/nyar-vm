//! OVM bytecode lowering from semantic MIR.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    backend_plan_views::{
        ExecutableBlock as MirBlock, ExecutableBlockRef as MirBlockRef, ExecutableConstant as MirConstant, ExecutableFunction as MirFunction,
        ExecutableInstruction as MirInstruction, ExecutableInstructionKind as MirInstructionKind, ExecutableOperand as MirOperand,
        ExecutableTerminator as MirTerminator, ExecutableValueRef as MirValueRef, NyarType,
    },
    contracts::{EffectKind, ValueOrigin},
};
use nyar::{ExternalImportLink, NamePath};
use nyar_types::{AggregateLayout, ItemInstanceId, LayoutId, NominalInstanceId, VariantId};
use odin_bytecode::{
    IntrinsicOpcode, Odin_VERSION, OdinConstant, OdinExport, OdinExportKind, OdinFunction, OdinHeadCode, OdinImport, OdinImportKind,
    OdinLayout, OdinModuleData,
};

use super::executable::{ExecutableLoweringContext, block_label, collect_reachable_blocks, slots::ExecutableSlotPlan};
use crate::{
    FragmentSubmission, Result, TargetExecutionPlan,
    target_operator::{
        TargetOperandStep, TargetOperandType, TargetOperatorBinding, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe,
    },
};

/// 宿主 builtin 导入模块名（链接名在 imports section；热路径只用下标）。
const HOST_IMPORT_MODULE: &str = "nyar.host";

/// 整数取负没有原生头码：计划显式冻结 `0 - x` 的发射配方。
const I32_NEGATE_STEPS: &[TargetOperandStep] =
    &[TargetOperandStep::IntegerConstant { operand_type: TargetOperandType::I32, value: 0 }, TargetOperandStep::Arguments];

/// `[ovm_operator("...")]` 目标助记符到**冻结配方**的唯一映射表。
///
/// 只覆盖 `NyarHeadCode` 已有的 i32 算术/位运算/比较变体；i64、浮点没有对应
/// 单指令，必须保持宿主导入路径（由 runtime `parse_host_op` 决定是否存在）。
/// 映射在 `TargetExecutionPlan` 构造时写入计划；未知助记符返回 `None`，由计划
/// 冻结为 `TargetOperatorBinding::Unfrozen`，emit 结构化失败，禁止回落到宿主导入
/// 或默认指令。
pub(crate) fn freeze_ovm_operator_binding(mnemonic: &str) -> Option<TargetOperatorPlan> {
    let (arity, recipe, code) = match mnemonic {
        "i32.add" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32Add),
        "i32.sub" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32Sub),
        "i32.mul" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32Mul),
        "i32.div" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32DivS),
        "i32.rem" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32RemS),
        "i32.and" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32And),
        "i32.or" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32Or),
        "i32.xor" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32Xor),
        "i32.shl" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32Shl),
        "i32.shr_s" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32ShrS),
        "i32.shr_u" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32ShrU),
        "i32.eq" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32Eq),
        "i32.ne" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32Ne),
        "i32.lt" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32LtS),
        "i32.le" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32LeS),
        "i32.gt" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32GtS),
        "i32.ge" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32GeS),
        "i32.lt_u" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32LtU),
        "i32.le_u" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32LeU),
        "i32.gt_u" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32GtU),
        "i32.ge_u" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::I32GeU),
        "i32.neg" => (1, TargetOperatorRecipe::Steps(I32_NEGATE_STEPS), OdinHeadCode::I32Sub),
        _ => return None,
    };
    Some(TargetOperatorPlan {
        arity,
        operand_type: TargetOperandType::I32,
        result_type: TargetOperandType::I32,
        recipe,
        encoding: TargetOperatorEncoding::Ovm(code),
    })
}

struct BytecodeEmitter<'a> {
    constants: Vec<OdinConstant>,
    code_bytes: Vec<u8>,
    pending_jumps: Vec<(usize, MirBlockRef)>,
    block_starts: BTreeMap<MirBlockRef, usize>,
    constants_base: i32,
    imports: &'a mut Vec<OdinImport>,
}

impl<'a> BytecodeEmitter<'a> {
    fn new(constants_base: i32, imports: &'a mut Vec<OdinImport>) -> Self {
        Self {
            constants: Vec::new(),
            code_bytes: Vec::new(),
            pending_jumps: Vec::new(),
            block_starts: BTreeMap::new(),
            constants_base,
            imports,
        }
    }

    fn intern_string(&mut self, value: &str) -> i32 {
        let index = self.constants.len() as i32;
        self.constants.push(OdinConstant::String(value.to_string()));
        index + self.constants_base
    }

    /// 登记一个 i32 常量并返回模块常量池下标（复用 `constants_base`）。
    fn intern_i32(&mut self, value: i32) -> i32 {
        let index = self.constants.len() as i32;
        self.constants.push(OdinConstant::Integer32(value));
        index + self.constants_base
    }

    fn emit_plain(&mut self, opcode: OdinHeadCode) {
        odin_bytecode::emit_plain(&mut self.code_bytes, opcode);
    }

    fn emit_imm1(&mut self, opcode: OdinHeadCode, operand: i32) {
        odin_bytecode::emit_imm1(&mut self.code_bytes, opcode, operand);
    }

    fn ensure_host_import(&mut self, symbol: &str) -> i32 {
        if let Some((index, _)) =
            self.imports.iter().enumerate().find(|(_, import)| import.module_name == HOST_IMPORT_MODULE && import.symbol_name == symbol)
        {
            return index as i32;
        }
        let index = self.imports.len() as i32;
        self.imports.push(OdinImport {
            kind: OdinImportKind::Function,
            module_name: HOST_IMPORT_MODULE.to_string(),
            symbol_name: symbol.to_string(),
        });
        index
    }

    /// 按 imports 表下标调用宿主能力；operand1 = import index，operand2 = argc。
    fn emit_call_import(&mut self, symbol: &str, arg_count: i32) {
        let import_index = self.ensure_host_import(symbol);
        odin_bytecode::emit_imm2(&mut self.code_bytes, OdinHeadCode::CallImport, import_index, arg_count);
    }

    /// 按 [`IntrinsicOpcode`] 编码调用内置；operand1 = 指令编号，operand2 = argc。
    fn emit_call_intrinsic(&mut self, op: IntrinsicOpcode, arg_count: i32) {
        odin_bytecode::emit_imm2(&mut self.code_bytes, OdinHeadCode::CallIntrinsic, op.raw(), arg_count);
    }

    fn emit_jump_placeholder(&mut self, opcode: OdinHeadCode, target: MirBlockRef) -> usize {
        let position = self.code_bytes.len();
        self.emit_imm1(opcode, 0);
        self.pending_jumps.push((position, target));
        position
    }

    fn patch_jump_to_block(&mut self, jump_position: usize, target: MirBlockRef) {
        let target_pos = *self.block_starts.get(&target).expect("block start");
        let offset = (target_pos as i32) - (jump_position as i32);
        self.code_bytes[jump_position + 1..jump_position + 5].copy_from_slice(&offset.to_le_bytes());
    }

    fn patch_pending_jumps(&mut self) {
        let pending = std::mem::take(&mut self.pending_jumps);
        for (position, target) in pending {
            self.patch_jump_to_block(position, target);
        }
    }
}

/// Lower MIR-backed fragment operations into a `.nyar` module.
pub(crate) fn lower_fragment_mir_to_nyar_module(submission: &FragmentSubmission) -> Result<OdinModuleData> {
    let mut module = OdinModuleData {
        version: Odin_VERSION,
        name: format!(
            "{}__{}",
            super::sanitize_symbol(submission.backend_plan.module_name()),
            super::sanitize_symbol(submission.backend_plan.fragment_id().as_str())
        ),
        constants: Vec::new(),
        functions: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
        witness_entries: Vec::new(),
        code_bytes: Vec::new(),
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };

    let mut layout_index_by_id = BTreeMap::<LayoutId, i32>::new();
    // 定长数组的 `layout_id` 不保证进入 `AggregateLayoutPlan`（合成布局由前端按需登记），
    // OVM 的 Odin 槽位又没有类型，因此按字段数铸造后端私有布局，绝不以未冻结身份开新布局。
    let mut layout_index_by_field_count = BTreeMap::<i32, i32>::new();
    let sum_layout_index_by_nominal = register_sum_layouts(submission, &mut module.layouts)?;
    let external_link_by_instance = submission
        .backend_plan
        .imports()
        .iter()
        .map(|(instance, import)| (*instance, import.link.clone()))
        .collect::<BTreeMap<ItemInstanceId, ExternalImportLink>>();
    // 外部调用的实参位宽与内部调用同源：来自计划冻结的签名（`BackendImport.parameter_types`）。
    // 消费点缺失签名时必须结构化失败，不得回落到 semantic MIR 的 `Integer64` 默认。
    let external_param_types_by_instance = submission
        .backend_plan
        .imports()
        .iter()
        .map(|(instance, import)| (*instance, import.parameter_types.clone()))
        .collect::<BTreeMap<ItemInstanceId, Vec<NyarType>>>();
    // `[ovm_operator]` 绑定的冻结配方只从计划读取：助记符仅用于诊断，arity、操作数
    // 改写与头码全部来自 `TargetExecutionPlan`。
    let external_opcode_by_instance = submission
        .backend_plan
        .imports()
        .iter()
        .filter_map(|(instance, import)| {
            let mnemonic = crate::lowering::shared::interop::ovm_operator_target(&import.link)?;
            Some((*instance, (mnemonic.to_string(), import.target_operator.unwrap_or(TargetOperatorBinding::Unfrozen))))
        })
        .collect::<BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)>>();

    let export_names = nyar_export_name_map(submission);
    let mut function_index_by_instance = BTreeMap::<ItemInstanceId, i32>::new();
    {
        let exec = &submission.backend_plan;
        let operations: Vec<ItemInstanceId> = exec.instances().into_iter().filter(|instance| exec.get_function(instance).is_some()).collect();
        for (index, instance) in operations.iter().enumerate() {
            function_index_by_instance.insert(*instance, index as i32);
        }
        let function_entry_arities = build_nyar_function_entry_arities(exec.as_ref(), &operations);
        let param_types_by_function_index = build_nyar_function_param_types(exec.as_ref(), &operations)?;

        for instance in operations {
            let Some(view) = exec.get_function(&instance)
            else {
                continue;
            };
            let mir_fn = &view.function;
            let export_name = export_names.get(&instance).cloned().unwrap_or_else(|| nyar_mir_export_name(submission, instance));
            let code_offset = module.code_bytes.len() as i32;
            let constants_base = module.constants.len() as i32;
            let mut emitter = BytecodeEmitter::new(constants_base, &mut module.imports);

            lower_mir_function_to_bytecode(
                submission,
                mir_fn,
                &function_index_by_instance,
                &function_entry_arities,
                &param_types_by_function_index,
                &external_link_by_instance,
                &external_param_types_by_instance,
                &external_opcode_by_instance,
                &sum_layout_index_by_nominal,
                &mut emitter,
                &mut module.layouts,
                &mut layout_index_by_id,
                &mut layout_index_by_field_count,
            )?;

            module.constants.extend(emitter.constants);
            module.code_bytes.extend_from_slice(&emitter.code_bytes);

            // 与 JVM/CLR 对齐：调用约定 arity 以入口块 SSA 形参为准；`param_types` 可能含已废弃的 ABI 槽。
            let arity =
                mir_fn.blocks.get(mir_fn.entry.0 as usize).map(|block| block.parameters.len()).unwrap_or(mir_fn.param_types.len()) as i32;
            let local_count = ExecutableSlotPlan::plan_nyar(mir_fn).local_types.len() as i32;
            let function_index = module.functions.len() as i32;
            module.functions.push(OdinFunction {
                name: export_name.clone(),
                arity,
                local_count: local_count.max(arity),
                code_offset,
                code_length: module.code_bytes.len() as i32 - code_offset,
            });
            if nyar_should_export_operation(submission, instance) {
                module.exports.push(OdinExport { kind: OdinExportKind::Function, symbol_name: export_name, function_index });
            }
        }
    }

    Ok(module)
}

/// 为模块内出现过的 nominal sum 铸造后端私有物理布局，返回 nominal → 布局下标。
///
/// 物理合同由本文件固定：槽 0 = tag（i32），槽 1 = payload。字段顺序是 OVM 后端
/// 私有事实，只由计划冻结的 `SumRepresentation` 驱动；禁止按 variant 名 / 短名
/// 推断槽位。tag 宽度合同与 wasm `type_registry` 一致，只承诺 1..=4 字节。
fn register_sum_layouts(submission: &FragmentSubmission, layouts: &mut Vec<OdinLayout>) -> crate::Result<BTreeMap<NominalInstanceId, i32>> {
    let sums = submission.backend_plan.sum_representations();
    let exec = &submission.backend_plan;
    let mut nominals = BTreeSet::new();
    for instance in exec.instances() {
        let Some(view) = exec.get_function(&instance)
        else {
            continue;
        };
        for block in &view.function.blocks {
            for instruction in &block.instructions {
                let nominal = match &instruction.kind {
                    MirInstructionKind::SumNew { nominal, .. }
                    | MirInstructionKind::SumPayloadGet { nominal, .. }
                    | MirInstructionKind::SumVariantIs { nominal, .. } => *nominal,
                    _ => continue,
                };
                nominals.insert(nominal);
            }
        }
    }
    let mut map = BTreeMap::new();
    for nominal in nominals {
        let sum = sums
            .get(&nominal)
            .ok_or_else(|| format!("OVM sum 表示缺失：nominal {nominal:?} 没有冻结的 SumTypeLayout"))?;
        if sum.tag_width == 0 || sum.tag_width > 4 {
            return Err(format!(
                "OVM sum 表示只承诺 1..=4 字节 tag（nominal={nominal:?}, tag_width={}）",
                sum.tag_width
            ));
        }
        let index = layouts.len() as i32;
        layouts.push(OdinLayout { field_count: 2 });
        map.insert(nominal, index);
    }
    Ok(map)
}

/// 在降低函数体之前登记全部 operation → 稠密下标，供 `Call` 解析（含前向引用）。
fn build_nyar_function_entry_arities(exec: &TargetExecutionPlan, operations: &[ItemInstanceId]) -> BTreeMap<i32, usize> {
    let mut map = BTreeMap::new();
    for (index, instance) in operations.iter().enumerate() {
        let Some(view) = exec.get_function(instance)
        else {
            continue;
        };
        let entry_arity = view
            .function
            .blocks
            .get(view.function.entry.0 as usize)
            .map(|block| block.parameters.len())
            .unwrap_or(view.function.param_types.len());
        map.insert(index as i32, entry_arity);
    }
    map
}

/// 被调函数的入口形参槽类型表（与 arity 同源：入口块 SSA 形参的冻结槽类型）。
///
/// 常量实参的位宽必须由它决定；计划侧没有这张表时，Call 操作数位只能回落到
/// semantic MIR 合同（`Constant::Int` = Integer64），会把 i32 形参静默拓宽。
fn build_nyar_function_param_types(
    exec: &TargetExecutionPlan,
    operations: &[ItemInstanceId],
) -> crate::Result<BTreeMap<i32, Vec<NyarType>>> {
    let mut map = BTreeMap::new();
    for (index, instance) in operations.iter().enumerate() {
        let Some(view) = exec.get_function(instance)
        else {
            continue;
        };
        let mir_fn = &view.function;
        let slots = ExecutableSlotPlan::plan_nyar(mir_fn);
        let parameters = mir_fn.blocks.get(mir_fn.entry.0 as usize).map(|block| block.parameters.as_slice()).unwrap_or(&[]);
        let types = parameters
            .iter()
            .map(|parameter| {
                slots
                    .value_locals
                    .get(parameter)
                    .and_then(|local| slots.local_types.get(usize::from(*local)))
                    .cloned()
                    .ok_or_else(|| format!("OVM 形参 SSA 值缺少已规划槽类型：%{}（symbol=`{}`）", parameter.0, mir_fn.symbol))
            })
            .collect::<crate::Result<Vec<_>>>()?;
        map.insert(index as i32, types);
    }
    Ok(map)
}

/// 库模式只导出用户 `[export]` / `exported_operations`；闭包内 std 辅助函数保持内部 `Call` 可见性。
fn nyar_should_export_operation(submission: &FragmentSubmission, operation: ItemInstanceId) -> bool {
    if submission.backend_plan.wasm_export_names().contains_key(&operation) {
        return true;
    }
    submission.backend_plan.exported_operations().iter().any(|exported| *exported == operation)
}

fn nyar_mir_export_name(submission: &FragmentSubmission, operation: ItemInstanceId) -> String {
    if let Some(public_name) = submission.backend_plan.wasm_export_names().get(&operation) {
        return public_name.clone();
    }
    submission
        .backend_plan
        .abi_name_for_instance(operation)
        .map(|name| name.to_string())
        .unwrap_or_else(|| panic!("函数实例缺少 ABI 标签: {operation:?}"))
}

/// 片段内导出符号的实例 → 名称映射：公共导出名（`wasm_export_names`）保持原样；
/// abi 派生名在同一片段绑定多个实例时按实例下标消歧，导出表与入口决议共用同一映射。
fn nyar_export_name_map(submission: &FragmentSubmission) -> BTreeMap<ItemInstanceId, String> {
    let plan = &submission.backend_plan;
    let mut bases: BTreeMap<ItemInstanceId, String> = BTreeMap::new();
    for operation in plan.exported_operations().iter().copied().chain(plan.entry_operation()) {
        bases.entry(operation).or_insert_with(|| nyar_mir_export_name(submission, operation));
    }
    for operation in plan.wasm_export_names().keys() {
        bases.entry(*operation).or_insert_with(|| nyar_mir_export_name(submission, *operation));
    }
    let mut groups: BTreeMap<String, Vec<ItemInstanceId>> = BTreeMap::new();
    for (operation, name) in &bases {
        groups.entry(name.clone()).or_default().push(*operation);
    }
    let mut resolved = BTreeMap::new();
    for (name, operations) in groups {
        if operations.len() == 1 {
            resolved.insert(operations[0], name);
            continue;
        }
        for operation in operations {
            // 公共导出名是 run-contract 的稳定表面，冲突组内只给 abi 派生名追加实例下标。
            if plan.wasm_export_names().contains_key(&operation) {
                resolved.insert(operation, name.clone());
            }
            else {
                resolved.insert(operation, format!("{name}#{}", operation.index()));
            }
        }
    }
    resolved
}

/// Nyar VM run-contract 入口必须与模块内 `NyarExport.symbol_name` 一致；不能假定裸 `main`。
pub(crate) fn nyar_library_public_export_names(submission: &FragmentSubmission) -> Vec<String> {
    let plan = &submission.backend_plan;
    let names = nyar_export_name_map(submission);
    if plan.exported_operations().is_empty() {
        plan.entry_operation().and_then(|entry| names.get(&entry).cloned()).map(|name| vec![name]).unwrap_or_default()
    }
    else {
        plan.exported_operations().iter().filter_map(|operation| names.get(operation).cloned()).collect()
    }
}

fn lower_mir_function_to_bytecode(
    submission: &FragmentSubmission,
    mir_fn: &MirFunction,
    function_index_by_instance: &BTreeMap<ItemInstanceId, i32>,
    function_entry_arities: &BTreeMap<i32, usize>,
    param_types_by_function_index: &BTreeMap<i32, Vec<NyarType>>,
    external_link_by_instance: &BTreeMap<ItemInstanceId, ExternalImportLink>,
    external_param_types_by_instance: &BTreeMap<ItemInstanceId, Vec<NyarType>>,
    external_opcode_by_instance: &BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)>,
    sum_layout_index_by_nominal: &BTreeMap<NominalInstanceId, i32>,
    emitter: &mut BytecodeEmitter<'_>,
    layouts: &mut Vec<OdinLayout>,
    layout_index_by_id: &mut BTreeMap<LayoutId, i32>,
    layout_index_by_field_count: &mut BTreeMap<i32, i32>,
) -> crate::Result<()> {
    let ctx = ExecutableLoweringContext::new(submission);
    let slots = ExecutableSlotPlan::plan_nyar(mir_fn);
    let block_order = collect_reachable_blocks(mir_fn);

    let mut lowerer = NyarMirLowerer {
        submission,
        ctx,
        mir_fn,
        slots,
        emitter,
        function_index_by_instance,
        function_entry_arities,
        param_types_by_function_index,
        external_link_by_instance,
        external_param_types_by_instance,
        external_opcode_by_instance,
        sum_layout_index_by_nominal,
        layouts,
        layout_index_by_id,
        layout_index_by_field_count,
    };
    for block_id in block_order {
        lowerer.emitter.block_starts.insert(block_id, lowerer.emitter.code_bytes.len());
        if let Some(block) = mir_fn.blocks.get(block_id.0 as usize) {
            lowerer.emit_block(block)?;
        }
    }
    lowerer.emitter.patch_pending_jumps();
    Ok(())
}

struct NyarMirLowerer<'a, 'e> {
    submission: &'a FragmentSubmission,
    ctx: ExecutableLoweringContext<'a>,
    mir_fn: &'a MirFunction,
    slots: ExecutableSlotPlan,
    emitter: &'a mut BytecodeEmitter<'e>,
    function_index_by_instance: &'a BTreeMap<ItemInstanceId, i32>,
    function_entry_arities: &'a BTreeMap<i32, usize>,
    param_types_by_function_index: &'a BTreeMap<i32, Vec<NyarType>>,
    external_link_by_instance: &'a BTreeMap<ItemInstanceId, ExternalImportLink>,
    external_param_types_by_instance: &'a BTreeMap<ItemInstanceId, Vec<NyarType>>,
    external_opcode_by_instance: &'a BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)>,
    sum_layout_index_by_nominal: &'a BTreeMap<NominalInstanceId, i32>,
    layouts: &'a mut Vec<OdinLayout>,
    layout_index_by_id: &'a mut BTreeMap<LayoutId, i32>,
    layout_index_by_field_count: &'a mut BTreeMap<i32, i32>,
}

impl<'a, 'e> NyarMirLowerer<'a, 'e> {
    fn emit_block(&mut self, block: &MirBlock) -> crate::Result<()> {
        let _ = block_label(block.id);
        for instruction in &block.instructions {
            self.emit_instruction(instruction)?;
        }
        self.emit_terminator(block)?;
        Ok(())
    }

    fn emit_instruction(&mut self, instruction: &MirInstruction) -> crate::Result<()> {
        let output = self.slots.instruction_output(instruction);
        match &instruction.kind {
            MirInstructionKind::LoadConstant { constant, .. } => {
                // 常量位宽只由结果槽的冻结类型决定；槽缺失时回落到 semantic MIR
                // 对 `Constant::Int` 的合同（Integer64），绝不按数值大小猜。
                let output_ty = output.and_then(|value| self.value_slot_type(value));
                self.emit_load_constant(constant, output_ty.as_ref())?;
                if let Some(output) = output {
                    self.store_to_local(output);
                }
            }
            MirInstructionKind::StoreVar { name, value, .. } => {
                let output_ty = output.and_then(|value| self.value_slot_type(value));
                self.emit_operand_typed(value, output_ty.as_ref())?;
                if let Some(output) = output {
                    self.store_to_local(output);
                }
                else {
                    self.emitter.emit_plain(OdinHeadCode::Pop);
                }
                let _ = name;
            }
            MirInstructionKind::Copy { source } => {
                let output_ty = output.and_then(|value| self.value_slot_type(value));
                self.emit_operand_typed(source, output_ty.as_ref())?;
                if let Some(output) = output {
                    self.store_to_local(output);
                }
            }
            MirInstructionKind::StructNew { nominal, fields } => {
                let layout = self.ctx.layout_by_nominal(*nominal).cloned().ok_or_else(|| {
                    format!("OVM 聚合身份缺少布局合同：nominal {nominal:?} 不在计划的 AggregateLayoutPlan")
                })?;
                let layout_index = self.ensure_nyar_layout(&layout);
                self.emitter.emit_imm1(OdinHeadCode::ObjectNew, layout_index);
                let output = output.ok_or_else(|| format!("OVM StructNew 必须产出值槽（symbol=`{}`）", self.mir_fn.symbol))?;
                self.store_to_local(output);
                let output_operand = MirOperand::Value(output);
                for (field, value) in fields.iter() {
                    let slot = self.nyar_field_slot(*field)?;
                    let value_ty = self.field_value_type(*field);
                    self.emit_field_set_slot(&output_operand, slot, value, value_ty.as_ref())?;
                    self.emitter.emit_plain(OdinHeadCode::Pop);
                }
            }
            // 本块只有指令与不可达分支：`TupleGet` 尚无生产者，接入前禁止静默退化为近似读。
            MirInstructionKind::TupleGet { object, layout_id, index } => {
                return Err(format!(
                    "OVM 后端尚未提供 TupleGet 的精确表示编码（object={object:?}, layout_id={layout_id:?}, index={index}，symbol=`{}`）",
                    self.mir_fn.symbol
                ));
            }
            MirInstructionKind::TupleNew { layout_id, fields } => {
                let layout = self
                    .ctx
                    .layout_by_id(*layout_id)
                    .cloned()
                    .ok_or_else(|| format!("OVM tuple 布局身份缺少布局合同：layout_id={layout_id:?}"))?;
                let layout_index = self.ensure_nyar_layout(&layout);
                self.emitter.emit_imm1(OdinHeadCode::ObjectNew, layout_index);
                let output = output.ok_or_else(|| format!("OVM TupleNew 必须产出值槽（symbol=`{}`）", self.mir_fn.symbol))?;
                self.store_to_local(output);
                let output_operand = MirOperand::Value(output);
                for (index, value) in fields.iter().enumerate() {
                    let value_ty = layout.fields.get(index).map(|field| field.ty.clone());
                    self.emit_field_set_slot(&output_operand, index as i32, value, value_ty.as_ref())?;
                    self.emitter.emit_plain(OdinHeadCode::Pop);
                }
            }
            MirInstructionKind::ArrayFromElements { array_type, elements, .. } => {
                let Some(output) = output
                else {
                    return Err(format!("OVM ArrayFromElements 必须产出值槽（symbol=`{}`）", self.mir_fn.symbol));
                };
                // 合成的定长数组布局身份不保证进入 `AggregateLayoutPlan`；Odin 布局只记录
                // 字段数且槽位无类型，所以按元素个数铸造后端私有布局，禁止依赖未冻结的
                // `layout_id`，更不允许 panic 杀进程。
                if let NyarType::FixedArray { length, .. } = array_type
                    && *length != elements.len()
                {
                    return Err(format!(
                        "OVM ArrayFromElements 的元素个数与冻结的定长数组类型不符：type={array_type} declared={length} actual={}（symbol=`{}`）",
                        elements.len(),
                        self.mir_fn.symbol
                    ));
                }
                let field_count = i32::try_from(elements.len()).map_err(|_| {
                    format!("OVM ArrayFromElements 元素个数超出 i32 布局槽位：{}（symbol=`{}`）", elements.len(), self.mir_fn.symbol)
                })?;
                let element_ty = match array_type {
                    NyarType::FixedArray { element, .. } | NyarType::Array(element) => Some((**element).clone()),
                    _ => None,
                };
                let layout_index = self.ensure_nyar_layout_for_field_count(field_count);
                self.emitter.emit_imm1(OdinHeadCode::ObjectNew, layout_index);
                self.store_to_local(output);
                let output_operand = MirOperand::Value(output);
                for (index, value) in elements.iter().enumerate() {
                    self.emit_field_set_slot(&output_operand, index as i32, value, element_ty.as_ref())?;
                    self.emitter.emit_plain(OdinHeadCode::Pop);
                }
            }
            MirInstructionKind::AggregateCopy { layout_id, source, dest } => {
                let layout = self
                    .ctx
                    .layout_by_id(*layout_id)
                    .cloned()
                    .ok_or_else(|| format!("OVM aggregate-copy 布局身份缺少布局合同：layout_id={layout_id:?}"))?;
                let field_count = layout.fields.len() as i32;
                let layout_index = self.ensure_nyar_layout(&layout);
                self.emitter.emit_imm1(OdinHeadCode::ObjectNew, layout_index);
                if let MirOperand::Value(dest_value) = dest {
                    self.store_to_local(*dest_value);
                }
                for slot in 0..field_count {
                    self.emit_operand(dest)?;
                    self.emit_operand(source)?;
                    self.emitter.emit_imm1(OdinHeadCode::FieldGet, slot);
                    self.emitter.emit_imm1(OdinHeadCode::FieldSet, slot);
                    self.emitter.emit_plain(OdinHeadCode::Pop);
                }
            }
            MirInstructionKind::FieldGet { object, field } => {
                let slot = self.nyar_field_slot(*field)?;
                self.emit_operand(object)?;
                self.emitter.emit_imm1(OdinHeadCode::FieldGet, slot);
                if let Some(output) = output {
                    self.store_to_local(output);
                }
                else {
                    // 无 SSA 绑定的 FieldGet 不得污染后续 Call 的操作数栈；实参由 `emit_call_operand` 再求值。
                    self.emitter.emit_plain(OdinHeadCode::Pop);
                }
            }
            MirInstructionKind::FieldSet { object, field, value } => {
                let slot = self.nyar_field_slot(*field)?;
                let value_ty = self.field_value_type(*field);
                self.emit_field_set_slot(object, slot, value, value_ty.as_ref())?;
                self.emitter.emit_plain(OdinHeadCode::Pop);
            }
            // sum 的物理载体是后端私有的 `[i32 tag, payload]` 对象；语义身份（nominal /
            // variant）只来自计划冻结的 `SumRepresentation`，绝不按名字或字面 tag 推断。
            // OVM 的 Odin 对象字段是无类型槽，标量载荷直接入槽；wasm 的 payload 槽是
            // anyref 才需要装箱，这里不得照抄 wasm 的装箱路径。
            MirInstructionKind::SumNew { nominal, variant, payload } => {
                let (tag, planned_payload_type) = {
                    let sum = self
                        .submission
                        .backend_plan
                        .sum_representations()
                        .get(nominal)
                        .ok_or_else(|| format!("OVM sum 表示缺失：nominal {nominal:?} 没有冻结的 SumTypeLayout"))?;
                    let variant_layout = sum
                        .variants
                        .get(variant)
                        .ok_or_else(|| format!("OVM SumNew 的 variant {variant:?} 不属于 nominal {nominal:?} 的冻结变体表"))?;
                    (variant_layout.tag, variant_layout.payload_type)
                };
                if planned_payload_type.is_some() != payload.is_some() {
                    return Err(format!(
                        "OVM SumNew 的 variant {variant:?} 载荷与计划合同不一致：计划 payload_type={planned_payload_type:?}，指令 payload={}",
                        payload.is_some()
                    ));
                }
                let Some(output) = output
                else {
                    return Err(format!("OVM SumNew 必须产出值槽（symbol=`{}`）", self.mir_fn.symbol));
                };
                let sum_layout_index = self.ensure_sum_layout(*nominal)?;
                self.emitter.emit_imm1(OdinHeadCode::ObjectNew, sum_layout_index);
                self.store_to_local(output);
                let object = MirOperand::Value(output);
                // tag 槽的合同宽度是 i32（`SumRepresentation.tag_width` <= 4），超界即失败。
                let tag_value = i32::try_from(tag).map_err(|_| {
                    format!(
                        "OVM SumNew 的 tag {tag} 超出 i32 槽合同（nominal={nominal:?}, variant={variant:?}, symbol=`{}`）",
                        self.mir_fn.symbol
                    )
                })?;
                let tag_ty = NyarType::Integer32 { signed: true };
                self.emit_field_set_slot(&object, 0, &MirOperand::Constant(MirConstant::Int(i64::from(tag_value))), Some(&tag_ty))?;
                self.emitter.emit_plain(OdinHeadCode::Pop);
                if let Some(payload) = payload {
                    // payload 槽本身不带类型；SSA 载荷用其冻结语义类型，常量载荷
                    // 回落到 semantic MIR 合同（`Constant::Int` = Integer64）。
                    let payload_ty = match payload {
                        MirOperand::Value(value) => self.value_semantic_type(*value),
                        _ => None,
                    };
                    self.emit_field_set_slot(&object, 1, payload, payload_ty.as_ref())?;
                    self.emitter.emit_plain(OdinHeadCode::Pop);
                }
            }
            MirInstructionKind::SumVariantIs { nominal, variant, object } => {
                let tag = self.sum_variant_tag(*nominal, *variant, "SumVariantIs")?;
                self.emit_operand(object)?;
                self.emitter.emit_imm1(OdinHeadCode::FieldGet, 0);
                self.emitter.emit_const_i32(tag as i32);
                self.emitter.emit_plain(OdinHeadCode::I32Eq);
                if let Some(output) = output {
                    self.store_to_local(output);
                }
                else {
                    self.emitter.emit_plain(OdinHeadCode::Pop);
                }
            }
            MirInstructionKind::SumPayloadGet { nominal, variant, object } => {
                let payload_type = self.sum_variant_payload_type(*nominal, *variant, "SumPayloadGet")?;
                if payload_type.is_none() {
                    return Err(format!("OVM SumPayloadGet 对无 payload 的 variant {variant:?} 取值（nominal={nominal:?}）"));
                }
                self.emit_operand(object)?;
                self.emitter.emit_imm1(OdinHeadCode::FieldGet, 1);
                if let Some(output) = output {
                    self.store_to_local(output);
                }
                else {
                    self.emitter.emit_plain(OdinHeadCode::Pop);
                }
            }
            MirInstructionKind::Call { callee, arguments } => {
                if let MirOperand::Item(instance) = callee {
                    if let Some(link) = self.external_link_by_instance.get(instance).cloned() {
                        let opcode = self.external_opcode_by_instance.get(instance);
                        self.emit_external_call(&link, *instance, opcode, arguments, output)?;
                        return Ok(());
                    }
                    let index = self
                        .function_index_by_instance
                        .get(instance)
                        .copied()
                        .unwrap_or_else(|| panic!("validated callable identity has no Nyar function index: {instance:?}"));
                    let name = self
                        .submission
                        .backend_plan
                        .abi_name_for_instance(*instance)
                        .unwrap_or_else(|| panic!("validated callable identity has no diagnostic name: {instance:?}"));
                    let path = NamePath::new(name.parts().to_vec());
                    self.emit_direct_call(index, &path, arguments, output)?;
                    return Ok(());
                }
                panic!("Semantic MIR 调用未绑定 ItemInstanceId，拒绝进入 Nyar 后端: {callee:?}");
            }
            MirInstructionKind::ArrayGet { array, index } => {
                self.emit_call_operand(array)?;
                let index_ty = NyarType::Integer32 { signed: true };
                self.emit_operand_typed(index, Some(&index_ty))?;
                self.emitter.emit_call_intrinsic(IntrinsicOpcode::ArrayGet, 2);
                if let Some(output) = output {
                    self.store_to_local(output);
                }
            }
            MirInstructionKind::ArraySet { array, index, value } => {
                self.emit_call_operand(array)?;
                let index_ty = NyarType::Integer32 { signed: true };
                self.emit_operand_typed(index, Some(&index_ty))?;
                let value_ty = self.array_element_type(array);
                self.emit_operand_typed(value, value_ty.as_ref())?;
                self.emitter.emit_call_intrinsic(IntrinsicOpcode::ArraySet, 3);
                self.emitter.emit_plain(OdinHeadCode::Pop);
            }
            MirInstructionKind::ArrayLength { array } => {
                // 禁止 Const(0) 回退：会把 ArrayLen 的 array 实参变成 i32，运行时报 expected object。
                if !self.emit_call_operand(array)? {
                    panic!("ovm ArrayLength operand not materializable in `{}`", self.mir_fn.symbol);
                }
                self.emitter.emit_call_intrinsic(IntrinsicOpcode::ArrayLen, 1);
                if let Some(output) = output {
                    self.store_to_local(output);
                }
            }
            MirInstructionKind::NullIsNull { value } => {
                if !self.emit_call_operand(value)? {
                    panic!("ovm NullIsNull operand not materializable in `{}`", self.mir_fn.symbol);
                }
                self.emitter.emit_call_intrinsic(IntrinsicOpcode::IsNull, 1);
                if let Some(output) = output {
                    self.store_to_local(output);
                }
            }
            MirInstructionKind::NullUnwrap { value } => {
                if !self.emit_call_operand(value)? {
                    panic!("ovm NullUnwrap operand not materializable in `{}`", self.mir_fn.symbol);
                }
                self.emitter.emit_call_intrinsic(IntrinsicOpcode::UnwrapNull, 1);
                if let Some(output) = output {
                    self.store_to_local(output);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn emit_terminator(&mut self, block: &MirBlock) -> crate::Result<()> {
        match &block.terminator {
            MirTerminator::Return { value } => {
                if let Some(value) = value {
                    // 返回槽类型是函数签名的冻结事实；常量必须按它选位宽。
                    let return_ty = self.mir_fn.return_type.clone();
                    self.emit_operand_typed(value, Some(&return_ty))?;
                }
                else {
                    // Call 约定固定压回 1 个返回值；unit / void 用 i32(0) 占位，供调用方 Pop。
                    self.emitter.emit_const_i32(0);
                }
                self.emitter.emit_plain(OdinHeadCode::Return);
            }
            MirTerminator::Jump { target, arguments } => {
                self.emit_block_argument_copies(*target, arguments)?;
                self.emitter.emit_jump_placeholder(OdinHeadCode::Jump, *target);
            }
            MirTerminator::Branch { condition, then_target, else_target } => {
                let condition_ty = NyarType::Boolean;
                self.emit_operand_typed(condition, Some(&condition_ty))?;
                self.emitter.emit_jump_placeholder(OdinHeadCode::JumpIfFalse, *else_target);
                self.emit_block_argument_copies(*then_target, &[])?;
                self.emitter.emit_jump_placeholder(OdinHeadCode::Jump, *then_target);
                let _ = else_target;
            }
            MirTerminator::PerformEffect { effect, payload, resume_target } => {
                match effect {
                    EffectKind::Yield | EffectKind::DelegateYield => {
                        // `yield expr` / `yield from expr`：把 payload 压栈后发射 `Yield` opcode。
                        // VM 弹出 yielded 值、捕获当前帧为 `CoroutineState`、挂起返回父帧；
                        // 当父帧 `Resume` 时从 `Yield` 之后继续，跳到 resume_target。
                        if let Some(payload) = payload {
                            self.emit_operand(payload)?;
                        }
                        else {
                            // 无 payload 的 yield：压 i32(0) 作为 unit 占位后再发 Yield。
                            self.emitter.emit_const_i32(0);
                        }
                        self.emitter.emit_plain(OdinHeadCode::Yield);
                        self.emit_block_argument_copies(*resume_target, &[])?;
                        self.emitter.emit_jump_placeholder(OdinHeadCode::Jump, *resume_target);
                    }
                    EffectKind::Raise => {
                        // `raise expr`：将 payload 压栈后发射 `PerformEffect` opcode。
                        // operand1 = 常量池索引（effect 的 method_name 字符串）。
                        // VM 用此字符串在 witness_entries 中查找 handler。
                        // handler 可选择 resume（控制流回到 resume_target，resume 值在栈顶）；
                        // 若未 resume 则按未处理效果传播。
                        // 对 `Effectful::Resume = !`：resume 不可达时 VM 不必保留 resume 续体。
                        if let Some(payload) = payload {
                            self.emit_operand(payload)?;
                        }
                        else {
                            // 无 payload 的 raise：压 i32(0) 作为 unit 占位。
                            self.emitter.emit_const_i32(0);
                        }
                        let effect_name_index = self.emitter.intern_string("raise");
                        self.emitter.emit_imm1(OdinHeadCode::PerformEffect, effect_name_index);
                        // resume 时 VM 把 resume 值压到栈顶；resume 块的唯一形参直接从栈顶落槽。
                        // 零形参 resume 块（`Resume = !` 的未捕获 raise）没有落槽对象，
                        // 栈顶值由 VM 的续体语义持有，本层不补造操作数。
                        let resume_block = self.mir_fn.blocks.iter().find(|block| block.id == *resume_target).expect("validated resume target");
                        match resume_block.parameters.len() {
                            0 => {}
                            1 => {
                                let local = self.slots.block_param_locals[&(*resume_target, 0)];
                                self.emit_store_local(local);
                            }
                            count => panic!("ovm `raise` resume 块只接受至多一个 resume 形参，实际 {count}: {}", self.mir_fn.symbol),
                        }
                        self.emitter.emit_jump_placeholder(OdinHeadCode::Jump, *resume_target);
                    }
                    EffectKind::Await | EffectKind::AsyncSpawn | EffectKind::AsyncBlock => {
                        // future / async 相关 effect 暂未落地 VM opcode，保留 console_log 调试桩。
                        if let Some(payload) = payload {
                            self.emit_operand(payload)?;
                            self.emitter.emit_call_import("console_log", 1);
                        }
                        self.emit_block_argument_copies(*resume_target, &[])?;
                        self.emitter.emit_jump_placeholder(OdinHeadCode::Jump, *resume_target);
                    }
                }
            }
            MirTerminator::YieldToRuntime { effect, payload, resume_state: _ } => {
                // 状态机重写后的 effect：与 `PerformEffect` 同构地发射对应 opcode。
                // resume_state 由后续 `StateDispatch` 在恢复时读取（暂用 fallthrough 桩处理）。
                match effect {
                    EffectKind::Yield | EffectKind::DelegateYield => {
                        if let Some(payload) = payload {
                            self.emit_operand(payload)?;
                        }
                        else {
                            self.emitter.emit_const_i32(0);
                        }
                        self.emitter.emit_plain(OdinHeadCode::Yield);
                    }
                    EffectKind::Raise => {
                        if let Some(payload) = payload {
                            self.emit_operand(payload)?;
                        }
                        else {
                            self.emitter.emit_const_i32(0);
                        }
                        let effect_name_index = self.emitter.intern_string("raise");
                        self.emitter.emit_imm1(OdinHeadCode::PerformEffect, effect_name_index);
                    }
                    EffectKind::Await | EffectKind::AsyncSpawn | EffectKind::AsyncBlock => {
                        // future / async 相关 effect 暂未落地 VM opcode，保留 console_log 调试桩。
                        if let Some(payload) = payload {
                            self.emit_operand(payload)?;
                            self.emitter.emit_call_import("console_log", 1);
                        }
                    }
                }
            }
            MirTerminator::StateDispatch { state, cases, default_target } => {
                // 状态机入口：读取 state local，与每个 case_key 比较；
                // 匹配则跳到对应 target，否则跳到 default_target。
                for (case_key, target) in cases {
                    // `case_key` 是 `u32`，`as i32` 会把 ≥ 2^31 的键静默折回负值。
                    // 没有任何生产者把它约束在 i32 内，因此超界必须结构化失败，
                    // 不得发出一个永远匹配不上的错误常量。
                    let case_key = i32::try_from(*case_key).map_err(|_| {
                        format!(
                            "OVM StateDispatch 的 case_key {} 超出 i32 表示范围（symbol=`{}`）",
                            *case_key, self.mir_fn.symbol
                        )
                    })?;
                    self.emit_operand(&MirOperand::Value(*state))?;
                    self.emitter.emit_const_i32(case_key);
                    self.emitter.emit_plain(OdinHeadCode::I32Eq);
                    self.emitter.emit_jump_placeholder(OdinHeadCode::JumpIfTrue, *target);
                }
                self.emitter.emit_jump_placeholder(OdinHeadCode::Jump, *default_target);
            }
            MirTerminator::Unreachable => {
                // 不可达块不得物化为裸 `Return`：加载期 verifier 按 CFG 可达性把每条
                // `Return` 计入返回形状汇合（与位置无关），空栈 `Return` 会与值返回
                // 冲突。改为发射真 `Trap`：无后继、不贡献返回形状、不落入下一函数；
                // 语义正确，且一旦可达性保证被破坏（前端把可达块标成 `Unreachable`），
                // 运行期会得到带 function/pc 的结构化 trap 失败，而不是自旋挂死。
                self.emitter.emit_plain(OdinHeadCode::Trap);
            }
        }
        Ok(())
    }

    /// 发射 `backend_plan.imports()` 中的外部 provider 调用。
    ///
    /// 先按计划冻结的 `[ovm_operator("...")]` 配方内联为指令：未冻结的绑定与 arity
    /// 不符必须结构化失败，不得回落为宿主导入或默认成功。其余已绑定目标 provider
    /// （`host` 边界 + `vm` 目标标签 + 恰好一段符号）保持 `CallImport`，由 runtime
    /// `parse_host_op` 判定。未绑定 `[host_contract]` 或其它目标标签必须失败，
    /// 不得回落为空体、默认值或 `0`。
    fn emit_external_call(
        &mut self,
        link: &ExternalImportLink,
        instance: ItemInstanceId,
        opcode: Option<&(String, TargetOperatorBinding)>,
        arguments: &[MirOperand],
        output: Option<MirValueRef>,
    ) -> crate::Result<()> {
        let callee_name = self
            .submission
            .backend_plan
            .abi_name_for_instance(instance)
            .map(|name| name.to_string())
            .unwrap_or_else(|| format!("{instance:?}"));

        if let Some((mnemonic, binding)) = opcode {
            let TargetOperatorBinding::Frozen(plan) = binding
            else {
                return Err(format!(
                    "Nyar 目标 `ovm_operator` 绑定缺少可编码助记符 `{mnemonic}`: callee=`{callee_name}` function=`{}`",
                    self.mir_fn.symbol
                ));
            };
            let TargetOperatorEncoding::Ovm(code) = plan.encoding
            else {
                return Err(format!(
                    "Nyar 目标 `ovm_operator` 绑定 `{mnemonic}` 冻结了非 OVM 编码: callee=`{callee_name}` function=`{}`",
                    self.mir_fn.symbol
                ));
            };
            if arguments.len() != plan.arity {
                return Err(format!(
                    "Nyar 目标外部数值调用 `{callee_name}` 实参数量与 `{mnemonic}` 冻结 arity 不符: function=`{}` expected={} actual={}",
                    self.mir_fn.symbol,
                    plan.arity,
                    arguments.len()
                ));
            }
            match plan.recipe {
                TargetOperatorRecipe::Operands => {
                    self.emit_external_arguments(&callee_name, instance, arguments)?;
                    self.emitter.emit_plain(code);
                }
                TargetOperatorRecipe::Steps(steps) => {
                    for step in steps {
                        match step {
                            TargetOperandStep::Arguments => self.emit_external_arguments(&callee_name, instance, arguments)?,
                            TargetOperandStep::IntegerConstant { operand_type: TargetOperandType::I32, value } => {
                                let index = self.emitter.intern_i32(*value);
                                self.emitter.emit_imm1(OdinHeadCode::Constant, index);
                            }
                            TargetOperandStep::IntegerConstant { operand_type, .. } => {
                                return Err(format!(
                                    "Nyar 目标 `ovm_operator` 绑定 `{mnemonic}` 冻结了非 i32 常量配方 {operand_type:?}，OVM 没有该常量载体: callee=`{callee_name}` function=`{}`",
                                    self.mir_fn.symbol
                                ));
                            }
                            TargetOperandStep::NarrowTopI64ToI32 => {
                                return Err(format!(
                                    "Nyar 目标 `ovm_operator` 绑定 `{mnemonic}` 冻结了 i64→i32 截窄配方，OVM 没有 i64 载体可截窄: callee=`{callee_name}` function=`{}`",
                                    self.mir_fn.symbol
                                ));
                            }
                        }
                    }
                    self.emitter.emit_plain(code);
                }
            }
        }
        else {
            let Some(symbol) = (link.matches_boundary("host") && link.matches_platform_tag("ovm"))
                .then(|| match link.locator_segments() {
                    [symbol] => Some(symbol.as_str()),
                    _ => None,
                })
                .flatten()
            else {
                panic!(
                    "Nyar 目标不支持外部调用 `{callee_name}`：链接合同 {link}（contract/tag）不是已绑定的目标 provider；\
                     需要 host 边界 + 目标标签 `ovm-operator` + 恰好一段符号，未绑定目标 provider 不得回落"
                );
            };
            self.emit_external_arguments(&callee_name, instance, arguments)?;
            self.emitter.emit_call_import(symbol, arguments.len() as i32);
        }

        if let Some(output) = output {
            self.store_to_local(output);
        }
        else {
            self.emitter.emit_plain(OdinHeadCode::Pop);
        }
        Ok(())
    }

    /// 按计划冻结的 `BackendImport.parameter_types` 发射外部调用实参。
    ///
    /// 位宽权威与内部调用同源（签名表），此处不按字面量值域猜测：签名表缺项时
    /// 结构化失败，绝不回落到 semantic MIR 的 `Integer64` 默认。
    fn emit_external_arguments(
        &mut self,
        callee_name: &str,
        instance: ItemInstanceId,
        arguments: &[MirOperand],
    ) -> crate::Result<()> {
        let parameter_types = self.external_param_types_by_instance.get(&instance).cloned();
        let import_symbol = self
            .external_link_by_instance
            .get(&instance)
            .map(|link| link.locator_segments().join("::"))
            .unwrap_or_default();
        for (index, argument) in arguments.iter().enumerate() {
            let actual = self.operand_actual_type(argument);
            let Some(expected) = parameter_types.as_ref().and_then(|types| types.get(index)).cloned()
            else {
                return Err(format!(
                    "OVM 外部调用 `{callee_name}`（import={import_symbol} instance={instance:?}）的实参 #{index} 缺少已规划签名类型：\
                     期望类型=<`BackendImport.parameter_types` 未覆盖该参数>，实际类型={actual}；\
                     禁止回落到 semantic MIR 的 `Integer64` 默认"
                ));
            };
            match argument {
                MirOperand::Value(_) => {
                    if !self.emit_call_operand(argument)? {
                        panic!("unmaterialized Nyar external call operand in `{callee_name}`: {argument:?}");
                    }
                }
                _ => self.emit_operand_typed(argument, Some(&expected)).map_err(|error| {
                    format!(
                        "OVM 外部调用 `{callee_name}`（import={import_symbol} instance={instance:?}）的实参 #{index} 发射失败：\
                         期望类型={expected}，实际类型={actual}；{error}"
                    )
                })?,
            }
        }
        Ok(())
    }

    /// 实参当前的实际载体/字面量描述，只用于结构化失败文本，不参与发射决策。
    fn operand_actual_type(&self, operand: &MirOperand) -> String {
        match operand {
            MirOperand::Value(value) => self
                .value_slot_type(*value)
                .or_else(|| self.value_semantic_type(*value))
                .map(|ty| ty.to_string())
                .unwrap_or_else(|| format!("%{}（未冻结类型）", value.0)),
            MirOperand::Constant(constant) => format!("{constant:?}（无槽字面量）"),
            other => format!("{other:?}"),
        }
    }

    fn emit_direct_call(
        &mut self,
        function_index: i32,
        path: &nyar::NamePath,
        arguments: &[MirOperand],
        output: Option<MirValueRef>,
    ) -> crate::Result<()> {
        let expected = self.function_entry_arities.get(&function_index).copied().unwrap_or(arguments.len());
        assert_eq!(arguments.len(), expected, "validated call arity differs from function entry: {path:?} index={function_index}");
        for (index, argument) in arguments.iter().enumerate() {
            // 常量实参的位宽来自被调函数已冻结的入口形参槽类型；没有该槽时
            // 才回落到 semantic MIR 合同，绝不按字面量大小选位宽。
            let param_ty = self.param_types_by_function_index.get(&function_index).and_then(|types| types.get(index));
            match argument {
                MirOperand::Value(_) => {
                    if !self.emit_call_operand(argument)? {
                        panic!("unmaterialized Nyar call operand in `{path:?}`: {argument:?}");
                    }
                }
                _ => self.emit_operand_typed(argument, param_ty)?,
            }
        }
        self.emitter.emit_imm1(OdinHeadCode::Call, function_index);
        if let Some(output) = output {
            self.store_to_local(output);
        }
        else {
            self.emitter.emit_plain(OdinHeadCode::Pop);
        }
        Ok(())
    }

    /// Call 实参只能读取已规划的 SSA/local 值；操作数位常量按 semantic MIR 合同发射。
    fn emit_call_operand(&mut self, operand: &MirOperand) -> crate::Result<bool> {
        match operand {
            MirOperand::Value(value) => {
                let Some(local) = self.slots.value_locals.get(value).copied()
                else {
                    return Ok(false);
                };
                self.emit_load_local(local);
                Ok(true)
            }
            MirOperand::Constant(_) => {
                self.emit_operand(operand)?;
                Ok(true)
            }
            MirOperand::Symbol(_) | MirOperand::Item(_) => Ok(false),
        }
    }

    fn nyar_field_slot(&self, field: nyar_types::FieldId) -> crate::Result<i32> {
        self.ctx
            .submission
            .backend_plan
            .aggregate_layout_by_field()
            .get(&field)
            .map(|(_, slot)| *slot as i32)
            .ok_or_else(|| format!("OVM 字段身份缺少布局槽位合同：field={field:?}"))
    }

    fn ensure_nyar_layout(&mut self, aggregate: &AggregateLayout) -> i32 {
        if let Some(index) = self.layout_index_by_id.get(&aggregate.id) {
            return *index;
        }
        let index = self.layouts.len() as i32;
        self.layouts.push(OdinLayout { field_count: aggregate.fields.len() as i32 });
        self.layout_index_by_id.insert(aggregate.id, index);
        index
    }

    /// 定长数组用的后端私有布局：Odin 对象只有字段数，没有字段类型，因此同字段数的
    /// 数组可以共享布局；身份只用于物理容纳，不参与语义匹配。
    fn ensure_nyar_layout_for_field_count(&mut self, field_count: i32) -> i32 {
        if let Some(index) = self.layout_index_by_field_count.get(&field_count) {
            return *index;
        }
        let index = self.layouts.len() as i32;
        self.layouts.push(OdinLayout { field_count });
        self.layout_index_by_field_count.insert(field_count, index);
        index
    }

    /// 注册阶段已为每个出现过的 nominal 铸造布局；此查询是后端自洽性护栏。
    fn ensure_sum_layout(&self, nominal: NominalInstanceId) -> crate::Result<i32> {
        self.sum_layout_index_by_nominal
            .get(&nominal)
            .copied()
            .ok_or_else(|| format!("OVM sum 表示缺失：nominal {nominal:?} 没有冻结的 SumTypeLayout"))
    }

    fn sum_variant_tag(&self, nominal: NominalInstanceId, variant: VariantId, operation: &str) -> crate::Result<u32> {
        self.submission
            .backend_plan
            .sum_representations()
            .get(&nominal)
            .ok_or_else(|| format!("OVM sum 表示缺失：nominal {nominal:?} 没有冻结的 SumTypeLayout"))?
            .variants
            .get(&variant)
            .map(|variant_layout| variant_layout.tag)
            .ok_or_else(|| format!("OVM {operation} 的 variant {variant:?} 不属于 nominal {nominal:?} 的冻结变体表"))
    }

    fn sum_variant_payload_type(
        &self,
        nominal: NominalInstanceId,
        variant: VariantId,
        operation: &str,
    ) -> crate::Result<Option<nyar_types::TypeId>> {
        self.submission
            .backend_plan
            .sum_representations()
            .get(&nominal)
            .ok_or_else(|| format!("OVM sum 表示缺失：nominal {nominal:?} 没有冻结的 SumTypeLayout"))?
            .variants
            .get(&variant)
            .map(|variant_layout| variant_layout.payload_type)
            .ok_or_else(|| format!("OVM {operation} 的 variant {variant:?} 不属于 nominal {nominal:?} 的冻结变体表"))
    }

    /// `FieldSet` 栈效果为 `[obj, value] -> [obj]`；调用方随后 `Pop`。
    /// `value_ty` 是制品槽的冻结类型（Odin 槽本身无类型），常量位宽必须由它决定。
    fn emit_field_set_slot(
        &mut self,
        object: &MirOperand,
        field_slot: i32,
        value: &MirOperand,
        value_ty: Option<&NyarType>,
    ) -> crate::Result<()> {
        self.emit_operand(object)?;
        self.emit_operand_typed(value, value_ty)?;
        self.emitter.emit_imm1(OdinHeadCode::FieldSet, field_slot);
        Ok(())
    }

    fn emit_block_argument_copies(&mut self, target: MirBlockRef, arguments: &[MirOperand]) -> crate::Result<()> {
        let target_block = self.mir_fn.blocks.iter().find(|block| block.id == target).expect("validated branch target");
        assert_eq!(arguments.len(), target_block.parameters.len(), "validated branch arity");
        for (index, argument) in arguments.iter().enumerate() {
            let param_local = self.slots.block_param_locals[&(target, index)];
            let param_ty = self.slots.local_types[usize::from(param_local)].clone();
            self.emit_operand_typed(argument, Some(&param_ty))?;
        }
        for index in (0..arguments.len()).rev() {
            let param_local = self.slots.block_param_locals[&(target, index)];
            self.emit_store_local(param_local);
        }
        Ok(())
    }

    /// 发射常量。整数位宽只由目标槽的冻结类型决定：
    /// - `Some(Integer32)`：必须落在 i32 表示范围内，否则结构化失败；
    /// - `Some(Integer64)`：发 `Integer64` 载体；
    /// - `None`（操作数位没有槽上下文）：回落到 semantic MIR 合同
    ///   `Constant::Int(_) => Integer64 { signed: true }`，**不按数值大小选位宽**；
    /// - 其它槽类型：结构化失败。
    fn emit_load_constant(&mut self, constant: &MirConstant, slot_ty: Option<&NyarType>) -> crate::Result<()> {
        match constant {
            MirConstant::Int(value) => match slot_ty {
                Some(NyarType::Integer32 { .. }) => {
                    let narrowed = i32::try_from(*value).map_err(|_| {
                        format!(
                            "OVM 整数常量 {value} 超出目标 Integer32 槽的表示范围（slot={}, symbol=`{}`）",
                            slot_ty.map(ToString::to_string).unwrap_or_else(|| "<none>".to_string()),
                            self.mir_fn.symbol
                        )
                    })?;
                    self.emitter.emit_const_i32(narrowed);
                }
                Some(NyarType::Integer64 { .. }) | None => {
                    self.emitter.emit_const_i64(*value);
                }
                Some(other) => {
                    return Err(format!(
                        "OVM 整数常量 {value} 与已规划槽类型不匹配：slot={other} symbol=`{}`；\
                         禁止把整数载荷塞进非整数槽",
                        self.mir_fn.symbol
                    ));
                }
            },
            MirConstant::Float64(value) => {
                let index = self.emitter.constants.len() as i32 + self.emitter.constants_base;
                self.emitter.constants.push(OdinConstant::Float64(value.into_inner()));
                self.emitter.emit_imm1(OdinHeadCode::Constant, index);
            }
            MirConstant::Bool(value) => {
                self.emitter.emit_const_bool(*value);
            }
            MirConstant::Utf8(text) => {
                let index = self.emitter.intern_string(text);
                self.emitter.emit_imm1(OdinHeadCode::Constant, index);
            }
            MirConstant::Utf16(_) => panic!("ovm lowering has no explicit UTF-16 text ABI"),
            // OVM 目标没有可空空引用的 ABI 合同；不得用 i32 0 冒充 null。
            MirConstant::Null => panic!("ovm lowering has no nullable reference ABI"),
            MirConstant::Unit => {
                self.emitter.emit_const_i32(0);
            }
        }
        Ok(())
    }

    fn emit_operand(&mut self, operand: &MirOperand) -> crate::Result<()> {
        self.emit_operand_typed(operand, None)
    }

    fn emit_operand_typed(&mut self, operand: &MirOperand, slot_ty: Option<&NyarType>) -> crate::Result<()> {
        match operand {
            MirOperand::Value(value) => {
                let local = self.slots.value_locals.get(value).copied().ok_or_else(|| {
                    format!("OVM SSA 值缺少预规划槽：%{}（symbol=`{}`）", value.0, self.mir_fn.symbol)
                })?;
                self.emit_load_local(local);
                Ok(())
            }
            MirOperand::Constant(constant) => self.emit_load_constant(constant, slot_ty),
            MirOperand::Symbol(path) => Err(format!("OVM 操作数不支持符号引用 {path:?}（symbol=`{}`）", self.mir_fn.symbol)),
            MirOperand::Item(instance) => Err(format!("OVM 操作数不支持 callable 身份 {instance:?}（symbol=`{}`）", self.mir_fn.symbol)),
        }
    }

    /// SSA 值的物理槽类型（来自 `ExecutableSlotPlan` 的冻结类型）。
    fn value_slot_type(&self, value: MirValueRef) -> Option<NyarType> {
        self.slots
            .value_locals
            .get(&value)
            .and_then(|local| self.slots.local_types.get(usize::from(*local)))
            .cloned()
    }

    /// SSA 值的语义类型（操作数位常量回落用的同一张冻结表）。
    fn value_semantic_type(&self, value: MirValueRef) -> Option<NyarType> {
        self.mir_fn.value_types.get(&value).cloned()
    }

    /// 制品字段的冻结类型；Odin 槽无类型，常量位宽只能由布局合同给出。
    fn field_value_type(&self, field: nyar_types::FieldId) -> Option<NyarType> {
        self.ctx.field_layout_by_id(field).map(|(_, layout, _)| layout.ty.clone())
    }

    /// 数组元素类型：只从 SSA 值的冻结语义类型读取，不回查名字。
    fn array_element_type(&self, array: &MirOperand) -> Option<NyarType> {
        let MirOperand::Value(value) = array
        else {
            return None;
        };
        match self.mir_fn.value_types.get(value)? {
            NyarType::Array(element) | NyarType::FixedArray { element, .. } => Some((**element).clone()),
            _ => None,
        }
    }

    fn ensure_value_local(&mut self, value: MirValueRef) -> u16 {
        self.slots.value_locals[&value]
    }

    fn store_to_local(&mut self, value: MirValueRef) {
        let local = self.ensure_value_local(value);
        self.emit_store_local(local);
    }

    fn emit_load_local(&mut self, local: u16) {
        self.emitter.emit_imm1(OdinHeadCode::LoadLocal, local as i32);
    }

    fn emit_store_local(&mut self, local: u16) {
        self.emitter.emit_imm1(OdinHeadCode::StoreLocal, local as i32);
    }
}

impl BytecodeEmitter<'_> {
    fn emit_const_i32(&mut self, value: i32) {
        let index = self.intern_i32(value);
        self.emit_imm1(OdinHeadCode::Constant, index);
    }

    fn emit_const_i64(&mut self, value: i64) {
        if let Ok(value) = i32::try_from(value) {
            self.emit_const_i32(value);
            self.emit_call_import("i32_to_i64", 1);
            return;
        }
        // 超出 `i32` 表示范围必须用 `Integer64`：`value as i32` + `i32_to_i64`
        // 只保留低 32 位并符号扩展，会把 `i64::MAX` 静默编成 `-1`。
        let index = self.constants.len() as i32 + self.constants_base;
        self.constants.push(OdinConstant::Integer64(value));
        self.emit_imm1(OdinHeadCode::Constant, index);
    }

    fn emit_const_bool(&mut self, value: bool) {
        let index = self.constants.len() as i32 + self.constants_base;
        self.constants.push(OdinConstant::Boolean(value));
        self.emit_imm1(OdinHeadCode::Constant, index);
    }
}

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

/// OVM 私有文本载体桥：raw 宿主串 → `[u8]` 字节数组（不补 NUL、不截断）。
const TEXT_TO_BYTES_IMPORT: &str = "utf8.to_bytes";

/// OVM 私有文本载体桥：`[u8]` 字节数组 → raw 宿主串（不要求 NUL 终止）。
const TEXT_FROM_BYTES_IMPORT: &str = "utf8.from_bytes";

/// `utf8` 槽在 OVM 上的两种物理载体。
///
/// 目标计划用 `value_layouts` / `return_layout` 冻结「这个槽是托管文本对象」；
/// 宿主导入的返回值与字面量仍是 raw 串。只在边界按冻结事实转换，运行期不看
/// `Value` 的实际类型兜底。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TextCarrier {
    /// 宿主 ABI raw 串（`Value::String`）。
    Raw,
    /// 托管文本对象（`Value::Object`），携带计划冻结的布局身份。
    Managed(LayoutId),
}

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
///
/// 位模式重解释（`i64.reinterpret_f64` / `f64.reinterpret_i64`）登记为
/// `I64ReinterpretF64` / `F64ReinterpretI64` 头码：语义是纯位搬运，属于指令层，
/// 不引入宿主函数、不新增 capability；载体按声明签名冻结（`f64 ↔ i64`），不得降级
/// 成数值转换，也不得回流到宿主。
///
/// f32 同样走指令层，且**载体独立**：
///
/// 1. **载体身份**：f32 助记符的操作数/结果载体是 `TargetOperandType::F32`，
///    运行时值只存在于 `Value::F32`；禁止借 `I32` 或 `F64` 槽。
/// 2. **转换规则**：`f32.demote_f64` / `f64.promote_f32` 分别是 `F32DemoteF64` /
///    `F64PromoteF32` 单步头码（RNE / 精确）；本表不冻结隐式拓宽。
/// 3. **运算与位模式**：f32 算术/比较在单精度上做；`i32.reinterpret_f32` /
///    `f32.reinterpret_i32` 是 32 位 `i32` 载体的纯位搬运，NaN payload 23 位保位。
pub(crate) fn freeze_ovm_operator_binding(mnemonic: &str) -> Option<TargetOperatorPlan> {
    // 跨载体助记符显式点名两侧载体；其余既有助记符两侧都是 i32。
    let (operand_type, result_type) = match mnemonic {
        "i64.reinterpret_f64" => (TargetOperandType::F64, TargetOperandType::I64),
        "f64.reinterpret_i64" => (TargetOperandType::I64, TargetOperandType::F64),
        "i32.reinterpret_f32" => (TargetOperandType::F32, TargetOperandType::I32),
        "f32.reinterpret_i32" => (TargetOperandType::I32, TargetOperandType::F32),
        "f64.promote_f32" => (TargetOperandType::F32, TargetOperandType::F64),
        "f32.demote_f64" => (TargetOperandType::F64, TargetOperandType::F32),
        m if m.starts_with("f32.") => (TargetOperandType::F32, TargetOperandType::F32),
        _ => (TargetOperandType::I32, TargetOperandType::I32),
    };
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
        "i64.reinterpret_f64" => (1, TargetOperatorRecipe::Operands, OdinHeadCode::I64ReinterpretF64),
        "f64.reinterpret_i64" => (1, TargetOperatorRecipe::Operands, OdinHeadCode::F64ReinterpretI64),
        "i32.reinterpret_f32" => (1, TargetOperatorRecipe::Operands, OdinHeadCode::I32ReinterpretF32),
        "f32.reinterpret_i32" => (1, TargetOperatorRecipe::Operands, OdinHeadCode::F32ReinterpretI32),
        "f32.add" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::F32Add),
        "f32.sub" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::F32Sub),
        "f32.mul" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::F32Mul),
        "f32.div" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::F32Div),
        "f32.neg" => (1, TargetOperatorRecipe::Operands, OdinHeadCode::F32Neg),
        "f32.eq" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::F32Eq),
        "f32.lt" => (2, TargetOperatorRecipe::Operands, OdinHeadCode::F32Lt),
        "f32.demote_f64" => (1, TargetOperatorRecipe::Operands, OdinHeadCode::F32DemoteF64),
        "f64.promote_f32" => (1, TargetOperatorRecipe::Operands, OdinHeadCode::F64PromoteF32),
        _ => return None,
    };
    // 比较结果固定为 i32 布尔：载体身份由助记符语义决定，不靠默认 i32 蒙混。
    let result_type = match mnemonic {
        "f32.eq" | "f32.lt" => TargetOperandType::I32,
        _ => result_type,
    };
    Some(TargetOperatorPlan { arity, operand_type, result_type, recipe, encoding: TargetOperatorEncoding::Ovm(code) })
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

/// Lower MIR-backed fragment operations into a `.odin` module.
///
/// 只发射模块本身；库导出 ABI 合同由 [`lower_fragment_mir_to_nyar_module_with_abi`]
/// 派生（测试 / 发射形状断言不经过合同，避免把 ABI 失败混进布局断言）。
pub(crate) fn lower_fragment_mir_to_nyar_module(submission: &FragmentSubmission) -> Result<OdinModuleData> {
    lower_fragment_mir_to_nyar_module_impl(submission, false).map(|(module, _)| module)
}

/// 同时派生 OVM 库导出 ABI 合同（生产路径）。
pub(crate) fn lower_fragment_mir_to_nyar_module_with_abi(
    submission: &FragmentSubmission,
) -> Result<(OdinModuleData, Option<crate::ovm_abi::OvmAbiContract>)> {
    lower_fragment_mir_to_nyar_module_impl(submission, true)
}

fn lower_fragment_mir_to_nyar_module_impl(
    submission: &FragmentSubmission,
    derive_abi_contract: bool,
) -> Result<(OdinModuleData, Option<crate::ovm_abi::OvmAbiContract>)> {
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
        let param_layouts_by_function_index = build_nyar_function_param_layouts(exec.as_ref(), &operations);

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
                &param_layouts_by_function_index,
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

    // OVM 库导出 ABI 合同：只由计划冻结的签名 / 布局 / sum 事实派生；任何不支持
    // 的类型都在这里结构化失败，绝不降级成“先跑再说”。只有存在 `[export]` 面
    // （库模式）时才派生；无导出面由驱动层的 library 检查失败闭合。
    let abi_contract = if derive_abi_contract {
        // 库导出 ABI 面必须与 wasm 的 `library_invoke` 同源：只覆盖 `[export]` 公共
        // 导出面。`exported_operations` 还包含被链接进来的泛型 std 实例（例如
        // `std::get#401`），它们的签名尚未实例化、本来就没有可冻结的结构；把
        // 它们拉进 ABI 合同只会让整个模块无法产出，而不是让调用更安全。
        let export_names = nyar_export_name_map(submission);
        let exports = submission
            .backend_plan
            .wasm_export_names()
            .keys()
            .filter_map(|instance| export_names.get(instance).map(|name| (*instance, name.clone())))
            .collect::<Vec<_>>();
        if exports.is_empty() {
            None
        }
        else {
            Some(crate::ovm_abi::derive_contract(
                submission.backend_plan.as_ref(),
                &exports,
                &mut module.layouts,
                &mut layout_index_by_id,
                &sum_layout_index_by_nominal,
            )?)
        }
    }
    else {
        None
    };
    Ok((module, abi_contract))
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
        let sum = sums.get(&nominal).ok_or_else(|| format!("OVM sum 表示缺失：nominal {nominal:?} 没有冻结的 SumTypeLayout"))?;
        if sum.tag_width == 0 || sum.tag_width > 4 {
            return Err(format!("OVM sum 表示只承诺 1..=4 字节 tag（nominal={nominal:?}, tag_width={}）", sum.tag_width));
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
fn build_nyar_function_param_types(exec: &TargetExecutionPlan, operations: &[ItemInstanceId]) -> crate::Result<BTreeMap<i32, Vec<NyarType>>> {
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

/// 被调函数入口形参槽的冻结布局身份表。
///
/// 槽布局有两个已冻结来源：`value_layouts` 只登记按 SSA 值类型（TypeId）能找到
/// 聚合布局的值；文本载体在签名里常被折叠成基类型（`utf8` 形参的 TypeId 是
/// 折叠后的 `NyarType::Utf8`），该 TypeId 本身没有 `aggregate_layout_by_type`
/// 条目，但同一载体布局仍以折叠后类型为键冻结在
/// `aggregate_layout_by_nyar_type`。
///
/// 两处都查不到表示目标计划**没有**为该槽冻结托管布局身份（纯 raw 片段不编译
/// `std.text` 文本聚合，字面量直传），此时 `None` 就是槽的冻结事实：raw 宿主串
/// 槽，不是缺布局错误。
fn build_nyar_function_param_layouts(exec: &TargetExecutionPlan, operations: &[ItemInstanceId]) -> BTreeMap<i32, Vec<Option<LayoutId>>> {
    let mut map = BTreeMap::new();
    for (index, instance) in operations.iter().enumerate() {
        let Some(view) = exec.get_function(instance)
        else {
            continue;
        };
        // 与 OVM ABI 合同共用同一派生点：合同与 lowering 的实际槽布局必须同源，
        // 禁止在这里另写一份回落。
        let layouts = crate::ovm_abi::frozen_entry_parameter_layouts(exec, &view.function);
        map.insert(index as i32, layouts);
    }
    map
}

/// 当前函数内 SSA 值 → 定义指令。载体分类沿 `StoreVar`/`Copy` 链走到源头，
/// 只消费语义 MIR 的冻结定义，不在运行期观察 `Value` 的实际类型。
fn build_nyar_value_definitions<'a>(mir_fn: &'a MirFunction, slots: &ExecutableSlotPlan) -> BTreeMap<MirValueRef, &'a MirInstruction> {
    let mut map = BTreeMap::new();
    for block in &mir_fn.blocks {
        for instruction in &block.instructions {
            if let Some(output) = slots.instruction_output(instruction) {
                map.insert(output, instruction);
            }
        }
    }
    map
}

/// 库模式只导出**声明导出面**（`[export]` 名 ∪ 程序入口）；闭包内辅助函数保持内部 `Call` 可见性。
/// 判据与 wasm 共用同一派生点，禁止按「可达即导出」重算。
fn nyar_should_export_operation(submission: &FragmentSubmission, operation: ItemInstanceId) -> bool {
    crate::lowering::shared::export_surface::contains(&submission.backend_plan, operation)
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

/// 导出面的实例 → 名称映射：公共导出名（`wasm_export_names`）保持原样；
/// abi 派生名在同一导出面绑定多个实例时按实例下标消歧，导出表与入口决议共用同一映射。
/// 集合来源是共享派生点 [`crate::lowering::shared::export_surface`]，内部 callee 不参与。
fn nyar_export_name_map(submission: &FragmentSubmission) -> BTreeMap<ItemInstanceId, String> {
    let plan = &submission.backend_plan;
    let mut bases: BTreeMap<ItemInstanceId, String> = BTreeMap::new();
    for operation in crate::lowering::shared::export_surface::export_surface(plan) {
        bases.entry(operation).or_insert_with(|| nyar_mir_export_name(submission, operation));
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

/// 库导出面的 `(实例身份, 逻辑导出名)`：与模块内 `NyarExport.symbol_name` 同一映射。
///
/// 集合来自共享派生点 [`crate::lowering::shared::export_surface::export_surface`]：
/// `[export]` 冻结名 ∪ 程序入口。可达闭包不是导出面，内部带函数类型形参的辅助函数
/// 因此不会进入 ABI 派生。ABI 合同与 run-contract 入口决议共用这份事实，避免两处各自拼名字。
pub(crate) fn nyar_library_export_pairs(submission: &FragmentSubmission) -> Vec<(ItemInstanceId, String)> {
    let plan = &submission.backend_plan;
    let names = nyar_export_name_map(submission);
    crate::lowering::shared::export_surface::export_surface(plan)
        .into_iter()
        .filter_map(|operation| names.get(&operation).cloned().map(|name| (operation, name)))
        .collect()
}

/// Nyar VM run-contract 入口必须与模块内 `NyarExport.symbol_name` 一致；不能假定裸 `main`。
pub(crate) fn nyar_library_public_export_names(submission: &FragmentSubmission) -> Vec<String> {
    nyar_library_export_pairs(submission).into_iter().map(|(_, name)| name).collect()
}

fn lower_mir_function_to_bytecode(
    submission: &FragmentSubmission,
    mir_fn: &MirFunction,
    function_index_by_instance: &BTreeMap<ItemInstanceId, i32>,
    function_entry_arities: &BTreeMap<i32, usize>,
    param_types_by_function_index: &BTreeMap<i32, Vec<NyarType>>,
    param_layouts_by_function_index: &BTreeMap<i32, Vec<Option<LayoutId>>>,
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
    let value_definitions = build_nyar_value_definitions(mir_fn, &slots);

    let mut lowerer = NyarMirLowerer {
        submission,
        ctx,
        mir_fn,
        slots,
        value_definitions,
        emitter,
        function_index_by_instance,
        function_entry_arities,
        param_types_by_function_index,
        param_layouts_by_function_index,
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
    /// SSA 值 → 定义它的指令；文本载体分类沿定义链判定，不看运行期 `Value` 类型。
    value_definitions: BTreeMap<MirValueRef, &'a MirInstruction>,
    emitter: &'a mut BytecodeEmitter<'e>,
    function_index_by_instance: &'a BTreeMap<ItemInstanceId, i32>,
    function_entry_arities: &'a BTreeMap<i32, usize>,
    param_types_by_function_index: &'a BTreeMap<i32, Vec<NyarType>>,
    param_layouts_by_function_index: &'a BTreeMap<i32, Vec<Option<LayoutId>>>,
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
            MirInstructionKind::LoadFunctionRef { instance, environment, captured } => {
                // 函数值 = 两槽闭包对象：槽 0 = 本模块函数表下标（code i32），
                // 槽 1 = 捕获环境聚合或 null。跨模块/导入实例没有稳定下标，
                // 只能结构化失败：不回落符号、不猜，也不把闭包降回裸 i32。
                let Some(index) = self.function_index_by_instance.get(instance).copied()
                else {
                    return Err(format!(
                        "OVM 闭包载体要求 instance 属于本模块函数表（跨模块/导入函数值未支持）：{instance:?}（symbol=`{}`）",
                        self.mir_fn.symbol
                    ));
                };
                let Some(output) = output
                else {
                    return Err(format!(
                        "OVM 闭包载体必须有结果槽：instance={instance:?} captured=[{}]（symbol=`{}`）",
                        captured.join(", "),
                        self.mir_fn.symbol
                    ));
                };
                let object = MirOperand::Value(output);
                let layout = self.ensure_nyar_layout_for_field_count(2);
                self.emitter.emit_imm1(OdinHeadCode::ObjectNew, layout);
                self.store_to_local(output);
                // 槽 0：code 下标。Odin 槽无类型，常量位宽由这里固定为 i32。
                self.emit_field_set_slot(
                    &object,
                    0,
                    &MirOperand::Constant(MirConstant::Int(index as i64)),
                    Some(&NyarType::Integer32 { signed: true }),
                )?;
                self.emitter.emit_plain(OdinHeadCode::Pop);
                // 槽 1：环境聚合或显式 null（无捕获也必须仍是闭包对象）。
                match environment {
                    Some(environment) => {
                        let environment_ty = match environment {
                            MirOperand::Value(value) => self.value_semantic_type(*value),
                            _ => None,
                        };
                        self.emit_field_set_slot(&object, 1, environment, environment_ty.as_ref())?;
                    }
                    None => {
                        self.emit_operand(&object)?;
                        self.emitter.emit_const_null();
                        self.emitter.emit_imm1(OdinHeadCode::FieldSet, 1);
                    }
                }
                self.emitter.emit_plain(OdinHeadCode::Pop);
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
            MirInstructionKind::Cast { source, ty } => {
                // 源类型只从 semantic MIR 的冻结表读取（常量位宽回落同一合同），
                // 不按数值大小或目标类型反推。
                let source_ty = self.operand_semantic_type(source)?;
                self.emit_operand_typed(source, Some(&source_ty))?;
                self.emit_cast_conversion(&source_ty, ty)?;
                if let Some(output) = output {
                    self.store_to_local(output);
                }
                else {
                    self.emitter.emit_plain(OdinHeadCode::Pop);
                }
            }
            MirInstructionKind::StructNew { nominal, fields } => {
                let layout = self
                    .ctx
                    .layout_by_nominal(*nominal)
                    .cloned()
                    .ok_or_else(|| format!("OVM 聚合身份缺少布局合同：nominal {nominal:?} 不在计划的 AggregateLayoutPlan"))?;
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
            MirInstructionKind::TupleGet { object, layout_id, index } => {
                let layout =
                    self.ctx.layout_by_id(*layout_id).cloned().ok_or_else(|| {
                        format!("OVM TupleGet 布局身份缺少布局合同：layout_id={layout_id:?}（symbol=`{}`）", self.mir_fn.symbol)
                    })?;
                if (*index as usize) >= layout.fields.len() {
                    return Err(format!(
                        "OVM TupleGet 位置索引 {index} 越界：layout `{}` 长度 {}（symbol=`{}`）",
                        layout.name,
                        layout.fields.len(),
                        self.mir_fn.symbol
                    ));
                }
                let slot = i32::try_from(*index)
                    .map_err(|_| format!("OVM TupleGet 位置索引 {index} 超出 i32 槽合同（symbol=`{}`）", self.mir_fn.symbol))?;
                // 目标对象按位置槽读取；标签只在类型身份层，不能作为 OVM 槽位来源。
                self.emit_operand(object)?;
                self.emitter.emit_imm1(OdinHeadCode::FieldGet, slot);
                if let Some(output) = output {
                    self.store_to_local(output);
                }
                else {
                    self.emitter.emit_plain(OdinHeadCode::Pop);
                }
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
                // 函数值间接调用：callee 是携带冻结 `Function` 签名的 SSA 值，
                // 运行期目标是槽里的函数索引；实参位宽由签名决定，绝不按字面量猜。
                let MirOperand::Value(callee_value) = callee
                else {
                    return Err(format!("OVM Call 的 callee 既不是可调用实例也不是函数值：{callee:?}（symbol=`{}`）", self.mir_fn.symbol));
                };
                let semantic = self.value_semantic_type(*callee_value);
                let Some(NyarType::Function(signature)) = semantic.clone()
                else {
                    let actual = semantic.as_ref().map(ToString::to_string).unwrap_or_else(|| "<未冻结>".to_string());
                    return Err(format!(
                        "OVM 间接调用要求 callee 携带冻结 Function 签名，实际类型 {actual}（symbol=`{}`）",
                        self.mir_fn.symbol
                    ));
                };
                if arguments.len() != signature.params.len() {
                    return Err(format!(
                        "OVM 间接调用实参个数 {} 与冻结签名形参个数 {} 不符（symbol=`{}`）",
                        arguments.len(),
                        signature.params.len(),
                        self.mir_fn.symbol
                    ));
                }
                // 先压实参、再压环境槽、最后压 code：`CallIndirect` 只弹 callee，
                // 实参与环境槽留在栈上；Odin 没有 swap，顺序错了取不出来。
                // 环境槽是合成 lambda 的尾形参，因此 argc = 可见实参 + 1。
                for (index, argument) in arguments.iter().enumerate() {
                    let param_ty = signature.params.get(index);
                    match argument {
                        MirOperand::Value(_) => {
                            if !self.emit_call_operand(argument)? {
                                return Err(format!("OVM 间接调用实参缺少预规划槽：{argument:?}（symbol=`{}`）", self.mir_fn.symbol));
                            }
                        }
                        _ => self.emit_operand_typed(argument, param_ty)?,
                    }
                }
                // 闭包载体两槽：0 = code i32 函数下标，1 = env 聚合对象或 null。
                self.emit_operand_typed(callee, None)?;
                self.emitter.emit_imm1(OdinHeadCode::FieldGet, 1);
                self.emit_operand_typed(callee, None)?;
                self.emitter.emit_imm1(OdinHeadCode::FieldGet, 0);
                self.emitter.emit_imm1(OdinHeadCode::CallIndirect, arguments.len() as i32 + 1);
                if let Some(output) = output {
                    self.store_to_local(output);
                }
                else {
                    self.emitter.emit_plain(OdinHeadCode::Pop);
                }
                return Ok(());
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
                // 数组元素槽是嵌套文本槽：托管实参先抽取成 raw 再入槽。
                if value_ty.as_ref() == Some(&NyarType::Utf8) {
                    self.emit_text_boundary_operand(value, &NyarType::Utf8, None, "数组元素槽")?;
                }
                else {
                    self.emit_operand_typed(value, value_ty.as_ref())?;
                }
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
            // fail-closed：未实现的指令必须报错，绝不静默吞掉（静默会让输出槽保持
            // `Value::Null`，把编译期错误推迟成运行期错值）。
            kind => {
                return Err(format!("OVM 后端未实现指令 {kind:?}（symbol=`{}`）", self.mir_fn.symbol));
            }
        }
        Ok(())
    }

    fn emit_terminator(&mut self, block: &MirBlock) -> crate::Result<()> {
        match &block.terminator {
            MirTerminator::Return { value } => {
                if let Some(value) = value {
                    // 返回槽类型是函数签名的冻结事实；常量必须按它选位宽。
                    let return_ty = self.mir_fn.return_type.clone();
                    if matches!(return_ty, NyarType::Utf8) {
                        let Some(return_layout) = self.mir_fn.return_layout
                        else {
                            return Err(format!(
                                "OVM 文本返回值缺少目标计划冻结的 `return_layout`（函数 `{}`，返回类型 `{return_ty}`）：\
                                 `utf8` 返回值必须按冻结布局物化，禁止把宿主 raw 串直接逃逸出函数边界",
                                self.mir_fn.symbol
                            ));
                        };
                        self.emit_text_boundary_operand(value, &return_ty, Some(return_layout), "返回值")?;
                    }
                    else {
                        self.emit_operand_typed(value, Some(&return_ty))?;
                    }
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
                        format!("OVM StateDispatch 的 case_key {} 超出 i32 表示范围（symbol=`{}`）", *case_key, self.mir_fn.symbol)
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
                return Err(format!(
                    "Nyar 目标不支持外部调用 `{callee_name}`：链接合同 {link}（contract/tag）不是已绑定的目标 provider；\
                     需要 host 边界 + 目标标签 `ovm-operator` + 恰好一段符号，未绑定目标 provider 不得回落"
                ));
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
    fn emit_external_arguments(&mut self, callee_name: &str, instance: ItemInstanceId, arguments: &[MirOperand]) -> crate::Result<()> {
        let parameter_types = self.external_param_types_by_instance.get(&instance).cloned();
        let import_symbol = self.external_link_by_instance.get(&instance).map(|link| link.locator_segments().join("::")).unwrap_or_default();
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
            if matches!(expected, NyarType::Utf8) {
                let site = format!("OVM 外部调用 `{callee_name}`（import={import_symbol} instance={instance:?}）的实参 #{index}");
                self.emit_text_boundary_operand(argument, &expected, None, &site)?;
                continue;
            }
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
            let param_ty = self.param_types_by_function_index.get(&function_index).and_then(|types| types.get(index)).cloned();
            if param_ty.as_ref() == Some(&NyarType::Utf8) {
                let slot_layout =
                    self.param_layouts_by_function_index.get(&function_index).and_then(|layouts| layouts.get(index)).copied().flatten();
                // 入口槽没有布局身份，只有目标计划本身也没冻结任何 `utf8` 聚合布局
                // 才是合法的 raw 槽事实（纯 raw 片段：字面量直传，不编译文本载体）。
                // 计划已登记 `Utf8` 布局而本槽解析不到，说明查表链断了，必须失败关闭。
                if slot_layout.is_none() && self.ctx.submission.backend_plan.aggregate_layout_by_nyar_type().contains_key(&NyarType::Utf8) {
                    return Err(format!(
                        "OVM 内部调用 `{path:?}`（function index={function_index}）的文本实参 #{index} 未能解析入口槽的冻结 layout：\
                         目标计划已登记 `utf8` 聚合布局身份，同一签名槽必须解析到该身份"
                    ));
                }
                let site = format!("内部调用 `{path:?}` 实参 #{index}");
                self.emit_text_boundary_operand(argument, &NyarType::Utf8, slot_layout, &site)?;
                continue;
            }
            match argument {
                MirOperand::Value(_) => {
                    if !self.emit_call_operand(argument)? {
                        panic!("unmaterialized Nyar call operand in `{path:?}`: {argument:?}");
                    }
                }
                _ => self.emit_operand_typed(argument, param_ty.as_ref())?,
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
        // 嵌套槽是 raw 宿主串：托管文本实参入槽前必须抽取成 raw，与读取侧
        // `value_text_carrier_at_depth` 对嵌套读取按 raw 的分类同源；否则字段槽
        // 里混入托管对象，读取方按 raw 解释就会在运行期炸出类型错。
        if value_ty == Some(&NyarType::Utf8) {
            self.emit_text_boundary_operand(value, &NyarType::Utf8, None, "嵌套文本槽")?;
        }
        else {
            self.emit_operand_typed(value, value_ty)?;
        }
        self.emitter.emit_imm1(OdinHeadCode::FieldSet, field_slot);
        Ok(())
    }

    fn emit_block_argument_copies(&mut self, target: MirBlockRef, arguments: &[MirOperand]) -> crate::Result<()> {
        let target_block = self.mir_fn.blocks.iter().find(|block| block.id == target).expect("validated branch target");
        assert_eq!(arguments.len(), target_block.parameters.len(), "validated branch arity");
        for (index, argument) in arguments.iter().enumerate() {
            let param_local = self.slots.block_param_locals[&(target, index)];
            let param_ty = self.slots.local_types[usize::from(param_local)].clone();
            // SSA 合并块参数是文本载体边界的一种：值位置 `if` 的一臂给 raw 字面量、另一臂给
            // 拼接产出的托管文本对象时，块参数槽按目标计划冻结的 layout 决定其物理载体。与
            // `Return`／内部调用实参共用 `emit_text_boundary_operand`，让 raw 入边物化为托管对象，
            // 否则下游 `self._repr` 字段访问会以 `expected object, got string` 崩溃。
            if matches!(param_ty, NyarType::Utf8) {
                let slot_layout = self.mir_fn.value_layouts.get(&target_block.parameters[index]).copied();
                self.emit_text_boundary_operand(argument, &param_ty, slot_layout, "块参数")?;
            }
            else {
                self.emit_operand_typed(argument, Some(&param_ty))?;
            }
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
                // 窄整数（`u8`/`i8`/`u16`/`i16`）与字符码位槽在 OVM 里以 i32 载体承载；
                // 位宽只影响后续转换，这里按 i32 常量发射（不得发 i64，运行期会拒绝）。
                Some(other) if ovm_int_shape(other).is_some_and(|shape| shape.bits <= 32) => {
                    let narrowed = i32::try_from(*value)
                        .map_err(|_| format!("OVM 整数常量 {value} 超出目标 {other} 槽的 i32 载体范围；symbol=`{}`", self.mir_fn.symbol))?;
                    self.emitter.emit_const_i32(narrowed);
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
            MirConstant::Utf16(_) => {
                return Err("OVM 常量 Utf16 缺少显式 UTF-16 文本 ABI 合同".to_string());
            }
            // OVM 目标没有可空空引用的 ABI 合同；不得用 i32 0 冒充 null。
            // 缺合同必须在 backend 边界结构化失败，不得 panic 击穿进程。
            MirConstant::Null => {
                return Err("OVM 常量 Null 缺少可空引用 ABI 合同；禁止用 i32 0 冒充 null".to_string());
            }
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
                let local = self
                    .slots
                    .value_locals
                    .get(value)
                    .copied()
                    .ok_or_else(|| format!("OVM SSA 值缺少预规划槽：%{}（symbol=`{}`）", value.0, self.mir_fn.symbol))?;
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
        self.slots.value_locals.get(&value).and_then(|local| self.slots.local_types.get(usize::from(*local))).cloned()
    }

    /// SSA 值的语义类型（操作数位常量回落用的同一张冻结表）。
    fn value_semantic_type(&self, value: MirValueRef) -> Option<NyarType> {
        self.mir_fn.value_types.get(&value).cloned()
    }

    /// 该 SSA 值是否按 `utf8` 文本语义使用。
    fn value_is_text(&self, value: MirValueRef) -> bool {
        matches!(self.value_semantic_type(value), Some(NyarType::Utf8))
    }

    /// 文本操作数的冻结载体；非文本操作数返回 `None`。
    ///
    /// 分类只消费语义 MIR 定义链与目标计划的 `value_layouts`，不看运行期
    /// `Value` 的实际类型。
    fn operand_text_carrier(&self, operand: &MirOperand) -> Option<TextCarrier> {
        self.operand_text_carrier_at_depth(operand, 0)
    }

    fn operand_text_carrier_at_depth(&self, operand: &MirOperand, depth: u32) -> Option<TextCarrier> {
        match operand {
            MirOperand::Value(value) => self.value_text_carrier_at_depth(*value, depth),
            MirOperand::Constant(MirConstant::Utf8(_)) => Some(TextCarrier::Raw),
            _ => None,
        }
    }

    /// 沿 SSA 定义链判定文本载体：
    /// - 字面量与宿主导入输出是 raw（宿主 ABI 形态）；
    /// - `StoreVar`/`Copy` 只跟源，本相不在槽内改载体；
    /// - 其余带冻结 `value_layouts` 的文本值是托管对象，缺布局则视为 raw。
    fn value_text_carrier_at_depth(&self, value: MirValueRef, depth: u32) -> Option<TextCarrier> {
        if !self.value_is_text(value) || depth > 8 {
            return None;
        }
        if let Some(instruction) = self.value_definitions.get(&value) {
            match &instruction.kind {
                MirInstructionKind::LoadConstant { constant: MirConstant::Utf8(_), .. } => return Some(TextCarrier::Raw),
                MirInstructionKind::StoreVar { value: source, .. } | MirInstructionKind::Copy { source } => {
                    return self.operand_text_carrier_at_depth(source, depth + 1);
                }
                MirInstructionKind::Call { callee: MirOperand::Item(instance), .. }
                    if self.external_link_by_instance.contains_key(instance) =>
                {
                    return Some(TextCarrier::Raw);
                }
                // 嵌套槽（聚合字段 / 元组位 / 数组元素 / sum 载荷）在 OVM ABI 里冻结为
                // raw 宿主串（`ovm_abi::Projection::project_text` 的 `TextSlot::Nested`
                // 显式落 raw，绝不回查全局布局）：读取必须按 raw 分类，不得因为槽类型
                // 带聚合布局就把运行期字符串当成托管文本对象透传。
                MirInstructionKind::FieldGet { .. } | MirInstructionKind::TupleGet { .. } | MirInstructionKind::ArrayGet { .. }
                | MirInstructionKind::SumPayloadGet { .. } => return Some(TextCarrier::Raw),
                _ => {}
            }
        }
        Some(self.mir_fn.value_layouts.get(&value).copied().map(TextCarrier::Managed).unwrap_or(TextCarrier::Raw))
    }

    /// 在文本边界（内部调用实参 / `Return` / 宿主导入实参）按冻结事实发射。
    ///
    /// `slot_layout = Some(layout)` 表示目标槽是托管对象：raw 必须物化为该布局，
    /// 已托管对象必须与该布局同一身份。`None` 表示目标槽是宿主 raw：托管对象必须
    /// 抽取成 raw 串。
    fn emit_text_boundary_operand(
        &mut self,
        operand: &MirOperand,
        expected: &NyarType,
        slot_layout: Option<LayoutId>,
        site: &str,
    ) -> crate::Result<()> {
        if !matches!(expected, NyarType::Utf8) {
            return self.emit_operand_typed(operand, Some(expected));
        }
        let Some(carrier) = self.operand_text_carrier(operand)
        else {
            return Err(format!(
                "OVM 文本边界要求 `utf8` 操作数，实际类型 {}（{site}，symbol=`{}`）：\
                 raw 宿主串与文本对象只在冻结的 `utf8` 载体边界上互转，禁止按名字或运行期类型猜探",
                self.operand_actual_type(operand),
                self.mir_fn.symbol
            ));
        };
        match (slot_layout, carrier) {
            (Some(target), TextCarrier::Raw) => self.emit_text_materialize(target, operand, site),
            (Some(target), TextCarrier::Managed(actual)) if actual == target => self.emit_text_raw_operand(operand, site),
            (Some(target), TextCarrier::Managed(actual)) => Err(format!(
                "OVM 文本载体布局不匹配：{} 的冻结 layout 是 {actual:?}，目标槽冻结 layout 是 {target:?}（{site}，symbol=`{}`）；\
                 不同文本布局不得跨边界直接传递",
                self.operand_actual_type(operand),
                self.mir_fn.symbol
            )),
            (None, TextCarrier::Managed(_)) => self.emit_text_extract(operand, site),
            (None, TextCarrier::Raw) => self.emit_text_raw_operand(operand, site),
        }
    }

    /// 发射 raw 载体操作数（Value 读槽，常量按 `Utf8` 发射）。
    fn emit_text_raw_operand(&mut self, operand: &MirOperand, _site: &str) -> crate::Result<()> {
        self.emit_operand_typed(operand, Some(&NyarType::Utf8))
    }

    /// raw → 托管：`ObjectNew(layout)` → raw 串 → `utf8.to_bytes` → `FieldSet 0`。
    fn emit_text_materialize(&mut self, layout_id: LayoutId, operand: &MirOperand, site: &str) -> crate::Result<()> {
        let layout = self.ctx.layout_by_id(layout_id).cloned().ok_or_else(|| {
            format!(
                "OVM 文本载体 layout id={layout_id:?} 不在目标计划的布局表（{site}，symbol=`{}`）：\
                 物化配方必须来自冻结布局，禁止猜测物理槽形状",
                self.mir_fn.symbol
            )
        })?;
        let layout_index = self.ensure_text_repr_layout(&layout, site)?;
        self.emitter.emit_imm1(OdinHeadCode::ObjectNew, layout_index);
        self.emit_text_raw_operand(operand, site)?;
        self.emitter.emit_call_import(TEXT_TO_BYTES_IMPORT, 1);
        self.emitter.emit_imm1(OdinHeadCode::FieldSet, 0);
        Ok(())
    }

    /// 托管 → raw：操作数 → `FieldGet 0` → `utf8.from_bytes`。
    fn emit_text_extract(&mut self, operand: &MirOperand, site: &str) -> crate::Result<()> {
        self.emit_text_raw_operand(operand, site)?;
        self.emitter.emit_imm1(OdinHeadCode::FieldGet, 0);
        self.emitter.emit_call_import(TEXT_FROM_BYTES_IMPORT, 1);
        Ok(())
    }

    /// 校验并登记文本布局：唯一冻结配方是单个 `[u8]` 字段槽，返回 OVM 布局下标。
    fn ensure_text_repr_layout(&mut self, layout: &AggregateLayout, site: &str) -> crate::Result<i32> {
        let is_byte_slice = matches!(
            layout.fields.as_slice(),
            [field]
                if matches!(
                    &field.ty,
                    NyarType::Array(element) | NyarType::FixedArray { element, .. }
                        if matches!(element.as_ref(), NyarType::Integer8 { signed: false })
                )
        );
        if !is_byte_slice {
            let shape = layout.fields.iter().map(|field| format!("{}:{}", field.name, field.ty)).collect::<Vec<_>>().join(", ");
            return Err(format!(
                "OVM 文本载体 layout `{}`（id={:?}）缺少冻结的单一 `[u8]` 槽配方：实际 fields=[{shape}]（{site}，symbol=`{}`）；\
                 禁止用非冻结形状物化或抽取文本",
                layout.name, layout.id, self.mir_fn.symbol
            ));
        }
        Ok(self.ensure_nyar_layout(layout))
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

    /// 操作数的 semantic MIR 冻结类型。
    ///
    /// `Value` 读 [`Self::value_semantic_type`]；常量回落 `semantic_mir_contract`
    /// 的同一张合同（`Int → Integer64{signed}` 等），符号/实例身份没有源类型。
    fn operand_semantic_type(&self, operand: &MirOperand) -> crate::Result<NyarType> {
        match operand {
            MirOperand::Value(value) => self
                .value_semantic_type(*value)
                .ok_or_else(|| format!("OVM `Cast` 源 SSA 值缺少冻结语义类型：%{}（symbol=`{}`）", value.0, self.mir_fn.symbol)),
            MirOperand::Constant(constant) => match constant {
                MirConstant::Int(_) => Ok(NyarType::Integer64 { signed: true }),
                MirConstant::Float64(_) => Ok(NyarType::Float64),
                MirConstant::Bool(_) => Ok(NyarType::Boolean),
                MirConstant::Utf8(_) => Ok(NyarType::Utf8),
                MirConstant::Utf16(_) => Ok(NyarType::Utf16),
                MirConstant::Unit => Ok(NyarType::Unit),
                MirConstant::Null => Err(format!("OVM `Cast` 的空值操作数没有源类型，无法确定转换语义（symbol=`{}`）", self.mir_fn.symbol)),
            },
            MirOperand::Symbol(path) => Err(format!("OVM `Cast` 源操作数不支持符号引用 {path:?}（symbol=`{}`）", self.mir_fn.symbol)),
            MirOperand::Item(instance) => {
                Err(format!("OVM `Cast` 源操作数不支持 callable 身份 {instance:?}（symbol=`{}`）", self.mir_fn.symbol))
            }
        }
    }
    /// `as` 的表示转换：只使用「源有符号性 + 目标位宽」的通用规则，不按类型名特判。
    ///
    /// 整数↔整数（含 `Character` 的 32 位无符号码位）、整数↔`f64` 与 `f32`↔`f64`
    /// 有确定语义；布尔/文本↔数值与整数↔`f32` 没有目标合同，必须结构化拒绝，
    /// 禁止编造近似转换或借另一宽度载体中转。
    fn emit_cast_conversion(&mut self, source_ty: &NyarType, target_ty: &NyarType) -> crate::Result<()> {
        if source_ty == target_ty {
            return Ok(());
        }
        // 浮点宽度之间的转换是显式单步头码，不是载体恒等：
        // `f64 → f32` 用 `F32DemoteF64`（RNE，溢出 `inf`、下溢 0/次正规）；
        // `f32 → f64` 用 `F64PromoteF32`（精确无损）。两者都不经过其他精度。
        match (source_ty, target_ty) {
            (NyarType::Float32, NyarType::Float64) => {
                self.emitter.emit_plain(OdinHeadCode::F64PromoteF32);
                return Ok(());
            }
            (NyarType::Float64, NyarType::Float32) => {
                self.emitter.emit_plain(OdinHeadCode::F32DemoteF64);
                return Ok(());
            }
            // 整数↔f32 的 `as` 需要独立的转换指令/宿主合同，本轮未冻结；
            // 必须响亮失败，不得先转 f64 再降（double rounding）或借 i32 载体冒充。
            (_, NyarType::Float32) => {
                return Err(format!(
                    "OVM 后端没有 `{source_ty}` → `f32` 的确定转换语义（symbol=`{}`）：\
                     整数→`f32` 的精确单步转换尚未冻结，禁止先转 `f64` 再降回（double rounding）",
                    self.mir_fn.symbol
                ));
            }
            (NyarType::Float32, _) => {
                return Err(format!(
                    "OVM 后端没有 `f32` → `{target_ty}` 的确定转换语义（symbol=`{}`）：\
                     `f32` 是独立载体，禁止借 `f64` 中转或按 i32 位模式冒充数值转换",
                    self.mir_fn.symbol
                ));
            }
            _ => {}
        }
        if matches!(target_ty, NyarType::Float64) {
            if let Some(source) = ovm_int_shape(source_ty) {
                self.emit_int_to_f64(source);
                return Ok(());
            }
        }
        if matches!(source_ty, NyarType::Float64) {
            if let Some(target) = ovm_int_shape(target_ty) {
                self.emit_f64_to_int(target);
                return Ok(());
            }
        }
        let (Some(source), Some(target)) = (ovm_int_shape(source_ty), ovm_int_shape(target_ty))
        else {
            return Err(format!(
                "OVM 后端没有 `{source_ty}` → `{target_ty}` 的确定转换语义（symbol=`{}`）：\
                 当前只冻结整数↔整数（含 `Character`）、整数↔`f64` 与 `f32`↔`f64` 的位宽/符号/截断规则；\
                 布尔/文本与数值之间的 `as` 缺少目标合同，不得退化为近似转换",
                self.mir_fn.symbol
            ));
        };
        self.emit_int_cast_steps(source, target);
        Ok(())
    }

    /// 整数 → `f64` 的落地序列。
    ///
    /// 无符号窄整数先在 `i32` 载体上规范化（掩码成非负），再按符号性选择
    /// `i32_to_f64` / `u32_to_f64`，64 位同理选择 `i64_to_f64` / `u64_to_f64`。
    /// 有符号与无符号必须分开：混用会让 `u64` 的 2^63 以上值静默变成负数。
    fn emit_int_to_f64(&mut self, source: OvmIntShape) {
        if source.bits < 32 {
            self.emit_i32_canonicalize(source);
        }
        let symbol = match (source.bits == 64, source.signed) {
            (true, true) => "i64_to_f64",
            (true, false) => "u64_to_f64",
            (false, true) => "i32_to_f64",
            (false, false) => "u32_to_f64",
        };
        self.emitter.emit_call_import(symbol, 1);
    }

    /// `f64` → 整数的落地序列：checked 转换（朝零截断，NaN / 越界结构化失败），
    /// 窄目标再按目标位宽规范化（与整数收窄的位截断口径一致）。
    fn emit_f64_to_int(&mut self, target: OvmIntShape) {
        let symbol = match (target.bits == 64, target.signed) {
            (true, true) => "f64_to_i64_checked",
            (true, false) => "f64_to_u64_checked",
            (false, true) => "f64_to_i32_checked",
            (false, false) => "f64_to_u32_checked",
        };
        self.emitter.emit_call_import(symbol, 1);
        if target.bits < 32 {
            self.emit_i32_canonicalize(target);
        }
    }

    /// 整数表示转换的落地指令序列。
    ///
    /// 位宽 ≤ 32 的值以 `i32` 为载体（窄整数必须处于自身类型的规范形态），
    /// 位宽 64 以 `i64` 为载体。规则只看源的符号性与两端位宽：
    /// 拓宽按源符号性做符号/零扩展，收窄按目标位宽截断，等宽换符号性保持位模式。
    fn emit_int_cast_steps(&mut self, source: OvmIntShape, target: OvmIntShape) {
        if source.bits < 32 {
            self.emit_i32_canonicalize(source);
        }
        if target.bits == 64 {
            if source.bits == 64 {
                // `u64 ↔ i64`：载体与位模式不变，只有解释变。
            }
            else if source.bits == 32 && !source.signed {
                self.emit_u32_to_i64();
            }
            else {
                // 源已处于规范形态：有符号按符号扩展，窄无符号值非负，符号扩展即零扩展。
                self.emitter.emit_call_import("i32_to_i64", 1);
            }
        }
        else {
            if source.bits == 64 {
                self.emit_i64_truncate_to_i32();
            }
            if target.bits < 32 {
                self.emit_i32_canonicalize(target);
            }
        }
    }

    /// 把 `i32` 载体上的值按 8/16 位宽规范化：有符号做符号扩展，无符号做掩码截断。
    fn emit_i32_canonicalize(&mut self, shape: OvmIntShape) {
        debug_assert!(shape.bits == 8 || shape.bits == 16);
        let shift = 32 - i32::from(shape.bits);
        if shape.signed {
            // `(x << (32-b)) >>s (32-b)`：先保留低 b 位，再按第 b-1 位符号扩展。
            self.emitter.emit_const_i32(shift);
            self.emitter.emit_plain(OdinHeadCode::I32Shl);
            self.emitter.emit_const_i32(shift);
            self.emitter.emit_plain(OdinHeadCode::I32ShrS);
        }
        else {
            let mask = (1i32 << shape.bits) - 1;
            self.emitter.emit_const_i32(mask);
            self.emitter.emit_plain(OdinHeadCode::I32And);
        }
    }

    /// `i64` → `i32` 的位截断。
    ///
    /// 宿主的 `i64_to_i32` 带范围校验（溢出即结构化失败），直接调用会把 `as i32`
    /// 的位截断误编成运行期错误。先 `(x << 32) >>s 32` 把低 32 位符号扩展进
    /// `i32` 表示范围，再走范围校验必定成功的 `i64_to_i32`。
    fn emit_i64_truncate_to_i32(&mut self) {
        self.emitter.emit_const_i64(32);
        self.emitter.emit_call_import("i64_shl", 2);
        self.emitter.emit_const_i64(32);
        self.emitter.emit_call_import("i64_shr", 2);
        self.emitter.emit_call_import("i64_to_i32", 1);
    }

    /// `u32` → `i64` 的零扩展。
    ///
    /// `i32_to_i64` 恒为符号扩展；`u32` 值 ≥ 2^31 时以负数落在 `i32` 载体上，
    /// 半程必须补 2^32。无分支配方：`sext(x) + ((sext(x) >>>u 63) * 2^32)`。
    fn emit_u32_to_i64(&mut self) {
        self.emitter.emit_call_import("i32_to_i64", 1);
        self.emitter.emit_plain(OdinHeadCode::Dup);
        self.emitter.emit_const_i64(63);
        self.emitter.emit_call_import("i64_shr_u", 2);
        self.emitter.emit_const_i64(1i64 << 32);
        self.emitter.emit_call_import("i64_mul", 2);
        self.emitter.emit_call_import("i64_add", 2);
    }
}

/// 整数表示形状：只携带位宽与符号性，`Character` 视为 32 位无符号码位。
///
/// 转换规则必须由这两项决定，不得按 `NyarType` 变体名逐条特判。
#[derive(Clone, Copy)]
struct OvmIntShape {
    bits: u16,
    signed: bool,
}

fn ovm_int_shape(ty: &NyarType) -> Option<OvmIntShape> {
    match ty {
        NyarType::Integer8 { signed } => Some(OvmIntShape { bits: 8, signed: *signed }),
        NyarType::Integer16 { signed } => Some(OvmIntShape { bits: 16, signed: *signed }),
        NyarType::Integer32 { signed } => Some(OvmIntShape { bits: 32, signed: *signed }),
        NyarType::Integer64 { signed } => Some(OvmIntShape { bits: 64, signed: *signed }),
        NyarType::Character => Some(OvmIntShape { bits: 32, signed: false }),
        _ => None,
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

    /// 压入 `null`：无捕获闭包的环境槽需要显式空引用，不能用 `i32 0` 冒充。
    fn emit_const_null(&mut self) {
        let index = self.constants.len() as i32 + self.constants_base;
        self.constants.push(OdinConstant::Null);
        self.emit_imm1(OdinHeadCode::Constant, index);
    }
}

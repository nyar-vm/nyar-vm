//! CLR 最小 MIR → `MSIL` 模块降低。
//!
//! 只消费计划已经冻结的标量事实：整型/浮点常量、SSA local、冻结 operator、
//! 块标签与条件分支、同片段 `call` 以及 `Return`。聚合、数组、sum、宿主导入等
//! 形状在这里返回结构化错误，不补造指令、默认值或空体方法。
//!
//! 与 JVM 不同，CLR 的局部变量与方法形参是两套编号：方法入口必须先把 `ldarg`
//! 写入 entry 块的参数局部，否则 `ldloc` 读到未初始化槽。块跳转按目标块参数
//! 局部逆序写回，比较 operator 是单 opcode（`ceq`/`clt`/`cgt`），不展开分支。

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    backend_plan_views::{
        ExecutableBlock as MirBlock, ExecutableBlockRef as MirBlockRef, ExecutableFunction as MirFunction, ExecutableInstruction as MirInstruction,
        ExecutableValueRef as MirValueRef, NyarType,
    },
    contracts::{Constant, InstructionKind, Operand, Terminator},
    lowering::shared::executable::{block_label, collect_reachable_blocks, slots::ExecutableSlotPlan},
    target_operator::{TargetOperandStep, TargetOperandType, TargetOperatorBinding, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe},
    transitional::msil::{
        MsilAssembly, MsilInstruction, MsilInstructionOperand, MsilMethodBody, MsilMethodRef, MsilMethodSignature, MsilModule, MsilOpcode, MsilType,
    },
    FragmentSubmission, Result,
};
use nyar_types::ItemInstanceId;

/// CLR 栈上标量载体类别：`int32` 覆盖 bool/char/窄整型，比较结果与布尔同槽。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClrValueKind {
    /// `int32` 载体。
    Int32,
    /// `int64` 载体。
    Int64,
    /// `float32` 载体。
    Float32,
    /// `float64` 载体。
    Float64,
    /// 引用载体（字符串等）。
    Reference,
}

impl ClrValueKind {
    fn of_operand_type(operand_type: TargetOperandType) -> Self {
        match operand_type {
            TargetOperandType::I32 => Self::Int32,
            TargetOperandType::I64 => Self::Int64,
            TargetOperandType::F64 => Self::Float64,
        }
    }

    fn of_msil_type(ty: &MsilType) -> Result<Self> {
        Ok(match ty {
            MsilType::Void => return Err("`void` 不能作为局部变量或操作数载体".to_string()),
            MsilType::Bool
            | MsilType::Char
            | MsilType::Int8 { .. }
            | MsilType::Int16 { .. }
            | MsilType::Int32 { .. }
            | MsilType::IntPtr { .. } => Self::Int32,
            MsilType::Int64 { .. } => Self::Int64,
            MsilType::Float32 => Self::Float32,
            MsilType::Float64 => Self::Float64,
            MsilType::String | MsilType::Object | MsilType::SzArray(_) | MsilType::Named(_) => Self::Reference,
        })
    }
}

/// 函数级 CLR 方法计划：稳定名字与冻结签名。
#[derive(Debug, Clone)]
struct ClrMethodPlan {
    name: String,
    signature: MsilMethodSignature,
}

/// 将片段降低为单个 `MSIL` 模块。
///
/// 只降低导出面（`wasm_export_names` / `exported_operations` / 入口操作）以及
/// 从它们出发、经同片段静态调用可达的函数；其余函数不进入模块，避免为不可达
/// 形状发明载体。
pub(crate) fn lower_fragment_to_clr_module(submission: &FragmentSubmission) -> Result<MsilModule> {
    let plan = &submission.backend_plan;
    let operations = collect_clr_operations(submission);
    if operations.is_empty() {
        return Err("CLR 最小 lowering 没有可导出的可执行函数：片段既无 `wasm_export_names` 也无入口操作，拒绝凭空生成模块".to_string());
    }

    let mut used_names = BTreeMap::<String, ()>::new();
    let mut method_plans = BTreeMap::<ItemInstanceId, ClrMethodPlan>::new();
    for (ordinal, instance) in operations.iter().enumerate() {
        let view = plan
            .get_function(instance)
            .ok_or_else(|| format!("CLR 最小 lowering：可达操作 `{instance:?}` 缺少函数体"))?;
        let base = plan
            .wasm_export_names()
            .get(instance)
            .cloned()
            .or_else(|| plan.abi_name_for_instance(*instance).map(|name| crate::lowering::sanitize_operation_symbol(&name)))
            .unwrap_or_else(|| format!("function_{ordinal}"));
        let base = crate::lowering::sanitize_symbol(&base);
        let mut name = base.clone();
        let mut suffix = 0usize;
        while used_names.contains_key(&name) {
            suffix += 1;
            name = format!("{base}__{suffix}");
        }
        used_names.insert(name.clone(), ());
        method_plans.insert(*instance, ClrMethodPlan { name, signature: function_signature(&view.function)? });
    }

    let operator_by_instance = external_operator_by_instance(submission);
    let entry_operation = plan.entry_operation();
    let mut module = MsilModule {
        assembly: MsilAssembly {
            name: crate::lowering::sanitize_symbol(plan.module_name()),
            externs: Vec::new(),
        },
        types: Vec::new(),
        global_methods: Vec::new(),
    };
    for instance in &operations {
        let Some(view) = plan.get_function(instance)
        else {
            continue;
        };
        let method = method_plans
            .get(instance)
            .cloned()
            .ok_or_else(|| format!("CLR 最小 lowering：可达操作 `{instance:?}` 缺少方法计划"))?;
        module.global_methods.push(lower_function(
            submission,
            &view.function,
            &method,
            &method_plans,
            &operator_by_instance,
            entry_operation == Some(*instance),
        )?);
    }
    Ok(module)
}

/// 从导出面出发收集可达操作（按实例身份升序，保证稳定输出顺序）。
fn collect_clr_operations(submission: &FragmentSubmission) -> Vec<ItemInstanceId> {
    let plan = &submission.backend_plan;
    let mut roots = BTreeSet::new();
    roots.extend(plan.wasm_export_names().keys().copied());
    roots.extend(plan.exported_operations().iter().copied());
    if let Some(entry) = plan.entry_operation() {
        roots.insert(entry);
    }

    let mut visited = BTreeSet::new();
    let mut ordered = Vec::new();
    loop {
        let Some(instance) = roots.iter().copied().find(|instance| !visited.contains(instance))
        else {
            break;
        };
        visited.insert(instance);
        let Some(view) = plan.get_function(&instance)
        else {
            continue;
        };
        for block in &view.function.blocks {
            for instruction in &block.instructions {
                let InstructionKind::Call { callee: Operand::Item(callee), .. } = &instruction.kind
                else {
                    continue;
                };
                if plan.get_function(callee).is_some() && !plan.imports().contains_key(callee) {
                    roots.insert(*callee);
                }
            }
        }
        ordered.push(instance);
    }
    ordered
}

/// 冻结的 `[clr_operator("...")]` 绑定：助记符只用于诊断，配方来自计划。
fn external_operator_by_instance(submission: &FragmentSubmission) -> BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)> {
    submission
        .backend_plan
        .imports()
        .iter()
        .filter_map(|(instance, import)| {
            let mnemonic = crate::lowering::shared::interop::clr_operator_target(&import.link)?;
            Some((*instance, (mnemonic.to_string(), import.target_operator.unwrap_or(TargetOperatorBinding::Unfrozen))))
        })
        .collect()
}

fn function_signature(function: &MirFunction) -> Result<MsilMethodSignature> {
    let entry = function
        .blocks
        .get(function.entry.0 as usize)
        .ok_or_else(|| format!("CLR 最小 lowering：函数 `{}` 缺少入口块", function.symbol))?;
    let mut parameter_types = Vec::with_capacity(entry.parameters.len());
    for parameter in &entry.parameters {
        let ty = function
            .value_types
            .get(parameter)
            .ok_or_else(|| format!("CLR 最小 lowering：函数 `{}` 入口形参缺少冻结类型", function.symbol))?;
        parameter_types.push(msil_parameter_type(ty)?);
    }
    Ok(MsilMethodSignature::new(msil_return_type(&function.return_type)?, parameter_types))
}

/// 局部 / 形参位置的 CLR 载体：`Unit`/`Bottom` 没有值槽。
fn msil_parameter_type(ty: &NyarType) -> Result<MsilType> {
    let msil_type = msil_type(ty)?;
    if msil_type == MsilType::Void {
        return Err(format!("CLR 最小 lowering：`{ty}` 是 void 形态，不能作为方法形参或局部值"));
    }
    Ok(msil_type)
}

fn msil_return_type(ty: &NyarType) -> Result<MsilType> {
    msil_type(ty)
}

fn msil_type(ty: &NyarType) -> Result<MsilType> {
    Ok(match ty {
        NyarType::Bottom | NyarType::Unit => MsilType::Void,
        NyarType::Boolean => MsilType::Bool,
        NyarType::Character => MsilType::Char,
        NyarType::Integer8 { signed } => MsilType::Int8 { signed: *signed },
        NyarType::Integer16 { signed } => MsilType::Int16 { signed: *signed },
        NyarType::Integer32 { signed } => MsilType::Int32 { signed: *signed },
        NyarType::Integer64 { signed } => MsilType::Int64 { signed: *signed },
        NyarType::Float32 => MsilType::Float32,
        NyarType::Float64 => MsilType::Float64,
        NyarType::Utf8 | NyarType::Utf16 => MsilType::String,
        other => {
            return Err(format!("CLR 最小 lowering 不支持类型 `{other}`：缺少该形状的 CLR 载体合同"));
        }
    })
}

struct ClrMethodEmitter<'a> {
    submission: &'a FragmentSubmission,
    function: &'a MirFunction,
    return_type: MsilType,
    slots: ExecutableSlotPlan,
    method_plans: &'a BTreeMap<ItemInstanceId, ClrMethodPlan>,
    operator_by_instance: &'a BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)>,
    instructions: Vec<MsilInstruction>,
}

fn lower_function(
    submission: &FragmentSubmission,
    function: &MirFunction,
    method: &ClrMethodPlan,
    method_plans: &BTreeMap<ItemInstanceId, ClrMethodPlan>,
    operator_by_instance: &BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)>,
    is_entry_point: bool,
) -> Result<MsilMethodBody> {
    let slots = ExecutableSlotPlan::plan_nyar(function);
    let locals = slots.local_types.iter().map(msil_parameter_type).collect::<Result<Vec<_>>>()?;
    let mut emitter = ClrMethodEmitter {
        submission,
        function,
        return_type: method.signature.return_type.clone(),
        slots,
        method_plans,
        operator_by_instance,
        instructions: Vec::new(),
    };
    emitter.emit_entry_parameter_prologue()?;
    for block_id in collect_reachable_blocks(function) {
        let Some(block) = function.blocks.get(block_id.0 as usize)
        else {
            continue;
        };
        emitter.emit_block(block)?;
    }

    let max_stack = u16::try_from(emitter.instructions.len().saturating_mul(2).saturating_add(2)).unwrap_or(u16::MAX);
    Ok(MsilMethodBody {
        method: MsilMethodRef { owner: None, name: method.name.clone(), signature: method.signature.clone() },
        locals,
        instructions: emitter.instructions,
        max_stack,
        is_entry_point,
        is_async: false,
    })
}

impl ClrMethodEmitter<'_> {
    /// 方法入口把 `ldarg` 写入 entry 块参数局部，保证后续 `ldloc` 有定义。
    fn emit_entry_parameter_prologue(&mut self) -> Result<()> {
        let entry = self
            .function
            .blocks
            .get(self.function.entry.0 as usize)
            .ok_or_else(|| format!("CLR 最小 lowering：函数 `{}` 缺少入口块", self.function.symbol))?;
        for index in 0..entry.parameters.len() {
            let local = *self.slots.block_param_locals.get(&(self.function.entry, index)).ok_or_else(|| {
                format!("CLR 最小 lowering：入口形参 {index} 缺少局部槽: function={}", self.function.symbol)
            })?;
            self.local_kind(local)?;
            self.instructions.push(MsilInstruction {
                label: None,
                opcode: MsilOpcode::Ldarg,
                operand: Some(MsilInstructionOperand::Integer(index as i64)),
            });
            self.instructions.push(MsilInstruction { label: None, opcode: MsilOpcode::Stloc, operand: Some(MsilInstructionOperand::Integer(i64::from(local))) });
        }
        Ok(())
    }

    fn emit_block(&mut self, block: &MirBlock) -> Result<()> {
        self.instructions.push(MsilInstruction { label: Some(block_label(block.id)), opcode: MsilOpcode::Nop, operand: None });
        for instruction in &block.instructions {
            self.emit_instruction(instruction)?;
        }
        self.emit_terminator(block)
    }

    fn emit_instruction(&mut self, instruction: &MirInstruction) -> Result<()> {
        if instruction.results.len() > 1 {
            return Err(format!(
                "CLR 最小 lowering 不支持多结果指令（results={}）: function={}",
                instruction.results.len(),
                self.function.symbol
            ));
        }
        match &instruction.kind {
            InstructionKind::LoadConstant { constant, .. } => {
                let (slot, kind) = self.require_output(instruction)?;
                self.emit_constant(constant, Some(kind))?;
                self.store_local(slot);
            }
            InstructionKind::Copy { source } => {
                let (slot, kind) = self.require_output(instruction)?;
                self.emit_operand(source, Some(kind))?;
                self.store_local(slot);
            }
            InstructionKind::StoreVar { value, .. } => {
                let output = self.output_of(instruction)?;
                let expected = match (&output, value) {
                    (Some((_, kind)), _) => Some(*kind),
                    (None, Operand::Value(value)) => Some(self.value_slot(*value)?.1),
                    _ => None,
                };
                self.emit_operand(value, expected)?;
                match output {
                    Some((slot, _)) => self.store_local(slot),
                    None => self.emit_pop(),
                }
            }
            InstructionKind::Call { callee, arguments } => {
                let output = self.output_of(instruction)?;
                self.emit_call(callee, arguments, output)?;
            }
            other => {
                return Err(format!(
                    "CLR 最小 lowering 尚不支持指令 `{}`：function={}",
                    instruction_kind_name(other),
                    self.function.symbol
                ));
            }
        }
        Ok(())
    }

    fn emit_terminator(&mut self, block: &MirBlock) -> Result<()> {
        match &block.terminator {
            Terminator::Return { value } => {
                let is_void = self.return_type == MsilType::Void;
                match (value, is_void) {
                    (Some(value), false) => {
                        let kind = ClrValueKind::of_msil_type(&self.return_type)?;
                        self.emit_operand(value, Some(kind))?;
                        self.push_opcode(MsilOpcode::Ret);
                    }
                    (None, true) => self.push_opcode(MsilOpcode::Ret),
                    (Some(_), true) => {
                        return Err(format!("CLR 最小 lowering：函数 `{}` 返回了值但签名是 void", self.function.symbol));
                    }
                    (None, false) => {
                        return Err(format!("CLR 最小 lowering：非 void 函数 `{}` 缺少返回值", self.function.symbol));
                    }
                }
            }
            Terminator::Jump { target, arguments } => {
                let target_label = block_label(*target);
                let (parameter_count, kinds) = {
                    let target_block = self.function.blocks.get(target.0 as usize).ok_or_else(|| {
                        format!("CLR 最小 lowering：函数 `{}` 跳转目标块 {:?} 不存在", self.function.symbol, target)
                    })?;
                    let kinds = target_block
                        .parameters
                        .iter()
                        .map(|parameter| {
                            let ty = self
                                .function
                                .value_types
                                .get(parameter)
                                .ok_or_else(|| format!("CLR 最小 lowering：块参数缺少冻结类型: function={}", self.function.symbol))?;
                            let kind = ClrValueKind::of_msil_type(&msil_parameter_type(ty)?)?;
                            Ok(kind)
                        })
                        .collect::<Result<Vec<_>>>()?;
                    (target_block.parameters.len(), kinds)
                };
                if arguments.len() != parameter_count {
                    return Err(format!(
                        "CLR 最小 lowering：跳转目标块 {:?} 形参数量与实参不符: function={} expected={} actual={}",
                        target,
                        self.function.symbol,
                        parameter_count,
                        arguments.len()
                    ));
                }
                for (argument, kind) in arguments.iter().zip(&kinds) {
                    self.emit_operand(argument, Some(*kind))?;
                }
                for index in (0..arguments.len()).rev() {
                    let local = *self.slots.block_param_locals.get(&(*target, index)).ok_or_else(|| {
                        format!("CLR 最小 lowering：跳转目标块 {:?} 形参 {index} 缺少局部槽", target)
                    })?;
                    self.store_local(local);
                }
                self.push_branch(MsilOpcode::Br, target_label);
            }
            Terminator::Branch { condition, then_target, else_target } => {
                let targets_have_parameters = self
                    .function
                    .blocks
                    .iter()
                    .filter(|block| block.id == *then_target || block.id == *else_target)
                    .any(|block| !block.parameters.is_empty());
                if targets_have_parameters {
                    return Err(format!("CLR 最小 lowering 只支持无块参数的条件分支：function={}", self.function.symbol));
                }
                self.emit_operand(condition, Some(ClrValueKind::Int32))?;
                self.push_branch(MsilOpcode::Brfalse, block_label(*else_target));
                self.push_branch(MsilOpcode::Br, block_label(*then_target));
            }
            Terminator::Unreachable => {
                if self.return_type == MsilType::Void {
                    self.push_opcode(MsilOpcode::Ret);
                }
                else {
                    return Err(format!("CLR 最小 lowering：非 void 函数 `{}` 的 `Unreachable` 终结缺少返回指令", self.function.symbol));
                }
            }
            other => {
                return Err(format!("CLR 最小 lowering 尚不支持终结符 `{other:?}`: function={}", self.function.symbol));
            }
        }
        Ok(())
    }

    fn emit_call(&mut self, callee: &Operand, arguments: &[Operand], output: Option<(u16, ClrValueKind)>) -> Result<()> {
        let Operand::Item(instance) = callee
        else {
            return Err(format!("CLR 最小 lowering 只支持 `Item` 身份的静态调用: function={} callee={callee:?}", self.function.symbol));
        };
        if let Some((mnemonic, binding)) = self.operator_by_instance.get(instance) {
            return self.emit_operator_call(mnemonic, binding, arguments, output);
        }
        if let Some(import) = self.submission.backend_plan.imports().get(instance) {
            return Err(format!(
                "CLR 最小 lowering 尚不支持宿主导入 `{instance:?}`（链接 `{}`）：缺少 CLR host 编码，function={}",
                import.link, self.function.symbol
            ));
        }
        let Some(method) = self.method_plans.get(instance).cloned()
        else {
            return Err(format!(
                "CLR 最小 lowering：同片段调用目标 `{instance:?}` 不在可达方法集中: function={}",
                self.function.symbol
            ));
        };
        let parameter_kinds = method.signature.parameter_types.iter().map(ClrValueKind::of_msil_type).collect::<Result<Vec<_>>>()?;
        if arguments.len() != parameter_kinds.len() {
            return Err(format!(
                "CLR 最小 lowering：调用 `{}` 实参数量与冻结签名不符: function={} expected={} actual={}",
                method.name,
                self.function.symbol,
                parameter_kinds.len(),
                arguments.len()
            ));
        }
        for (argument, kind) in arguments.iter().zip(&parameter_kinds) {
            self.emit_operand(argument, Some(*kind))?;
        }
        self.instructions.push(MsilInstruction {
            label: None,
            opcode: MsilOpcode::Call,
            operand: Some(MsilInstructionOperand::Method(MsilMethodRef {
                owner: None,
                name: method.name.clone(),
                signature: method.signature.clone(),
            })),
        });
        self.store_call_result(&method, output)
    }

    /// `[clr_operator("...")]` 绑定：只机械展开计划冻结的配方。
    fn emit_operator_call(
        &mut self,
        mnemonic: &str,
        binding: &TargetOperatorBinding,
        arguments: &[Operand],
        output: Option<(u16, ClrValueKind)>,
    ) -> Result<()> {
        let TargetOperatorBinding::Frozen(plan) = binding
        else {
            return Err(format!(
                "CLR 目标 `clr_operator` 绑定缺少可编码助记符 `{mnemonic}`: function={}",
                self.function.symbol
            ));
        };
        let TargetOperatorEncoding::Clr(opcode) = plan.encoding
        else {
            return Err(format!(
                "CLR 目标 `clr_operator` 绑定 `{mnemonic}` 冻结了非 CLR 编码: function={}",
                self.function.symbol
            ));
        };
        if arguments.len() != plan.arity {
            return Err(format!(
                "CLR 目标 operator `{mnemonic}` 实参数量与冻结 arity 不符: function={} expected={} actual={}",
                self.function.symbol,
                plan.arity,
                arguments.len()
            ));
        }
        let expected_kind = ClrValueKind::of_operand_type(plan.operand_type);
        self.emit_operator_operands(mnemonic, expected_kind, plan, arguments)?;
        self.push_opcode(opcode);
        match output {
            Some((slot, result_kind)) => {
                if result_kind != ClrValueKind::of_operand_type(plan.result_type) {
                    return Err(format!(
                        "CLR 目标 operator `{mnemonic}` 结果槽载体与冻结 result_type 不符: function={} expected={:?} actual={result_kind:?}",
                        self.function.symbol, plan.result_type
                    ));
                }
                self.store_local(slot);
            }
            // CLR `pop` 与栈值宽度无关，允许按需丢弃 operator 结果。
            None => self.emit_pop(),
        }
        Ok(())
    }

    fn emit_operator_operands(
        &mut self,
        mnemonic: &str,
        expected_kind: ClrValueKind,
        plan: &TargetOperatorPlan,
        arguments: &[Operand],
    ) -> Result<()> {
        match plan.recipe {
            TargetOperatorRecipe::Operands => self.emit_operator_arguments(mnemonic, expected_kind, arguments),
            TargetOperatorRecipe::Steps(steps) => {
                for step in steps {
                    match step {
                        TargetOperandStep::Arguments => self.emit_operator_arguments(mnemonic, expected_kind, arguments)?,
                        TargetOperandStep::NarrowTopI64ToI32 => self.push_opcode(MsilOpcode::ConvI4),
                        TargetOperandStep::IntegerConstant { operand_type, value } => {
                            let kind = ClrValueKind::of_operand_type(*operand_type);
                            if kind != expected_kind {
                                return Err(format!(
                                    "CLR 目标 operator `{mnemonic}` 的常量配方载体与冻结操作数载体不符: function={} expected={expected_kind:?} actual={kind:?}",
                                    self.function.symbol
                                ));
                            }
                            match kind {
                                ClrValueKind::Int32 => {
                                    self.push_integer_constant(i64::from(*value), MsilOpcode::LdcI4);
                                }
                                ClrValueKind::Int64 => {
                                    self.push_integer_constant(i64::from(*value), MsilOpcode::LdcI8);
                                }
                                ClrValueKind::Float32 | ClrValueKind::Float64 | ClrValueKind::Reference => {
                                    return Err(format!(
                                        "CLR 目标 operator `{mnemonic}` 冻结了非整数常量配方 {operand_type:?}: function={}",
                                        self.function.symbol
                                    ));
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
        }
    }

    fn emit_operator_arguments(&mut self, mnemonic: &str, expected_kind: ClrValueKind, arguments: &[Operand]) -> Result<()> {
        for argument in arguments {
            self.emit_operand(argument, Some(expected_kind)).map_err(|error| {
                format!("CLR 目标 operator `{mnemonic}` 实参载体与冻结 operand_type 不符: function={} detail={error}", self.function.symbol)
            })?;
        }
        Ok(())
    }

    fn store_call_result(&mut self, method: &ClrMethodPlan, output: Option<(u16, ClrValueKind)>) -> Result<()> {
        if method.signature.return_type == MsilType::Void {
            if output.is_some() {
                return Err(format!(
                    "CLR 最小 lowering：void 调用 `{}` 不得绑定结果槽: function={}",
                    method.name, self.function.symbol
                ));
            }
            return Ok(());
        }
        let return_kind = ClrValueKind::of_msil_type(&method.signature.return_type)?;
        match output {
            Some((slot, kind)) => {
                if kind != return_kind {
                    return Err(format!(
                        "CLR 最小 lowering：调用 `{}` 结果载体与冻结返回签名不符: function={} expected={return_kind:?} actual={kind:?}",
                        method.name, self.function.symbol
                    ));
                }
                self.store_local(slot);
                Ok(())
            }
            None => {
                self.emit_pop();
                Ok(())
            }
        }
    }

    fn emit_operand(&mut self, operand: &Operand, expected: Option<ClrValueKind>) -> Result<()> {
        match operand {
            Operand::Value(value) => {
                let (slot, kind) = self.value_slot(*value)?;
                if let Some(expected) = expected
                    && expected != kind
                {
                    return Err(format!(
                        "CLR 最小 lowering：SSA 值 {:?} 的槽载体 {kind:?} 与期望 {expected:?} 不符: function={}",
                        value, self.function.symbol
                    ));
                }
                self.load_local(slot);
                Ok(())
            }
            Operand::Constant(constant) => self.emit_constant(constant, expected),
            Operand::Symbol(path) => Err(format!("CLR 最小 lowering 不支持符号操作数 `{path:?}`: function={}", self.function.symbol)),
            Operand::Item(instance) => Err(format!(
                "CLR 最小 lowering 不支持把可调用身份 `{instance:?}` 当值使用: function={}",
                self.function.symbol
            )),
        }
    }

    fn emit_constant(&mut self, constant: &Constant, expected: Option<ClrValueKind>) -> Result<()> {
        match constant {
            Constant::Int(value) => match expected {
                Some(ClrValueKind::Int64) => {
                    self.push_integer_constant(*value, MsilOpcode::LdcI8);
                    Ok(())
                }
                Some(ClrValueKind::Int32) | None => {
                    let narrowed = i32::try_from(*value).map_err(|_| {
                        format!(
                            "CLR 最小 lowering：整数常量 {value} 超出 `int32` 载体，且目标槽位未声明 `int64`: function={}",
                            self.function.symbol
                        )
                    })?;
                    self.push_integer_constant(i64::from(narrowed), MsilOpcode::LdcI4);
                    Ok(())
                }
                Some(other) => Err(format!("CLR 最小 lowering：整数常量不能进入 {other:?} 槽: function={}", self.function.symbol)),
            },
            Constant::Float64(value) => match expected {
                Some(ClrValueKind::Float64) => {
                    let value = value.into_inner();
                    if !value.is_finite() {
                        return Err(format!(
                            "CLR 最小 lowering：非有限 f64 常量 `{value}` 缺少文本载体，拒绝猜测编码: function={}",
                            self.function.symbol
                        ));
                    }
                    self.instructions.push(MsilInstruction {
                        label: None,
                        opcode: MsilOpcode::LdcR8,
                        operand: Some(MsilInstructionOperand::Float(format!("{value:?}"))),
                    });
                    Ok(())
                }
                Some(ClrValueKind::Float32) => Err(format!(
                    "CLR 最小 lowering 不支持把 f64 常量压入 `float32` 槽：当前 MSIL 模型没有 `ldc.r4`，function={}",
                    self.function.symbol
                )),
                Some(other) => Err(format!("CLR 最小 lowering：f64 常量不能进入 {other:?} 槽: function={}", self.function.symbol)),
                None => Err(format!(
                    "CLR 最小 lowering：f64 常量缺少明确的 `float64` 结果槽，拒绝猜测载体: function={}",
                    self.function.symbol
                )),
            },
            Constant::Bool(value) => {
                if !matches!(expected, Some(ClrValueKind::Int32) | None) {
                    return Err(format!("CLR 最小 lowering：布尔常量不能进入 {expected:?} 槽: function={}", self.function.symbol));
                }
                self.push_integer_constant(i64::from(*value), MsilOpcode::LdcI4);
                Ok(())
            }
            Constant::Utf8(value) | Constant::Utf16(value) => match expected {
                Some(ClrValueKind::Reference) | None => {
                    self.instructions.push(MsilInstruction {
                        label: None,
                        opcode: MsilOpcode::Ldstr,
                        operand: Some(MsilInstructionOperand::StringLiteral(value.clone())),
                    });
                    Ok(())
                }
                Some(other) => Err(format!("CLR 最小 lowering：字符串常量不能进入 {other:?} 槽: function={}", self.function.symbol)),
            },
            other => Err(format!(
                "CLR 最小 lowering 尚不支持常量 `{other:?}`：缺少 CLR 常量载体合同，function={}",
                self.function.symbol
            )),
        }
    }

    fn push_integer_constant(&mut self, value: i64, opcode: MsilOpcode) {
        self.instructions.push(MsilInstruction {
            label: None,
            opcode,
            operand: Some(MsilInstructionOperand::Integer(value)),
        });
    }

    fn load_local(&mut self, slot: u16) {
        self.instructions.push(MsilInstruction {
            label: None,
            opcode: MsilOpcode::Ldloc,
            operand: Some(MsilInstructionOperand::Integer(i64::from(slot))),
        });
    }

    fn store_local(&mut self, slot: u16) {
        self.instructions.push(MsilInstruction {
            label: None,
            opcode: MsilOpcode::Stloc,
            operand: Some(MsilInstructionOperand::Integer(i64::from(slot))),
        });
    }

    fn push_opcode(&mut self, opcode: MsilOpcode) {
        self.instructions.push(MsilInstruction { label: None, opcode, operand: None });
    }

    fn push_branch(&mut self, opcode: MsilOpcode, target: String) {
        self.instructions.push(MsilInstruction { label: None, opcode, operand: Some(MsilInstructionOperand::BranchTarget(target)) });
    }

    fn emit_pop(&mut self) {
        self.push_opcode(MsilOpcode::Pop);
    }

    fn output_of(&self, instruction: &MirInstruction) -> Result<Option<(u16, ClrValueKind)>> {
        match self.slots.instruction_output(instruction) {
            None => Ok(None),
            Some(value) => self.value_slot(value).map(Some),
        }
    }

    fn require_output(&self, instruction: &MirInstruction) -> Result<(u16, ClrValueKind)> {
        self.output_of(instruction)?.ok_or_else(|| {
            format!("CLR 最小 lowering：指令缺少 SSA 结果槽，无法确定局部载体: function={}", self.function.symbol)
        })
    }

    fn value_slot(&self, value: MirValueRef) -> Result<(u16, ClrValueKind)> {
        let local = *self.slots.value_locals.get(&value).ok_or_else(|| {
            format!("CLR 最小 lowering：SSA 值 {:?} 缺少规划的局部槽: function={}", value, self.function.symbol)
        })?;
        Ok((local, self.local_kind(local)?))
    }

    fn local_kind(&self, local: u16) -> Result<ClrValueKind> {
        let ty = self
            .slots
            .local_types
            .get(local as usize)
            .ok_or_else(|| format!("CLR 最小 lowering：局部槽 {local} 超出规划表: function={}", self.function.symbol))?;
        ClrValueKind::of_msil_type(&msil_parameter_type(ty)?)
    }
}

/// 指令种类的稳定诊断名（不依赖 Debug 输出细节）。
fn instruction_kind_name(kind: &InstructionKind) -> &'static str {
    match kind {
        InstructionKind::LoadConstant { .. } => "LoadConstant",
        InstructionKind::LoadSymbol { .. } => "LoadSymbol",
        InstructionKind::Copy { .. } => "Copy",
        InstructionKind::Cast { .. } => "Cast",
        InstructionKind::StoreVar { .. } => "StoreVar",
        InstructionKind::Call { .. } => "Call",
        InstructionKind::LoadFunctionRef { .. } => "LoadFunctionRef",
        InstructionKind::StructNew { .. } => "StructNew",
        InstructionKind::TupleNew { .. } => "TupleNew",
        InstructionKind::AggregateCopy { .. } => "AggregateCopy",
        InstructionKind::FieldGet { .. } => "FieldGet",
        InstructionKind::FieldSet { .. } => "FieldSet",
        InstructionKind::SumNew { .. } => "SumNew",
        InstructionKind::SumPayloadGet { .. } => "SumPayloadGet",
        InstructionKind::SumVariantIs { .. } => "SumVariantIs",
        InstructionKind::PatternMatch { .. } => "PatternMatch",
        InstructionKind::ArrayNew { .. } => "ArrayNew",
        InstructionKind::ArrayFromElements { .. } => "ArrayFromElements",
        InstructionKind::ArrayGet { .. } => "ArrayGet",
        InstructionKind::ArraySet { .. } => "ArraySet",
        InstructionKind::ArrayLength { .. } => "ArrayLength",
        InstructionKind::NullIsNull { .. } => "NullIsNull",
        InstructionKind::NullUnwrap { .. } => "NullUnwrap",
    }
}

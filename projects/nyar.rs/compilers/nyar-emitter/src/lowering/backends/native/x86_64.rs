//! x86-64 最小 MIR → 机器码降低与 Windows `PE32+` 写出。
//!
//! 本文件是 **x86-64 ISA 后端**，由 [`super::lower_fragment_to_native_image`] 按
//! 规范目标四元组（`x86_64-pc-windows-msvc`）分派进入；这里不判断「native」形态，
//! 只实现该四元组确定的指令编码、Win64 调用约定与 `PE32+` 容器。
//!
//! 只消费计划已冻结的事实：整数/布尔常量、SSA 局部槽、冻结 x86-64 operator、
//! 块标签与条件分支、同片段内部调用以及 `Return`。局部值统一占 8 字节栈槽
//! （`[rbp - 8 * (local + 1)]`），调用按 Win64 ABI 走 `rcx/rdx/r8/r9` 与
//! `call rel32`；栈帧序言后 `rsp` 保持 16 字节对齐，并在帧底预留 32 字节
//! shadow space。未冻结或超出边界的形状一律结构化失败。

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    FragmentSubmission, Result,
    backend_plan_views::{
        ExecutableBlock as MirBlock, ExecutableBlockRef as MirBlockRef, ExecutableFunction as MirFunction,
        ExecutableInstruction as MirInstruction, ExecutableValueRef as MirValueRef, NyarType,
    },
    contracts::{Constant, InstructionKind, Operand, Terminator},
    lowering::shared::executable::{collect_reachable_blocks, slots::ExecutableSlotPlan},
    nyar_backend_native::{NativeExecutableKind, classify_native_image},
    target_operator::{TargetOperandType, TargetOperatorBinding, TargetOperatorEncoding, TargetOperatorRecipe},
    transitional::binary::pe::{NativePeImage, NativePeWriter},
};
use nyar::abstractions::BinaryArch;
use nyar_types::ItemInstanceId;

use super::NativeLoweredFragment;

/// 已冻结的 x86-64 operator 编码。
///
/// 编码按 **ISA** 冻结，不按「native」命名：`native` / `node` / `python` 只是同一
/// 四元组上的产物与宿主形态。算术/位/移位码要求 `operand_type = I64` 且
/// `result_type = I64`；比较码要求 `operand_type = I64` 且 `result_type = I32`
/// （`bool` 载体用 `I32`，比较结果按 `setcc` + `movzx` 归一为 0/1）。任何载体不符
/// 都由 backend 结构化拒绝，不做隐式截窄或符号扩展。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64OperatorCode {
    /// `add rax, rcx`：64 位整数加法。
    I64Add,
    /// `sub rax, rcx`：64 位整数减法。
    I64Subtract,
    /// `imul rax, rcx`：64 位整数乘法。
    I64Multiply,
    /// `neg rax`：64 位整数取负（一元）。
    I64Negate,
    /// `and rax, rcx`：64 位按位与。
    I64BitAnd,
    /// `or rax, rcx`：64 位按位或。
    I64BitOr,
    /// `xor rax, rcx`：64 位按位异或。
    I64BitXor,
    /// `shl rax, cl`：64 位左移（位移量取 `cl`，x86 按 6 位取模）。
    I64ShiftLeft,
    /// `sar rax, cl`：64 位算术右移（有符号）。
    I64ShiftRight,
    /// `cmp` + `sete`：64 位相等比较，结果 0/1。
    I64CompareEqual,
    /// `cmp` + `setne`：64 位不等比较，结果 0/1。
    I64CompareNotEqual,
    /// `cmp` + `setl`：64 位有符号小于比较，结果 0/1。
    I64CompareLessThan,
    /// `cmp` + `setle`：64 位有符号小于等于比较，结果 0/1。
    I64CompareLessOrEqual,
    /// `cmp` + `setg`：64 位有符号大于比较，结果 0/1。
    I64CompareGreaterThan,
    /// `cmp` + `setge`：64 位有符号大于等于比较，结果 0/1。
    I64CompareGreaterOrEqual,
}

impl X86_64OperatorCode {
    /// 该编码是否为比较码（结果为 0/1，载体 `I32`）。
    pub fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::I64CompareEqual
                | Self::I64CompareNotEqual
                | Self::I64CompareLessThan
                | Self::I64CompareLessOrEqual
                | Self::I64CompareGreaterThan
                | Self::I64CompareGreaterOrEqual
        )
    }

    /// 该编码冻结的实参个数（只有取负是一元）。
    pub fn arity(self) -> usize {
        match self {
            Self::I64Negate => 1,
            _ => 2,
        }
    }
}

/// Win64 整数形参寄存器。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParamRegister {
    /// `rcx`：第 1 个形参。
    Rcx,
    /// `rdx`：第 2 个形参。
    Rdx,
    /// `r8`：第 3 个形参。
    R8,
    /// `r9`：第 4 个形参。
    R9,
}

impl ParamRegister {
    const ALL: [Self; 4] = [Self::Rcx, Self::Rdx, Self::R8, Self::R9];

    /// 把该寄存器写入 `[rbp + disp32]` 的编码前缀（含 REX 与 ModRM）。
    fn store_prefix(self) -> [u8; 3] {
        match self {
            Self::Rcx => [0x48, 0x89, 0x8D],
            Self::Rdx => [0x48, 0x89, 0x95],
            Self::R8 => [0x4C, 0x89, 0x85],
            Self::R9 => [0x4C, 0x89, 0x8D],
        }
    }

    /// 把该寄存器读入 `[rbp + disp32]` 的编码前缀。
    fn load_prefix(self) -> [u8; 3] {
        match self {
            Self::Rcx => [0x48, 0x8B, 0x8D],
            Self::Rdx => [0x48, 0x8B, 0x95],
            Self::R8 => [0x4C, 0x8B, 0x85],
            Self::R9 => [0x4C, 0x8B, 0x8D],
        }
    }

    /// `mov reg, imm64` 的编码前缀。
    fn mov_imm64_prefix(self) -> [u8; 2] {
        match self {
            Self::Rcx => [0x48, 0xB9],
            Self::Rdx => [0x48, 0xBA],
            Self::R8 => [0x49, 0xB8],
            Self::R9 => [0x49, 0xB9],
        }
    }
}

/// 把 `x86_64-pc-windows-msvc` 片段降低为 `PE32+` 可执行文件。
pub(crate) fn lower_x86_64_windows_pe(submission: &FragmentSubmission) -> Result<NativeLoweredFragment> {
    let plan = &submission.backend_plan;
    let reachable = collect_native_functions(submission);
    if reachable.is_empty() {
        return Err("x86_64 最小 lowering 没有可达函数：片段既无导出也无入口操作，拒绝凭空生成 PE".to_string());
    }

    let entry =
        plan.entry_operation().ok_or_else(|| "x86_64 最小 lowering 缺少入口操作：PE 入口点必须来自计划冻结的 `entry_operation`".to_string())?;
    let entry_view =
        plan.get_function(&entry).ok_or_else(|| format!("x86_64 最小 lowering：入口操作 `{entry:?}` 没有函数体，不能作为 PE 入口"))?;
    validate_entry(&entry_view.function)?;

    let mut text = Vec::<u8>::new();
    let mut function_offsets = BTreeMap::<ItemInstanceId, usize>::new();
    let mut call_fixups = Vec::<(usize, ItemInstanceId)>::new();
    let mut entry_offset = None;

    for instance in &reachable {
        let view = plan.get_function(instance).ok_or_else(|| format!("x86_64 最小 lowering：可达操作 `{instance:?}` 缺少函数体"))?;
        let base = text.len();
        if function_offsets.insert(*instance, base).is_some() {
            return Err(format!("x86_64 最小 lowering：操作 `{instance:?}` 被重复登记为函数入口"));
        }
        if *instance == entry {
            entry_offset = Some(base);
        }
        let function_code = emit_function(submission, &view.function, &reachable, base, &mut call_fixups)?;
        text.extend_from_slice(&function_code);
    }

    for (field_offset, callee) in call_fixups {
        let target = function_offsets
            .get(&callee)
            .copied()
            .ok_or_else(|| format!("x86_64 最小 lowering：内部调用目标 `{callee:?}` 不在可达函数集中"))?;
        let displacement = i64::try_from(target).unwrap_or(i64::MAX) - (i64::try_from(field_offset).unwrap_or(i64::MAX) + 4);
        let displacement =
            i32::try_from(displacement).map_err(|_| format!("x86_64 最小 lowering：内部调用位移超出 `rel32`：callee={callee:?}"))?;
        text[field_offset..field_offset + 4].copy_from_slice(&displacement.to_le_bytes());
    }

    let entry_offset = u32::try_from(entry_offset.ok_or_else(|| "x86_64 最小 lowering：入口函数没有生成代码".to_string())?)
        .map_err(|_| "x86_64 最小 lowering：入口偏移超出 `u32`".to_string())?;
    let text_len = text.len();
    let executable = NativePeWriter::write_executable(&NativePeImage {
        text,
        rdata: Vec::new(),
        idata: Vec::new(),
        imports: Vec::new(),
        entry_point: entry_offset,
    })
    .map_err(|error| format!("x86_64 `PE` 编码失败：{error}"))?;

    // 自检：产物必须是 AMD64 可执行 PE，而不是任意字节或固定返回值镜像。
    let facts = classify_native_image(&executable).map_err(|error| format!("x86_64 产物自检失败：{error}"))?;
    if facts.kind != NativeExecutableKind::Pe || facts.arch != BinaryArch::X64 || facts.shared {
        return Err(format!("x86_64 产物自检失败：期望 AMD64 可执行 PE，实际 {facts:?}"));
    }

    Ok(NativeLoweredFragment { executable, entry_offset, text_len })
}

/// 入口函数必须是空形参、返回整数/布尔/字符或 unit。
///
/// Windows loader 调用入口时不提供有意义的形参，且以 `rax`（低 32 位）作为进程
/// 退出码；因此入口只接受可直接作为退出码的标量返回。
fn validate_entry(function: &MirFunction) -> Result<()> {
    let entry_block = function
        .blocks
        .get(function.entry.0 as usize)
        .ok_or_else(|| format!("x86_64 最小 lowering：入口函数 `{}` 缺少入口块", function.symbol))?;
    if !entry_block.parameters.is_empty() || !function.param_types.is_empty() {
        return Err(format!(
            "x86_64 最小 lowering：PE 入口函数 `{}` 必须无参数（loader 不传递形参），实际 {} 个",
            function.symbol,
            function.param_types.len()
        ));
    }
    if !is_exit_code_type(&function.return_type) {
        return Err(format!(
            "x86_64 最小 lowering：PE 入口函数 `{}` 的返回类型 `{}` 不能作为进程退出码（只接受整数/布尔/字符/unit）",
            function.symbol, function.return_type
        ));
    }
    Ok(())
}

/// 收集可达函数（导出面 + 入口 + 同片段调用闭包），按实例身份升序稳定输出。
fn collect_native_functions(submission: &FragmentSubmission) -> Vec<ItemInstanceId> {
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

fn emit_function(
    submission: &FragmentSubmission,
    function: &MirFunction,
    reachable: &[ItemInstanceId],
    base: usize,
    call_fixups: &mut Vec<(usize, ItemInstanceId)>,
) -> Result<Vec<u8>> {
    let slots = ExecutableSlotPlan::plan_nyar(function);
    let local_bytes = slots.local_types.len().checked_mul(8).ok_or_else(|| "x86_64 最小 lowering：局部槽数量溢出".to_string())?;
    let frame_bytes = align16(local_bytes.checked_add(32).ok_or_else(|| "x86_64 最小 lowering：栈帧大小溢出".to_string())?)?;
    let frame_bytes = u32::try_from(frame_bytes).map_err(|_| "x86_64 最小 lowering：栈帧大小超出 `u32`".to_string())?;

    let mut emitter = FunctionEmitter {
        submission,
        function,
        slots,
        reachable,
        base,
        call_fixups,
        code: Vec::new(),
        label_offsets: BTreeMap::new(),
        jump_fixups: Vec::new(),
    };

    // 序言：`push rbp; mov rbp, rsp; sub rsp, frame`。
    emitter.code.push(0x55);
    emitter.code.extend_from_slice(&[0x48, 0x89, 0xE5]);
    emitter.code.extend_from_slice(&[0x48, 0x81, 0xEC]);
    emitter.code.extend_from_slice(&frame_bytes.to_le_bytes());
    emitter.store_entry_parameters()?;

    let mut order = vec![function.entry];
    for block in collect_reachable_blocks(function) {
        if block != function.entry {
            order.push(block);
        }
    }
    for block_id in order {
        let Some(block) = function.blocks.get(block_id.0 as usize)
        else {
            return Err(format!("x86_64 最小 lowering：函数 `{}` 引用了不存在的块 {:?}", function.symbol, block_id));
        };
        emitter.emit_block(block)?;
    }

    emitter.patch_jumps()?;
    Ok(emitter.code)
}

fn align16(value: usize) -> Result<usize> {
    value.checked_add(15).map(|value| value & !15).ok_or_else(|| "x86_64 最小 lowering：栈帧对齐溢出".to_string())
}

/// 单函数发射器：所有标签与跳转位移都在本函数内闭合。
struct FunctionEmitter<'a> {
    submission: &'a FragmentSubmission,
    function: &'a MirFunction,
    slots: ExecutableSlotPlan,
    reachable: &'a [ItemInstanceId],
    /// 本函数在整段 `.text` 中的起始偏移，用于把 `call` 位移 fixup 记成绝对位置。
    base: usize,
    call_fixups: &'a mut Vec<(usize, ItemInstanceId)>,
    code: Vec<u8>,
    label_offsets: BTreeMap<MirBlockRef, usize>,
    /// 待回填的相对跳转：`(rel32 字段偏移, 目标块)`。
    jump_fixups: Vec<(usize, MirBlockRef)>,
}

impl FunctionEmitter<'_> {
    fn store_entry_parameters(&mut self) -> Result<()> {
        let entry_block = self
            .function
            .blocks
            .get(self.function.entry.0 as usize)
            .ok_or_else(|| format!("x86_64 最小 lowering：函数 `{}` 缺少入口块", self.function.symbol))?;
        if entry_block.parameters.len() != self.function.param_types.len() {
            return Err(format!(
                "x86_64 最小 lowering：函数 `{}` 入口块形参 {} 个与冻结签名 {} 个不一致",
                self.function.symbol,
                entry_block.parameters.len(),
                self.function.param_types.len()
            ));
        }
        if entry_block.parameters.len() > ParamRegister::ALL.len() {
            return Err(format!(
                "x86_64 最小 lowering：函数 `{}` 有 {} 个形参，超过 Win64 寄存器传参上限 {}（栈上传参未实现）",
                self.function.symbol,
                entry_block.parameters.len(),
                ParamRegister::ALL.len()
            ));
        }
        for (index, parameter) in entry_block.parameters.iter().enumerate() {
            let local = self
                .slots
                .block_param_locals
                .get(&(self.function.entry, index))
                .copied()
                .ok_or_else(|| format!("x86_64 最小 lowering：函数 `{}` 入口形参 {index} 缺少局部槽", self.function.symbol))?;
            if let Some(ty) = self.function.value_types.get(parameter)
                && !is_slot_type(ty)
            {
                return Err(format!("x86_64 最小 lowering：函数 `{}` 的形参类型 `{ty}` 没有 x86_64 标量载体", self.function.symbol));
            }
            let register = ParamRegister::ALL[index];
            self.code.extend_from_slice(&register.store_prefix());
            self.code.extend_from_slice(&slot_disp(local).to_le_bytes());
        }
        Ok(())
    }

    fn emit_block(&mut self, block: &MirBlock) -> Result<()> {
        self.label_offsets.insert(block.id, self.code.len());
        for instruction in &block.instructions {
            self.emit_instruction(instruction)?;
        }
        self.emit_terminator(&block.terminator)
    }

    fn emit_instruction(&mut self, instruction: &MirInstruction) -> Result<()> {
        if instruction.results.len() > 1 {
            return Err(format!(
                "x86_64 最小 lowering 不支持多结果指令（results={}）: function={}",
                instruction.results.len(),
                self.function.symbol
            ));
        }
        match &instruction.kind {
            InstructionKind::LoadConstant { constant, .. } => {
                let local = self.require_output_local(instruction)?;
                self.emit_constant_to_rax(constant)?;
                self.store_rax(local);
                Ok(())
            }
            InstructionKind::Copy { source } => {
                let local = self.require_output_local(instruction)?;
                self.emit_operand_to_rax(source)?;
                self.store_rax(local);
                Ok(())
            }
            InstructionKind::StoreVar { value, .. } => {
                if self.output_local(instruction)?.is_none() {
                    // 具名局部不在 native 后端物化；操作数本身是纯值，丢弃不产生副作用。
                    return Ok(());
                }
                let local = self.require_output_local(instruction)?;
                self.emit_operand_to_rax(value)?;
                self.store_rax(local);
                Ok(())
            }
            InstructionKind::Call { callee, arguments } => self.emit_call(instruction, callee, arguments),
            other => Err(format!("x86_64 最小 lowering 尚不支持指令 `{}`：function={}", instruction_kind_name(other), self.function.symbol)),
        }
    }

    fn emit_terminator(&mut self, terminator: &Terminator) -> Result<()> {
        match terminator {
            Terminator::Return { value } => self.emit_return(value.as_ref()),
            Terminator::Jump { target, arguments } => self.emit_jump(*target, arguments),
            Terminator::Branch { condition, then_target, else_target } => {
                let targets_have_parameters = self
                    .function
                    .blocks
                    .iter()
                    .filter(|block| block.id == *then_target || block.id == *else_target)
                    .any(|block| !block.parameters.is_empty());
                if targets_have_parameters {
                    return Err(format!("x86_64 最小 lowering 只支持无块参数的条件分支：function={}", self.function.symbol));
                }
                self.emit_operand_to_rax(condition)?;
                // `test eax, eax`：只按低 32 位判定布尔载体。
                self.code.extend_from_slice(&[0x85, 0xC0]);
                self.emit_jump_to(*else_target, &[0x0F, 0x84]);
                self.emit_jump_to(*then_target, &[0xE9]);
                Ok(())
            }
            Terminator::Unreachable => {
                // `ud2`：到达即故障，不伪造返回。
                self.code.extend_from_slice(&[0x0F, 0x0B]);
                Ok(())
            }
            other => Err(format!("x86_64 最小 lowering 尚不支持终结符 `{other:?}`: function={}", self.function.symbol)),
        }
    }

    fn emit_return(&mut self, value: Option<&Operand>) -> Result<()> {
        let returns_unit = matches!(self.function.return_type, NyarType::Unit | NyarType::Bottom);
        match (value, returns_unit) {
            (None, true) => self.code.extend_from_slice(&[0x31, 0xC0]),
            (None, false) => {
                return Err(format!("x86_64 最小 lowering：非 void 函数 `{}` 缺少返回值", self.function.symbol));
            }
            (Some(Operand::Constant(Constant::Unit)), true) => self.code.extend_from_slice(&[0x31, 0xC0]),
            (Some(Operand::Constant(Constant::Unit)), false) => {
                return Err(format!("x86_64 最小 lowering：非 void 函数 `{}` 返回了 unit 常量", self.function.symbol));
            }
            (Some(operand), false) => self.emit_operand_to_rax(operand)?,
            (Some(_), true) => {
                return Err(format!("x86_64 最小 lowering：函数 `{}` 返回了值但签名是 unit", self.function.symbol));
            }
        }
        // 尾声内联：`mov rsp, rbp; pop rbp; ret`。
        self.code.extend_from_slice(&[0x48, 0x89, 0xEC, 0x5D, 0xC3]);
        Ok(())
    }

    fn emit_jump(&mut self, target: MirBlockRef, arguments: &[Operand]) -> Result<()> {
        let target_block = self
            .function
            .blocks
            .get(target.0 as usize)
            .ok_or_else(|| format!("x86_64 最小 lowering：函数 `{}` 跳转目标块 {:?} 不存在", self.function.symbol, target))?;
        if arguments.len() != target_block.parameters.len() {
            return Err(format!(
                "x86_64 最小 lowering：跳转目标块 {:?} 形参数量与实参不符: function={} expected={} actual={}",
                target,
                self.function.symbol,
                target_block.parameters.len(),
                arguments.len()
            ));
        }
        if arguments.len() > ParamRegister::ALL.len() {
            return Err(format!(
                "x86_64 最小 lowering：块 {:?} 的跳转实参 {} 个超过 x86_64 scratch 上限 {}（栈上传参未实现）: function={}",
                target,
                arguments.len(),
                ParamRegister::ALL.len(),
                self.function.symbol
            ));
        }
        // 先按序落到 `[rsp + 8*i]` 周转区，再从周转区装入目标形参槽：
        // 这样源与目标槽重叠时也不会互相覆盖。
        for (index, argument) in arguments.iter().enumerate() {
            self.emit_operand_to_rax(argument)?;
            self.code.extend_from_slice(&[0x48, 0x89, 0x44, 0x24, (index as u8) * 8]);
        }
        for index in (0..arguments.len()).rev() {
            let local = self
                .slots
                .block_param_locals
                .get(&(target, index))
                .copied()
                .ok_or_else(|| format!("x86_64 最小 lowering：跳转目标块 {:?} 形参 {index} 缺少局部槽", target))?;
            self.code.extend_from_slice(&[0x48, 0x8B, 0x44, 0x24, (index as u8) * 8]);
            self.store_rax(local);
        }
        self.emit_jump_to(target, &[0xE9]);
        Ok(())
    }

    /// 记录一个相对跳转，位移在函数末尾统一回填。
    fn emit_jump_to(&mut self, target: MirBlockRef, opcode: &[u8]) {
        self.code.extend_from_slice(opcode);
        let field = self.code.len();
        self.code.extend_from_slice(&[0, 0, 0, 0]);
        self.jump_fixups.push((field, target));
    }

    fn patch_jumps(&mut self) -> Result<()> {
        for (field, target) in std::mem::take(&mut self.jump_fixups) {
            let Some(target_offset) = self.label_offsets.get(&target).copied()
            else {
                return Err(format!("x86_64 最小 lowering：跳转目标块 {:?} 不在可达块集中: function={}", target, self.function.symbol));
            };
            let displacement = i64::try_from(target_offset).unwrap_or(i64::MAX) - (i64::try_from(field).unwrap_or(i64::MAX) + 4);
            let displacement = i32::try_from(displacement)
                .map_err(|_| format!("x86_64 最小 lowering：块跳转位移超出 `rel32`: function={}", self.function.symbol))?;
            self.code[field..field + 4].copy_from_slice(&displacement.to_le_bytes());
        }
        Ok(())
    }

    fn emit_call(&mut self, instruction: &MirInstruction, callee: &Operand, arguments: &[Operand]) -> Result<()> {
        let Operand::Item(instance) = callee
        else {
            return Err(format!("x86_64 最小 lowering 只支持 `Item` 身份的调用: function={} callee={callee:?}", self.function.symbol));
        };
        if self.submission.backend_plan.imports().contains_key(instance) {
            return self.emit_operator_call(instruction, *instance, arguments);
        }
        if !self.reachable.contains(instance) {
            return Err(format!("x86_64 最小 lowering：同片段调用目标 `{instance:?}` 不在可达函数集中: function={}", self.function.symbol));
        }
        let view = self
            .submission
            .backend_plan
            .get_function(instance)
            .ok_or_else(|| format!("x86_64 最小 lowering：调用目标 `{instance:?}` 缺少函数体: function={}", self.function.symbol))?;
        let callee_function = &view.function;
        if callee_function.param_types.len() != arguments.len() {
            return Err(format!(
                "x86_64 最小 lowering：调用 `{}` 实参数量与冻结签名不符: function={} expected={} actual={}",
                callee_function.symbol,
                self.function.symbol,
                callee_function.param_types.len(),
                arguments.len()
            ));
        }
        if arguments.len() > ParamRegister::ALL.len() {
            return Err(format!(
                "x86_64 最小 lowering：调用 `{}` 传递 {} 个实参，超过 Win64 寄存器传参上限 {}（栈上传参未实现）",
                callee_function.symbol,
                arguments.len(),
                ParamRegister::ALL.len()
            ));
        }
        for (argument, register) in arguments.iter().zip(ParamRegister::ALL) {
            self.emit_operand_to_register(argument, register)?;
        }
        self.code.push(0xE8);
        let field = self.base + self.code.len();
        self.code.extend_from_slice(&[0, 0, 0, 0]);
        self.call_fixups.push((field, *instance));

        if matches!(callee_function.return_type, NyarType::Unit | NyarType::Bottom) {
            if self.output_local(instruction)?.is_some() {
                return Err(format!(
                    "x86_64 最小 lowering：void 调用 `{}` 不得绑定结果槽: function={}",
                    callee_function.symbol, self.function.symbol
                ));
            }
            return Ok(());
        }
        let local = self.require_output_local(instruction)?;
        self.store_rax(local);
        Ok(())
    }

    /// `[native_operator(...)]` 冻结绑定：只机械展开计划冻结的编码。
    fn emit_operator_call(&mut self, instruction: &MirInstruction, instance: ItemInstanceId, arguments: &[Operand]) -> Result<()> {
        let (binding, link) = {
            let import = self
                .submission
                .backend_plan
                .imports()
                .get(&instance)
                .ok_or_else(|| format!("x86_64 最小 lowering：导入实例 `{instance:?}` 缺失: function={}", self.function.symbol))?;
            (import.target_operator, import.link.clone())
        };
        let Some(binding) = binding
        else {
            return Err(format!(
                "x86_64 最小 lowering 尚不支持宿主导入 `{instance:?}`（链接 `{link}`）：缺少宿主导入编码，function={}",
                self.function.symbol
            ));
        };
        let TargetOperatorBinding::Frozen(plan) = binding
        else {
            return Err(format!(
                "x86_64 目标 operator `{instance:?}` 绑定未冻结（链接 `{link}`）：拒绝补造指令: function={}",
                self.function.symbol
            ));
        };
        let TargetOperatorEncoding::X86_64(code) = plan.encoding
        else {
            return Err(format!(
                "x86_64 目标 operator `{instance:?}` 冻结了非 x86-64 编码 {:?}: function={}",
                plan.encoding, self.function.symbol
            ));
        };
        if arguments.len() != plan.arity || plan.arity != code.arity() {
            return Err(format!(
                "x86_64 目标 operator `{instance:?}` 实参数量与冻结 arity 不符: function={} expected={} actual={}",
                self.function.symbol,
                plan.arity,
                arguments.len()
            ));
        }
        if plan.recipe != TargetOperatorRecipe::Operands {
            return Err(format!(
                "x86_64 目标 operator `{instance:?}` 冻结了非 `Operands` 配方：x86_64 只执行机械实参发射: function={}",
                self.function.symbol
            ));
        }
        if plan.operand_type != TargetOperandType::I64 {
            return Err(format!(
                "x86_64 目标 operator `{instance:?}` 的 operand_type={:?} 与 x86_64 冻结码不符（只接受 I64）: function={}",
                plan.operand_type, self.function.symbol
            ));
        }
        let expected_result = if code.is_comparison() { TargetOperandType::I32 } else { TargetOperandType::I64 };
        if plan.result_type != expected_result {
            return Err(format!(
                "x86_64 目标 operator `{instance:?}` 的 result_type={:?} 与冻结码不符（期望 {expected_result:?}）: function={}",
                plan.result_type, self.function.symbol
            ));
        }
        let local = self.require_output_local(instruction)?;
        self.emit_operand_to_rax(&arguments[0])?;
        if plan.arity == 2 {
            self.emit_operand_to_rcx(&arguments[1])?;
        }
        self.emit_operator_code(code);
        self.store_rax(local);
        Ok(())
    }

    fn emit_operator_code(&mut self, code: X86_64OperatorCode) {
        match code {
            X86_64OperatorCode::I64Add => self.code.extend_from_slice(&[0x48, 0x01, 0xC8]),
            X86_64OperatorCode::I64Subtract => self.code.extend_from_slice(&[0x48, 0x29, 0xC8]),
            X86_64OperatorCode::I64Multiply => self.code.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC1]),
            X86_64OperatorCode::I64Negate => self.code.extend_from_slice(&[0x48, 0xF7, 0xD8]),
            X86_64OperatorCode::I64BitAnd => self.code.extend_from_slice(&[0x48, 0x21, 0xC8]),
            X86_64OperatorCode::I64BitOr => self.code.extend_from_slice(&[0x48, 0x09, 0xC8]),
            X86_64OperatorCode::I64BitXor => self.code.extend_from_slice(&[0x48, 0x31, 0xC8]),
            X86_64OperatorCode::I64ShiftLeft => self.code.extend_from_slice(&[0x48, 0xD3, 0xE0]),
            X86_64OperatorCode::I64ShiftRight => self.code.extend_from_slice(&[0x48, 0xD3, 0xF8]),
            X86_64OperatorCode::I64CompareEqual => self.emit_compare(0x94),
            X86_64OperatorCode::I64CompareNotEqual => self.emit_compare(0x95),
            X86_64OperatorCode::I64CompareLessThan => self.emit_compare(0x9C),
            X86_64OperatorCode::I64CompareLessOrEqual => self.emit_compare(0x9E),
            X86_64OperatorCode::I64CompareGreaterThan => self.emit_compare(0x9F),
            X86_64OperatorCode::I64CompareGreaterOrEqual => self.emit_compare(0x9D),
        }
    }

    /// `cmp rax, rcx` + `setcc al` + `movzx eax, al`：结果为 0/1 的 32 位载体。
    fn emit_compare(&mut self, setcc: u8) {
        self.code.extend_from_slice(&[0x48, 0x39, 0xC8]);
        self.code.extend_from_slice(&[0x0F, setcc, 0xC0]);
        self.code.extend_from_slice(&[0x0F, 0xB6, 0xC0]);
    }

    fn emit_constant_to_rax(&mut self, constant: &Constant) -> Result<()> {
        match constant {
            Constant::Int(value) => {
                self.code.extend_from_slice(&[0x48, 0xB8]);
                self.code.extend_from_slice(&value.to_le_bytes());
                Ok(())
            }
            Constant::Bool(value) => {
                self.code.push(0xB8);
                self.code.extend_from_slice(&u32::from(*value).to_le_bytes());
                Ok(())
            }
            other => Err(format!("x86_64 最小 lowering 尚不支持常量 `{other:?}`：缺少 x86_64 常量载体合同，function={}", self.function.symbol)),
        }
    }

    fn emit_operand_to_rax(&mut self, operand: &Operand) -> Result<()> {
        match operand {
            Operand::Value(value) => {
                let local = self.value_local(*value)?;
                self.code.extend_from_slice(&[0x48, 0x8B, 0x85]);
                self.code.extend_from_slice(&slot_disp(local).to_le_bytes());
                Ok(())
            }
            Operand::Constant(constant) => self.emit_constant_to_rax(constant),
            Operand::Item(instance) => {
                Err(format!("x86_64 最小 lowering 不支持把可调用身份 `{instance:?}` 当值使用: function={}", self.function.symbol))
            }
            Operand::Symbol(path) => Err(format!("x86_64 最小 lowering 不支持符号操作数 `{path:?}`: function={}", self.function.symbol)),
        }
    }

    fn emit_operand_to_rcx(&mut self, operand: &Operand) -> Result<()> {
        self.emit_operand_to_register(operand, ParamRegister::Rcx)
    }

    fn emit_operand_to_register(&mut self, operand: &Operand, register: ParamRegister) -> Result<()> {
        match operand {
            Operand::Value(value) => {
                let local = self.value_local(*value)?;
                self.code.extend_from_slice(&register.load_prefix());
                self.code.extend_from_slice(&slot_disp(local).to_le_bytes());
                Ok(())
            }
            Operand::Constant(Constant::Int(value)) => {
                self.code.extend_from_slice(&register.mov_imm64_prefix());
                self.code.extend_from_slice(&value.to_le_bytes());
                Ok(())
            }
            Operand::Constant(Constant::Bool(value)) => {
                self.code.extend_from_slice(&register.mov_imm64_prefix());
                self.code.extend_from_slice(&u64::from(*value).to_le_bytes());
                Ok(())
            }
            Operand::Constant(constant) => Err(format!(
                "x86_64 最小 lowering 尚不支持常量 `{constant:?}` 作为实参：缺少 x86_64 常量载体合同，function={}",
                self.function.symbol
            )),
            Operand::Item(instance) => {
                Err(format!("x86_64 最小 lowering 不支持把可调用身份 `{instance:?}` 当值使用: function={}", self.function.symbol))
            }
            Operand::Symbol(path) => Err(format!("x86_64 最小 lowering 不支持符号操作数 `{path:?}`: function={}", self.function.symbol)),
        }
    }

    fn store_rax(&mut self, local: u16) {
        self.code.extend_from_slice(&[0x48, 0x89, 0x85]);
        self.code.extend_from_slice(&slot_disp(local).to_le_bytes());
    }

    fn output_local(&self, instruction: &MirInstruction) -> Result<Option<u16>> {
        match self.slots.instruction_output(instruction) {
            None => Ok(None),
            Some(value) => self.value_local(value).map(Some),
        }
    }

    fn require_output_local(&self, instruction: &MirInstruction) -> Result<u16> {
        self.output_local(instruction)?.ok_or_else(|| format!("x86_64 最小 lowering：指令缺少 SSA 结果槽: function={}", self.function.symbol))
    }

    fn value_local(&self, value: MirValueRef) -> Result<u16> {
        let local = *self
            .slots
            .value_locals
            .get(&value)
            .ok_or_else(|| format!("x86_64 最小 lowering：SSA 值 {:?} 缺少规划的局部槽: function={}", value, self.function.symbol))?;
        if let Some(ty) = self.slots.local_types.get(local as usize)
            && !is_slot_type(ty)
        {
            return Err(format!(
                "x86_64 最小 lowering：SSA 值 {:?} 的类型 `{ty}` 没有 x86_64 标量载体: function={}",
                value, self.function.symbol
            ));
        }
        Ok(local)
    }
}

/// 局部槽到 `rbp` 的负位移：`[rbp - 8 * (local + 1)]`。
fn slot_disp(local: u16) -> i32 {
    -8 * (i32::from(local) + 1)
}

/// 可作为 native 8 字节标量槽的类型。
fn is_slot_type(ty: &NyarType) -> bool {
    matches!(
        ty,
        NyarType::Boolean
            | NyarType::Character
            | NyarType::Integer8 { .. }
            | NyarType::Integer16 { .. }
            | NyarType::Integer32 { .. }
            | NyarType::Integer64 { .. }
    )
}

/// 可直接作为进程退出码的返回类型。
fn is_exit_code_type(ty: &NyarType) -> bool {
    matches!(ty, NyarType::Unit | NyarType::Bottom) || is_slot_type(ty)
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
        InstructionKind::TupleGet { .. } => "TupleGet",
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

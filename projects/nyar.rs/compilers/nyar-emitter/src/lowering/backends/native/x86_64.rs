//! x86-64 最小 MIR → 机器码降低与 `PE32+` / `ELF64` 容器写出。
//!
//! 本文件是 **x86-64 ISA 后端**，由 [`super::lower_fragment_to_native_image`] 按
//! 规范目标四元组分派进入；这里不判断「native」形态，只实现四元组确定的指令编码、
//! 调用约定与容器：
//!
//! - `x86_64-pc-windows-msvc`：Win64 调用约定（`rcx/rdx/r8/r9` + 32 字节 shadow
//!   space）与 `PE32+`；
//! - `x86_64-unknown-linux-gnu`（及 `linux` + `systemv`）：SysV64 调用约定
//!   （`rdi/rsi/rdx/rcx/r8/r9`、无 shadow space）与 `ELF64 ET_EXEC`，入口桩以
//!   `exit(2)` 结束进程。
//!
//! 只消费计划已冻结的事实：整数/布尔常量、SSA 局部槽、冻结 x86-64 operator、
//! 块标签与条件分支、同片段内部调用以及 `Return`。局部值统一占 8 字节栈槽
//! （`[rbp - 8 * (local + 1)]`）；栈帧序言后 `rsp` 始终 16 字节对齐，Win64 额外
//! 在帧底预留 32 字节 shadow space。未冻结或超出边界的形状一律结构化失败。

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
    target_operator::{TargetOperandType, TargetOperatorPlan, TargetOperatorRecipe},
    transitional::binary::pe::{NativePeImage, NativePeWriter},
};
use nyar::abstractions::BinaryArch;
use nyar_types::ItemInstanceId;

use acorn_pe::elf::{NativeElfImage, NativeElfWriter};

use super::{
    NativeAbi, NativeLoweredFragment,
    x64_lir::{X64Function, X64Instruction, X64Location, X64Reg, X64Value, encode_x64_function},
};

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
    /// `add eax, ecx`：32 位整数加法（结果零扩展到 64 位槽）。
    I32Add,
    /// `sub eax, ecx`：32 位整数减法。
    I32Subtract,
    /// `imul eax, ecx`：32 位整数乘法。
    I32Multiply,
    /// `neg eax`：32 位整数取负（一元）。
    I32Negate,
    /// `and eax, ecx`：32 位按位与。
    I32BitAnd,
    /// `or eax, ecx`：32 位按位或。
    I32BitOr,
    /// `xor eax, ecx`：32 位按位异或。
    I32BitXor,
    /// `shl eax, cl`：32 位左移（位移量按 5 位取模）。
    I32ShiftLeft,
    /// `sar eax, cl`：32 位算术右移（有符号）。
    I32ShiftRight,
    /// `cmp` + `sete`：32 位相等比较，结果 0/1。
    I32CompareEqual,
    /// `cmp` + `setne`：32 位不等比较，结果 0/1。
    I32CompareNotEqual,
    /// `cmp` + `setl`：32 位有符号小于比较，结果 0/1。
    I32CompareLessThan,
    /// `cmp` + `setle`：32 位有符号小于等于比较，结果 0/1。
    I32CompareLessOrEqual,
    /// `cmp` + `setg`：32 位有符号大于比较，结果 0/1。
    I32CompareGreaterThan,
    /// `cmp` + `setge`：32 位有符号大于等于比较，结果 0/1。
    I32CompareGreaterOrEqual,
    /// `shr rax, cl`：64 位逻辑右移（无符号）。
    I64ShiftRightUnsigned,
    /// `shr eax, cl`：32 位逻辑右移（无符号）。
    I32ShiftRightUnsigned,
    /// `cmp` + `setb`：64 位无符号小于比较，结果 0/1。
    I64CompareLessThanUnsigned,
    /// `cmp` + `setb`：32 位无符号小于比较，结果 0/1。
    I32CompareLessThanUnsigned,
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
                | Self::I32CompareEqual
                | Self::I32CompareNotEqual
                | Self::I32CompareLessThan
                | Self::I32CompareLessOrEqual
                | Self::I32CompareGreaterThan
                | Self::I32CompareGreaterOrEqual
                | Self::I64CompareLessThanUnsigned
                | Self::I32CompareLessThanUnsigned
        )
    }

    /// 该编码的操作数载体。
    pub fn operand_type(self) -> TargetOperandType {
        match self {
            Self::I32Add
            | Self::I32Subtract
            | Self::I32Multiply
            | Self::I32Negate
            | Self::I32BitAnd
            | Self::I32BitOr
            | Self::I32BitXor
            | Self::I32ShiftLeft
            | Self::I32ShiftRight
            | Self::I32ShiftRightUnsigned
            | Self::I32CompareEqual
            | Self::I32CompareNotEqual
            | Self::I32CompareLessThan
            | Self::I32CompareLessOrEqual
            | Self::I32CompareGreaterThan
            | Self::I32CompareGreaterOrEqual
            | Self::I32CompareLessThanUnsigned => TargetOperandType::I32,
            _ => TargetOperandType::I64,
        }
    }

    /// 该编码的结果载体：比较码固定 `I32`（bool），其余与操作数同宽。
    pub fn result_type(self) -> TargetOperandType {
        if self.is_comparison() { TargetOperandType::I32 } else { self.operand_type() }
    }

    /// 该编码冻结的实参个数（只有取负是一元）。
    pub fn arity(self) -> usize {
        match self {
            Self::I64Negate | Self::I32Negate => 1,
            _ => 2,
        }
    }
}

/// 可执行文件容器：决定入口接收方式与最终字节格式。
///
/// 容器选择来自规范目标的 `os`，不由 `arch` 推断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeContainer {
    /// Windows `PE32+`：loader 通过 `call` 进入入口，`rax` 作为退出码。
    Pe,
    /// Linux `ELF64 ET_EXEC`：内核直接 `jmp` 到入口，过程末尾必须以 `exit(2)` 结束。
    Elf,
}

impl NativeContainer {
    fn name(self) -> &'static str {
        match self {
            Self::Pe => "PE",
            Self::Elf => "ELF",
        }
    }
}

/// 把 `x86_64-pc-windows-msvc` 片段降低为 `PE32+` 可执行文件。
pub(crate) fn lower_x86_64_windows_pe(submission: &FragmentSubmission) -> Result<NativeLoweredFragment> {
    lower_x86_64(submission, NativeAbi::Win64, NativeContainer::Pe)
}

/// 把 `x86_64-unknown-linux-gnu` 片段降低为 `ELF64 ET_EXEC` 可执行文件。
pub(crate) fn lower_x86_64_linux_elf(submission: &FragmentSubmission) -> Result<NativeLoweredFragment> {
    lower_x86_64(submission, NativeAbi::SystemV64, NativeContainer::Elf)
}

/// 共享的 x86-64 降低流程：计划事实 → 目标 LIR → 机器码 → 指定容器。
///
/// 调用约定与容器都由调用方按规范目标显式给出；本函数不按 `arch` 猜 `os`/`abi`。
fn lower_x86_64(submission: &FragmentSubmission, abi: NativeAbi, container: NativeContainer) -> Result<NativeLoweredFragment> {
    let plan = &submission.backend_plan;
    let reachable = collect_native_functions(submission);
    if reachable.is_empty() {
        return Err(format!(
            "x86_64 最小 lowering 没有可达函数：片段既无导出也无入口操作，拒绝凭空生成 {}",
            container.name()
        ));
    }

    let entry = plan.entry_operation().ok_or_else(|| {
        format!(
            "x86_64 最小 lowering 缺少入口操作：{} 入口点必须来自计划冻结的 `entry_operation`",
            container.name()
        )
    })?;
    let entry_view =
        plan.get_function(&entry).ok_or_else(|| format!("x86_64 最小 lowering：入口操作 `{entry:?}` 没有函数体，不能作为 {} 入口", container.name()))?;
    validate_entry(&entry_view.function, container)?;

    let mut text = Vec::<u8>::new();
    let mut function_offsets = BTreeMap::<ItemInstanceId, usize>::new();
    let mut relocations = Vec::<(usize, ItemInstanceId)>::new();
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
        // 计划 → 目标私有 LIR → 编码器：编码器不再读取 `ExecutableFunction`。
        let function = lower_function_to_x64(submission, &view.function, &reachable, abi)?;
        encode_x64_function(&function, &mut text, &mut relocations)?;
    }

    for (field_offset, callee) in relocations {
        let target = function_offsets
            .get(&callee)
            .copied()
            .ok_or_else(|| format!("x86_64 最小 lowering：内部调用目标 `{callee:?}` 不在可达函数集中"))?;
        let displacement = i64::try_from(target).unwrap_or(i64::MAX) - (i64::try_from(field_offset).unwrap_or(i64::MAX) + 4);
        let displacement =
            i32::try_from(displacement).map_err(|_| format!("x86_64 最小 lowering：内部调用位移超出 `rel32`：callee={callee:?}"))?;
        text[field_offset..field_offset + 4].copy_from_slice(&displacement.to_le_bytes());
    }

    let entry_function = entry_offset.ok_or_else(|| "x86_64 最小 lowering：入口函数没有生成代码".to_string())?;
    let (executable, entry_offset, text_len) = match container {
        NativeContainer::Pe => {
            let entry_offset = u32::try_from(entry_function).map_err(|_| "x86_64 最小 lowering：入口偏移超出 `u32`".to_string())?;
            let text_len = text.len();
            let executable = NativePeWriter::write_executable(&NativePeImage {
                text,
                rdata: Vec::new(),
                idata: Vec::new(),
                imports: Vec::new(),
                exports: Vec::new(),
                text_rel32_fixups: Vec::new(),
                is_dll: false,
                entry_point: entry_offset,
            })
            .map_err(|error| format!("x86_64 `PE` 编码失败：{error}"))?;
            (executable, entry_offset, text_len)
        }
        NativeContainer::Elf => {
            let (text, entry_point) = append_elf_entry_exit_stub(text, entry_function)?;
            let text_len = text.len();
            let executable = NativeElfWriter::write_executable(&NativeElfImage { text, rodata: Vec::new(), entry_point })
                .map_err(|error| format!("x86_64 `ELF` 编码失败：{error}"))?;
            (executable, entry_point, text_len)
        }
    };

    // 自检：产物必须是 AMD64 可执行镜像，且容器与规范目标一致。
    let facts = classify_native_image(&executable).map_err(|error| format!("x86_64 产物自检失败：{error}"))?;
    let (expected_kind, expected_arch) = match container {
        NativeContainer::Pe => (NativeExecutableKind::Pe, BinaryArch::X64),
        NativeContainer::Elf => (NativeExecutableKind::Elf, BinaryArch::X64),
    };
    if facts.kind != expected_kind || facts.arch != expected_arch || facts.shared {
        return Err(format!(
            "x86_64 产物自检失败：期望 AMD64 可执行 {}，实际 {facts:?}",
            container.name()
        ));
    }

    Ok(NativeLoweredFragment { executable, entry_offset, text_len })
}

/// 在 `.text` 末尾追加 ELF 进程入口桩，返回新文本与入口偏移。
///
/// 内核直接跳到 `e_entry`，因此入口不能像 PE 那样 `ret` 回 loader：桩先 `call`
/// 语言入口函数，把 `rax`（i32 退出码）移到 `rdi`，再执行 `exit(2)` 系统调用。
/// SysV64 要求调用点 `rsp` 16 字节对齐；进程入口正好满足，所以 `call` 后目标函数
/// 看到的是标准 `rsp ≡ 8 (mod 16)`，与 Win64 loader 调用一致。
fn append_elf_entry_exit_stub(mut text: Vec<u8>, entry_function: usize) -> Result<(Vec<u8>, u32)> {
    let entry_point = u32::try_from(text.len()).map_err(|_| "x86_64 ELF 入口桩偏移超出 `u32`".to_string())?;
    // `call rel32` 到语言入口函数。
    text.push(0xE8);
    let field = text.len();
    let displacement = i64::try_from(entry_function).unwrap_or(i64::MAX) - (i64::try_from(field).unwrap_or(i64::MAX) + 4);
    let displacement = i32::try_from(displacement).map_err(|_| "x86_64 ELF 入口桩调用位移超出 `rel32`".to_string())?;
    text.extend_from_slice(&displacement.to_le_bytes());
    // `mov rdi, rax`；`mov eax, 60`（SYS_exit）；`syscall`。
    text.extend_from_slice(&[0x48, 0x89, 0xC7, 0xB8, 0x3C, 0x00, 0x00, 0x00, 0x0F, 0x05]);
    Ok((text, entry_point))
}

/// 入口函数必须是空形参、返回整数/布尔/字符或 unit。
///
/// 两个容器都以 `rax` 作为进程退出码（PE loader 读取返回值，ELF 入口桩把它传给
/// `exit`）；loader 不提供有意义的形参，因此入口只接受可直接作为退出码的标量返回。
fn validate_entry(function: &MirFunction, container: NativeContainer) -> Result<()> {
    let entry_block = function
        .blocks
        .get(function.entry.0 as usize)
        .ok_or_else(|| format!("x86_64 最小 lowering：入口函数 `{}` 缺少入口块", function.symbol))?;
    if !entry_block.parameters.is_empty() || !function.param_types.is_empty() {
        return Err(format!(
            "x86_64 最小 lowering：{} 入口函数 `{}` 必须无参数（loader 不传递形参），实际 {} 个",
            container.name(),
            function.symbol,
            function.param_types.len()
        ));
    }
    if !is_exit_code_type(&function.return_type) {
        return Err(format!(
            "x86_64 最小 lowering：{} 入口函数 `{}` 的返回类型 `{}` 不能作为进程退出码（只接受整数/布尔/字符/unit）",
            container.name(),
            function.symbol,
            function.return_type
        ));
    }
    Ok(())
}

/// 收集可达函数（导出面 + 入口 + 同片段调用闭包），按实例身份升序稳定输出。
fn collect_native_functions(submission: &FragmentSubmission) -> Vec<ItemInstanceId> {
    let plan = &submission.backend_plan;
    let mut roots = BTreeSet::new();
    roots.extend(plan.export_names().keys().copied());
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

fn lower_function_to_x64(
    submission: &FragmentSubmission,
    function: &MirFunction,
    reachable: &[ItemInstanceId],
    abi: NativeAbi,
) -> Result<X64Function> {
    let slots = ExecutableSlotPlan::plan_nyar(function);
    let local_bytes = slots.local_types.len().checked_mul(8).ok_or_else(|| "x86_64 最小 lowering：局部槽数量溢出".to_string())?;
    // 帧必须 16 字节对齐；帧底按调用约定预留 shadow space / 块参数周转区。
    let frame_bytes = align16(
        local_bytes
            .checked_add(abi.frame_reserve())
            .ok_or_else(|| "x86_64 最小 lowering：栈帧大小溢出".to_string())?,
    )?;
    let frame_bytes = u32::try_from(frame_bytes).map_err(|_| "x86_64 最小 lowering：栈帧大小超出 `u32`".to_string())?;

    let mut lowerer = FunctionLowerer { submission, function, slots, reachable, abi, instructions: Vec::new() };
    lowerer.store_entry_parameters()?;

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
        lowerer.lower_block(block)?;
    }

    Ok(X64Function { frame_bytes, instructions: lowerer.instructions })
}

fn align16(value: usize) -> Result<usize> {
    value.checked_add(15).map(|value| value & !15).ok_or_else(|| "x86_64 最小 lowering：栈帧对齐溢出".to_string())
}

/// 单函数降低器：把计划事实转成 x86-64 LIR，不产生任何字节。
struct FunctionLowerer<'a> {
    submission: &'a FragmentSubmission,
    function: &'a MirFunction,
    slots: ExecutableSlotPlan,
    reachable: &'a [ItemInstanceId],
    /// 由规范目标 `os`/`abi` 冻结的调用约定。
    abi: NativeAbi,
    instructions: Vec<X64Instruction>,
}

impl FunctionLowerer<'_> {
    fn push(&mut self, instruction: X64Instruction) {
        self.instructions.push(instruction);
    }

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
        let argument_registers = X64Reg::argument_registers(self.abi);
        if entry_block.parameters.len() > argument_registers.len() {
            return Err(format!(
                "x86_64 最小 lowering：函数 `{}` 有 {} 个形参，超过 {} 寄存器传参上限 {}（栈上传参未实现）",
                self.function.symbol,
                entry_block.parameters.len(),
                self.abi.name(),
                argument_registers.len()
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
            let register = argument_registers[index];
            self.push(X64Instruction::StoreFrame { slot: local, source: register });
        }
        Ok(())
    }

    fn lower_block(&mut self, block: &MirBlock) -> Result<()> {
        self.push(X64Instruction::Label(block.id));
        for instruction in &block.instructions {
            self.lower_instruction(instruction)?;
        }
        self.lower_terminator(&block.terminator)
    }

    fn lower_instruction(&mut self, instruction: &MirInstruction) -> Result<()> {
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
                self.push(X64Instruction::StoreFrame { slot: local, source: X64Reg::Rax });
                Ok(())
            }
            InstructionKind::Copy { source } => {
                let local = self.require_output_local(instruction)?;
                self.emit_operand_to_rax(source)?;
                self.push(X64Instruction::StoreFrame { slot: local, source: X64Reg::Rax });
                Ok(())
            }
            InstructionKind::StoreVar { value, .. } => {
                if self.output_local(instruction)?.is_none() {
                    // 具名局部不在 native 后端物化；操作数本身是纯值，丢弃不产生副作用。
                    return Ok(());
                }
                let local = self.require_output_local(instruction)?;
                self.emit_operand_to_rax(value)?;
                self.push(X64Instruction::StoreFrame { slot: local, source: X64Reg::Rax });
                Ok(())
            }
            InstructionKind::Call { callee, arguments } => self.emit_call(instruction, callee, arguments),
            other => Err(format!("x86_64 最小 lowering 尚不支持指令 `{}`：function={}", instruction_kind_name(other), self.function.symbol)),
        }
    }

    fn lower_terminator(&mut self, terminator: &Terminator) -> Result<()> {
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
                self.push(X64Instruction::JumpIfZero(*else_target));
                self.push(X64Instruction::Jump(*then_target));
                Ok(())
            }
            Terminator::Unreachable => {
                // `ud2`：到达即故障，不伪造返回。
                self.push(X64Instruction::Trap);
                Ok(())
            }
            other => Err(format!("x86_64 最小 lowering 尚不支持终结符 `{other:?}`: function={}", self.function.symbol)),
        }
    }

    fn emit_return(&mut self, value: Option<&Operand>) -> Result<()> {
        let returns_unit = matches!(self.function.return_type, NyarType::Unit | NyarType::Bottom);
        match (value, returns_unit) {
            (None, true) => self.push(X64Instruction::Load { register: X64Reg::Rax, source: X64Value::Immediate(0) }),
            (None, false) => {
                return Err(format!("x86_64 最小 lowering：非 void 函数 `{}` 缺少返回值", self.function.symbol));
            }
            (Some(Operand::Constant(Constant::Unit)), true) => {
                self.push(X64Instruction::Load { register: X64Reg::Rax, source: X64Value::Immediate(0) })
            }
            (Some(Operand::Constant(Constant::Unit)), false) => {
                return Err(format!("x86_64 最小 lowering：非 void 函数 `{}` 返回了 unit 常量", self.function.symbol));
            }
            (Some(operand), false) => self.emit_operand_to_rax(operand)?,
            (Some(_), true) => {
                return Err(format!("x86_64 最小 lowering：函数 `{}` 返回了值但签名是 unit", self.function.symbol));
            }
        }
        // 尾声由编码器按 LIR `Return` 统一产生。
        self.push(X64Instruction::Return);
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
        if arguments.len() > self.abi.integer_argument_limit() {
            return Err(format!(
                "x86_64 最小 lowering：块 {:?} 的跳转实参 {} 个超过 {} scratch 上限 {}（栈上传参未实现）: function={}",
                target,
                arguments.len(),
                self.abi.name(),
                self.abi.integer_argument_limit(),
                self.function.symbol
            ));
        }
        // 先按序落到 `[rsp + 8*i]` 周转区，再从周转区装入目标形参槽：
        // 这样源与目标槽重叠时也不会互相覆盖。
        for (index, argument) in arguments.iter().enumerate() {
            self.emit_operand_to_rax(argument)?;
            self.push(X64Instruction::StoreScratch { index: index as u8, source: X64Reg::Rax });
        }
        for index in (0..arguments.len()).rev() {
            let local = self
                .slots
                .block_param_locals
                .get(&(target, index))
                .copied()
                .ok_or_else(|| format!("x86_64 最小 lowering：跳转目标块 {:?} 形参 {index} 缺少局部槽", target))?;
            self.push(X64Instruction::Load { register: X64Reg::Rax, source: X64Value::Location(X64Location::Scratch(index as u8)) });
            self.push(X64Instruction::StoreFrame { slot: local, source: X64Reg::Rax });
        }
        self.push(X64Instruction::Jump(target));
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
        let argument_registers = X64Reg::argument_registers(self.abi);
        if arguments.len() > argument_registers.len() {
            return Err(format!(
                "x86_64 最小 lowering：调用 `{}` 传递 {} 个实参，超过 {} 寄存器传参上限 {}（栈上传参未实现）",
                callee_function.symbol,
                arguments.len(),
                self.abi.name(),
                argument_registers.len()
            ));
        }
        for (argument, register) in arguments.iter().zip(argument_registers) {
            self.emit_operand_to_register(argument, *register)?;
        }
        self.push(X64Instruction::Call(*instance));

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
        self.push(X64Instruction::StoreFrame { slot: local, source: X64Reg::Rax });
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
            let symbol = self
                .submission
                .backend_plan
                .abi_name_for_instance(instance)
                .map(|name| name.to_string())
                .unwrap_or_default();
            let binding = freeze_x86_64_operator(&import.link, instance, &symbol, &import.parameter_types, &import.return_type)?;
            (binding, import.link.clone())
        };
        let Some(binding) = binding
        else {
            return Err(format!(
                "x86_64 最小 lowering 尚不支持宿主导入 `{instance:?}`（链接 `{link}`）：缺少宿主导入编码，function={}",
                self.function.symbol
            ));
        };
        let X86OperatorBinding::Frozen(plan) = binding
        else {
            return Err(format!(
                "x86_64 目标 operator `{instance:?}` 绑定未冻结（链接 `{link}`）：拒绝补造指令: function={}",
                self.function.symbol
            ));
        };
        let code = plan.encoding;
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
        let expected_operand = code.operand_type();
        if plan.operand_type != expected_operand {
            return Err(format!(
                "x86_64 目标 operator `{instance:?}` 的 operand_type={:?} 与 x86_64 冻结码不符（期望 {expected_operand:?}）: function={}",
                plan.operand_type, self.function.symbol
            ));
        }
        let expected_result = code.result_type();
        if plan.result_type != expected_result {
            return Err(format!(
                "x86_64 目标 operator `{instance:?}` 的 result_type={:?} 与冻结码不符（期望 {expected_result:?}）: function={}",
                plan.result_type, self.function.symbol
            ));
        }
        let local = self.require_output_local(instruction)?;
        self.emit_operand_to_rax(&arguments[0])?;
        if plan.arity == 2 {
            self.emit_operand_to_register(&arguments[1], X64Reg::Rcx)?;
        }
        self.push(X64Instruction::Operator(code));
        self.push(X64Instruction::StoreFrame { slot: local, source: X64Reg::Rax });
        Ok(())
    }

    fn emit_constant_to_rax(&mut self, constant: &Constant) -> Result<()> {
        match constant {
            Constant::Int(value) => {
                self.push(X64Instruction::Load { register: X64Reg::Rax, source: X64Value::Immediate(*value) });
                Ok(())
            }
            Constant::Bool(value) => {
                self.push(X64Instruction::Load { register: X64Reg::Rax, source: X64Value::Immediate(i64::from(*value)) });
                Ok(())
            }
            other => Err(format!("x86_64 最小 lowering 尚不支持常量 `{other:?}`：缺少 x86_64 常量载体合同，function={}", self.function.symbol)),
        }
    }

    fn emit_operand_to_rax(&mut self, operand: &Operand) -> Result<()> {
        match operand {
            Operand::Value(value) => {
                let local = self.value_local(*value)?;
                self.push(X64Instruction::Load { register: X64Reg::Rax, source: X64Value::Location(X64Location::Frame(local)) });
                Ok(())
            }
            Operand::Constant(constant) => self.emit_constant_to_rax(constant),
            Operand::Item(instance) => {
                Err(format!("x86_64 最小 lowering 不支持把可调用身份 `{instance:?}` 当值使用: function={}", self.function.symbol))
            }
            Operand::Symbol(path) => Err(format!("x86_64 最小 lowering 不支持符号操作数 `{path:?}`: function={}", self.function.symbol)),
        }
    }

    fn emit_operand_to_register(&mut self, operand: &Operand, register: X64Reg) -> Result<()> {
        match operand {
            Operand::Value(value) => {
                let local = self.value_local(*value)?;
                self.push(X64Instruction::Load { register, source: X64Value::Location(X64Location::Frame(local)) });
                Ok(())
            }
            Operand::Constant(Constant::Int(value)) => {
                self.push(X64Instruction::Load { register, source: X64Value::Immediate(*value) });
                Ok(())
            }
            Operand::Constant(Constant::Bool(value)) => {
                self.push(X64Instruction::Load { register, source: X64Value::Immediate(i64::from(*value)) });
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

/// 唯一 x86-64 operator 配方表。未知名字返回 `None`。
///
/// 与 JVM/CLR 使用同一组 provider 名（`integer.*` / `long.*`），便于 std 按目标替换
/// 实现而不改调用点。未实现的 provider（无符号除/余、逻辑右移、浮点）返回 `None`，
/// 计划落 `Unfrozen`，由 emit 结构化失败，不猜编码。
pub(crate) fn freeze_x86_64_operator_binding(operator_name: &str) -> Option<TargetOperatorPlan<X86_64OperatorCode>> {
    let freeze = |code: X86_64OperatorCode| TargetOperatorPlan {
        arity: code.arity(),
        operand_type: code.operand_type(),
        result_type: code.result_type(),
        recipe: TargetOperatorRecipe::Operands,
        encoding: code,
    };
    Some(freeze(match operator_name {
        // 32 位整数算术/位/移位：`eax` 载体，结果零扩展到 64 位槽。
        "integer.add" => X86_64OperatorCode::I32Add,
        "integer.subtract" => X86_64OperatorCode::I32Subtract,
        "integer.multiply" => X86_64OperatorCode::I32Multiply,
        "integer.negate" => X86_64OperatorCode::I32Negate,
        "integer.and" => X86_64OperatorCode::I32BitAnd,
        "integer.or" => X86_64OperatorCode::I32BitOr,
        "integer.xor" => X86_64OperatorCode::I32BitXor,
        "integer.shift_left" => X86_64OperatorCode::I32ShiftLeft,
        "integer.shift_right" => X86_64OperatorCode::I32ShiftRight,
        "integer.shift_right_unsigned" => X86_64OperatorCode::I32ShiftRightUnsigned,
        "integer.compare_equal" => X86_64OperatorCode::I32CompareEqual,
        "integer.compare_not_equal" => X86_64OperatorCode::I32CompareNotEqual,
        "integer.compare_less_than" => X86_64OperatorCode::I32CompareLessThan,
        "integer.compare_less_or_equal" => X86_64OperatorCode::I32CompareLessOrEqual,
        "integer.compare_greater_than" => X86_64OperatorCode::I32CompareGreaterThan,
        "integer.compare_greater_or_equal" => X86_64OperatorCode::I32CompareGreaterOrEqual,
        "integer.compare_less_than_unsigned" => X86_64OperatorCode::I32CompareLessThanUnsigned,
        // 64 位整数算术/位/移位：`rax` 载体。
        "long.add" => X86_64OperatorCode::I64Add,
        "long.subtract" => X86_64OperatorCode::I64Subtract,
        "long.multiply" => X86_64OperatorCode::I64Multiply,
        "long.negate" => X86_64OperatorCode::I64Negate,
        "long.and" => X86_64OperatorCode::I64BitAnd,
        "long.or" => X86_64OperatorCode::I64BitOr,
        "long.xor" => X86_64OperatorCode::I64BitXor,
        "long.shift_left" => X86_64OperatorCode::I64ShiftLeft,
        "long.shift_right" => X86_64OperatorCode::I64ShiftRight,
        "long.shift_right_unsigned" => X86_64OperatorCode::I64ShiftRightUnsigned,
        "long.compare_equal" => X86_64OperatorCode::I64CompareEqual,
        "long.compare_not_equal" => X86_64OperatorCode::I64CompareNotEqual,
        "long.compare_less_than" => X86_64OperatorCode::I64CompareLessThan,
        "long.compare_less_or_equal" => X86_64OperatorCode::I64CompareLessOrEqual,
        "long.compare_greater_than" => X86_64OperatorCode::I64CompareGreaterThan,
        "long.compare_greater_or_equal" => X86_64OperatorCode::I64CompareGreaterOrEqual,
        "long.compare_less_than_unsigned" => X86_64OperatorCode::I64CompareLessThanUnsigned,
        _ => return None,
    }))
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

/// x86-64 平台私有的 operator 绑定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum X86OperatorBinding {
    /// 已冻结的 x86-64 算子编码。
    Frozen(TargetOperatorPlan<X86_64OperatorCode>),
    /// 声明了 x86-64 operator，但没有可编码配方；emit 必须结构化失败。
    Unfrozen,
}

/// 按声明的导入签名冻结 x86-64 operator 绑定。
pub(crate) fn freeze_x86_64_operator(
    link: &nyar::ExternalImportLink,
    instance: nyar_types::ItemInstanceId,
    symbol: &str,
    parameter_types: &[nyar_types::NyarType],
    return_type: &nyar_types::NyarType,
) -> crate::Result<Option<X86OperatorBinding>> {
    let Some(mnemonic) = crate::lowering::shared::interop::x86_64_operator_target(link)
    else {
        return Ok(None);
    };
    let Some(plan) = freeze_x86_64_operator_binding(mnemonic)
    else {
        return Ok(Some(X86OperatorBinding::Unfrozen));
    };
    crate::target_operator::validate_frozen_operator(mnemonic, instance, symbol, parameter_types, return_type, &plan)?;
    Ok(Some(X86OperatorBinding::Frozen(plan)))
}

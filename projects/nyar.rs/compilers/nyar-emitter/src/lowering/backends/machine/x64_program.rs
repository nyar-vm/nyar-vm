//! 多函数原生文本程序编码器（adapter-facing LIR → x64 文本 + 符号偏移）。
//!
//! 适配器（如 guest residual lowering）只表达槽/寄存器/跳转/命名调用；帧序言、
//! 返回尾声、`rel32` 回填与符号布局全部由本模块负责，适配器不拼机器码。
//!
//! 本模块只做**程序级装配**（多函数布局、字符串池、导入表、数据段引用与修复）；
//! 所有 x86-64 指令字节来自 [`super::x64_lir`] 的图元——机器码 lane 唯一的字节
//! 权威。这里不得出现任何手写的指令编码表。

use std::collections::BTreeMap;

use crate::Result;

use super::x64_lir::{
    X64Reg, argument_register, emit_add_rax_imm32, emit_add_rdx_imm32, emit_call_import_placeholder, emit_call_placeholder, emit_copy_bytes,
    emit_divide_by_ten, emit_function_return, emit_jump_if_zero_placeholder, emit_jump_placeholder, emit_lea_rax_from_frame,
    emit_lea_rip_placeholder, emit_load_byte_from_rax, emit_load_descriptor_length, emit_load_from_rax, emit_mov_from_frame, emit_mov_imm64,
    emit_mov_to_frame, emit_neg_rax, emit_prologue, emit_store_byte_at_rax_from_rdx, emit_store_dword_imm_to_rax, emit_store_dword_rcx_to_rax,
    emit_store_rax_to_rcx, emit_store_rax_to_stack, emit_store_rcx_to_rax, emit_sub_rax_imm32, emit_syscall_write, emit_trap, emit_zero_stack_arg,
    encode_operator, patch_rel32,
};
use super::x86_64::X86_64OperatorCode;
use super::{X64CallConv, X64DataTarget, X64TextInstruction, X64ProgramFunction, X64ProgramImage};

fn align16(value: u32) -> u32 {
    (value + 15) & !15
}

fn intern_string(strings: &mut Vec<String>, value: &str) -> usize {
    match strings.iter().position(|existing| existing == value) {
        Some(index) => index,
        None => {
            strings.push(value.to_string());
            strings.len() - 1
        }
    }
}

/// 二元算子的文本 LIR → 冻结 operator 身份（字节表只在 `x64_lir` 一处）。
fn binary_operator(instruction: &X64TextInstruction) -> Option<X86_64OperatorCode> {
    Some(match instruction {
        X64TextInstruction::Add => X86_64OperatorCode::I64Add,
        X64TextInstruction::Sub => X86_64OperatorCode::I64Subtract,
        X64TextInstruction::Mul => X86_64OperatorCode::I64Multiply,
        X64TextInstruction::Eq => X86_64OperatorCode::I64CompareEqual,
        X64TextInstruction::Ne => X86_64OperatorCode::I64CompareNotEqual,
        X64TextInstruction::Lt => X86_64OperatorCode::I64CompareLessThan,
        X64TextInstruction::Le => X86_64OperatorCode::I64CompareLessOrEqual,
        X64TextInstruction::Gt => X86_64OperatorCode::I64CompareGreaterThan,
        X64TextInstruction::Ge => X86_64OperatorCode::I64CompareGreaterOrEqual,
        _ => return None,
    })
}

/// 超出寄存器上限的形参/实参索引：结构化错误，禁止借用其他 ABI。
fn argument_out_of_range(abi: X64CallConv, index: u8, what: &str) -> String {
    format!(
        "{} 文本 ABI 只支持 {} 个寄存器整数{what}（index={index}；栈上传参未实现）",
        abi.name(),
        abi.integer_argument_limit()
    )
}

/// 把多函数文本 LIR 编码为一段 `.text` 与导出符号偏移（无宿主导入）。
///
/// `abi` 必须来自规范目标四元组（`os` + `abi`），不提供默认值。
pub fn encode_x64_program(functions: &[X64ProgramFunction], abi: X64CallConv) -> Result<X64ProgramImage> {
    encode_x64_program_with_imports(functions, &[], abi)
}

/// 把多函数文本 LIR 编码为 `.text`、导出符号与宿主导入表。
///
/// `imports` 按 `(module, field)` 声明；`.idata` 与 IAT 槽由 acorn 的导入表构造器
/// 冻结，`CallImport` 的 `call [rip+disp32]` 由 PE 写入器在最终布局后回填。
/// 宿主导入是 PE/IAT 专属合同，`ELF` 目标当前没有对应编码，必须显式拒绝而不是
/// 生成一张无人解析的 `.idata`。
pub fn encode_x64_program_with_imports(
    functions: &[X64ProgramFunction],
    imports: &[(String, String)],
    abi: X64CallConv,
) -> Result<X64ProgramImage> {
    encode_x64_program_with_data(functions, imports, abi, &[], &[])
}

/// 把多函数文本 LIR 编码为 `.text` + `.data`、导出符号与宿主导入表。
///
/// `data` 与 `data_relocations` 属于适配器：容器把数据段按 `p_vaddr == 文件偏移`
/// 布局，`LoadDataAddress` 产生的 `.text → .data` 引用由容器回填。数据段只对
/// 共享库容器（`ET_DYN`）有意义，可执行镜像不得携带数据段。
pub fn encode_x64_program_with_data(
    functions: &[X64ProgramFunction],
    imports: &[(String, String)],
    abi: X64CallConv,
    data: &[u8],
    data_relocations: &[(u32, X64DataTarget)],
) -> Result<X64ProgramImage> {
    encode_x64_program_with_dependencies(functions, imports, abi, data, data_relocations, &[])
}

/// 同 [`encode_x64_program_with_data`]，并声明产物依赖的宿主库 `SONAME`。
pub fn encode_x64_program_with_dependencies(
    functions: &[X64ProgramFunction],
    imports: &[(String, String)],
    abi: X64CallConv,
    data: &[u8],
    data_relocations: &[(u32, X64DataTarget)],
    host_dependencies: &[String],
) -> Result<X64ProgramImage> {
    if functions.is_empty() {
        return Err("native text program has no functions".to_string());
    }
    // SysV64 的导入按全局符号解析，不存在模块装载概念：`module` 必须为空。
    if abi != X64CallConv::Win64
        && let Some((module, _)) = imports.iter().find(|(module, _)| !module.is_empty())
    {
        return Err(format!("native 文本 ABI `{}` 的宿主导入按全局符号解析，模块名必须为空（实际 `{module}`）", abi.name()));
    }
    let table = if abi == X64CallConv::Win64 {
        acorn_pe::pe::build_native_import_table(imports).map_err(|error| format!("native 导入表构造失败：{error}"))?
    }
    else {
        // `PE` 专属导入表在 ELF 目标上不生成；GOT/未定义符号由 ELF 容器写出。
        acorn_pe::pe::build_native_import_table(&[]).map_err(|error| format!("native 空导入表构造失败：{error}"))?
    };
    let mut text = Vec::new();
    let mut symbols = Vec::with_capacity(functions.len());
    // (rel32 字段位置, 被调用名)
    let mut calls: Vec<(usize, String)> = Vec::new();
    // 静态字符串池与 (lea disp32 字段位置, 字符串序号, 描述符内附加偏移)。
    let mut strings: Vec<String> = Vec::new();
    let mut string_uses: Vec<(usize, usize, u32)> = Vec::new();
    // (call [rip+disp32] 的字段位置, IAT 槽在 .idata 内的偏移) — Win64。
    let mut import_fixups: Vec<(u32, u32)> = Vec::new();
    // (call [rip+disp32] 的字段位置, 导入序号) — SysV64 GOT 槽。
    let mut elf_got_fixups: Vec<(u32, u32)> = Vec::new();
    // (lea disp32 的字段位置, `.data` 内偏移) — 由容器在布局确定后回填。
    let mut data_references: Vec<(u32, u32)> = Vec::new();
    // (lea disp32 的字段位置, 目标函数名) — 编码器在文本布局完成后解析。
    let mut function_addresses: Vec<(usize, String)> = Vec::new();
    for (offset, _) in data_relocations {
        if *offset as usize + 8 > data.len() {
            return Err(format!("native 数据段重定位偏移 {offset} 超出数据块长度 {}", data.len()));
        }
    }
    let mut symbol_offsets = BTreeMap::<String, u32>::new();

    for function in functions {
        if symbol_offsets.insert(function.name.clone(), text.len() as u32).is_some() {
            return Err(format!("duplicate native text function `{}`", function.name));
        }
        symbols.push((function.name.clone(), text.len() as u32));

        // 帧大小：槽位 + shadow space + 栈实参空间，16 字节对齐。Win64 的 32 字节
        // shadow space 同时充当块参数周转区。
        let stack_arguments = function
            .instructions
            .iter()
            .filter_map(|instruction| match instruction {
                X64TextInstruction::ZeroStackArg(index)
                | X64TextInstruction::StoreSlotToStackArg { index, .. }
                | X64TextInstruction::StoreSlotAddressToStackArg { index, .. } => Some(u32::from(*index) + 1),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        let frame = align16(u32::from(function.slot_count) * 8 + abi.shadow_space() + stack_arguments * 8);
        emit_prologue(&mut text, frame);

        let mut labels = BTreeMap::<u32, usize>::new();
        let mut jumps: Vec<(usize, u32)> = Vec::new();
        for instruction in &function.instructions {
            match instruction {
                X64TextInstruction::LoadI64(value) => emit_mov_imm64(&mut text, X64Reg::Rax, *value),
                X64TextInstruction::LoadParam { index, slot } => {
                    let register = argument_register(abi, *index).ok_or_else(|| argument_out_of_range(abi, *index, "形参"))?;
                    emit_mov_to_frame(&mut text, register, *slot);
                }
                X64TextInstruction::LoadSlot(slot) => emit_mov_from_frame(&mut text, X64Reg::Rax, *slot),
                X64TextInstruction::LoadSlotToRcx(slot) => emit_mov_from_frame(&mut text, X64Reg::Rcx, *slot),
                X64TextInstruction::LoadSlotToArg { slot, index } => {
                    let register = argument_register(abi, *index).ok_or_else(|| argument_out_of_range(abi, *index, "实参"))?;
                    emit_mov_from_frame(&mut text, register, *slot);
                }
                X64TextInstruction::StoreSlot(slot) => emit_mov_to_frame(&mut text, X64Reg::Rax, *slot),
                X64TextInstruction::Label(id) => {
                    labels.insert(*id, text.len());
                }
                X64TextInstruction::Jump(id) => jumps.push((emit_jump_placeholder(&mut text), *id)),
                X64TextInstruction::JumpIfZero(id) => jumps.push((emit_jump_if_zero_placeholder(&mut text), *id)),
                X64TextInstruction::Call(name) => calls.push((emit_call_placeholder(&mut text), name.clone())),
                X64TextInstruction::LoadString(value) => {
                    let index = intern_string(&mut strings, value);
                    string_uses.push((emit_lea_rip_placeholder(&mut text), index, 0));
                }
                X64TextInstruction::LoadStringPtr(value) => {
                    let index = intern_string(&mut strings, value);
                    // 指向描述符之后的字节区。
                    string_uses.push((emit_lea_rip_placeholder(&mut text), index, 8));
                }
                X64TextInstruction::LoadStringLen(value) => {
                    let len = i64::try_from(value.len()).map_err(|_| "native 静态字符串长度超出 i64".to_string())?;
                    emit_mov_imm64(&mut text, X64Reg::Rax, len);
                }
                X64TextInstruction::LoadDescriptorLength => emit_load_descriptor_length(&mut text),
                X64TextInstruction::LoadDataAddress(offset) => {
                    if *offset as usize >= data.len() {
                        return Err(format!("native 数据段引用偏移 {offset} 超出数据块长度 {}", data.len()));
                    }
                    data_references.push((
                        u32::try_from(emit_lea_rip_placeholder(&mut text)).map_err(|_| "native 数据引用偏移超出 u32".to_string())?,
                        *offset,
                    ));
                }
                X64TextInstruction::LoadFunctionAddress(name) => {
                    function_addresses.push((emit_lea_rip_placeholder(&mut text), name.clone()));
                }
                X64TextInstruction::ZeroStackArg(index) => {
                    if abi != X64CallConv::Win64 {
                        return Err(format!("{} 文本 ABI 不支持 `ZeroStackArg`：栈上传参是 Win64 shadow space 合同", abi.name()));
                    }
                    let displacement = 32u8.checked_add(index.saturating_mul(8)).ok_or_else(|| "栈实参偏移超出 u8".to_string())?;
                    emit_zero_stack_arg(&mut text, displacement);
                }
                X64TextInstruction::StoreSlotToStackArg { slot, index } => {
                    if abi != X64CallConv::Win64 {
                        return Err(format!("{} 文本 ABI 不支持 `StoreSlotToStackArg`：栈传参是 Win64 shadow space 合同", abi.name()));
                    }
                    emit_mov_from_frame(&mut text, X64Reg::Rax, *slot);
                    let offset = 32u32.checked_add(u32::from(*index) * 8).ok_or_else(|| "栈实参偏移超出 u32".to_string())?;
                    emit_store_rax_to_stack(&mut text, offset);
                }
                X64TextInstruction::StoreSlotAddressToStackArg { slot, index } => {
                    if abi != X64CallConv::Win64 {
                        return Err(format!(
                            "{} 文本 ABI 不支持 `StoreSlotAddressToStackArg`：栈传参是 Win64 shadow space 合同",
                            abi.name()
                        ));
                    }
                    emit_lea_rax_from_frame(&mut text, *slot);
                    let offset = 32u32.checked_add(u32::from(*index) * 8).ok_or_else(|| "栈实参偏移超出 u32".to_string())?;
                    emit_store_rax_to_stack(&mut text, offset);
                }
                X64TextInstruction::LoadSlotAddress(slot) => emit_lea_rax_from_frame(&mut text, *slot),
                X64TextInstruction::AddRaxImmediate(value) => emit_add_rax_imm32(&mut text, *value),
                X64TextInstruction::SubRaxImmediate(value) => emit_sub_rax_imm32(&mut text, *value),
                X64TextInstruction::NegateRax => emit_neg_rax(&mut text),
                X64TextInstruction::DivideByTen => emit_divide_by_ten(&mut text),
                X64TextInstruction::AddRdxImmediate(value) => emit_add_rdx_imm32(&mut text, *value),
                X64TextInstruction::StoreByteAtRaxFromRdx => emit_store_byte_at_rax_from_rdx(&mut text),
                X64TextInstruction::LoadByteFromRax => emit_load_byte_from_rax(&mut text),
                X64TextInstruction::LoadFromRax => emit_load_from_rax(&mut text),
                X64TextInstruction::StoreRcxToRax => emit_store_rcx_to_rax(&mut text),
                X64TextInstruction::StoreRaxToRcx => emit_store_rax_to_rcx(&mut text),
                X64TextInstruction::StoreDwordImmediateToRax(value) => emit_store_dword_imm_to_rax(&mut text, *value),
                X64TextInstruction::StoreDwordRcxToRax => emit_store_dword_rcx_to_rax(&mut text),
                X64TextInstruction::LoadSlotToRdx(slot) => emit_mov_from_frame(&mut text, X64Reg::Rdx, *slot),
                X64TextInstruction::CopyBytes => emit_copy_bytes(&mut text),
                X64TextInstruction::Trap => emit_trap(&mut text),
                X64TextInstruction::Operator(code) => encode_operator(&mut text, *code),
                X64TextInstruction::SyscallWrite => {
                    if abi != X64CallConv::SystemV64 {
                        return Err(format!("{} 文本 ABI 没有 `write` syscall 合同：effect 写入必须走宿主导入", abi.name()));
                    }
                    emit_syscall_write(&mut text);
                }
                X64TextInstruction::CallImport { module, field } => {
                    // 两种 ABI 的指令相同，修复去向不同。
                    let field_offset = emit_call_import_placeholder(&mut text);
                    if abi == X64CallConv::Win64 {
                        let slot = *table
                            .slot_offsets
                            .get(&(module.clone(), field.clone()))
                            .ok_or_else(|| format!("native 导入 `{module}!{field}` 没有 IAT 槽"))?;
                        import_fixups.push((u32::try_from(field_offset).map_err(|_| "native 导入调用偏移超出 u32".to_string())?, slot));
                    }
                    else {
                        let index = imports
                            .iter()
                            .position(|(_, name)| name == field)
                            .ok_or_else(|| format!("native 导入 `{field}` 没有声明的动态符号"))?;
                        elf_got_fixups.push((u32::try_from(field_offset).map_err(|_| "native 导入调用偏移超出 u32".to_string())?, index as u32));
                    }
                }
                X64TextInstruction::Return => emit_function_return(&mut text),
                binary if binary_operator(binary).is_some() => encode_operator(&mut text, binary_operator(binary).expect("matched above")),
                other => return Err(format!("native text program encoder 不支持该 LIR 指令：{other:?}")),
            }
        }
        // 兜底尾声：函数体末尾也安全返回（显式 Return 之后不可达）。
        emit_function_return(&mut text);

        for (field, target) in jumps {
            let target_offset = labels
                .get(&target)
                .copied()
                .ok_or_else(|| format!("native text function `{}` 的跳转标签 {target} 未定义", function.name))?;
            patch_rel32(&mut text, field, target_offset, || format!("native text function `{}` 的跳转位移超出 rel32", function.name))?;
        }
    }

    for (field, name) in calls {
        let target = symbol_offsets.get(&name).copied().ok_or_else(|| format!("native text program 调用了未定义函数 `{name}`"))?;
        patch_rel32(&mut text, field, target as usize, || format!("native text 调用位移超出 rel32：`{name}`"))?;
    }

    for (field, name) in function_addresses {
        let target = symbol_offsets.get(&name).copied().ok_or_else(|| format!("native 函数地址引用了未定义函数 `{name}`"))?;
        patch_rel32(&mut text, field, target as usize, || format!("native 函数地址位移超出 rel32：`{name}`"))?;
    }

    // 静态字符串：描述符 `{ i32 指向字节的相对偏移 = 8; u32 len }` 紧跟 UTF-8 字节，
    // 全部内联在 `.text` 尾部。`lea rax, [rip+disp32]` 在布局确定后回填。
    if !strings.is_empty() {
        let mut descriptor_offsets = Vec::with_capacity(strings.len());
        for value in &strings {
            let descriptor_offset = text.len() as u32;
            descriptor_offsets.push(descriptor_offset);
            text.extend_from_slice(&8i32.to_le_bytes());
            let len = u32::try_from(value.len()).map_err(|_| "native 静态字符串长度超出 u32".to_string())?;
            text.extend_from_slice(&len.to_le_bytes());
            text.extend_from_slice(value.as_bytes());
        }
        for (field, index, addend) in string_uses {
            let target = descriptor_offsets[index] as usize + addend as usize;
            patch_rel32(&mut text, field, target, || "native 静态字符串地址位移超出 rel32".to_string())?;
        }
    }

    // 数据段指针目标在布局完成后解析：命名函数变成 `.text` 符号偏移，容器只看到
    // 「节 + 节内偏移」，不依赖适配器预测布局。
    let mut resolved_relocations = Vec::with_capacity(data_relocations.len());
    for (offset, target) in data_relocations {
        let resolved = match target {
            X64DataTarget::Text(offset) => {
                if *offset as usize >= text.len() {
                    return Err(format!("native 数据段指针目标 `.text` 偏移 {offset} 超出文本长度 {}", text.len()));
                }
                X64DataTarget::Text(*offset)
            }
            X64DataTarget::Data(offset) => {
                if *offset as usize >= data.len() {
                    return Err(format!("native 数据段指针目标 `.data` 偏移 {offset} 超出数据块长度 {}", data.len()));
                }
                X64DataTarget::Data(*offset)
            }
            X64DataTarget::Function(name) => {
                let target_offset = symbol_offsets.get(name).copied().ok_or_else(|| format!("native 数据段指针引用了未定义函数 `{name}`"))?;
                X64DataTarget::Text(target_offset)
            }
        };
        resolved_relocations.push((*offset, resolved));
    }

    Ok(X64ProgramImage {
        text,
        symbols,
        idata: table.bytes,
        imports: table.imports,
        text_rel32_fixups: import_fixups,
        elf_got_fixups,
        elf_imports: if abi == X64CallConv::SystemV64 { imports.iter().map(|(_, field)| field.clone()).collect() } else { Vec::new() },
        data: data.to_vec(),
        data_relocations: resolved_relocations,
        data_references,
        host_dependencies: host_dependencies.to_vec(),
    })
}

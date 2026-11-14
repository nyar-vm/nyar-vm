//! 多函数原生文本程序编码器（adapter-facing LIR → x64 文本 + 符号偏移）。
//!
//! 适配器（如 guest residual lowering）只表达槽/寄存器/跳转/命名调用；帧序言、
//! 返回尾声、`rel32` 回填与符号布局全部由本模块负责，适配器不拼机器码。

use std::collections::BTreeMap;

use crate::Result;

use super::{NativeAbi, NativeTextInstruction, NativeTextProgramFunction, NativeTextProgramImage};

/// Win64 整数形参寄存器 rcx / rdx / r8 / r9 的 `mov [rbp-disp32], reg` 前缀。
const WIN64_ARG_STORE_PREFIX: [[u8; 3]; 4] = [[0x48, 0x89, 0x8D], [0x48, 0x89, 0x95], [0x4C, 0x89, 0x85], [0x4C, 0x89, 0x8D]];
/// Win64 整数形参寄存器的 `mov reg, [rbp-disp32]` 前缀。
const WIN64_ARG_LOAD_PREFIX: [[u8; 3]; 4] = [[0x48, 0x8B, 0x8D], [0x48, 0x8B, 0x95], [0x4C, 0x8B, 0x85], [0x4C, 0x8B, 0x8D]];
/// SysV64 整数形参寄存器 rdi / rsi / rdx / rcx / r8 / r9 的 `mov [rbp-disp32], reg` 前缀。
const SYSTEM_V_ARG_STORE_PREFIX: [[u8; 3]; 6] =
    [[0x48, 0x89, 0xBD], [0x48, 0x89, 0xB5], [0x48, 0x89, 0x95], [0x48, 0x89, 0x8D], [0x4C, 0x89, 0x85], [0x4C, 0x89, 0x8D]];
/// SysV64 整数形参寄存器的 `mov reg, [rbp-disp32]` 前缀。
const SYSTEM_V_ARG_LOAD_PREFIX: [[u8; 3]; 6] =
    [[0x48, 0x8B, 0xBD], [0x48, 0x8B, 0xB5], [0x48, 0x8B, 0x95], [0x48, 0x8B, 0x8D], [0x4C, 0x8B, 0x85], [0x4C, 0x8B, 0x8D]];

/// 该调用约定的形参寄存器 `mov [rbp-disp32], reg` 前缀表。
fn arg_store_prefixes(abi: NativeAbi) -> &'static [[u8; 3]] {
    match abi {
        NativeAbi::Win64 => &WIN64_ARG_STORE_PREFIX,
        NativeAbi::SystemV64 => &SYSTEM_V_ARG_STORE_PREFIX,
    }
}

/// 该调用约定的形参寄存器 `mov reg, [rbp-disp32]` 前缀表。
fn arg_load_prefixes(abi: NativeAbi) -> &'static [[u8; 3]] {
    match abi {
        NativeAbi::Win64 => &WIN64_ARG_LOAD_PREFIX,
        NativeAbi::SystemV64 => &SYSTEM_V_ARG_LOAD_PREFIX,
    }
}

/// `mov [rbp-disp32], rax`。
const RAX_STORE: [u8; 3] = [0x48, 0x89, 0x85];
/// `mov rax, [rbp-disp32]`。
const RAX_LOAD: [u8; 3] = [0x48, 0x8B, 0x85];
/// `mov rcx, [rbp-disp32]`。
const RCX_LOAD: [u8; 3] = [0x48, 0x8B, 0x8D];

fn align16(value: u32) -> u32 {
    (value + 15) & !15
}

fn frame_disp(slot: u16) -> i32 {
    -8 * (i32::from(slot) + 1)
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

fn emit_binary(text: &mut Vec<u8>, instruction: &NativeTextInstruction) -> bool {
    match instruction {
        NativeTextInstruction::Add => text.extend_from_slice(&[0x48, 0x01, 0xC8]),
        NativeTextInstruction::Sub => text.extend_from_slice(&[0x48, 0x29, 0xC8]),
        NativeTextInstruction::Mul => text.extend_from_slice(&[0x48, 0x0F, 0xAF, 0xC1]),
        NativeTextInstruction::Eq => {
            text.extend_from_slice(&[0x48, 0x39, 0xC8, 0x0F, 0x94, 0xC0, 0x0F, 0xB6, 0xC0]);
        }
        NativeTextInstruction::Ne => {
            text.extend_from_slice(&[0x48, 0x39, 0xC8, 0x0F, 0x95, 0xC0, 0x0F, 0xB6, 0xC0]);
        }
        NativeTextInstruction::Lt => {
            text.extend_from_slice(&[0x48, 0x39, 0xC8, 0x0F, 0x9C, 0xC0, 0x0F, 0xB6, 0xC0]);
        }
        NativeTextInstruction::Le => {
            text.extend_from_slice(&[0x48, 0x39, 0xC8, 0x0F, 0x9E, 0xC0, 0x0F, 0xB6, 0xC0]);
        }
        NativeTextInstruction::Gt => {
            text.extend_from_slice(&[0x48, 0x39, 0xC8, 0x0F, 0x9F, 0xC0, 0x0F, 0xB6, 0xC0]);
        }
        NativeTextInstruction::Ge => {
            text.extend_from_slice(&[0x48, 0x39, 0xC8, 0x0F, 0x9D, 0xC0, 0x0F, 0xB6, 0xC0]);
        }
        _ => return false,
    }
    true
}

/// 把多函数文本 LIR 编码为一段 `.text` 与导出符号偏移（无宿主导入）。
///
/// `abi` 必须来自规范目标四元组（`os` + `abi`），不提供默认值。
pub fn encode_native_text_program(functions: &[NativeTextProgramFunction], abi: NativeAbi) -> Result<NativeTextProgramImage> {
    encode_native_text_program_with_imports(functions, &[], abi)
}

/// 把多函数文本 LIR 编码为 `.text`、导出符号与宿主导入表。
///
/// `imports` 按 `(module, field)` 声明；`.idata` 与 IAT 槽由 acorn 的导入表构造器
/// 冻结，`CallImport` 的 `call [rip+disp32]` 由 PE 写入器在最终布局后回填。
/// 宿主导入是 PE/IAT 专属合同，`ELF` 目标当前没有对应编码，必须显式拒绝而不是
/// 生成一张无人解析的 `.idata`。
pub fn encode_native_text_program_with_imports(
    functions: &[NativeTextProgramFunction],
    imports: &[(String, String)],
    abi: NativeAbi,
) -> Result<NativeTextProgramImage> {
    if functions.is_empty() {
        return Err("native text program has no functions".to_string());
    }
    // SysV64 的导入按全局符号解析，不存在模块装载概念：`module` 必须为空。
    if abi != NativeAbi::Win64
        && let Some((module, _)) = imports.iter().find(|(module, _)| !module.is_empty())
    {
        return Err(format!("native 文本 ABI `{}` 的宿主导入按全局符号解析，模块名必须为空（实际 `{module}`）", abi.name()));
    }
    let table = if abi == NativeAbi::Win64 {
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
    let mut symbol_offsets = BTreeMap::<String, u32>::new();

    for function in functions {
        if symbol_offsets.insert(function.name.clone(), text.len() as u32).is_some() {
            return Err(format!("duplicate native text function `{}`", function.name));
        }
        symbols.push((function.name.clone(), text.len() as u32));

        // 序言：`push rbp; mov rbp, rsp; sub rsp, frame`。Win64 额外预留 32 字节
        // shadow space，SysV64 不需要；两者都保持 `rsp` 16 字节对齐。
        let frame = align16(u32::from(function.slot_count) * 8 + abi.shadow_space());
        text.extend_from_slice(&[0x55, 0x48, 0x89, 0xE5, 0x48, 0x81, 0xEC]);
        text.extend_from_slice(&frame.to_le_bytes());

        let mut labels = BTreeMap::<u32, usize>::new();
        let mut jumps: Vec<(usize, u32)> = Vec::new();
        for instruction in &function.instructions {
            match instruction {
                NativeTextInstruction::LoadI64(value) => {
                    text.extend_from_slice(&[0x48, 0xB8]);
                    text.extend_from_slice(&value.to_le_bytes());
                }
                NativeTextInstruction::LoadParam { index, slot } => {
                    let prefix = arg_store_prefixes(abi).get(*index as usize).ok_or_else(|| {
                        format!(
                            "{} 文本 ABI 只支持 {} 个寄存器整数形参（index={index}；栈上传参未实现）",
                            abi.name(),
                            abi.integer_argument_limit()
                        )
                    })?;
                    text.extend_from_slice(prefix);
                    text.extend_from_slice(&frame_disp(*slot).to_le_bytes());
                }
                NativeTextInstruction::LoadSlot(slot) => {
                    text.extend_from_slice(&RAX_LOAD);
                    text.extend_from_slice(&frame_disp(*slot).to_le_bytes());
                }
                NativeTextInstruction::LoadSlotToRcx(slot) => {
                    text.extend_from_slice(&RCX_LOAD);
                    text.extend_from_slice(&frame_disp(*slot).to_le_bytes());
                }
                NativeTextInstruction::LoadSlotToArg { slot, index } => {
                    let prefix = arg_load_prefixes(abi).get(*index as usize).ok_or_else(|| {
                        format!(
                            "{} 文本 ABI 只支持 {} 个寄存器整数实参（index={index}；栈上传参未实现）",
                            abi.name(),
                            abi.integer_argument_limit()
                        )
                    })?;
                    text.extend_from_slice(prefix);
                    text.extend_from_slice(&frame_disp(*slot).to_le_bytes());
                }
                NativeTextInstruction::StoreSlot(slot) => {
                    text.extend_from_slice(&RAX_STORE);
                    text.extend_from_slice(&frame_disp(*slot).to_le_bytes());
                }
                NativeTextInstruction::Label(id) => {
                    labels.insert(*id, text.len());
                }
                NativeTextInstruction::Jump(id) => {
                    text.push(0xE9);
                    jumps.push((text.len(), *id));
                    text.extend_from_slice(&[0, 0, 0, 0]);
                }
                NativeTextInstruction::JumpIfZero(id) => {
                    text.extend_from_slice(&[0x85, 0xC0, 0x0F, 0x84]);
                    jumps.push((text.len(), *id));
                    text.extend_from_slice(&[0, 0, 0, 0]);
                }
                NativeTextInstruction::Call(name) => {
                    text.push(0xE8);
                    calls.push((text.len(), name.clone()));
                    text.extend_from_slice(&[0, 0, 0, 0]);
                }
                NativeTextInstruction::LoadString(value) => {
                    let index = intern_string(&mut strings, value);
                    // `lea rax, [rip + disp32]`，disp32 在数据区布局后回填。
                    text.extend_from_slice(&[0x48, 0x8D, 0x05]);
                    string_uses.push((text.len(), index, 0));
                    text.extend_from_slice(&[0, 0, 0, 0]);
                }
                NativeTextInstruction::LoadStringPtr(value) => {
                    let index = intern_string(&mut strings, value);
                    // 指向描述符之后的字节区。
                    text.extend_from_slice(&[0x48, 0x8D, 0x05]);
                    string_uses.push((text.len(), index, 8));
                    text.extend_from_slice(&[0, 0, 0, 0]);
                }
                NativeTextInstruction::LoadStringLen(value) => {
                    let len = i64::try_from(value.len()).map_err(|_| "native 静态字符串长度超出 i64".to_string())?;
                    text.extend_from_slice(&[0x48, 0xB8]);
                    text.extend_from_slice(&(len as u64).to_le_bytes());
                }
                NativeTextInstruction::ZeroStackArg(index) => {
                    // 该指令清的是 Win64 shadow space 之后的第 5+ 个栈实参槽；
                    // SysV64 的栈实参布局不同，本 lane 不借用 Win64 偏移。
                    if abi != NativeAbi::Win64 {
                        return Err(format!("{} 文本 ABI 不支持 `ZeroStackArg`：栈上传参是 Win64 shadow space 合同", abi.name()));
                    }
                    let displacement = 32u8.checked_add(index.saturating_mul(8)).ok_or_else(|| "栈实参偏移超出 u8".to_string())?;
                    // `mov qword ptr [rsp + disp8], 0`。
                    text.extend_from_slice(&[0x48, 0xC7, 0x44, 0x24, displacement, 0, 0, 0, 0]);
                }
                NativeTextInstruction::LoadSlotAddress(slot) => {
                    // `lea rax, [rbp + disp32]`。
                    text.extend_from_slice(&[0x48, 0x8D, 0x85]);
                    text.extend_from_slice(&frame_disp(*slot).to_le_bytes());
                }
                NativeTextInstruction::AddRaxImmediate(value) => {
                    text.extend_from_slice(&[0x48, 0x81, 0xC0]);
                    text.extend_from_slice(&value.to_le_bytes());
                }
                NativeTextInstruction::SubRaxImmediate(value) => {
                    text.extend_from_slice(&[0x48, 0x81, 0xE8]);
                    text.extend_from_slice(&value.to_le_bytes());
                }
                NativeTextInstruction::NegateRax => text.extend_from_slice(&[0x48, 0xF7, 0xD8]),
                NativeTextInstruction::DivideByTen => {
                    // `xor rdx, rdx; mov rcx, 10; div rcx`。
                    text.extend_from_slice(&[0x48, 0x31, 0xD2, 0x48, 0xB9]);
                    text.extend_from_slice(&10u64.to_le_bytes());
                    text.extend_from_slice(&[0x48, 0xF7, 0xF1]);
                }
                NativeTextInstruction::AddRdxImmediate(value) => {
                    text.extend_from_slice(&[0x48, 0x81, 0xC2]);
                    text.extend_from_slice(&value.to_le_bytes());
                }
                NativeTextInstruction::StoreByteAtRaxFromRdx => text.extend_from_slice(&[0x88, 0x10]),
                NativeTextInstruction::SyscallWrite => {
                    if abi != NativeAbi::SystemV64 {
                        return Err(format!(
                            "{} 文本 ABI 没有 `write` syscall 合同：effect 写入必须走宿主导入",
                            abi.name()
                        ));
                    }
                    // `mov eax, 1`（SYS_write）；`syscall`。实参已在 rdi/rsi/rdx。
                    text.extend_from_slice(&[0xB8, 0x01, 0x00, 0x00, 0x00, 0x0F, 0x05]);
                }
                NativeTextInstruction::CallImport { module, field } => {
                    // `call qword ptr [rip + disp32]`：两种 ABI 的指令相同，修复去向不同。
                    let field_offset = {
                        text.extend_from_slice(&[0xFF, 0x15]);
                        u32::try_from(text.len()).map_err(|_| "native 导入调用偏移超出 u32".to_string())?
                    };
                    text.extend_from_slice(&[0, 0, 0, 0]);
                    if abi == NativeAbi::Win64 {
                        let slot = *table
                            .slot_offsets
                            .get(&(module.clone(), field.clone()))
                            .ok_or_else(|| format!("native 导入 `{module}!{field}` 没有 IAT 槽"))?;
                        import_fixups.push((field_offset, slot));
                    }
                    else {
                        let index = imports
                            .iter()
                            .position(|(_, name)| name == field)
                            .ok_or_else(|| format!("native 导入 `{field}` 没有声明的动态符号"))?;
                        elf_got_fixups.push((field_offset, index as u32));
                    }
                }
                NativeTextInstruction::Return => {
                    text.extend_from_slice(&[0x48, 0x89, 0xEC, 0x5D, 0xC3]);
                }
                binary if emit_binary(&mut text, binary) => {}
                other => return Err(format!("native text program encoder 不支持该 LIR 指令：{other:?}")),
            }
        }
        // 兜底尾声：函数体末尾也安全返回（显式 Return 之后不可达）。
        text.extend_from_slice(&[0x48, 0x89, 0xEC, 0x5D, 0xC3]);

        for (field, target) in jumps {
            let target_offset = labels
                .get(&target)
                .copied()
                .ok_or_else(|| format!("native text function `{}` 的跳转标签 {target} 未定义", function.name))?;
            let displacement = target_offset as i64 - (field as i64 + 4);
            let displacement =
                i32::try_from(displacement).map_err(|_| format!("native text function `{}` 的跳转位移超出 rel32", function.name))?;
            text[field..field + 4].copy_from_slice(&displacement.to_le_bytes());
        }
    }

    for (field, name) in calls {
        let target = symbol_offsets.get(&name).copied().ok_or_else(|| format!("native text program 调用了未定义函数 `{name}`"))?;
        let displacement = target as i64 - (field as i64 + 4);
        let displacement = i32::try_from(displacement).map_err(|_| format!("native text 调用位移超出 rel32：`{name}`"))?;
        text[field..field + 4].copy_from_slice(&displacement.to_le_bytes());
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
            // 尾部 NUL 让 `LoadStringPtr` 同时可作宿主 C 字符串（长度字段仍不含它）。
            text.push(0);
        }
        for (field, index, addend) in string_uses {
            let target = descriptor_offsets[index] as i64 + i64::from(addend);
            let displacement = target - (field as i64 + 4);
            let displacement = i32::try_from(displacement).map_err(|_| "native 静态字符串地址位移超出 rel32".to_string())?;
            text[field..field + 4].copy_from_slice(&displacement.to_le_bytes());
        }
    }

    Ok(NativeTextProgramImage {
        text,
        symbols,
        idata: table.bytes,
        imports: table.imports,
        text_rel32_fixups: import_fixups,
        elf_got_fixups,
        elf_imports: if abi == NativeAbi::SystemV64 { imports.iter().map(|(_, field)| field.clone()).collect() } else { Vec::new() },
    })
}

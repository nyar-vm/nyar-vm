//! 多函数原生文本程序编码器（adapter-facing LIR → x64 文本 + 符号偏移）。
//!
//! 适配器（如 guest residual lowering）只表达槽/寄存器/跳转/命名调用；帧序言、
//! 返回尾声、`rel32` 回填与符号布局全部由本模块负责，适配器不拼机器码。

use std::collections::BTreeMap;

use crate::Result;

use super::{NativeTextInstruction, NativeTextProgramFunction, NativeTextProgramImage};

/// Win64 整数形参寄存器：rcx / rdx / r8 / r9 的 `mov [rbp-disp32], reg` 前缀。
const ARG_STORE_PREFIX: [[u8; 3]; 4] = [[0x48, 0x89, 0x8D], [0x48, 0x89, 0x95], [0x4C, 0x89, 0x85], [0x4C, 0x89, 0x8D]];
/// 同上的 `mov reg, [rbp-disp32]` 前缀。
const ARG_LOAD_PREFIX: [[u8; 3]; 4] = [[0x48, 0x8B, 0x8D], [0x48, 0x8B, 0x95], [0x4C, 0x8B, 0x85], [0x4C, 0x8B, 0x8D]];
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
pub fn encode_native_text_program(functions: &[NativeTextProgramFunction]) -> Result<NativeTextProgramImage> {
    encode_native_text_program_with_imports(functions, &[])
}

/// 把多函数文本 LIR 编码为 `.text`、导出符号与宿主导入表。
///
/// `imports` 按 `(module, field)` 声明；`.idata` 与 IAT 槽由 acorn 的导入表构造器
/// 冻结，`CallImport` 的 `call [rip+disp32]` 由 PE 写入器在最终布局后回填。
pub fn encode_native_text_program_with_imports(
    functions: &[NativeTextProgramFunction],
    imports: &[(String, String)],
) -> Result<NativeTextProgramImage> {
    if functions.is_empty() {
        return Err("native text program has no functions".to_string());
    }
    let table = acorn_pe::pe::build_native_import_table(imports).map_err(|error| format!("native 导入表构造失败：{error}"))?;
    let mut text = Vec::new();
    let mut symbols = Vec::with_capacity(functions.len());
    // (rel32 字段位置, 被调用名)
    let mut calls: Vec<(usize, String)> = Vec::new();
    // 静态字符串池与 (lea disp32 字段位置, 字符串序号)。
    let mut strings: Vec<String> = Vec::new();
    let mut string_uses: Vec<(usize, usize)> = Vec::new();
    // (call [rip+disp32] 的字段位置, IAT 槽在 .idata 内的偏移)
    let mut import_fixups: Vec<(u32, u32)> = Vec::new();
    let mut symbol_offsets = BTreeMap::<String, u32>::new();

    for function in functions {
        if symbol_offsets.insert(function.name.clone(), text.len() as u32).is_some() {
            return Err(format!("duplicate native text function `{}`", function.name));
        }
        symbols.push((function.name.clone(), text.len() as u32));

        // 序言：`push rbp; mov rbp, rsp; sub rsp, frame`。
        let frame = align16(u32::from(function.slot_count) * 8 + 32);
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
                    let prefix = ARG_STORE_PREFIX
                        .get(*index as usize)
                        .ok_or_else(|| format!("Win64 文本 ABI 只支持 4 个整数形参（index={index}）"))?;
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
                    let prefix = ARG_LOAD_PREFIX
                        .get(*index as usize)
                        .ok_or_else(|| format!("Win64 文本 ABI 只支持 4 个整数实参（index={index}）"))?;
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
                    let index = match strings.iter().position(|existing| existing == value) {
                        Some(index) => index,
                        None => {
                            strings.push(value.clone());
                            strings.len() - 1
                        }
                    };
                    // `lea rax, [rip + disp32]`，disp32 在数据区布局后回填。
                    text.extend_from_slice(&[0x48, 0x8D, 0x05]);
                    string_uses.push((text.len(), index));
                    text.extend_from_slice(&[0, 0, 0, 0]);
                }
                NativeTextInstruction::CallImport { module, field } => {
                    let slot = *table
                        .slot_offsets
                        .get(&(module.clone(), field.clone()))
                        .ok_or_else(|| format!("native 导入 `{module}!{field}` 没有 IAT 槽"))?;
                    // `call qword ptr [rip + disp32]`。
                    text.extend_from_slice(&[0xFF, 0x15]);
                    import_fixups.push((u32::try_from(text.len()).map_err(|_| "native 导入调用偏移超出 u32".to_string())?, slot));
                    text.extend_from_slice(&[0, 0, 0, 0]);
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
        }
        for (field, index) in string_uses {
            let target = descriptor_offsets[index] as i64;
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
    })
}

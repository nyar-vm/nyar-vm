#![doc = include_str!("readme.md")]

mod decode;
mod report;

use crate::legion::cmds::error::{LegionError, LegionResult};
use std::{fs, path::Path, process::ExitCode};

use acorn_wasm::WasmBinaryModule;

use super::{SpyOptions, SpyTargetOptions};

use decode::{disassemble_function, dump_gc_audit, dump_glue_audit, dump_type_section, locate_offset, print_overview};
use report::{detect_component_version, dump_component_overview};

/// 执行 WASM 二进制反汇编。
///
/// 用法：`legion spy wasm <file> [--func <index|name>] [--list] [--offset <abs>] [--json] [--hex]`
pub fn run(options: &SpyOptions) -> LegionResult<ExitCode> {
    let (_, opts) = options.split();
    let Some(target) = &opts.input
    else {
        return Err(LegionError::msg(report!(
            r#"用法：legion spy wasm <file> [--func <index|name>] [--list] [--offset <abs>] [--json] [--hex] [--glue-audit]
  file             目标 WASM / WASI component（`.wasm` / `.wasi` / `.core.wasm`）
  --func <i|name>  反汇编指定函数（索引或名称；仅 core module）
  --list           列出所有函数 / 段（component 则列出 section id）
  --offset <abs>   定位绝对偏移量处的指令（仅 core module）
  --json           以 JSON 格式输出
  --hex            dump 函数体原始字节（配合 --func）
  --glue-audit     审计 Node JS-glue：cli_get_* 导入与 help/version/build/main 导出匹配性"#
        )));
    };

    if !Path::exists(Path::new(target)) {
        return Err(LegionError::msg(report!("文件不存在：{}", target)));
    }

    let data = fs::read(target).map_err(|e| LegionError::msg(format!("无法读取文件 {}: {e}", target)))?;

    // Component-model binaries share the `\0asm` magic but use a non-1 version.
    // Prefer a dedicated overview over failing the core-module parser.
    if let Some(component_version) = detect_component_version(&data) {
        return dump_component_overview(target, &data, component_version, opts);
    }

    let module = WasmBinaryModule::from_bytes(&data).map_err(|error| LegionError::msg(report!("WASM 解析失败：{error}")))?;

    // Type 段结构化解析模式（优先级仅次于偏移定位）
    if opts.types {
        return dump_type_section(&module, &data, opts);
    }

    // GC layout 覆盖审计
    if opts.gc_audit {
        return dump_gc_audit(&module, &data, opts);
    }

    // Node JS-glue 契约审计
    if opts.glue_audit {
        return dump_glue_audit(&module, opts);
    }

    // 偏移定位模式优先级最高
    if let Some(offset) = opts.offset {
        return locate_offset(&data, offset as usize, opts);
    }

    // 函数反汇编模式
    if let Some(func_spec) = &opts.func {
        return disassemble_function(&module, &data, func_spec, opts);
    }

    // 默认 / 列表模式
    print_overview(&module, &data, opts);
    Ok(ExitCode::SUCCESS)
}

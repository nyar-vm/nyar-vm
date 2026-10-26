use crate::legion::cmds::error::{LegionError, LegionResult};
use acorn_wasm::{
    DecodedInstruction, SECTION_CUSTOM, WasmBinaryModule, WasmFunctionEntry, decode_code_body, parse_code_section, parse_export_section,
    parse_import_section, parse_type_section, section_name, uleb128_size, wasm_value_type_name,
};
use std::process::ExitCode;

use super::{SpyTargetOptions, decode};

use decode::{disassemble_function, external_kind_byte, format_import_type};

pub(super) fn print_json_overview(module: &WasmBinaryModule, data: &[u8]) {
    let sections: Vec<String> = module
        .sections
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let name = if s.id == SECTION_CUSTOM { s.name.clone().unwrap_or_default() } else { section_name(s.id).to_string() };
            format!("    {{\"index\": {}, \"id\": {}, \"name\": {}, \"size\": {}}}", i, s.id, json_string(&name), s.bytes.len())
        })
        .collect();

    let imports = parse_import_section(module);
    let imports_json: Vec<String> = imports
        .iter()
        .map(|i| {
            format!(
                "    {{\"module\": {}, \"field\": {}, \"kind\": {}, \"kind_str\": {}, \"type\": {}}}",
                json_string(&i.module),
                json_string(&i.field),
                external_kind_byte(&i.kind),
                json_string(i.kind.name()),
                json_string(&format_import_type(i))
            )
        })
        .collect();

    let exports = parse_export_section(module);
    let exports_json: Vec<String> = exports
        .iter()
        .map(|e| {
            format!(
                "    {{\"name\": {}, \"kind\": {}, \"kind_str\": {}, \"index\": {}}}",
                json_string(&e.name),
                external_kind_byte(&e.kind),
                json_string(e.kind.name()),
                e.index
            )
        })
        .collect();

    let functions = parse_code_section(module);
    let functions_json: Vec<String> = functions
        .iter()
        .map(|f| {
            format!(
                "    {{\"index\": {}, \"name\": {}, \"code_offset\": {}, \"body_len\": {}, \"local_groups\": {}}}",
                f.index,
                json_string(f.name.as_deref().unwrap_or("")),
                f.code_offset,
                f.body_len,
                f.local_groups
            )
        })
        .collect();

    println!(
        r#"{{
  "size": {},
  "version": {},
  "sections": [
{}
  ],
  "imports": [
{}
  ],
  "exports": [
{}
  ],
  "functions": [
{}
  ]
}}"#,
        data.len(),
        module.version,
        sections.join(",\n"),
        imports_json.join(",\n"),
        exports_json.join(",\n"),
        functions_json.join(",\n")
    );
}

pub(super) fn print_json_function(func: &WasmFunctionEntry, instructions: &[DecodedInstruction]) {
    let json = serde_json::json!({
        "index": func.index,
        "name": func.name,
        "code_offset": func.code_offset,
        "body_len": func.body_len,
        "local_groups": func.local_groups,
        "instructions": instructions,
    });
    println!("{}", serde_json::to_string_pretty(&json).unwrap_or_default());
}

pub(super) fn print_json_offset(offset: usize, func: Option<&WasmFunctionEntry>, _data: &[u8]) {
    match func {
        Some(f) => {
            println!(
                r#"{{
  "offset": {},
  "in_function": true,
  "function_index": {},
  "function_name": {},
  "function_range": [{}, {}]
}}"#,
                offset,
                f.index,
                json_string(f.name.as_deref().unwrap_or("")),
                f.code_offset,
                f.code_offset + f.body_len
            );
        }
        None => {
            println!(
                r#"{{
  "offset": {},
  "in_function": false
}}"#,
                offset
            );
        }
    }
}

/// JSON 字符串转义。
pub(super) fn json_string(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 2);
    result.push('"');
    for ch in value.chars() {
        match ch {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            c if c.is_control() => result.push_str(&format!("\\u{:04x}", c as u32)),
            c => result.push(c),
        }
    }
    result.push('"');
    result
}

/// Component-model binaries use magic `\0asm` with version `0x0d000001` (LE).
pub(super) fn detect_component_version(data: &[u8]) -> Option<u32> {
    if data.len() < 8 || &data[0..4] != b"\0asm" {
        return None;
    }
    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    // Core modules use version 1; component-model uses 0x0d000001.
    if version == 1 { None } else { Some(version) }
}

/// Dump a high-level overview of a WASI / component-model binary.
///
/// Full instruction disassembly still targets the paired `.core.wasm` (or a
/// nested core module extracted below when present as raw `\0asm` payload).
pub(super) fn dump_component_overview(path: &str, data: &[u8], version: u32, opts: &SpyTargetOptions) -> LegionResult<ExitCode> {
    let sections = scan_component_sections(data)?;
    let nested_cores = find_nested_core_modules_from_component(data, &sections);

    if opts.json {
        println!("{{");
        println!("  \"kind\": \"component\",");
        println!("  \"path\": {},", json_string(path));
        println!("  \"bytes\": {},", data.len());
        println!("  \"version\": {},", version);
        println!("  \"sections\": [");
        for (index, section) in sections.iter().enumerate() {
            let comma = if index + 1 == sections.len() { "" } else { "," };
            println!("    {{\"index\": {}, \"id\": {}, \"size\": {}, \"offset\": {}}}{comma}", index, section.id, section.size, section.offset);
        }
        println!("  ],");
        println!("  \"nested_core_modules\": [");
        for (index, core) in nested_cores.iter().enumerate() {
            let comma = if index + 1 == nested_cores.len() { "" } else { "," };
            println!("    {{\"offset\": {}, \"bytes\": {}}}{comma}", core.offset, core.bytes.len());
        }
        println!("  ]");
        println!("}}");
        return Ok(ExitCode::SUCCESS);
    }

    // Function disassembly short-circuits overview noise.
    if let Some(func_spec) = &opts.func {
        if nested_cores.is_empty() {
            return Err(LegionError::msg(report!("component 中没有可反汇编的嵌套 core module")));
        }
        let core = &nested_cores[0];
        let module = WasmBinaryModule::from_bytes(&core.bytes).map_err(|error| report!("嵌套 core 解析失败：{error}"))?;
        return disassemble_function(&module, &core.bytes, func_spec, opts);
    }

    println!("WASI component（{} 字节，version=0x{version:08x}）", data.len());
    println!("文件：{path}");
    println!();
    if let Some(wit_text) = dump_component_wit_via_wasm_tools(path) {
        println!("=== Component WIT（wasm-tools）===");
        for line in wit_text.lines().take(40) {
            println!("{line}");
        }
        if wit_text.lines().count() > 40 {
            println!("…（已截断）");
        }
        println!();
    }
    println!("=== Component 段列表 ===");
    for (index, section) in sections.iter().enumerate() {
        println!("  [{index}] id={} size={} offset=0x{:X}", section.id, section.size, section.offset);
    }
    println!();
    if nested_cores.is_empty() {
        println!("未在 component 载荷中发现嵌套 core module。");
        println!("提示：构建通常同时写出 `<name>.core.wasm`，可用 `legion spy wasm <name>.core.wasm --list` 反汇编。");
    }
    else {
        println!("=== 嵌套 core module ===");
        for (index, core) in nested_cores.iter().enumerate() {
            println!("  [{index}] offset=0x{:X} bytes={}", core.offset, core.bytes.len());
            match WasmBinaryModule::from_bytes(&core.bytes) {
                Ok(module) => {
                    let exports = parse_export_section(&module);
                    if !exports.is_empty() {
                        println!("    exports:");
                        for exp in &exports {
                            println!("      {} {} : index={}", exp.kind.name(), exp.name, exp.index);
                        }
                    }
                    if opts.list {
                        println!("    core 段数：{}", module.sections.len());
                        for (section_index, section) in module.sections.iter().enumerate() {
                            println!("      [{section_index}] {} size={}", section_name(section.id), section.bytes.len());
                        }
                    }
                }
                Err(error) => println!("    core 解析失败：{error}"),
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn dump_component_wit_via_wasm_tools(path: &str) -> Option<String> {
    let output = std::process::Command::new("wasm-tools").args(["component", "wit", path]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

#[derive(Debug, Clone, Copy)]
struct ComponentSectionRef {
    id: u8,
    size: usize,
    offset: usize,
}

#[derive(Debug, Clone)]
struct NestedCoreModule {
    offset: usize,
    bytes: Vec<u8>,
}

fn scan_component_sections(data: &[u8]) -> LegionResult<Vec<ComponentSectionRef>> {
    if data.len() < 8 {
        return Err(LegionError::msg(report!("component 文件过短")));
    }
    let mut cursor = 8usize;
    let mut sections = Vec::new();
    while cursor < data.len() {
        let id = data[cursor];
        cursor += 1;
        let (size, size_len) = read_uleb128_at(data, cursor)?;
        cursor += size_len;
        let size = size as usize;
        if cursor + size > data.len() {
            return Err(LegionError::msg(report!("component 段越界：id={id} size={size} offset=0x{cursor:X}")));
        }
        sections.push(ComponentSectionRef { id, size, offset: cursor });
        cursor += size;
    }
    Ok(sections)
}

fn find_nested_core_modules_from_component(data: &[u8], sections: &[ComponentSectionRef]) -> Vec<NestedCoreModule> {
    // Component section id 1 is a core `module` whose payload is a complete `\0asm` binary.
    let mut found = Vec::new();
    for section in sections {
        if section.id != 1 {
            continue;
        }
        let end = section.offset.saturating_add(section.size);
        if end > data.len() || section.size < 8 {
            continue;
        }
        let bytes = data[section.offset..end].to_vec();
        if &bytes[0..4] != b"\0asm" {
            continue;
        }
        let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        if version != 1 {
            continue;
        }
        if WasmBinaryModule::from_bytes(&bytes).is_ok() {
            found.push(NestedCoreModule { offset: section.offset, bytes });
        }
    }
    if !found.is_empty() {
        return found;
    }
    // Fallback: scan for embedded `\0asm` version-1 payloads (best-effort).
    find_nested_core_modules(data)
}

fn find_nested_core_modules(data: &[u8]) -> Vec<NestedCoreModule> {
    let mut found = Vec::new();
    let mut index = 0usize;
    while index + 8 <= data.len() {
        if &data[index..index + 4] == b"\0asm" {
            let version = u32::from_le_bytes([data[index + 4], data[index + 5], data[index + 6], data[index + 7]]);
            if version == 1 {
                if let Some(end) = measure_core_module_end(&data[index..]) {
                    found.push(NestedCoreModule { offset: index, bytes: data[index..index + end].to_vec() });
                    index += end;
                    continue;
                }
            }
        }
        index += 1;
    }
    found
}

fn measure_core_module_end(data: &[u8]) -> Option<usize> {
    if data.len() < 8 || &data[0..4] != b"\0asm" {
        return None;
    }
    let mut cursor = 8usize;
    while cursor < data.len() {
        let id = *data.get(cursor)?;
        // Core module section ids are 0..=12; component sections use other ids.
        if id > 12 {
            return Some(cursor);
        }
        cursor += 1;
        let (size, size_len) = read_uleb128_at(data, cursor).ok()?;
        cursor += size_len;
        let size = size as usize;
        cursor = cursor.checked_add(size)?;
        if cursor > data.len() {
            return None;
        }
    }
    Some(cursor)
}

fn read_uleb128_at(data: &[u8], start: usize) -> LegionResult<(u64, usize)> {
    let mut result = 0u64;
    let mut shift = 0u32;
    let mut consumed = 0usize;
    loop {
        let byte = *data.get(start + consumed).ok_or_else(|| LegionError::msg(report!("uleb128 越界 @ {}", start + consumed)))?;
        consumed += 1;
        result |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok((result, consumed));
        }
        shift += 7;
        if shift > 63 {
            return Err(LegionError::msg(report!("uleb128 过长 @ {start}")));
        }
    }
}

//! `.nyar` 模块解码：入口与各 section 解析器。

use crate::{
    errors::OdinDecodeError,
    model::{
        OdinConstant, OdinExport, OdinExportKind, OdinFunction, OdinGlobal, OdinImport, OdinImportKind, OdinLayout, OdinModuleData,
        OdinWitnessDispatchEntry,
    },
    opcode::{HEADER_SIZE, ODIN_MAGIC, SECTION_HEADER_SIZE},
};

/// Decode a `.nyar` module from bytes.
pub fn decode_module(data: &[u8]) -> Result<OdinModuleData, OdinDecodeError> {
    if data.len() < HEADER_SIZE {
        return Err(OdinDecodeError::TooShort);
    }

    let magic = u32::from_be_bytes(data[0..4].try_into().expect("header"));
    if magic != ODIN_MAGIC {
        return Err(OdinDecodeError::InvalidMagic(magic));
    }

    let version = u32::from_le_bytes(data[4..8].try_into().expect("version"));
    let section_count = i32::from_le_bytes(data[8..12].try_into().expect("sections")) as usize;
    let name_offset = i32::from_le_bytes(data[12..16].try_into().expect("name")) as usize;

    let headers_start = HEADER_SIZE;
    let headers_end = headers_start + section_count * SECTION_HEADER_SIZE;
    if data.len() < headers_end {
        return Err(OdinDecodeError::TooShort);
    }

    let mut sections = Vec::with_capacity(section_count);
    for i in 0..section_count {
        let base = headers_start + i * SECTION_HEADER_SIZE;
        sections.push(NyarSectionHeader {
            kind: data[base],
            offset: i32::from_le_bytes(data[base + 1..base + 5].try_into().expect("offset")),
            size: i32::from_le_bytes(data[base + 5..base + 9].try_into().expect("size")),
        });
    }

    let name = read_length_prefixed_string(data, name_offset)?;

    let mut module = OdinModuleData {
        version,
        name,
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

    for section in sections {
        let offset = section.offset as usize;
        let size = section.size as usize;
        if offset + size > data.len() {
            return Err(OdinDecodeError::TooShort);
        }

        let slice = &data[offset..offset + size];
        match section.kind {
            0x01 => decode_constants(slice, &mut module.constants)?,
            0x02 => decode_functions(slice, &mut module.functions)?,
            0x03 => module.code_bytes = slice.to_vec(),
            0x04 => decode_imports(slice, &mut module.imports)?,
            0x05 => decode_exports(slice, &mut module.exports)?,
            0x06 => decode_witness_entries(slice, &mut module.witness_entries)?,
            0x07 => decode_globals(slice, &mut module.globals)?,
            0x08 => decode_init_functions(slice, &mut module.init_function_indices)?,
            0x09 => decode_layouts(slice, &mut module.layouts)?,
            kind => return Err(OdinDecodeError::InvalidSectionKind(kind)),
        }
    }

    Ok(module)
}

#[derive(Debug, Clone, Copy)]
struct NyarSectionHeader {
    kind: u8,
    offset: i32,
    size: i32,
}

fn read_length_prefixed_string(data: &[u8], offset: usize) -> Result<String, OdinDecodeError> {
    if offset + 4 > data.len() {
        return Err(OdinDecodeError::TooShort);
    }
    let len = i32::from_le_bytes(data[offset..offset + 4].try_into().expect("len")) as usize;
    let start = offset + 4;
    let end = start + len;
    if end > data.len() {
        return Err(OdinDecodeError::TooShort);
    }
    String::from_utf8(data[start..end].to_vec()).map_err(|_| OdinDecodeError::InvalidUtf8)
}

fn decode_constants(slice: &[u8], out: &mut Vec<OdinConstant>) -> Result<(), OdinDecodeError> {
    if slice.len() < 4 {
        return Err(OdinDecodeError::TooShort);
    }
    let count = i32::from_le_bytes(slice[0..4].try_into().expect("count")) as usize;
    let mut offset = 4;
    for _ in 0..count {
        if offset >= slice.len() {
            return Err(OdinDecodeError::TooShort);
        }
        let kind = slice[offset];
        offset += 1;
        let (constant, consumed) = decode_constant(kind, &slice[offset..])?;
        offset += consumed;
        out.push(constant);
    }
    Ok(())
}

fn decode_constant(kind: u8, slice: &[u8]) -> Result<(OdinConstant, usize), OdinDecodeError> {
    match kind {
        0x00 => Ok((OdinConstant::Null, 0)),
        0x01 => {
            if slice.is_empty() {
                return Err(OdinDecodeError::TooShort);
            }
            Ok((OdinConstant::Boolean(slice[0] != 0), 1))
        }
        0x11 => {
            if slice.len() < 4 {
                return Err(OdinDecodeError::TooShort);
            }
            Ok((OdinConstant::Integer32(i32::from_le_bytes(slice[0..4].try_into().expect("i32"))), 4))
        }
        0x22 => {
            if slice.len() < 8 {
                return Err(OdinDecodeError::TooShort);
            }
            Ok((OdinConstant::Float64(f64::from_le_bytes(slice[0..8].try_into().expect("f64"))), 8))
        }
        0x05 => {
            if slice.len() < 4 {
                return Err(OdinDecodeError::TooShort);
            }
            let len = i32::from_le_bytes(slice[0..4].try_into().expect("len")) as usize;
            if slice.len() < 4 + len {
                return Err(OdinDecodeError::TooShort);
            }
            let value = String::from_utf8(slice[4..4 + len].to_vec()).map_err(|_| OdinDecodeError::InvalidUtf8)?;
            Ok((OdinConstant::String(value), 4 + len))
        }
        0x06 => {
            if slice.len() < 4 {
                return Err(OdinDecodeError::TooShort);
            }
            let len = i32::from_le_bytes(slice[0..4].try_into().expect("len")) as usize;
            if slice.len() < 4 + len {
                return Err(OdinDecodeError::TooShort);
            }
            Ok((OdinConstant::BigInt(slice[4..4 + len].to_vec()), 4 + len))
        }
        kind => Err(OdinDecodeError::InvalidConstantKind(kind)),
    }
}

fn decode_functions(slice: &[u8], out: &mut Vec<OdinFunction>) -> Result<(), OdinDecodeError> {
    if slice.len() < 4 {
        return Err(OdinDecodeError::TooShort);
    }
    let count = i32::from_le_bytes(slice[0..4].try_into().expect("count")) as usize;
    let mut offset = 4;
    for _ in 0..count {
        let (name, consumed) = read_string_at(slice, offset)?;
        offset += consumed;
        if offset + 16 > slice.len() {
            return Err(OdinDecodeError::TooShort);
        }
        out.push(OdinFunction {
            name,
            arity: i32::from_le_bytes(slice[offset..offset + 4].try_into().expect("arity")),
            local_count: i32::from_le_bytes(slice[offset + 4..offset + 8].try_into().expect("locals")),
            code_offset: i32::from_le_bytes(slice[offset + 8..offset + 12].try_into().expect("off")),
            code_length: i32::from_le_bytes(slice[offset + 12..offset + 16].try_into().expect("len")),
        });
        offset += 16;
    }
    Ok(())
}

fn decode_imports(slice: &[u8], out: &mut Vec<OdinImport>) -> Result<(), OdinDecodeError> {
    if slice.len() < 4 {
        return Err(OdinDecodeError::TooShort);
    }
    let count = i32::from_le_bytes(slice[0..4].try_into().expect("count")) as usize;
    let mut offset = 4;
    for _ in 0..count {
        if offset >= slice.len() {
            return Err(OdinDecodeError::TooShort);
        }
        let kind = match slice[offset] {
            0 => OdinImportKind::Function,
            kind => return Err(OdinDecodeError::InvalidImportKind(kind)),
        };
        offset += 1;
        let (module_name, consumed) = read_string_at(slice, offset)?;
        offset += consumed;
        let (symbol_name, consumed) = read_string_at(slice, offset)?;
        offset += consumed;
        out.push(OdinImport { kind, module_name, symbol_name });
    }
    Ok(())
}

fn decode_exports(slice: &[u8], out: &mut Vec<OdinExport>) -> Result<(), OdinDecodeError> {
    if slice.len() < 4 {
        return Err(OdinDecodeError::TooShort);
    }
    let count = i32::from_le_bytes(slice[0..4].try_into().expect("count")) as usize;
    let mut offset = 4;
    for _ in 0..count {
        if offset >= slice.len() {
            return Err(OdinDecodeError::TooShort);
        }
        let kind = match slice[offset] {
            0 => OdinExportKind::Function,
            1 => OdinExportKind::Global,
            kind => return Err(OdinDecodeError::InvalidExportKind(kind)),
        };
        offset += 1;
        let (symbol_name, consumed) = read_string_at(slice, offset)?;
        offset += consumed;
        if offset + 4 > slice.len() {
            return Err(OdinDecodeError::TooShort);
        }
        out.push(OdinExport { kind, symbol_name, function_index: i32::from_le_bytes(slice[offset..offset + 4].try_into().expect("idx")) });
        offset += 4;
    }
    Ok(())
}

fn decode_globals(slice: &[u8], out: &mut Vec<OdinGlobal>) -> Result<(), OdinDecodeError> {
    if slice.len() < 4 {
        return Err(OdinDecodeError::TooShort);
    }
    let count = i32::from_le_bytes(slice[0..4].try_into().expect("count")) as usize;
    let mut offset = 4;
    for _ in 0..count {
        let (name, consumed) = read_string_at(slice, offset)?;
        offset += consumed;
        let (type_name, consumed) = read_string_at(slice, offset)?;
        offset += consumed;
        out.push(OdinGlobal { name, type_name });
    }
    Ok(())
}

fn decode_init_functions(slice: &[u8], out: &mut Vec<i32>) -> Result<(), OdinDecodeError> {
    if slice.len() < 4 {
        return Err(OdinDecodeError::TooShort);
    }
    let count = i32::from_le_bytes(slice[0..4].try_into().expect("count")) as usize;
    let mut offset = 4;
    for _ in 0..count {
        if offset + 4 > slice.len() {
            return Err(OdinDecodeError::TooShort);
        }
        out.push(i32::from_le_bytes(slice[offset..offset + 4].try_into().expect("idx")));
        offset += 4;
    }
    Ok(())
}

fn decode_layouts(slice: &[u8], out: &mut Vec<OdinLayout>) -> Result<(), OdinDecodeError> {
    if slice.len() < 4 {
        return Err(OdinDecodeError::TooShort);
    }
    let count = i32::from_le_bytes(slice[0..4].try_into().expect("count")) as usize;
    let mut offset = 4;
    for _ in 0..count {
        if offset + 4 > slice.len() {
            return Err(OdinDecodeError::TooShort);
        }
        let field_count = i32::from_le_bytes(slice[offset..offset + 4].try_into().expect("field_count"));
        if field_count < 0 {
            return Err(OdinDecodeError::TooShort);
        }
        out.push(OdinLayout { field_count });
        offset += 4;
    }
    Ok(())
}

fn decode_witness_entries(slice: &[u8], out: &mut Vec<OdinWitnessDispatchEntry>) -> Result<(), OdinDecodeError> {
    if slice.len() < 4 {
        return Err(OdinDecodeError::TooShort);
    }
    let count = i32::from_le_bytes(slice[0..4].try_into().expect("count")) as usize;
    let mut offset = 4;
    for _ in 0..count {
        if offset + 12 > slice.len() {
            return Err(OdinDecodeError::TooShort);
        }
        let method_id = i32::from_le_bytes(slice[offset..offset + 4].try_into().expect("mid"));
        let type_id = i32::from_le_bytes(slice[offset + 4..offset + 8].try_into().expect("tid"));
        offset += 8;
        let (method_name, consumed) = read_string_at(slice, offset)?;
        offset += consumed;
        if offset + 12 > slice.len() {
            return Err(OdinDecodeError::TooShort);
        }
        out.push(OdinWitnessDispatchEntry {
            method_id,
            type_id,
            method_name,
            function_index: i32::from_le_bytes(slice[offset..offset + 4].try_into().expect("fi")),
            interface_id: i32::from_le_bytes(slice[offset + 4..offset + 8].try_into().expect("ii")),
            interface_method_index: i32::from_le_bytes(slice[offset + 8..offset + 12].try_into().expect("imi")),
        });
        offset += 12;
    }
    Ok(())
}

fn read_string_at(slice: &[u8], offset: usize) -> Result<(String, usize), OdinDecodeError> {
    if offset + 4 > slice.len() {
        return Err(OdinDecodeError::TooShort);
    }
    let len = i32::from_le_bytes(slice[offset..offset + 4].try_into().expect("len")) as usize;
    let start = offset + 4;
    let end = start + len;
    if end > slice.len() {
        return Err(OdinDecodeError::TooShort);
    }
    let value = String::from_utf8(slice[start..end].to_vec()).map_err(|_| OdinDecodeError::InvalidUtf8)?;
    Ok((value, 4 + len))
}

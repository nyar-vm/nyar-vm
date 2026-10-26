//! WASI string-literal data section helpers.

use std::collections::BTreeMap;

use crate::lowering::backends::wasm::cabi::LINEAR_HEAP_MIN_BASE;

use super::super::sections::decode_uleb128;

pub(super) const WASI_STRING_DATA_OFFSET: u32 = LINEAR_HEAP_MIN_BASE as u32;

pub(super) fn build_wasi_string_literal_offsets(literals: &[String]) -> BTreeMap<String, u32> {
    let mut map = BTreeMap::new();
    let mut offset = WASI_STRING_DATA_OFFSET;
    for text in literals {
        map.insert(text.clone(), offset);
        let byte_len = text.as_bytes().len() as u32;
        offset += 4 + byte_len;
        offset = (offset + 3) & !3;
    }
    map
}

pub(super) fn wasi_string_data_section_size(literals: &[String]) -> u32 {
    let mut offset = WASI_STRING_DATA_OFFSET;
    for text in literals {
        let byte_len = text.as_bytes().len() as u32;
        offset += 4 + byte_len;
        offset = (offset + 3) & !3;
    }
    offset
}

pub(super) fn build_wasi_string_data_section(literals: &[String]) -> Vec<u8> {
    let total_size = wasi_string_data_section_size(literals) as usize;
    let mut data = vec![0u8; total_size];
    let mut offset = WASI_STRING_DATA_OFFSET;
    for text in literals {
        let bytes = text.as_bytes();
        let byte_len = bytes.len() as u32;
        data[offset as usize..(offset as usize + 4)].copy_from_slice(&byte_len.to_le_bytes());
        data[(offset as usize + 4)..(offset as usize + 4 + bytes.len())].copy_from_slice(bytes);
        offset += 4 + byte_len;
        offset = (offset + 3) & !3;
    }
    data
}

pub(super) fn wasm_function_type_param_bytes(type_bytes: &[u8]) -> Vec<u8> {
    if type_bytes.first() != Some(&acorn_wasm::TYPE_FORM_FUNC) {
        return Vec::new();
    }
    let mut offset = 1usize;
    let param_count = decode_uleb128(type_bytes, &mut offset) as usize;
    let end = offset.saturating_add(param_count).min(type_bytes.len());
    type_bytes[offset..end].to_vec()
}

pub(super) fn wasm_function_type_result_byte(type_bytes: &[u8]) -> Option<u8> {
    if type_bytes.first() != Some(&acorn_wasm::TYPE_FORM_FUNC) {
        return None;
    }
    let mut offset = 1usize;
    let param_count = decode_uleb128(type_bytes, &mut offset) as usize;
    offset = offset.saturating_add(param_count);
    if offset >= type_bytes.len() {
        return None;
    }
    let result_count = decode_uleb128(type_bytes, &mut offset) as usize;
    if result_count == 0 {
        return None;
    }
    type_bytes.get(offset).copied()
}

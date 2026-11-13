//! Node JS glue 宿主导入的物理签名合同。
//!
//! `env` 下的合成宿主导入是 JS 启动壳协议的一部分：字段名、参数载体与返回载体
//! 由本模块统一声明，wasm 降层只消费这里的签名，不再自行按字段名拼指令类型。
//! 协议未声明的字段一律失败闭合，不得现场猜载体。

use acorn_wasm::{VALTYPE_I32, VALTYPE_I64};

/// 一条 JS glue 宿主导入的物理签名。
#[derive(Debug, Clone, Copy)]
pub(crate) struct GlueImportSignature {
    /// `env` 模块下的字段名。
    pub field: &'static str,
    /// 参数 wasm 值类型。
    pub params: &'static [u8],
    /// 返回 wasm 值类型。
    pub results: &'static [u8],
}

/// `env` 模块的物理签名目录。
///
/// 只包含 JS 启动壳协议实际实现的字段；名称本身就是协议 wire 身份，
/// 但签名只从这里取，wasm 降层不得另写一份字段名匹配表。
pub(crate) const GLUE_IMPORT_SIGNATURES: &[GlueImportSignature] = &[
    GlueImportSignature { field: "cli_get_verbose", params: &[], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "const_utf8", params: &[VALTYPE_I32], results: &[VALTYPE_I32] },
    // 目标计划合成的 managed→host 句柄转换入口：(linear_ptr, len) → host i32 handle。
    GlueImportSignature { field: "host_intern_utf8", params: &[VALTYPE_I32, VALTYPE_I32], results: &[VALTYPE_I32] },
    // 目标计划合成的 host 句柄→managed 字符串物化入口：handle → (linear_ptr, len) 多值返回。
    GlueImportSignature { field: "host_utf8_extract", params: &[VALTYPE_I32], results: &[VALTYPE_I32, VALTYPE_I32] },
    // 未捕获 raise 的宿主边界：文本句柄 → 抛出宿主错误，无返回值。
    GlueImportSignature { field: "host_raise", params: &[VALTYPE_I32], results: &[] },
    GlueImportSignature { field: "cli_get_project", params: &[], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "cli_get_target", params: &[], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "cli_get_output", params: &[], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "get_current_directory", params: &[], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "get_input", params: &[], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "read_source_byte", params: &[], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "read_file_text", params: &[VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "file_exists", params: &[VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "directory_exists", params: &[VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "create_directory", params: &[VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "write_file_text", params: &[VALTYPE_I32, VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature {
        field: "get_files",
        params: &[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32],
        results: &[VALTYPE_I32],
    },
    GlueImportSignature { field: "utf8_length", params: &[VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "utf8_index_of", params: &[VALTYPE_I32, VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "utf8_trim", params: &[VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "utf8_to_lower", params: &[VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "utf8_to_upper", params: &[VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "utf8_concat", params: &[VALTYPE_I32, VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "utf8_starts_with", params: &[VALTYPE_I32, VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "utf8_ends_with", params: &[VALTYPE_I32, VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "utf8_contains", params: &[VALTYPE_I32, VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature { field: "utf8_equals", params: &[VALTYPE_I32, VALTYPE_I32], results: &[VALTYPE_I32] },
    GlueImportSignature {
        field: "utf8_replace",
        params: &[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32],
        results: &[VALTYPE_I32],
    },
    GlueImportSignature {
        field: "utf8_slice",
        params: &[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32],
        results: &[VALTYPE_I32],
    },
    GlueImportSignature { field: "emit_byte", params: &[VALTYPE_I32], results: &[] },
    GlueImportSignature { field: "System.Console", params: &[VALTYPE_I32], results: &[] },
    GlueImportSignature { field: "System.Diagnostics.Process", params: &[VALTYPE_I32], results: &[] },
];

/// 协议合成的 Node 宿主导入字段（由 wasm 降层无条件登记）。
///
/// 只放 JS 启动壳协议内建能力；`const_utf8` 由字面量物化开关单独登记，
/// 其余 CLI/文件能力来自计划声明的导入。
pub(crate) const SYNTHETIC_NODE_IMPORT_FIELDS: &[&str] = &[
    "host_raise",
    "utf8_trim",
    "utf8_length",
    "utf8_concat",
    "utf8_starts_with",
    "utf8_ends_with",
    "utf8_contains",
    "utf8_equals",
    "utf8_replace",
    "utf8_index_of",
    "utf8_slice",
    "utf8_to_lower",
    "utf8_to_upper",
];

/// 查询 `env` 字段的物理签名。
pub(crate) fn glue_import_signature(field: &str) -> Option<(&'static [u8], &'static [u8])> {
    GLUE_IMPORT_SIGNATURES
        .iter()
        .find(|signature| signature.field == field)
        .map(|signature| (signature.params, signature.results))
}

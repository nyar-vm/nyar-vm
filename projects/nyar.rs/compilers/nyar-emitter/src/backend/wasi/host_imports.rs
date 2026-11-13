//! WASI 预览宿主导入的物理签名与协议展开。
//!
//! WASI 导入的签名由预览协议（preview2 适配后的 core 导入）定义，wasm 降层只消费
//! 本模块的协议事实，不再按字段名自行拼指令类型。协议未声明的字段一律失败闭合。

use acorn_wasm::{VALTYPE_I32, VALTYPE_I64};

/// 查询 WASI core 导入的物理签名。
///
/// `module` 是适配后的 core 导入模块名（可能带版本后缀），协议按模块族判定时钟/流
/// 语义；字段名是 WIT world 的 wire 身份。
pub(crate) fn wasi_import_signature(module: &str, field: &str) -> Option<(&'static [u8], &'static [u8])> {
    let signature = match field {
        "get-arguments" | "get-environment" => (&[VALTYPE_I32][..], &[][..]),
        "exit" => (&[VALTYPE_I32][..], &[][..]),
        "now" | "resolution" | "get-resolution" if module.contains("monotonic-clock") => (&[][..], &[VALTYPE_I64][..]),
        "now" | "resolution" | "get-resolution" if module.contains("wall-clock") || module.contains("system-clock") => {
            (&[][..], &[VALTYPE_I64, VALTYPE_I32][..])
        }
        "get-random-u64" | "get-insecure-random-u64" => (&[][..], &[VALTYPE_I64][..]),
        "get-random-bytes" | "get-insecure-random-bytes" => (&[VALTYPE_I32][..], &[VALTYPE_I32, VALTYPE_I32][..]),
        "insecure-seed" if module.contains("wasi:random/insecure-seed") => (&[][..], &[VALTYPE_I64, VALTYPE_I64][..]),
        "write-via-stream" if module.contains("wasi:cli/stdout") || module.contains("wasi:cli/stderr") => {
            (&[VALTYPE_I32][..], &[VALTYPE_I32][..])
        }
        field if field.starts_with("[stream-new-") => (&[][..], &[VALTYPE_I64][..]),
        field if field.starts_with("[stream-write-") || field.starts_with("[async][stream-write-") => {
            (&[VALTYPE_I32, VALTYPE_I32, VALTYPE_I32][..], &[VALTYPE_I32][..])
        }
        field if field.starts_with("[stream-drop-writable-") => (&[VALTYPE_I32][..], &[][..]),
        field if field.starts_with("[future-drop-readable-") => (&[VALTYPE_I32][..], &[][..]),
        "read-via-stream" if module.contains("wasi:cli/stdin") => (&[VALTYPE_I32][..], &[VALTYPE_I32][..]),
        "blocking-write-and-flush" | "blocking-read-and-skip" => (&[VALTYPE_I32, VALTYPE_I32][..], &[VALTYPE_I32][..]),
        _ => return None,
    };
    Some(signature)
}

/// 展开 WASI CLI 流的适配器内建导入。
///
/// preview2 适配器把一个 `write-via-stream` 导入展开为 stream/future 内建；
/// 这是适配器协议的固定产物，由本模块负责声明，wasm 降层不参与命名推断。
pub(crate) fn expand_wasi_cli_stream_imports(imports: &mut Vec<(String, String)>) {
    let modules: Vec<String> = imports
        .iter()
        .filter(|(module, field)| field == "write-via-stream" && (module.contains("stdout") || module.contains("stderr")))
        .map(|(module, _)| module.clone())
        .collect();
    for module in modules {
        for field in [
            "[stream-new-0]write-via-stream",
            "[stream-write-0]write-via-stream",
            "[stream-drop-writable-0]write-via-stream",
            "[future-drop-readable-1]write-via-stream",
        ] {
            let key = (module.clone(), field.to_string());
            if !imports.iter().any(|item| item == &key) {
                imports.push(key);
            }
        }
    }
}

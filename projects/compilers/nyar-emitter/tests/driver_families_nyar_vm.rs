//! `nyar-vm` 家族入口合同测试。
//!
//! `resolve_entry_symbols` 只在 `nyar-vm-lane` 构建中存在（`lib.rs` 的重导出与
//! `driver::families::nyar_vm` 模块同为该 feature 门控），因此本目标只在启用该
//! feature 时可编译；断言与生产代码不变。
#![cfg(feature = "nyar-vm-lane")]

use nyar_bytecode::{NYAR_VERSION, NyarExport, NyarExportKind, NyarFunction, NyarModuleData};
use nyar_emitter::resolve_entry_symbols;

fn module() -> NyarModuleData {
    NyarModuleData {
        version: NYAR_VERSION,
        name: "test".to_owned(),
        constants: Vec::new(),
        functions: vec![NyarFunction { name: "owner::main".to_owned(), arity: 0, local_count: 0, code_offset: 0, code_length: 1 }],
        imports: Vec::new(),
        exports: vec![NyarExport { kind: NyarExportKind::Function, symbol_name: "owner::main".to_owned(), function_index: 0 }],
        witness_entries: Vec::new(),
        code_bytes: vec![0x05],
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    }
}

#[test]
fn entry_contract_rejects_suffix_and_unexported_function() {
    let mut module = module();
    assert!(resolve_entry_symbols(&module, &[]).is_err());
    assert_eq!(resolve_entry_symbols(&module, &["owner::main".to_owned()]).unwrap(), vec!["owner::main"]);
    module.exports.clear();
    assert!(resolve_entry_symbols(&module, &["owner::main".to_owned()]).is_err());
}

#[test]
fn entry_contract_rejects_ambiguous_and_invalid_exports() {
    let mut module = module();
    module.exports.push(module.exports[0].clone());
    assert!(resolve_entry_symbols(&module, &["owner::main".to_owned()]).is_err());
    module.exports.pop();
    module.exports[0].function_index = -1;
    assert!(resolve_entry_symbols(&module, &["owner::main".to_owned()]).is_err());
    module.exports[0].function_index = 1;
    assert!(resolve_entry_symbols(&module, &["owner::main".to_owned()]).is_err());
}

#[test]
fn entry_contract_preserves_all_declared_library_exports() {
    let mut module = module();
    module.exports.push(NyarExport { kind: NyarExportKind::Function, symbol_name: "second".to_owned(), function_index: 0 });
    let entries = vec!["second".to_owned(), "owner::main".to_owned()];
    assert_eq!(resolve_entry_symbols(&module, &entries).unwrap(), entries);
}

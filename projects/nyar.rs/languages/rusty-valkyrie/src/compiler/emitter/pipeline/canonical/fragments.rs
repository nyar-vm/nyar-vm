use std::collections::{BTreeMap, BTreeSet};

use nyar_types::{
    CanonicalCallEdge, CanonicalCallee, CanonicalConstant, CanonicalExternalCallEdge, CanonicalFragment, CanonicalFunction, CanonicalOperation,
    Identifier, ImportIndex, ItemInstanceId, LinkedSemanticProgram, StructuredDiagnosticSet,
};

use nyar::SemanticFragment;

use crate::compiler::mir::MirModule;

use super::errors::error;

pub(crate) fn canonical_fragments(
    module: &MirModule,
    linked: &LinkedSemanticProgram,
    functions: &BTreeMap<ItemInstanceId, CanonicalFunction>,
    fragments: &[SemanticFragment],
) -> Result<BTreeMap<Identifier, CanonicalFragment>, StructuredDiagnosticSet> {
    let mut result = BTreeMap::new();
    for fragment in fragments {
        let mut exported_operations = fragment.exported_operations.clone();
        for instance in &exported_operations {
            if !linked.item_instances.contains_key(instance) {
                return Err(error(module, "CAN042", format!("片段 {} 的操作缺少 callable 合同", fragment.id)));
            }
        }
        let entry_operation = fragment.entry_operation;
        if let Some(instance) = entry_operation {
            if !linked.item_instances.contains_key(&instance) {
                return Err(error(module, "CAN043", format!("片段 {} 的入口缺少 callable 合同", fragment.id)));
            }
        }
        let wasm_export_names = fragment.wasm_export_names.clone();
        let mut external_imports = BTreeMap::new();
        let mut internal_call_edges = Vec::new();
        let mut external_call_edges = Vec::new();
        let mut imports_by_instance = BTreeMap::new();
        for (index, record) in &linked.imports {
            if imports_by_instance.insert(record.callee, (*index, record)).is_some() {
                return Err(error(module, "CAN049", "导入实例绑定多个 ImportIndex"));
            }
        }
        let mut pending = exported_operations.clone();
        let mut visited = BTreeSet::new();
        while let Some(caller) = pending.pop() {
            if !visited.insert(caller) {
                continue;
            }
            if let Some((_, record)) = imports_by_instance.get(&caller) {
                if functions.contains_key(&caller) {
                    return Err(error(module, "CAN049", "同一实例同时绑定导入与函数体"));
                }
                external_imports.insert(caller, record.link.clone());
                continue;
            }
            let function = functions
                .get(&caller)
                .ok_or_else(|| error(module, "CAN045", format!("片段 {} 的实例 {caller:?} 没有函数体或导入", fragment.id)))?;
            let literals = function
                .blocks
                .values()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| {
                    if let CanonicalOperation::LoadConstant { constant: CanonicalConstant::Utf8(text) } = &instruction.operation {
                        instruction.results.first().map(|value| (*value, text.clone()))
                    }
                    else {
                        None
                    }
                })
                .collect::<BTreeMap<_, _>>();
            for instruction in function.blocks.values().flat_map(|block| &block.instructions) {
                let CanonicalOperation::Invoke { callee: CanonicalCallee::Item(callee), arguments } = &instruction.operation
                else {
                    continue;
                };
                if let Some((import, record)) = imports_by_instance.get(callee) {
                    if functions.contains_key(callee) {
                        return Err(error(module, "CAN049", "同一实例同时绑定导入与函数体"));
                    }
                    external_imports.insert(*callee, record.link.clone());
                    external_call_edges.push(CanonicalExternalCallEdge {
                        caller,
                        import: *import,
                        arguments: arguments
                            .iter()
                            .filter_map(|value| literals.get(value).cloned().map(nyar_types::ExternalCallArgument::StringLiteral))
                            .collect(),
                    });
                }
                else {
                    if !functions.contains_key(callee) {
                        return Err(error(module, "CAN046", format!("调用实例 {callee:?} 没有函数体或显式导入")));
                    }
                    internal_call_edges.push(CanonicalCallEdge { caller, callee: *callee });
                    pending.push(*callee);
                    if !exported_operations.contains(callee) {
                        exported_operations.push(*callee);
                    }
                }
            }
        }
        if result
            .insert(
                fragment.id.clone(),
                CanonicalFragment {
                    id: fragment.id.clone(),
                    exported_operations,
                    required_capabilities: fragment.required_capabilities.clone(),
                    entry_operation,
                    external_imports,
                    external_call_edges,
                    internal_call_edges,
                    wasm_export_names,
                },
            )
            .is_some()
        {
            return Err(error(module, "CAN051", format!("片段 identity `{}` 重复", fragment.id)));
        }
    }
    Ok(result)
}

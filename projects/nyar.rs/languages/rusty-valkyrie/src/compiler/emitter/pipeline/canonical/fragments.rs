use std::collections::{BTreeMap, BTreeSet};

use nyar_types::{
    CanonicalAggregateOperation, CanonicalCallEdge, CanonicalCallee, CanonicalConstant, CanonicalExternalCallEdge, CanonicalFragment,
    CanonicalFunction, CanonicalOperation, Identifier, ImportIndex, ItemInstanceId, LinkedSemanticProgram, StructuredDiagnosticSet,
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
        let mut wasm_export_names = fragment.wasm_export_names.clone();
        // 聚合直通操作按布局身份冻结导出名与 producer；布局缺失即失败闭合，
        // 不退回声明级布局，也不按类型名或成员名猜载体。
        let mut aggregate_operations = Vec::with_capacity(fragment.aggregate_operations.len());
        let mut aggregate_export_names = BTreeSet::new();
        for operation in &fragment.aggregate_operations {
            let Some(layout_id) = linked.aggregate_layout_by_type.get(&operation.type_id).copied()
            else {
                return Err(error(module, "CAN059", format!("片段 {} 的聚合操作缺少实例级布局: type_id={:?}", fragment.id, operation.type_id)));
            };
            let Some(layout) = linked.aggregate_layouts.layouts.iter().find(|layout| layout.id == layout_id)
            else {
                return Err(error(module, "CAN059", format!("片段 {} 的聚合操作布局记录缺失: layout_id={layout_id}", fragment.id)));
            };
            // 同一 (layout, operation) 恒同名；跨实例唯一。
            let export_name = format!("__glue|{}|{}|{}|{}", layout.namespace, layout.name, layout.id, operation.operation.as_str());
            if !aggregate_export_names.insert(export_name.clone()) {
                return Err(error(module, "CAN060", format!("片段 {} 的聚合导出名重复: {export_name}", fragment.id)));
            }
            if wasm_export_names.iter().any(|(instance, existing)| *instance != operation.instance && existing == &export_name) {
                return Err(error(module, "CAN060", format!("片段 {} 的聚合导出名与已有 wasm 导出名冲突: {export_name}", fragment.id)));
            }
            match wasm_export_names.insert(operation.instance, export_name.clone()) {
                Some(previous) if previous != export_name => {
                    return Err(error(
                        module,
                        "CAN060",
                        format!(
                            "片段 {} 的聚合 producer 已绑定不同导出名: instance={:?}; previous={previous}; next={export_name}",
                            fragment.id, operation.instance
                        ),
                    ));
                }
                _ => {}
            }
            // producer 必须作为片段根出现，`freeze_library_export_abis` 才会冻结它的 ABI。
            if !exported_operations.contains(&operation.instance) {
                exported_operations.push(operation.instance);
            }
            aggregate_operations.push(CanonicalAggregateOperation {
                layout_id,
                operation: operation.operation,
                export_name,
                producer: operation.instance,
            });
        }
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
                    aggregate_operations,
                },
            )
            .is_some()
        {
            return Err(error(module, "CAN051", format!("片段 identity `{}` 重复", fragment.id)));
        }
    }
    Ok(result)
}

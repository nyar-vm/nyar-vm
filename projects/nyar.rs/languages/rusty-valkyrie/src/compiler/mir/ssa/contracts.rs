//! 模块级外部调用、导出与入口合同收集，以及语义片段构造。

use super::model::*;
use super::LambdaInstanceAllocator;
use crate::compiler::types::{
    Identifier, NamePath,
    hir::{HirFunction, HirModule, ValkyrieType, parse_export_spec_from_annotations, resolve_attribute_id},
};
use nyar::{CapabilityTag, RewriteTheory, SemanticFragment};
use nyar_types::{ExternalImportLink, ItemInstanceId, builtin_attribute};
use std::collections::BTreeMap;

/// 检查类型里是否还有未闭合的泛型 binder（`ValkyrieType::Generic(generic)` 且 `generic.binder.is_some()`）。
/// 用于在收集 external_call_contract 时过滤掉 substitution 为空的 instantiated_callable。
fn contains_open_generic(ty: &ValkyrieType) -> bool {
    match ty {
        ValkyrieType::Generic(generic) => generic.binder.is_some(),
        ValkyrieType::Nominal { arguments, .. } => arguments.iter().any(contains_open_generic),
        ValkyrieType::Apply(base, arguments) => contains_open_generic(base) || arguments.iter().any(contains_open_generic),
        ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => contains_open_generic(element),
        ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => items.iter().any(contains_open_generic),
        ValkyrieType::Function(function) => {
            function.params.iter().any(|p| contains_open_generic(p)) || contains_open_generic(&function.return_type)
        }
        ValkyrieType::FixedArray { element, .. } => contains_open_generic(element),
        ValkyrieType::Associated(assoc) => contains_open_generic(&assoc.base) || assoc.type_arguments.iter().any(contains_open_generic),
        _ => false,
    }
}

/// 编译器内建的 workload phase 宿主调用；实例身份在 lowering 前冻结。
pub(crate) struct HostRuntimeCallables {
    pub contracts: Vec<MirExternalCallContract>,
    pub by_name: BTreeMap<String, ItemInstanceId>,
}

/// 跨源码组程序闭包：在全部 HIR 实例池之后分配唯一 host runtime 身份。
pub(crate) fn allocate_program_host_runtime(modules: &[&HirModule]) -> HostRuntimeCallables {
    let declaration_base = modules
        .iter()
        .filter_map(|module| crate::compiler::hir::lowering::max_hir_declaration_index(module))
        .max()
        .map(|index| index.saturating_add(1))
        .unwrap_or(0);
    let instance_base = modules
        .iter()
        .filter_map(|module| crate::compiler::hir::lowering::max_hir_callable_instance_index(module))
        .max()
        .map(|index| index.saturating_add(1))
        .unwrap_or(0);
    collect_host_runtime_callables(declaration_base, instance_base)
}

/// 跨源码组程序闭包：lambda 合成函数的身份必须越过全部 HIR 实例与 host runtime 合同。
///
/// 同一程序内所有模块共享返回的分配器，因此不同模块合成的 lambda 不会撞号。
pub(crate) fn allocate_program_lambda_instances(modules: &[&HirModule], host: &HostRuntimeCallables) -> LambdaInstanceAllocator {
    let declaration_base = modules
        .iter()
        .filter_map(|module| crate::compiler::hir::lowering::max_hir_declaration_index(module))
        .chain(host.contracts.iter().filter_map(|contract| contract.declaration).map(|declaration| declaration.index()))
        .max()
        .map(|index| index.saturating_add(1))
        .unwrap_or(0);
    let instance_base = modules
        .iter()
        .filter_map(|module| crate::compiler::hir::lowering::max_hir_callable_instance_index(module))
        .chain(host.contracts.iter().filter_map(|contract| contract.instance).map(|instance| instance.index()))
        .max()
        .map(|index| index.saturating_add(1))
        .unwrap_or(0);
    LambdaInstanceAllocator::new(declaration_base, instance_base)
}

pub(crate) fn collect_host_runtime_callables(declaration_base: u32, instance_base: u32) -> HostRuntimeCallables {
    let mut contracts = Vec::new();
    let mut by_name = BTreeMap::new();
    for (offset, name) in [(0u32, "begin_phase"), (1u32, "end_phase")] {
        let declaration = nyar_types::ItemId::from_index(declaration_base + offset).expect("host runtime declaration");
        let instance = ItemInstanceId::from_index(instance_base + offset).expect("host runtime instance");
        by_name.insert(name.to_string(), instance);
        contracts.push(MirExternalCallContract {
            declaration: Some(declaration),
            instance: Some(instance),
            symbol: NamePath::new(vec![Identifier::new(name)]),
            link: ExternalImportLink::host(None, vec![name.to_string()]),
            parameter_types: vec![ValkyrieType::Utf8],
            return_type: ValkyrieType::Unit,
        });
    }
    HostRuntimeCallables { contracts, by_name }
}

pub(super) fn collect_external_call_contracts(
    module: &HirModule,
    gate: Option<&crate::compiler::hir::HirMaterializationGate>,
) -> Vec<MirExternalCallContract> {
    fn collect_abstract_functions(
        module: &HirModule,
        gate: Option<&crate::compiler::hir::HirMaterializationGate>,
        out: &mut Vec<MirExternalCallContract>,
    ) {
        // 声明期 abstract 函数（目标绑定属性 / host_contract，无泛型）
        out.extend(
            module
                .functions
                .iter()
                .filter(|function| {
                    function.is_abstract
                        && function.generics.is_empty()
                        && gate.is_none_or(|gate| gate.permits_declaration(function.declaration))
                        && crate::compiler::emitter::backend_contract::interop::function_interop_contract(function).is_some()
                })
                .filter_map(|function| {
                    let link = crate::compiler::emitter::backend_contract::interop::function_interop_contract(function)?;
                    Some(MirExternalCallContract {
                        declaration: function.declaration,
                        instance: function.instance,
                        symbol: crate::compiler::hir::symbols::stable_hir_function_name_path(&module.name, function),
                        link,
                        parameter_types: function.params.iter().map(|parameter| parameter.ty.clone()).collect(),
                        return_type: function.return_type.clone(),
                    })
                }),
        );
        // 泛型实例化产生的 abstract 实例（如 __array_get<i64>）
        out.extend(
            module
                .instantiated_callables
                .iter()
                .filter(|callable| {
                    callable.function.is_abstract
                        && gate.is_none_or(|gate| gate.permits_instance(callable.function.instance))
                        && crate::compiler::emitter::backend_contract::interop::function_interop_contract(&callable.function).is_some()
                })
                .filter_map(|callable| {
                    let link = crate::compiler::emitter::backend_contract::interop::function_interop_contract(&callable.function)?;
                    let param_types: Vec<_> = callable.function.params.iter().map(|p| p.ty.clone()).collect();
                    let return_type = callable.function.return_type.clone();
                    // substitution 未闭合（还有 open generic binder）→ 跳过，避免 canonical 化时 CAN014
                    if param_types.iter().any(contains_open_generic) || contains_open_generic(&return_type) {
                        return None;
                    }
                    Some(MirExternalCallContract {
                        declaration: callable.function.declaration,
                        instance: callable.function.instance,
                        symbol: crate::compiler::hir::symbols::stable_hir_function_name_path(&module.name, &callable.function),
                        link,
                        parameter_types: param_types,
                        return_type,
                    })
                }),
        );
        for submodule in &module.submodules {
            collect_abstract_functions(submodule, gate, out);
        }
    }
    let mut contracts = Vec::new();
    collect_abstract_functions(module, gate, &mut contracts);
    contracts.extend(
        module
            .imported_semantic_exports
            .iter()
            .flat_map(|export| {
                export.functions.iter().filter_map(move |function| {
                    let link = crate::compiler::emitter::backend_contract::interop::function_interop_contract(function)?;
                    let symbol = crate::compiler::hir::symbols::stable_hir_function_name_path(&export.module, function);
                    Some(MirExternalCallContract {
                        declaration: function.declaration,
                        instance: function.instance,
                        symbol,
                        link,
                        parameter_types: function.params.iter().map(|parameter| parameter.ty.clone()).collect(),
                        return_type: function.return_type.clone(),
                    })
                })
            })
            .collect::<Vec<_>>(),
    );
    contracts
}

pub(super) fn collect_surface_contracts(module: &HirModule) -> (Vec<MirExportContract>, Vec<MirEntryContract>) {
    fn visit_function(
        module: &HirModule,
        function: &HirFunction,
        exports: &mut Vec<MirExportContract>,
        entries: &mut Vec<MirEntryContract>,
    ) {
        let symbol = crate::compiler::hir::symbols::stable_hir_function_name_path(&module.name, function);
        if let Some(spec) = parse_export_spec_from_annotations(&function.annotations) {
            exports.push(MirExportContract {
                instance: function.instance,
                symbol: symbol.clone(),
                exported_name: spec.resolve_exported_name(&function.name),
                partition: spec.primary_partition(),
            });
        }
        else if function.annotations.iter().any(|attribute| resolve_attribute_id(attribute) == Some(builtin_attribute::test())) {
            // `[test]` 是测试入口的显式声明：导出名与 run contract 身份就是声明名本身。
            // 这里不做 case 变换或重命名推导，下游按同一名字 invoke。
            exports.push(MirExportContract {
                instance: function.instance,
                symbol: symbol.clone(),
                exported_name: function.name.as_str().to_string(),
                partition: "default".to_string(),
            });
        }
        if function.annotations.iter().any(|attribute| resolve_attribute_id(attribute) == Some(builtin_attribute::main())) {
            entries.push(MirEntryContract { instance: function.instance, symbol });
        }
    }
    fn visit(module: &HirModule, exports: &mut Vec<MirExportContract>, entries: &mut Vec<MirEntryContract>) {
        for function in &module.functions {
            visit_function(module, function, exports, entries);
        }
        // 声明体方法与 impl 成员同样是 surface：`[export]` 挂在成员上就是导出根，
        // 不以“顶层函数”为限。只有已冻结 callable 实例的成员才产生合同——没有实例
        // 的成员（泛型 owner / 泛型 impl 的开放成员）由后续诊断负责，合同层不伪造身份。
        for item in &module.structs {
            for method in item.methods.iter().filter(|method| method.instance.is_some()) {
                visit_function(module, method, exports, entries);
            }
        }
        for item in &module.impls {
            for method in item.methods.iter().filter(|method| method.instance.is_some()) {
                visit_function(module, method, exports, entries);
            }
        }
        for submodule in &module.submodules {
            visit(submodule, exports, entries);
        }
    }

    let mut exports = Vec::new();
    let mut entries = Vec::new();
    visit(module, &mut exports, &mut entries);
    (exports, entries)
}

pub(super) fn collect_semantic_fragments(
    module: &HirModule,
    exports: &[MirExportContract],
    entries: &[MirEntryContract],
) -> Vec<SemanticFragment> {
    fn add(
        function: &HirFunction,
        exports: &[MirExportContract],
        entries: &[MirEntryContract],
        fragments: &mut BTreeMap<Identifier, SemanticFragment>,
    ) {
        let Some(instance) = function.instance
        else {
            return;
        };
        let export = exports.iter().find(|export| export.instance == Some(instance));
        let is_entry = entries.iter().any(|entry| entry.instance == Some(instance));
        if export.is_none() && !is_entry {
            return;
        }
        let id = if let Some(export) = export {
            Identifier::new(&format!("export__{}", export.partition.replace('.', "_")))
        }
        else if entries.len() > 1 {
            let name =
                function.name.as_str().chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' { ch } else { '_' }).collect::<String>();
            Identifier::new(&format!("main_{name}"))
        }
        else {
            Identifier::new("functions")
        };
        let fragment = fragments.entry(id.clone()).or_insert_with(|| SemanticFragment {
            id,
            exported_operations: Vec::new(),
            required_capabilities: Vec::new(),
            reference_management_hint: None,
            entry_operation: None,
            rewrite_theory: RewriteTheory::default(),
            wasm_export_names: BTreeMap::new(),
        });
        if !fragment.exported_operations.contains(&instance) {
            fragment.exported_operations.push(instance);
        }
        if is_entry {
            fragment.entry_operation = Some(instance);
        }
        if let Some(export) = export {
            fragment.wasm_export_names.insert(instance, export.exported_name.clone());
        }
        if crate::compiler::hir::control_flow_validation::function_needs_suspend_fragment(&function.body)
            && !fragment.required_capabilities.iter().any(|capability| capability.as_str() == "suspend")
        {
            fragment.required_capabilities.push(CapabilityTag::new("suspend"));
        }
    }
    let mut fragments = BTreeMap::<Identifier, SemanticFragment>::new();
    for function in &module.functions {
        add(function, exports, entries, &mut fragments);
    }
    // 声明体 / impl 成员与顶层函数共用同一条片段规则：成员上的 `[export]` 必须成为
    // `exported_operations` 与 `wasm_export_names`，否则 Canonical 的链接根拿不到该实例。
    for item in &module.structs {
        for method in &item.methods {
            add(method, exports, entries, &mut fragments);
        }
    }
    for item in &module.impls {
        for method in &item.methods {
            add(method, exports, entries, &mut fragments);
        }
    }
    if fragments.is_empty() {
        fragments.insert(
            Identifier::new("functions"),
            SemanticFragment {
                id: Identifier::new("functions"),
                exported_operations: Vec::new(),
                required_capabilities: Vec::new(),
                reference_management_hint: None,
                entry_operation: None,
                rewrite_theory: RewriteTheory::default(),
                wasm_export_names: BTreeMap::new(),
            },
        );
    }
    fragments.into_values().collect()
}

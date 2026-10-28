use std::collections::BTreeMap;

use nyar_types::{
    CanonicalProgram, CanonicalSemanticMir, Identifier, ImportCapability, ImportIndex, ImportRecord, ItemInstanceRecord, LinkedSemanticProgram,
    NominalInstanceRecord, QualifiedName, StructuredDiagnosticSet, TypeRecord,
    canonical_program::{EntryRecord, ExportRecord},
};

use crate::compiler::{mir::MirModule, types::hir::ValkyrieType};

use super::{
    errors::{canonical_error, ensure_instantiated_signature, error, error_without_module, type_id},
    fragments::canonical_fragments,
    identities::{
        AggregateDeclarations, canonical_type_kind, collect_aggregate_identities, collect_types, is_array_index_valkyrie_type,
        register_aggregate_nominal_instances, register_sum_nominal_instances, validate_callable_identities,
    },
    lower::lower_function,
};

/// 从已完成 HIR/Semantic MIR 合同的模块生成 canonical 成功值；集成测试夹具。
#[doc(hidden)]
pub fn canonical_program_from_semantic_mir(module: &MirModule) -> Result<CanonicalProgram, StructuredDiagnosticSet> {
    if !module.diagnostics.is_empty() {
        return Err(error(module, "CAN033", format!("Semantic MIR lowering 失败: {:?}", module.diagnostics)));
    }
    // witness 分派由 canonical `CanonicalCallee::Witness` 承载稳定身份；
    // 具体 self 的消去与开放泛型的字典 ABI 由表示/目标层按合同决定。
    let type_values = collect_types(module)?;
    validate_callable_identities(module)?;
    let AggregateDeclarations { mut nominals, mut field_records, declaration_nominals } = collect_aggregate_identities(module, &type_values)?;
    let mut linked = LinkedSemanticProgram { module_name: module.name.clone(), ..LinkedSemanticProgram::default() };
    linked.aggregate_layouts = module.aggregate_layouts.clone();
    for (ty, type_id) in &type_values {
        if let Some(layout_id) = crate::compiler::mir::layout_id_for_type(ty, &module.aggregate_layouts) {
            linked.aggregate_layout_by_type.insert(*type_id, layout_id);
        }
    }
    for aggregate in &module.structs {
        let Some(declaration) = aggregate.declaration
        else {
            return Err(error_without_module("CAN023", format!("聚合 `{}` 缺少 declaration identity", aggregate.qualified_name())));
        };
        // 开放泛型声明没有单一物理布局：布局计划按同口径跳过。这里只为单态声明绑定
        // 布局与字段槽位；泛型实例的布局由实例化后的具体名义身份决定。
        if !aggregate.generics.is_empty() {
            continue;
        }
        let Some(layout_id) = linked.aggregate_layouts.declaration_to_layout.get(&declaration).copied()
        else {
            return Err(error_without_module("CAN023", format!("聚合 `{}` 缺少布局绑定", aggregate.qualified_name())));
        };
        linked.aggregate_layout_by_nominal.insert(aggregate.nominal, layout_id);
        let Some(layout) = linked.aggregate_layouts.layouts.iter().find(|layout| layout.id == layout_id)
        else {
            return Err(error_without_module("CAN023", format!("聚合 `{}` 的布局记录缺失", aggregate.qualified_name())));
        };
        if layout.fields.len() != aggregate.fields.len() {
            return Err(error_without_module("CAN023", format!("聚合 `{}` 的字段布局数量不一致", aggregate.qualified_name())));
        }
        for (index, field) in aggregate.fields.iter().enumerate() {
            linked.aggregate_layout_by_field.insert(field.id, (layout_id, index as u32));
        }
    }
    linked.sum_types = module
        .sum_types
        .iter()
        .map(|sum| sum.physical_layout())
        .collect::<Result<_, _>>()
        .map_err(|failure| error(module, "CAN054", format!("sum 物理 payload 类型未完成实例化: {failure}")))?;
    linked.flags_types = module.flags_types.clone();
    linked.singleton_instances = module.singleton_instances.clone();
    for (instance, symbol, display) in module
        .functions
        .iter()
        .filter_map(|function| function.instance.map(|instance| (instance, &function.symbol, function.display_symbol.clone())))
        .chain(
            module
                .external_calls
                .iter()
                .filter_map(|contract| contract.instance.map(|instance| (instance, &contract.symbol, contract.symbol.to_string()))),
        )
    {
        if symbol.parts().is_empty() {
            return Err(error(module, "CAN040", format!("callable `{display}` 缺少冻结源码身份，无法登记限定名称")));
        }
        let name = QualifiedName::new(symbol.parts().to_vec());
        if let Some(previous) = linked.callable_names.insert(instance, name.clone())
            && previous != name
        {
            return Err(error(module, "CAN041", format!("callable identity `{instance}` 对应多个限定名称")));
        }
    }
    let mut declaration_spellings = BTreeMap::new();
    let mut item_spellings = BTreeMap::new();
    register_declaration_spellings(module, &type_values, &mut declaration_spellings, &mut item_spellings)?;
    for (ty, id) in &type_values {
        linked.types.insert(*id, TypeRecord { declaration: *id, kind: canonical_type_kind(ty, &type_values)? });
        if let Some(spelling) = declaration_spellings.get(id) {
            linked.type_surface_spellings.insert(*id, spelling.clone());
        }
        // `Nominal{usize}` / `Named("usize")` 保持名义 TypeId，不得改写成 Primitive；登记后供
        // ArrayGet/ArrayLength/ArrayNew 合同认整数形状（见 LinkedSemanticProgram::array_index_types）。
        if is_array_index_valkyrie_type(ty, module) {
            linked.array_index_types.insert(*id);
        }
    }
    for (nominal, (declaration, semantics)) in &nominals {
        let nominal_fields = field_records.iter().filter_map(|(field, record)| (record.owner == *nominal).then_some(*field)).collect();
        linked.nominal_instances.insert(
            *nominal,
            NominalInstanceRecord { declaration: *declaration, ty: *declaration, semantics: *semantics, fields: nominal_fields },
        );
    }
    let aggregate_bindings =
        register_aggregate_nominal_instances(module, &type_values, &declaration_nominals, &item_spellings, &mut linked, &mut nominals, &mut field_records)?;
    let sum_instance_by_type = register_sum_nominal_instances(module, &type_values, &declaration_spellings, &mut linked, &mut nominals)?;
    // 实例级字段身份同样只认已冻结的记录；声明级与实例级字段在同一张表内稠密互异。
    linked.fields = field_records.clone();
    let mut nominal_by_type = aggregate_bindings.by_type.clone();
    nominal_by_type.extend(sum_instance_by_type.iter().map(|(ty, nominal)| (*ty, *nominal)));
    for function in &module.functions {
        let instance = function
            .instance
            .ok_or_else(|| error(module, "CAN034", format!("函数 `{}` 缺少 Compiler callable identity", function.display_symbol)))?;
        ensure_instantiated_signature(function)?;
        linked.item_instances.insert(
            instance,
            ItemInstanceRecord {
                declaration: function
                    .declaration
                    .ok_or_else(|| error(module, "CAN053", format!("函数 `{}` 缺少声明 identity", function.display_symbol)))?,
                parameter_types: function.param_types.iter().map(|ty| type_id(&type_values, ty)).collect::<Result<_, _>>()?,
                return_type: type_id(&type_values, &function.return_type)?,
            },
        );
    }
    for export in &module.exports {
        let symbol = export.symbol.to_string();
        let instance = export.instance.ok_or_else(|| error(module, "CAN036", format!("导出 `{symbol}` 缺少 Compiler callable identity")))?;
        if linked.exports.insert(instance, ExportRecord { exported_name: export.exported_name.clone() }).is_some() {
            return Err(error(module, "CAN037", format!("callable `{symbol}` 存在重复导出合同")));
        }
    }
    for entry in &module.entries {
        let symbol = entry.symbol.to_string();
        let instance = entry.instance.ok_or_else(|| error(module, "CAN038", format!("入口 `{symbol}` 缺少 Compiler callable identity")))?;
        if linked.entries.insert(instance, EntryRecord).is_some() {
            return Err(error(module, "CAN039", format!("callable `{symbol}` 存在重复入口合同")));
        }
    }
    for (offset, contract) in module.external_calls.iter().enumerate() {
        let instance =
            contract.instance.ok_or_else(|| error(module, "CAN034", format!("导入 `{}` 缺少 Compiler callable identity", contract.symbol)))?;
        linked.item_instances.insert(
            instance,
            ItemInstanceRecord {
                declaration: contract
                    .declaration
                    .ok_or_else(|| error(module, "CAN053", format!("导入 `{}` 缺少声明 identity", contract.symbol)))?,
                parameter_types: contract.parameter_types.iter().map(|ty| type_id(&type_values, ty)).collect::<Result<_, _>>()?,
                return_type: type_id(&type_values, &contract.return_type)?,
            },
        );
        let import = ImportIndex::from_index(offset as u32).ok_or_else(|| error_without_module("CAN031", "import identity 溢出"))?;
        let capability = ImportCapability::new(module.name.clone(), contract.symbol.to_string());
        linked.imports.insert(
            import,
            ImportRecord {
                link: contract.link.clone(),
                capability,
                callee: instance,
                parameter_types: contract.parameter_types.iter().map(|ty| type_id(&type_values, ty)).collect::<Result<_, _>>()?,
                return_type: type_id(&type_values, &contract.return_type)?,
            },
        );
    }
    let mut field_names = BTreeMap::new();
    for structure in &module.structs {
        for field in &structure.fields {
            field_names.insert(field.id, field.name.clone());
        }
    }
    for sum in &module.sum_types {
        for variant in &sum.variants {
            for field in &variant.fields {
                field_names.insert(field.id, field.name.clone());
            }
        }
    }
    let mut next_instruction = 0u32;
    let functions = module
        .functions
        .iter()
        .map(|function| {
            let instance = function
                .instance
                .ok_or_else(|| error(module, "CAN034", format!("函数 `{}` 缺少 Compiler callable identity", function.display_symbol)))?;
            Ok((
                instance,
                lower_function(
                    function,
                    instance,
                    &type_values,
                    &nominal_by_type,
                    &field_records,
                    &aggregate_bindings.field_by_owner,
                    &field_names,
                    &mut next_instruction,
                )?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, StructuredDiagnosticSet>>()?;
    linked.fragments = canonical_fragments(module, &linked, &functions, &module.semantic_fragments)?;
    let program = CanonicalProgram { linked, mir: CanonicalSemanticMir { module_name: module.name.clone(), functions } };
    program.validate().map_err(|error| canonical_error(module, &program, error))?;
    Ok(program)
}

/// 为每个聚合**声明** TypeId 登记 qualified surface spelling。
///
/// `type_surface_spellings` 的键是声明身份，而不是具体实例：泛型实例
/// `Nominal{declaration, arguments}` 在目标降低时先回落取声明 TypeId 的拼写，
/// 再与实参组成 `Apply`；因此这里不能、也不应为实例 TypeId 伪造拼写。
///
/// struct 用 `MirStruct::qualified_name`（`namespace.name`）规范成 `::`；sum
/// 声明当前没有 namespace 事实，如实保留简单名，空 `module_path` 由下游解释。
fn register_declaration_spellings(
    module: &MirModule,
    type_values: &BTreeMap<ValkyrieType, nyar_types::TypeId>,
    spellings: &mut BTreeMap<nyar_types::TypeId, Identifier>,
    item_spellings: &mut BTreeMap<nyar_types::ItemId, String>,
) -> Result<(), StructuredDiagnosticSet> {
    for aggregate in &module.structs {
        let Some(declaration) = aggregate.declaration
        else {
            continue;
        };
        let id = type_id(type_values, &ValkyrieType::Nominal { declaration, arguments: Vec::new() })?;
        let spelling = canonical_surface_spelling(&aggregate.qualified_name());
        item_spellings.insert(declaration, spelling.clone());
        spellings.insert(id, Identifier::new(&spelling));
    }
    for sum in &module.sum_types {
        let Some(declaration) = sum.declaration
        else {
            continue;
        };
        let id = type_id(type_values, &ValkyrieType::Nominal { declaration, arguments: Vec::new() })?;
        item_spellings.insert(declaration, sum.name.clone());
        spellings.insert(id, Identifier::new(&sum.name));
    }
    Ok(())
}

/// 把 `MirStruct::qualified_name` 的点分 namespace 规范成 `QualifiedName` 的 `::` 分隔形式。
///
/// `type_surface_spellings` 是宿主 ABI identity 的上游事实；`module_path_of`
/// 依赖该分隔符拆出完整 module path，避免不同模块的同名 aggregate 在 JS glue 中混淆。
fn canonical_surface_spelling(qualified_name: &str) -> String {
    qualified_name.replace('.', "::")
}

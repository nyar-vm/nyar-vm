//! Valkyrie × 目标适配：把已验证的 Valkyrie MIR 降低为目标私有执行计划。
//!
//! 该 lowering 归语言 × 目标适配器所有：它读取语言自己的 IR 与表示计划，
//! 产生平台输入（`TargetExecutionPlan`），不把语言语义塞回 Nyar 公共类型。

use std::collections::BTreeMap;

use nyar_emitter::{
    BackendImport, ProgramTextRepresentation, SumRepresentation, SumVariantRepresentation, TargetExecutionPlan, TargetPlanInput,
    backend_plan_views::ExecutableFunction,
    contracts::{Block, BlockRef, ValueRef},
};
use nyar_types::{
    CanonicalPrimitiveType, CanonicalTypeKind, Constant, Instruction, InstructionKind, ItemInstanceId, NyarFunctionType, NyarType, Operand,
    Terminator, Value, ValueOrigin,
    semantic_ids::{MirValueId, NominalInstanceId, TypeId, VariantId},
};

use super::super::super::valkyrie_mir::{
    CompiledValkyrieProgram, InvokeLowering, TextRepresentation as PlannedTextRepresentation, ValueRepresentation, ValkyrieMir,
    ValkyrieMirArrayInitialization, ValkyrieMirBlockId, ValkyrieMirCallee, ValkyrieMirConstant, ValkyrieMirEffectKind, ValkyrieMirFunction,
    ValkyrieMirInstruction, ValkyrieMirOperand, ValkyrieMirOperation, ValkyrieMirProgram, ValkyrieMirTerminator, ValkyrieMirWitness,
};

type Result<T> = std::result::Result<T, String>;

/// 语言 × 目标适配失败：片段/根身份缺失有独立变体，其余降层原因保留细节文本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetPlanError {
    /// 片段身份不在链接结果中。
    UnknownFragment {
        /// 请求的片段身份。
        fragment: String,
    },
    /// 可达根既不是函数体，也不是已声明导入。
    UnknownRoot {
        /// 请求的根实例。
        instance: ItemInstanceId,
    },
    /// 语言合同无法无损投影到目标计划。
    Unsound {
        /// 具体原因。
        reason: String,
    },
}

impl std::fmt::Display for TargetPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownFragment { fragment } => write!(f, "Valkyrie 片段 `{fragment}` 不存在"),
            Self::UnknownRoot { instance } => write!(f, "Compiler callable 实例 `{instance:?}` 缺少函数体"),
            Self::Unsound { reason } => f.write_str(reason),
        }
    }
}

impl std::error::Error for TargetPlanError {}

impl From<String> for TargetPlanError {
    fn from(reason: String) -> Self {
        Self::Unsound { reason }
    }
}

/// 从已验证的 Valkyrie 程序生成目标私有执行计划。
///
/// 该函数是 Valkyrie × 目标适配器的入口：读取 Valkyrie MIR 与表示计划，
/// 产出平台输入；不可无损投影的语言事实按结构化错误失败。
pub fn from_compiled_program(
    program: &CompiledValkyrieProgram,
    fragment_id: &nyar::Identifier,
    theory_bundle: nyar::TheoryBundle,
    roots: &[ItemInstanceId],
    sum_tag_width: u32,
) -> std::result::Result<TargetExecutionPlan, TargetPlanError> {
    let canonical = program.mir_program();
    if !canonical.linked.fragments.contains_key(fragment_id) {
        return Err(TargetPlanError::UnknownFragment { fragment: fragment_id.to_string() });
    }
    for root in roots {
        if !canonical.mir.functions.contains_key(root) {
            return Err(TargetPlanError::UnknownRoot { instance: *root });
        }
    }
    build_plan(program, fragment_id, theory_bundle, roots, sum_tag_width).map_err(TargetPlanError::from)
}

fn build_plan(
    program: &CompiledValkyrieProgram,
    fragment_id: &nyar::Identifier,
    theory_bundle: nyar::TheoryBundle,
    roots: &[ItemInstanceId],
    sum_tag_width: u32,
) -> Result<TargetExecutionPlan> {
    let canonical = program.mir_program();
    let fragment = canonical.linked.fragments.get(fragment_id).ok_or_else(|| format!("Canonical 片段 `{fragment_id}` 不存在"))?;
    let mut declared_imports = BTreeMap::new();
    for (index, import) in &canonical.linked.imports {
        if canonical.mir.functions.contains_key(&import.callee) {
            return Err(format!("callable 实例 `{:?}` 同时绑定函数体与导入", import.callee));
        }
        if declared_imports.insert(import.callee, (*index, import)).is_some() {
            return Err(format!("callable 实例 `{:?}` 绑定多个 ImportIndex", import.callee));
        }
    }
    for (callee, (idx, import)) in &declared_imports {
        let link_str = format!("{:?}", import.link);
        let symbol = canonical.linked.callable_names.get(callee).cloned().unwrap_or_default();
    }
    let mut pending = roots
        .iter()
        .copied()
        .map(|instance| {
            if !canonical.mir.functions.contains_key(&instance) {
                return Err(format!("Compiler callable 实例 `{instance:?}` 缺少 canonical 函数体"));
            }
            Ok(instance)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut seen = std::collections::BTreeSet::new();
    let mut functions = BTreeMap::new();
    let mut abi_names = BTreeMap::new();
    let mut imports = BTreeMap::new();
    while let Some(instance) = pending.pop() {
        if !seen.insert(instance) {
            continue;
        }
        if let Some((index, import)) = declared_imports.get(&instance) {
            let parameter_types = import.parameter_types.iter().map(|ty| lower_type(canonical, *ty)).collect::<Result<Vec<_>>>()?;
            let return_type = lower_type(canonical, import.return_type)?;
            imports.insert(
                instance,
                BackendImport { index: *index, link: import.link.clone(), parameter_types, return_type },
            );
            continue;
        }
        let name = canonical.linked.callable_names.get(&instance).ok_or_else(|| format!("callable 实例 `{instance:?}` 缺少 ABI 名称"))?;
        abi_names.insert(instance, name.clone());
        let function = canonical.mir.functions.get(&instance).ok_or_else(|| format!("callable `{name}` 缺少 canonical 函数体"))?;
        // 可达调用的目标必须已有闭合实例。签名仍含 Generic 的声明模板没有被前端证明
        // 不可达，只是从前端单态化里漏掉；把它从闭包队列里滤掉等于把这处缺口伪装成
        // 编译成功，因此这里失败闭合，要求前端提供单态化/evidence 合同。
        if signature_contains_generic(canonical, function) {
            return Err(format!(
                "可达调用的目标缺少闭合实例: instance={instance:?} symbol={name} 的声明签名仍含 Generic 类型参数，\
                 backend 无法编码；需前端提供单态化/evidence 合同"
            ));
        }
        let (lowered, callees) = lower_function(program, function)?;
        pending.extend(callees);
        functions.insert(instance, lowered);
    }
    let mut aggregate_layout_by_nyar_type = BTreeMap::new();
    // 字段声明侧的身份：布局计划只记构造器拼写，SSA 记结算布局；字段合同
    // 必须以 canonical 为两侧冻结的同一布局身份比对，而不是比较拼写形态。
    let mut field_type_layouts = BTreeMap::new();
    for (field_id, record) in &canonical.linked.fields {
        if let Some(layout_id) = canonical.linked.aggregate_layout_by_type.get(&record.ty) {
            field_type_layouts.insert(*field_id, *layout_id);
        }
    }
    for (type_id, layout_id) in &canonical.linked.aggregate_layout_by_type {
        // lower_type 只对“可执行类型”有结果；函数签名等非值类型跳过即可，
        // 它们不会成为容器元素或导出位置。
        if let Ok(ty) = lower_type(canonical, *type_id) {
            aggregate_layout_by_nyar_type.insert(ty.clone(), *layout_id);
            // 同一实例类型有两种冻结形态：SSA/签名已结算为携带布局身份的
            // `Nominal`，而布局计划的字段事实仍以未结算的 `Apply` 记录。
            // 两者必须指向同一布局，否则字段合同会因形态差异误判为不同类型。
            // 键按 `contract_type_key` 归一（抹掉诊断拼写），两侧同一键即同一类型。
            aggregate_layout_by_nyar_type.insert(ty.contract_key(), *layout_id);
            if let Ok(unsettled) = lower_unsettled_contract_key(canonical, *type_id) {
                aggregate_layout_by_nyar_type.insert(unsettled, *layout_id);
            }
        }
        // 文本载体声明（`std.text::Utf8Text`/`Utf16Text`）在签名里折叠成编码基元，
        // 槽类型是 `NyarType::Utf8`/`Utf16` 而不是名义拼写；托管布局必须同时以
        // 折叠后的编码身份为键登记，否则按编码身份取槽的 ABI 派生点查不到布局，
        // 会把托管槽误解释成 raw 串。键来源是 canonical 已冻结的
        // `text_encoding_types`（01 §5.4：文本身份只来自唯一已冻结编码身份），
        // 不按名字拼写回查。
        if let Some(encoding) = canonical.linked.text_encoding_types.get(type_id) {
            let folded = match encoding {
                CanonicalPrimitiveType::Utf8 => NyarType::Utf8,
                CanonicalPrimitiveType::Utf16 => NyarType::Utf16,
                other => return Err(format!("文本载体 `TypeId` 登记了非文本编码身份: {other:?}")),
            };
            if let Some(existing) = aggregate_layout_by_nyar_type.get(&folded).copied() {
                if existing != *layout_id {
                    return Err(format!("文本编码身份 `{folded:?}` 登记了冲突的托管布局: {existing:?} vs {layout_id:?}"));
                }
            }
            aggregate_layout_by_nyar_type.insert(folded, *layout_id);
        }
    }
    // unite 没有物理布局身份，只能按已冻结的名义实例类型回查；因此用
    // `nominal_instances` 的声明 TypeId 逐一 `lower_type`，不按名字拼写匹配。
    let mut sum_layout_by_nyar_type = BTreeMap::new();
    for sum in &canonical.linked.sum_types {
        if !sum.is_unite {
            continue;
        }
        if let Some(record) = canonical.linked.nominal_instances.get(&sum.nominal) {
            if let Ok(ty) = lower_type(canonical, record.ty) {
                sum_layout_by_nyar_type.insert(ty, sum.clone());
            }
        }
    }
    let plan = TargetPlanInput {
        module_name: canonical.linked.module_name.clone(),
        fragment_id: fragment.id.clone(),
        exported_operations: fragment.exported_operations.clone(),
        required_capabilities: fragment.required_capabilities.clone(),
        theory_bundle,
        entry_operation: fragment.entry_operation,
        export_names: fragment.export_names.clone(),
        external_import_links: fragment.external_imports.clone(),
        external_call_edges: fragment.external_call_edges.clone(),
        internal_call_edges: fragment.internal_call_edges.clone(),
        aggregate_layouts: canonical.linked.aggregate_layouts.clone(),
        aggregate_layout_by_nominal: canonical.linked.aggregate_layout_by_nominal.clone(),
        aggregate_layout_by_field: canonical.linked.aggregate_layout_by_field.clone(),
        field_type_layouts,
        aggregate_layout_by_type: canonical.linked.aggregate_layout_by_type.clone(),
        aggregate_layout_by_nyar_type,
        sum_layout_by_nyar_type,
        flags_types: canonical.linked.flags_types.clone(),
        singleton_instances: canonical.linked.singleton_instances.clone(),
        functions,
        abi_names,
        imports,
        sum_reps: platform_sum_representations(canonical, sum_tag_width)?,
        // 程序级文本表示模式只来自正式表示计划的显式事实（`RepresentationPlan`）；
        // 计划未声明时保持 `None`，后端按 managed 语义 fail-closed，禁止在这里
        // 按布局注册表是否含 `utf8` 反推模式。
        program_text_representation: program.representation().text_representation.map(|representation| match representation {
            PlannedTextRepresentation::Raw => ProgramTextRepresentation::Raw,
            PlannedTextRepresentation::Managed => ProgramTextRepresentation::Managed,
        }),
    };
    Ok(TargetExecutionPlan::from_prepared_input(plan))
}

fn lower_function(program: &CompiledValkyrieProgram, function: &ValkyrieMirFunction) -> Result<(ExecutableFunction, Vec<ItemInstanceId>)> {
    let canonical = program.mir_program();
    for (value, type_id) in &function.value_types {
        let identity = nyar_types::ValueIdentity::new(function.instance, *value);
        match program.representation().value_reps.get(&identity) {
            Some(ValueRepresentation::Specialized) => {}
            // 实例级布局已由 canonical 冻结为 Reference 时，GC struct 就是该值的
            // 目标私有载体；没有实例级 Reference 布局仍失败关闭。
            Some(ValueRepresentation::Reified) if has_reference_instance_layout(canonical, *type_id) => {}
            Some(representation) => {
                return Err(format!("值 `{identity:?}` 的表示 `{representation:?}` 尚无目标私有载体合同，拒绝按语义类型重新选择表示"));
            }
            None => {
                return Err(format!("值 `{identity:?}` 缺少 RepresentationPlan 载体合同"));
            }
        }
    }
    for block in function.blocks.values() {
        for instruction in &block.instructions {
            for (nominal, variant) in canonical_sum_operations(instruction) {
                // variant/payload 的唯一来源是链接程序；平台适配只负责把它们映射成
                // 目标 tag 槽，不重新认定 variant 身份。
                let Some(sum) = canonical.linked.sum_types.iter().find(|sum| sum.nominal == nominal)
                else {
                    return Err(format!("sum `{nominal:?}` 缺少链接期 variant 合同"));
                };
                if !sum.variants.iter().any(|known| known.id == variant) {
                    return Err(format!("sum variant `{nominal:?}/{variant:?}` 缺少链接期 variant 合同"));
                }
            }
            if let ValkyrieMirOperation::Invoke { callee, .. } = &instruction.operation {
                match (callee, program.representation().invoke_lowerings.get(&instruction.id)) {
                    (ValkyrieMirCallee::Item(_), Some(InvokeLowering::Direct)) => {}
                    (ValkyrieMirCallee::Witness(_), Some(InvokeLowering::TypedWitness)) => {
                        // 放行！在 lower_operation 里尝试消去 Witness 为具体 Item 调用。
                        // self_type 仍开放时（`None`）由 lower_operation 返回结构化失败。
                    }
                    (ValkyrieMirCallee::Value(callee), Some(InvokeLowering::TypedReference)) => {
                        // 函数值 callee 的间接调用合同：callee SSA 必须携带 canonical 冻结的
                        // Function 类型事实，wasm 侧据此发射 `call_indirect`；缺事实失败闭合。
                        let Some(callee_type) = function.value_types.get(callee)
                        else {
                            return Err(format!(
                                "调用指令 `{}` 的 callee `Value({callee:?})` 缺少 Semantic MIR 类型事实",
                                instruction.id.index()
                            ));
                        };
                        if !matches!(
                            canonical.linked.types.get(callee_type).map(|record| &record.kind),
                            Some(CanonicalTypeKind::Function { .. })
                        ) {
                            return Err(format!(
                                "调用指令 `{}` 的 callee `Value({callee:?})` 类型 `{callee_type:?}` 不是函数类型，尚无目标私有调用合同，拒绝发射间接调用",
                                instruction.id.index()
                            ));
                        }
                    }
                    (_, None) => {
                        return Err(format!("调用指令 `{}` 缺少 RepresentationPlan 降低合同", instruction.id.index()));
                    }
                    (_, Some(lowering)) => {
                        return Err(format!(
                            "调用指令 `{}` 的表示 `{lowering:?}` 与 callee `{callee:?}` 尚无目标私有调用合同，拒绝改用普通调用",
                            instruction.id.index()
                        ));
                    }
                }
            }
        }
    }
    let mut values =
        function.value_types.keys().map(|value| Value { id: ValueRef(value.index()), origin: ValueOrigin::Temporary }).collect::<Vec<_>>();
    for (index, (value, _)) in function.parameters.iter().enumerate() {
        let row = values.iter_mut().find(|row| row.id.0 == value.index()).ok_or_else(|| format!("入口参数缺少 SSA 值合同"))?;
        // canonical 已保留 compiler schema 的声明参数名；缺失只是防御性回落。
        let name = function.parameter_names.get(index).cloned().unwrap_or_else(|| format!("arg{index}"));
        row.origin = ValueOrigin::Parameter { index, name };
    }
    let mut callees = Vec::new();
    let mut blocks = Vec::new();
    for block in function.blocks.values() {
        let instructions = block
            .instructions
            .iter()
            .map(|instruction| {
                Ok(Instruction {
                    id: instruction.id,
                    results: instruction.results.iter().map(|value| ValueRef(value.index())).collect(),
                    kind: lower_operation(canonical, function, instruction, &mut callees)?,
                    provenance: nyar_types::ProvenanceId::from_index(instruction.id.index()).ok_or_else(|| format!("指令 identity 溢出"))?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        blocks.push(Block {
            id: BlockRef(block.id.0),
            label: format!("block_{}", block.id.0),
            parameters: block.parameters.iter().map(|(value, _)| ValueRef(value.index())).collect(),
            instructions,
            terminator: lower_terminator(block.terminator.clone())?,
        });
    }
    let symbol = canonical.linked.callable_names.get(&function.instance).ok_or_else(|| format!("函数实例缺少 ABI 名称"))?.to_string();
    let type_of = |id| lower_type(canonical, id);
    let value_layouts = function
        .value_types
        .iter()
        .filter_map(|(value, type_id)| {
            canonical.linked.aggregate_layout_by_type.get(type_id).copied().map(|layout| (ValueRef(value.index()), layout))
        })
        .collect();
    let return_layout = canonical.linked.aggregate_layout_by_type.get(&function.return_type).copied();
    Ok((
        ExecutableFunction {
            symbol,
            return_type: type_of(function.return_type)?,
            param_types: function.parameters.iter().map(|(_, id)| type_of(*id)).collect::<Result<_>>()?,
            value_types: function.value_types.iter().map(|(value, id)| Ok((ValueRef(value.index()), type_of(*id)?))).collect::<Result<_>>()?,
            value_layouts,
            return_layout,
            entry: BlockRef(function.entry.0),
            values,
            suspend_points: Vec::new(),
            frame_layouts: Vec::new(),
            continuations: Vec::new(),
            case_chains: Vec::new(),
            #[allow(deprecated)]
            state_machine: None,
            suspend_plan: None,
            blocks,
            diagnostics: Vec::new(),
        },
        callees,
    ))
}

/// 该类型是否有 canonical 冻结的实例级 Reference 布局（GC struct 载体）。
fn has_reference_instance_layout(program: &ValkyrieMirProgram, type_id: nyar_types::TypeId) -> bool {
    let Some(layout_id) = program.linked.aggregate_layout_by_type.get(&type_id).copied()
    else {
        return false;
    };
    program
        .linked
        .aggregate_layouts
        .layouts
        .iter()
        .find(|layout| layout.id == layout_id)
        .is_some_and(|layout| layout.storage == nyar_types::StorageKind::Reference)
}

fn canonical_sum_operations(instruction: &crate::compiler::valkyrie_mir::ValkyrieMirInstruction) -> Vec<(nyar_types::NominalInstanceId, nyar_types::VariantId)> {
    match instruction.operation {
        ValkyrieMirOperation::SumNew { nominal, variant, .. }
        | ValkyrieMirOperation::SumPayloadGet { nominal, variant, .. }
        | ValkyrieMirOperation::SumVariantIs { nominal, variant, .. } => vec![(nominal, variant)],
        _ => Vec::new(),
    }
}

/// 把链接期 sum/variant 事实映射成平台 sum 计划。
///
/// variant 身份与 payload 类型来自链接程序；tag 宽度由平台提供，
/// 语言表示计划不再声明它。
fn platform_sum_representations(
    canonical: &ValkyrieMirProgram,
    tag_width: u32,
) -> Result<BTreeMap<NominalInstanceId, SumRepresentation>> {
    let mut reps = BTreeMap::new();
    for sum in &canonical.linked.sum_types {
        if !canonical.linked.nominal_instances.contains_key(&sum.nominal) {
            return Err(format!("sum 布局引用未知 nominal identity: {:?}", sum.nominal));
        }
        let mut variants = BTreeMap::new();
        for variant in &sum.variants {
            let Some(record) = canonical.linked.variants.get(&(sum.nominal, variant.id))
            else {
                return Err(format!("sum 布局引用未知 variant identity: {:?}/{:?}", sum.nominal, variant.id));
            };
            if record.payload_type.is_some() != variant.payload_type.is_some() {
                return Err(format!("sum variant payload 合同不一致: {:?}/{:?}", sum.nominal, variant.id));
            }
            let entry = SumVariantRepresentation { id: variant.id, tag: variant.tag, payload_type: record.payload_type };
            if variants.insert(variant.id, entry).is_some() {
                return Err(format!("sum variant identity 重复: {:?}/{:?}", sum.nominal, variant.id));
            }
        }
        if reps.insert(sum.nominal, SumRepresentation { nominal: sum.nominal, tag_width, variants }).is_some() {
            return Err(format!("sum nominal identity 重复: {:?}", sum.nominal));
        }
    }
    Ok(reps)
}

/// 在 target_execution_plan 边界把 trait Witness 分派降为具体 Item 调用。
///
/// 策略（按优先级）：
/// 1. 直接用 trait 方法槽 ItemId 在 `linked.item_instances` 查 —— 当 impl 方法的
///    declaration 正好等于 trait 方法槽时命中；
/// 2. 在 `mir.functions` 里找第一个参数（self）类型等于 witness.self_type 的函数实例；
///    多个候选时返回结构化失败，避免按名字/短名猜测。
/// 在 target_execution_plan 边界把 trait Witness 分派降为具体 Item 调用。
///
/// witness 的实例身份是前端事实：`self` 类型具体化后，前端本应在
/// `instantiate_generic_callables` 里换绑成真实实现。能走到这里只剩前端仍判为
/// 开放的 witness，因此这里只允许按已冻结的 `(method declaration, self 类型)`
/// 精确匹配唯一实例。不得按名字、候选顺序或「第一个参数看起来一样」猜实现——
/// 猜错会把 `Hash::hash` 绑到 `i64.infix -` 这种签名不同的算子上。
/// 目标层不得为 witness 分派扫描语义实例库。
///
/// 具体 witness 必须在 canonical 实例化固定点消去为 `ValkyrieMirCallee::Item`；
/// 走到目标层仍保留 `Witness` 说明上游未闭合，必须结构化失败而不是按
/// `(method declaration, self 类型)` 在这里再选一次实现。
fn resolve_witness_to_item(_program: &ValkyrieMirProgram, witness: &crate::compiler::valkyrie_mir::ValkyrieMirWitness) -> Result<ItemInstanceId> {
    Err(format!(
        "canonical 未消去的 trait witness 分派到达目标层：trait={:?} method={:?} self_type={:?}；\
         具体 witness 必须在实例化固定点冻结 `ItemInstanceId`",
        witness.trait_declaration, witness.method_declaration, witness.self_type
    ))
}

fn lower_operation(
    program: &ValkyrieMirProgram,
    function: &crate::compiler::valkyrie_mir::ValkyrieMirFunction,
    instruction: &crate::compiler::valkyrie_mir::ValkyrieMirInstruction,
    callees: &mut Vec<ItemInstanceId>,
) -> Result<InstructionKind> {
    let value = |id: nyar_types::MirValueId| Operand::Value(ValueRef(id.index()));
    // 捕获环境必须携带 SSA 身份；常量环境没有身份，禁止目标侧重造。
    let operand = |operand: &crate::compiler::valkyrie_mir::ValkyrieMirOperand| -> Result<Operand> {
        match operand {
            crate::compiler::valkyrie_mir::ValkyrieMirOperand::Value(id) => Ok(Operand::Value(ValueRef(id.index()))),
            crate::compiler::valkyrie_mir::ValkyrieMirOperand::Constant(_) => Err("闭包捕获环境必须是已定义的 SSA 值".to_string()),
        }
    };
    let layout_for_type = |type_id: nyar_types::TypeId, operation: &str| {
        program
            .linked
            .aggregate_layout_by_type
            .get(&type_id)
            .copied()
            .ok_or_else(|| format!("{operation} 缺少编译器绑定的布局身份: {type_id:?}"))
    };
    let value_type = |value: nyar_types::MirValueId| {
        function.value_types.get(&value).copied().ok_or_else(|| format!("聚合值 `{value:?}` 缺少 Semantic MIR 类型事实"))
    };
    Ok(match &instruction.operation {
        ValkyrieMirOperation::Invoke { callee: ValkyrieMirCallee::Item(instance), arguments } => {
            if !program.linked.item_instances.contains_key(instance) {
                return Err(format!("调用 identity `{instance:?}` 未解析"));
            }
            callees.push(*instance);
            InstructionKind::Call { callee: Operand::Item(*instance), arguments: arguments.iter().map(|id| value(*id)).collect() }
        }
        ValkyrieMirOperation::Invoke { callee: ValkyrieMirCallee::Witness(witness), arguments } => {
            let instance = resolve_witness_to_item(program, &witness)?;
            callees.push(instance);
            InstructionKind::Call { callee: Operand::Item(instance), arguments: arguments.iter().map(|id| value(*id)).collect() }
        }
        ValkyrieMirOperation::Invoke { callee: ValkyrieMirCallee::Value(callee), arguments } => {
            InstructionKind::Call { callee: value(*callee), arguments: arguments.iter().map(|id| value(*id)).collect() }
        }
        // 具名函数作值：实例身份由前端冻结，目标层只校验链接事实并保留身份。
        // 被作值引用的函数不出现在任何直接调用边上，可达性必须由本指令登记，
        // 否则单态化后的实现函数体会被可执行闭包漏掉。
        ValkyrieMirOperation::LoadFunctionRef { instance, environment, captured } => {
            if !program.linked.item_instances.contains_key(instance) {
                return Err(format!("函数值 identity `{instance:?}` 未解析"));
            }
            callees.push(*instance);
            InstructionKind::LoadFunctionRef {
                instance: *instance,
                environment: environment.as_ref().map(|environment| operand(environment)).transpose()?,
                captured: captured.clone(),
            }
        }
        ValkyrieMirOperation::Copy { source } => InstructionKind::Copy { source: value(*source) },
        ValkyrieMirOperation::StoreVar { name, value: stored, ty } => {
            InstructionKind::StoreVar { name: name.clone(), value: value(*stored), ty: ty.map(|id| lower_type(program, id)).transpose()? }
        }
        ValkyrieMirOperation::AggregateCopy { source, destination } => {
            let source_type = value_type(*source)?;
            let destination_type = value_type(*destination)?;
            if source_type != destination_type {
                return Err(format!("AggregateCopy 源与目标类型不一致: {source_type:?} != {destination_type:?}"));
            }
            InstructionKind::AggregateCopy {
                layout_id: layout_for_type(source_type, "AggregateCopy")?,
                source: value(*source),
                dest: value(*destination),
            }
        }
        // 文本常量的编码身份由常量变体自身决定（`Utf8` / `Utf16`），不靠名字或源码回推；
        // canonical 层不保存可被字符串重建的类型，因此在这里从编码恢复。
        ValkyrieMirOperation::LoadConstant { constant } => {
            InstructionKind::LoadConstant { constant: lower_constant(constant), ty: constant_encoding_type(constant) }
        }
        // 结构操作已由 frontend 依据自身内建结构操作身份判定完毕，事实所有权在此结束；
        // backend-private 计划只做载体映射，wasm backend 负责编码成 wasm GC 指令。
        // 在此重新拒绝会把已确认的结构语义降级成字符串错误，属禁止的向上推断。
        ValkyrieMirOperation::ArrayGet { array, index } => InstructionKind::ArrayGet { array: value(*array), index: value(*index) },
        ValkyrieMirOperation::ArraySet { array, index, value: stored } => {
            InstructionKind::ArraySet { array: value(*array), index: value(*index), value: value(*stored) }
        }
        ValkyrieMirOperation::ArrayLength { array } => InstructionKind::ArrayLength { array: value(*array) },
        ValkyrieMirOperation::NullIsNull { value: operand } => InstructionKind::NullIsNull { value: value(*operand) },
        ValkyrieMirOperation::NullUnwrap { value: operand } => InstructionKind::NullUnwrap { value: value(*operand) },
        ValkyrieMirOperation::ArrayNew { array_type, length, initialization } => InstructionKind::ArrayNew {
            array_type: lower_type(program, *array_type)?,
            length: value(*length),
            initialization: match initialization {
                ValkyrieMirArrayInitialization::Default => nyar_types::ArrayInitialization::Default,
                ValkyrieMirArrayInitialization::Fill(fill) => nyar_types::ArrayInitialization::Fill(value(*fill)),
            },
        },
        ValkyrieMirOperation::ArrayFromElements { array_type, elements } => {
            // 只有 FixedArray 才有聚合布局；动态数组没有对象 body 布局，
            // 不得用 dummy 身份污染共享计划。
            let layout_id = match program.linked.types.get(array_type).map(|row| &row.kind) {
                Some(nyar_types::CanonicalTypeKind::Array { length: Some(_), .. }) => Some(layout_for_type(*array_type, "ArrayFromElements")?),
                _ => None,
            };
            InstructionKind::ArrayFromElements {
                layout_id,
                array_type: lower_type(program, *array_type)?,
                elements: elements.iter().map(|id| value(*id)).collect(),
            }
        }
        ValkyrieMirOperation::TupleNew { fields, element_types } => {
            let result = instruction.results.first().copied().ok_or_else(|| format!("TupleNew 缺少结果值"))?;
            let result_type = value_type(result)?;
            // 标签只属于 canonical 身份；目标布局按位置，带标签元组与位置元组共享同一构造合同。
            if !matches!(
                program.linked.types.get(&result_type).map(|row| &row.kind),
                Some(nyar_types::CanonicalTypeKind::Tuple(_) | nyar_types::CanonicalTypeKind::LabeledTuple { .. })
            ) {
                return Err(format!("TupleNew 结果不是 tuple 类型: {result_type:?}"));
            }
            if fields.len() != element_types.len() {
                return Err(format!("TupleNew 元素合同长度不一致"));
            }
            InstructionKind::TupleNew {
                layout_id: layout_for_type(result_type, "TupleNew")?,
                fields: fields.iter().map(|id| value(*id)).collect(),
            }
        }
        ValkyrieMirOperation::TupleGet { object, tuple_type, index } => {
            let arity = match program.linked.types.get(tuple_type).map(|row| &row.kind) {
                Some(nyar_types::CanonicalTypeKind::Tuple(members)) => members.len(),
                Some(nyar_types::CanonicalTypeKind::LabeledTuple { fields }) => fields.len(),
                _ => return Err(format!("TupleGet 对象不是元组类型: {tuple_type:?}")),
            };
            if (*index as usize) >= arity {
                return Err(format!("TupleGet 位置索引 {index} 越界（元组长度 {arity}）"));
            }
            InstructionKind::TupleGet { object: value(*object), layout_id: layout_for_type(*tuple_type, "TupleGet")?, index: *index }
        }
        ValkyrieMirOperation::StructNew { nominal, fields } => {
            InstructionKind::StructNew { nominal: *nominal, fields: fields.iter().map(|(field, id)| (*field, value(*id))).collect() }
        }
        ValkyrieMirOperation::FieldGet { object, field } => InstructionKind::FieldGet { object: value(*object), field: *field },
        ValkyrieMirOperation::FieldSet { object, field, value: stored } => {
            InstructionKind::FieldSet { object: value(*object), field: *field, value: value(*stored) }
        }
        ValkyrieMirOperation::SumNew { nominal, variant, payload } => {
            InstructionKind::SumNew { nominal: *nominal, variant: *variant, payload: payload.map(value) }
        }
        ValkyrieMirOperation::SumPayloadGet { nominal, variant, object } => {
            InstructionKind::SumPayloadGet { nominal: *nominal, variant: *variant, object: value(*object) }
        }
        ValkyrieMirOperation::SumVariantIs { nominal, variant, object } => {
            InstructionKind::SumVariantIs { nominal: *nominal, variant: *variant, object: value(*object) }
        }
        // 语言层显式 `as`：目标类型是后端必须编码的事实，不能丢成 Copy。
        ValkyrieMirOperation::Cast { source, ty } => InstructionKind::Cast { source: value(*source), ty: lower_type(program, *ty)? },
        unsupported => return Err(format!("canonical 操作尚无目标私有合同: {unsupported:?}")),
    })
}

/// 文本常量的编码身份由常量变体自身决定；整数/浮点/空值没有在这里声称的编码身份。
fn constant_encoding_type(constant: &ValkyrieMirConstant) -> Option<NyarType> {
    match constant {
        ValkyrieMirConstant::Utf8(_) => Some(NyarType::Utf8),
        ValkyrieMirConstant::Utf16(_) => Some(NyarType::Utf16),
        _ => None,
    }
}

fn lower_constant(constant: &ValkyrieMirConstant) -> Constant {
    match constant {
        ValkyrieMirConstant::Int(v) => Constant::Int(*v),
        // 位模式原样重建：`-0.0` 与 NaN payload 不在计划层被规范化。
        ValkyrieMirConstant::Float64(bits) => Constant::from(f64::from_bits(*bits)),
        ValkyrieMirConstant::Bool(v) => Constant::Bool(*v),
        ValkyrieMirConstant::Utf8(v) => Constant::Utf8(v.clone()),
        ValkyrieMirConstant::Utf16(v) => Constant::Utf16(v.clone()),
        ValkyrieMirConstant::Unit => Constant::Unit,
        ValkyrieMirConstant::Null => Constant::Null,
    }
}

/// 函数声明的签名（参数与返回类型）是否仍含未闭合的 Generic。
///
/// 签名含 Generic 说明该实例是声明模板而非单态化后的闭合实例；目标层无法编码这种
/// 签名，因此它只能作为失败原因，不能作为「可以跳过」的判据。
fn signature_contains_generic(program: &ValkyrieMirProgram, function: &crate::compiler::valkyrie_mir::ValkyrieMirFunction) -> bool {
    function.parameters.iter().any(|(_, ty)| type_contains_generic_deep(program, *ty))
        || type_contains_generic_deep(program, function.return_type)
}

/// 递归检查 canonical 类型（含嵌套）是否包含 Generic。
fn type_contains_generic_deep(program: &ValkyrieMirProgram, id: nyar_types::TypeId) -> bool {
    let Some(row) = program.linked.types.get(&id)
    else {
        return false;
    };
    match &row.kind {
        nyar_types::CanonicalTypeKind::Generic { .. } => true,
        nyar_types::CanonicalTypeKind::Nominal { arguments, .. } => arguments.iter().any(|a| type_contains_generic_deep(program, *a)),
        nyar_types::CanonicalTypeKind::Tuple(items)
        | nyar_types::CanonicalTypeKind::Union(items)
        | nyar_types::CanonicalTypeKind::Intersection(items) => items.iter().any(|a| type_contains_generic_deep(program, *a)),
        nyar_types::CanonicalTypeKind::LabeledTuple { fields } => fields.iter().any(|(_, ty)| type_contains_generic_deep(program, *ty)),
        nyar_types::CanonicalTypeKind::Array { element, .. } => type_contains_generic_deep(program, *element),
        nyar_types::CanonicalTypeKind::Nullable(element) => type_contains_generic_deep(program, *element),
        nyar_types::CanonicalTypeKind::Function { parameters, return_type } => {
            parameters.iter().any(|a| type_contains_generic_deep(program, *a)) || type_contains_generic_deep(program, *return_type)
        }
        _ => false,
    }
}

/// canonical 原语拼写的载体映射。
///
/// 只接受 canonical 原语词汇（`i8`/`u16`/`utf8`…），不接受语言表面拼写
/// （`byte`/`sbyte`/`Utf8Text` 等）：那些必须在语言前端按 `[primitive]` 身份
/// 结算，目标计划不得再按名字猜载体。`usize`/`isize`/`f128` 没有稳定宽度，
/// 刻意缺席并失败闭合。
fn canonical_primitive_carrier(spelling: &str) -> Option<NyarType> {
    let simple = spelling.rsplit([':', '.']).next().unwrap_or(spelling);
    Some(match simple {
        "bool" => NyarType::Boolean,
        "u8" => NyarType::Integer8 { signed: false },
        "i8" => NyarType::Integer8 { signed: true },
        "u16" => NyarType::Integer16 { signed: false },
        "i16" => NyarType::Integer16 { signed: true },
        "u32" => NyarType::Integer32 { signed: false },
        "i32" => NyarType::Integer32 { signed: true },
        "u64" => NyarType::Integer64 { signed: false },
        "i64" => NyarType::Integer64 { signed: true },
        "u128" => NyarType::Integer128 { signed: false },
        "i128" => NyarType::Integer128 { signed: true },
        "f32" => NyarType::Float32,
        "f64" => NyarType::Float64,
        "char" => NyarType::Character,
        "utf8" => NyarType::Utf8,
        "utf16" => NyarType::Utf16,
        "unit" => NyarType::Unit,
        "void" => NyarType::Bottom,
        _ => return None,
    })
}


/// 按 canonical 类型图重建布局计划使用的**未结算**类型形态（`Nominal` 基 + `Apply` 实参）。
fn lower_unsettled_contract_key(program: &ValkyrieMirProgram, id: nyar_types::TypeId) -> Result<NyarType> {
    let kind = &program.linked.types.get(&id).ok_or_else(|| "canonical 类型事实缺失".to_string())?.kind;
    match kind {
        CanonicalTypeKind::Nominal { declaration, arguments } => {
            let base = match program.linked.aggregate_layout_by_type.get(declaration) {
                Some(layout) => NyarType::Nominal { layout: *layout, spelling: nyar_types::Identifier::new("") },
                None => lower_type(program, *declaration)?.contract_key(),
            };
            if arguments.is_empty() {
                Ok(base)
            }
            else {
                Ok(NyarType::Apply(
                    Box::new(base),
                    arguments.iter().map(|argument| lower_unsettled_contract_key(program, *argument)).collect::<Result<Vec<_>>>()?,
                ))
            }
        }
        CanonicalTypeKind::Array { element, length } => {
            let element = lower_unsettled_contract_key(program, *element)?;
            Ok(match length {
                None => NyarType::Array(Box::new(element)),
                Some(length) => NyarType::FixedArray {
                    element: Box::new(element),
                    length: usize::try_from(*length).map_err(|_| "固定数组长度溢出".to_string())?,
                },
            })
        }
        CanonicalTypeKind::Tuple(items) => {
            Ok(NyarType::Tuple(items.iter().map(|item| lower_unsettled_contract_key(program, *item)).collect::<Result<Vec<_>>>()?))
        }
        CanonicalTypeKind::Nullable(element) => Ok(NyarType::Nullable(Box::new(lower_unsettled_contract_key(program, *element)?))),
        CanonicalTypeKind::Union(items) => {
            Ok(NyarType::Union(items.iter().map(|item| lower_unsettled_contract_key(program, *item)).collect::<Result<Vec<_>>>()?))
        }
        _ => Ok(lower_type(program, id)?.contract_key()),
    }
}

fn lower_type(program: &ValkyrieMirProgram, id: nyar_types::TypeId) -> Result<NyarType> {
    let kind = &program.linked.types.get(&id).ok_or_else(|| format!("canonical 类型事实缺失"))?.kind;
    match kind {
        CanonicalTypeKind::Primitive(kind) => match kind {
            nyar_types::CanonicalPrimitiveType::Void => Ok(NyarType::Bottom),
            nyar_types::CanonicalPrimitiveType::Unit => Ok(NyarType::Unit),
            nyar_types::CanonicalPrimitiveType::Bool => Ok(NyarType::Boolean),
            // 窄整数保留宽度与符号，不能在计划层提前抹成 i32：表示转换的扩展/截断
            // 配方由两端语义类型决定，抹平宽度后无法区分 `i32.and 0xFF` 与 `i32.extend8_s`。
            nyar_types::CanonicalPrimitiveType::Integer { bits: 8, signed } => Ok(NyarType::Integer8 { signed: *signed }),
            nyar_types::CanonicalPrimitiveType::Integer { bits: 16, signed } => Ok(NyarType::Integer16 { signed: *signed }),
            nyar_types::CanonicalPrimitiveType::Integer { bits: 32, signed } => Ok(NyarType::Integer32 { signed: *signed }),
            nyar_types::CanonicalPrimitiveType::Integer { bits: 64, signed } => Ok(NyarType::Integer64 { signed: *signed }),
            // 浮点各自保留独立载体：`Value` 有 `F64` 与 `F32` 两个变体，
            // `bits: 32` 不得落到 64 位槽（那会让「32 位类型」实际承载 64 位载荷）。
            nyar_types::CanonicalPrimitiveType::Float { bits: 32 } => Ok(NyarType::Float32),
            nyar_types::CanonicalPrimitiveType::Float { bits: 64 } => Ok(NyarType::Float64),
            nyar_types::CanonicalPrimitiveType::Character => Ok(NyarType::Character),
            nyar_types::CanonicalPrimitiveType::Utf8 => Ok(NyarType::Utf8),
            nyar_types::CanonicalPrimitiveType::Utf16 => Ok(NyarType::Utf16),
            unsupported => Err(format!("primitive 尚无目标合同: {unsupported:?}")),
        },
        CanonicalTypeKind::Tuple(items) => Ok(NyarType::Tuple(items.iter().map(|id| lower_type(program, *id)).collect::<Result<_>>()?)),
        // 唯一标签擦除点：标签只属于 canonical 身份，目标层按位置布局，`NyarType`
        // 与 `LayoutKey` 保持无标签。类型身份不等但布局相同的元组在 ABI 上允许。
        CanonicalTypeKind::LabeledTuple { fields } => {
            Ok(NyarType::Tuple(fields.iter().map(|(_, ty)| lower_type(program, *ty)).collect::<Result<_>>()?))
        }
        CanonicalTypeKind::Array { element, length: None } => Ok(NyarType::Array(Box::new(lower_type(program, *element)?))),
        CanonicalTypeKind::Array { element, length: Some(length) } => Ok(NyarType::FixedArray {
            element: Box::new(lower_type(program, *element)?),
            length: usize::try_from(*length).map_err(|_| format!("固定数组长度溢出"))?,
        }),
        CanonicalTypeKind::Nullable(element) => Ok(NyarType::Nullable(Box::new(lower_type(program, *element)?))),
        CanonicalTypeKind::Nominal { declaration, arguments } => {
            if !arguments.is_empty() {
                // 实例级布局已由 canonical 冻结时，泛型实例必须在计划边界结算为携带
                // 布局身份的 `Nominal`：存储类、字段与载体由实例布局解析，后端不得把
                // “看起来像引用”的泛型实例一律擦成引用。未冻结实例布局的泛型形状保持
                // `Apply`（宿主句柄形态），由后端在值槽上按缺合同失败闭合。
                if let Some(layout) = program.linked.aggregate_layout_by_type.get(&id).copied() {
                    let spelling = program
                        .linked
                        .type_surface_spellings
                        .get(&id)
                        .or_else(|| program.linked.type_surface_spellings.get(declaration))
                        .cloned()
                        .unwrap_or_else(|| nyar_types::Identifier::new("Apply"));
                    return Ok(NyarType::Nominal { layout, spelling });
                }
                return Ok(NyarType::Apply(
                    Box::new(lower_type(program, *declaration)?),
                    arguments.iter().map(|argument| lower_type(program, *argument)).collect::<Result<Vec<_>>>()?,
                ));
            }
            // 文本载体身份只来自 canonical 冻结的编码身份，不按拼写（`Utf8Text` 等）
            // 回查；折叠后的 `Utf8`/`Utf16` 是后续托管布局与宿主边界的共同键。
            if let Some(encoding) = program.linked.text_encoding_types.get(&id).copied() {
                return match encoding {
                    CanonicalPrimitiveType::Utf8 => Ok(NyarType::Utf8),
                    CanonicalPrimitiveType::Utf16 => Ok(NyarType::Utf16),
                    other => Err(format!("文本载体 `TypeId` 登记了非文本编码身份: {other:?}")),
                };
            }
            let spelling = program
                .linked
                .type_surface_spellings
                .get(if *declaration == id { &id } else { declaration })
                .or_else(|| program.linked.type_surface_spellings.get(&id));
            // 零实参 nominal 的布局可能登记在声明 `TypeId` 上（`Self` 别名与声明
            // 使用不同 `TypeId`），只能按声明身份回退，禁止按短名回查。
            let layout =
                program.linked.aggregate_layout_by_type.get(&id).or_else(|| program.linked.aggregate_layout_by_type.get(declaration)).copied();
            if let Some(name) = spelling {
                if let Some(primitive) = canonical_primitive_carrier(name.as_str()) {
                    return Ok(primitive);
                }
                // 已冻结实例级布局的零实参 nominal 铸造为携带身份的 `Nominal`：
                // 存储类/GC 字段/ABI 由 `layout` 身份解析，拼写仅诊断显示。
                // 泛型声明本身没有物理布局，只作为 `Apply` 基的构造器拼写保留；
                // 闭合 nominal 有实例记录却无任何布局绑定时必须失败关闭，
                // 不得压回裸 `Named` 让下游按名字猜实现。
                if let Some(layout) = layout {
                    return Ok(NyarType::Nominal { layout, spelling: name.clone() });
                }
                if is_generic_declaration(program, *declaration) {
                    return Ok(NyarType::Named(name.clone()));
                }
                return Err(format!("nominal type `{name}` 缺少目标 layout 或 carrier 合同"));
            }
            if *declaration != id {
                return lower_type(program, *declaration);
            }
            // canonical 合成名义记录（多字段变体 payload）没有源面拼写：源码里不存在
            // 这个名字，只有变体声明身份。布局已冻结时目标身份由布局决定，拼写从布局
            // 限定名派生，仅作诊断显示；不得为它要求一条不存在的 surface spelling。
            if let Some(layout_id) = layout {
                let spelling = match program.linked.aggregate_layouts.layouts.iter().find(|layout| layout.id == layout_id) {
                    Some(layout) if !layout.namespace.is_empty() => {
                        format!("{}::{}", layout.namespace.replace('.', "::"), layout.name)
                    }
                    Some(layout) => layout.name.clone(),
                    None => format!("Nominal({id:?})"),
                };
                return Ok(NyarType::Nominal { layout: layout_id, spelling: nyar_types::Identifier::new(&spelling) });
            }
            Err(format!(
                "canonical 类型尚无目标合同: Nominal {{ declaration: {declaration:?}, arguments: [], {} }}",
                nominal_contract_context(program, id, *declaration)
            ))
        }
        CanonicalTypeKind::Function { parameters, return_type } => Ok(NyarType::Function(Box::new(NyarFunctionType {
            params: parameters.iter().map(|parameter| lower_type(program, *parameter)).collect::<Result<Vec<_>>>()?,
            return_type: lower_type(program, *return_type)?,
        }))),
        CanonicalTypeKind::Union(items) => {
            Ok(NyarType::Union(items.iter().map(|item| lower_type(program, *item)).collect::<Result<Vec<_>>>()?))
        }
        // `Intersection` 必须在进入平台表示计划前由 frontend 擦除（见
        // `frontend_contract::nyar_type` 的 `ConcretizeError`）。把它静默降成 `Union`
        // 会改变语义（`A & B` 不等于 `A | B`），属于错误成功路径，因此失败闭合。
        CanonicalTypeKind::Intersection(items) => {
            Err(format!("canonical 类型 Intersection 尚未在 frontend 擦除，不能进入平台表示计划: {items:?}"))
        }
        unsupported => {
            // Generic 类型只能出现在泛型函数的签名里——如果某个泛型函数被错误地留在了
            // 可达集并尝试编码，这里就会触发。打印引用 Generic 的闭合函数帮助定位。
            let canonical = program;
            let mut refs_debug = String::new();
            for (inst, func) in &program.mir.functions {
                // 跳过泛型声明函数
                let has_generic_sig = func.parameters.iter().any(|(_, tid)| {
                    canonical.linked.types.get(tid).is_some_and(|row| matches!(row.kind, nyar_types::CanonicalTypeKind::Generic { .. }))
                }) || canonical
                    .linked
                    .types
                    .get(&func.return_type)
                    .is_some_and(|row| matches!(row.kind, nyar_types::CanonicalTypeKind::Generic { .. }));
                if has_generic_sig {
                    continue;
                }
                // 递归检查 value_types 里有没有嵌套 Generic
                let mut found = false;
                for (_, tid) in &func.value_types {
                    if type_contains_generic_deep(canonical, *tid) {
                        found = true;
                        break;
                    }
                }
                if found {
                    let name = program.linked.callable_names.get(inst).cloned().unwrap_or_default();
                    refs_debug.push_str(&format!("function `{name}` instance {inst:?}; "));
                }
            }
            let mut type_debug = String::new();
            for (tid, row) in &program.linked.types {
                if matches!(row.kind, nyar_types::CanonicalTypeKind::Generic { .. }) {
                    type_debug.push_str(&format!("TypeId({:?})={:?}; ", tid, row.kind));
                }
            }
            return Err(format!(
                "canonical 类型尚无目标合同: {unsupported:?} (module={}, refs=[{}], generic_types=[{}])",
                program.mir.module_name, refs_debug, type_debug
            ));
        }
    }
}

/// 判定 `declaration` 是否为泛型声明：存在以该声明为 owner 的具实参 nominal 实例。
///
/// 泛型声明本身没有物理布局，只能作为 `Apply` 基的构造器拼写保留；闭合 nominal
/// 缺少布局是合同缺口，必须失败闭合，二者不能共用裸 `Named` 回落。
fn is_generic_declaration(program: &ValkyrieMirProgram, declaration: nyar_types::TypeId) -> bool {
    program.linked.types.values().any(|record| {
        matches!(
            &record.kind,
            CanonicalTypeKind::Nominal { declaration: owner, arguments } if *owner == declaration && !arguments.is_empty()
        )
    })
}

/// 失败闭合诊断的定位信息：声明拼写、名义实例与字段槽位。
///
/// 只打印 `TypeId` 无法区分「哪个声明/哪个字段」缺合同；这里优先用已冻结的
/// surface spelling，其次用名义实例的字段槽位名（布局已登记时），最后才退回
/// `FieldId`。
fn nominal_contract_context(program: &ValkyrieMirProgram, id: nyar_types::TypeId, declaration: nyar_types::TypeId) -> String {
    let mut parts = Vec::new();
    if let Some(name) = program.linked.type_surface_spellings.get(&declaration) {
        parts.push(format!("spelling={name}"));
    }
    if let Some((nominal, record)) = program.linked.nominal_instances.iter().find(|(_, record)| record.ty == id) {
        parts.push(format!("nominal={nominal:?}"));
        if !record.fields.is_empty() {
            let fields = record
                .fields
                .iter()
                .map(|field| {
                    program
                        .linked
                        .aggregate_layout_by_field
                        .get(field)
                        .and_then(|(layout_id, index)| {
                            program
                                .linked
                                .aggregate_layouts
                                .layouts
                                .iter()
                                .find(|layout| layout.id == *layout_id)
                                .and_then(|layout| layout.fields.get(*index as usize))
                                .map(|slot| slot.name.clone())
                        })
                        .unwrap_or_else(|| format!("FieldId({})", field.index()))
                })
                .collect::<Vec<_>>();
            parts.push(format!("fields=[{}]", fields.join(", ")));
        }
    }
    if parts.is_empty() { "无已登记拼写或布局".to_string() } else { parts.join(", ") }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::valkyrie_mir::{ValkyrieMirBlockId, ValkyrieMirFunction};
    use nyar_emitter::target_operator::{TargetOperandType, operand_carrier};
    use nyar_types::{CanonicalPrimitiveType, ItemId, TypeRecord};

    fn type_id(index: u32) -> nyar_types::TypeId {
        nyar_types::TypeId::from_index(index).expect("测试类型身份")
    }

    fn program_with_types(rows: Vec<(nyar_types::TypeId, CanonicalTypeKind)>) -> ValkyrieMirProgram {
        let mut program = ValkyrieMirProgram::default();
        for (id, kind) in rows {
            program.linked.types.insert(id, TypeRecord { declaration: id, kind });
        }
        program
    }

    fn function_with_signature(parameters: Vec<nyar_types::TypeId>, return_type: nyar_types::TypeId) -> crate::compiler::valkyrie_mir::ValkyrieMirFunction {
        ValkyrieMirFunction {
            instance: ItemInstanceId::from_index(0).expect("测试实例身份"),
            parameters: parameters
                .into_iter()
                .enumerate()
                .map(|(index, ty)| (nyar_types::MirValueId::from_index(u32::try_from(index).expect("测试值下标")).expect("测试值身份"), ty))
                .collect(),
            parameter_names: Vec::new(),
            return_type,
            value_types: BTreeMap::new(),
            entry: ValkyrieMirBlockId(0),
            blocks: BTreeMap::new(),
        }
    }

    /// 签名含 Generic 的实例只能是失败原因，不能是「可以跳过」的判据。
    #[test]
    fn generic_signature_is_detected_in_parameter_and_nested_and_return() {
        let generic = CanonicalTypeKind::Generic { owner: ItemId::from_index(0).expect("测试声明身份"), ordinal: 0 };
        let program = program_with_types(vec![
            (type_id(0), generic.clone()),
            (type_id(1), CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Integer { bits: 32, signed: true })),
            (type_id(2), CanonicalTypeKind::Tuple(vec![type_id(1), type_id(0)])),
        ]);
        assert!(signature_contains_generic(&program, &function_with_signature(vec![type_id(0)], type_id(1))), "直接泛型形参必须失败");
        assert!(signature_contains_generic(&program, &function_with_signature(vec![type_id(2)], type_id(1))), "嵌套泛型形参必须失败");
        assert!(signature_contains_generic(&program, &function_with_signature(vec![type_id(1)], type_id(0))), "泛型返回类型必须失败");
        assert!(!signature_contains_generic(&program, &function_with_signature(vec![type_id(1)], type_id(1))), "闭合标量签名不是泛型模板");
    }

    /// 窄整数与 `Character` 必须保留宽度与符号性：抹成 i32 后无法再区分掩码与符号扩展。
    #[test]
    fn narrow_integer_and_character_primitives_keep_their_identity() {
        let program = program_with_types(vec![
            (type_id(0), CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Integer { bits: 8, signed: false })),
            (type_id(1), CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Integer { bits: 16, signed: true })),
            (type_id(2), CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Character)),
        ]);
        assert_eq!(lower_type(&program, type_id(0)).expect("u8"), NyarType::Integer8 { signed: false });
        assert_eq!(lower_type(&program, type_id(1)).expect("i16"), NyarType::Integer16 { signed: true });
        assert_eq!(lower_type(&program, type_id(2)).expect("Character"), NyarType::Character);
    }

    /// 零实参 nominal 的 `Self` 别名 `TypeId` 未持有布局时，必须按声明身份回退；
    /// 有实例记录但无任何布局绑定时必须失败关闭，不得压回裸 `Named`。
    #[test]
    fn zero_argument_nominal_resolves_layout_through_declaration_identity() {
        let alias = type_id(0);
        let declaration = type_id(1);
        let mut program = program_with_types(vec![
            (alias, CanonicalTypeKind::Nominal { declaration, arguments: Vec::new() }),
            (declaration, CanonicalTypeKind::Nominal { declaration, arguments: Vec::new() }),
        ]);
        program.linked.type_surface_spellings.insert(declaration, nyar_types::Identifier::new("Endpoint"));
        program.linked.aggregate_layout_by_type.insert(declaration, 0);
        program.linked.aggregate_layouts.layouts.push(nyar_types::AggregateLayout {
            id: 0,
            name: "Endpoint".into(),
            namespace: String::new(),
            key: None,
            storage: nyar_types::StorageKind::Value,
            fields: vec![nyar_types::FieldLayout { name: "code".into(), ty: NyarType::Integer64 { signed: true } }],
        });
        program.linked.nominal_instances.insert(
            nyar_types::NominalInstanceId::from_index(0).expect("测试名义实例身份"),
            nyar_types::NominalInstanceRecord {
                declaration,
                ty: alias,
                semantics: nyar_types::NominalValueSemantics::Value,
                fields: Vec::new(),
            },
        );
        assert_eq!(
            lower_type(&program, alias).expect("声明身份布局"),
            NyarType::Nominal { layout: 0, spelling: nyar_types::Identifier::new("Endpoint") }
        );
    }

    #[test]
    fn nominal_instance_without_any_layout_fails_closed() {
        let alias = type_id(0);
        let declaration = type_id(1);
        let mut program = program_with_types(vec![
            (alias, CanonicalTypeKind::Nominal { declaration, arguments: Vec::new() }),
            (declaration, CanonicalTypeKind::Nominal { declaration, arguments: Vec::new() }),
        ]);
        program.linked.type_surface_spellings.insert(declaration, nyar_types::Identifier::new("Endpoint"));
        program.linked.nominal_instances.insert(
            nyar_types::NominalInstanceId::from_index(0).expect("测试名义实例身份"),
            nyar_types::NominalInstanceRecord {
                declaration,
                ty: alias,
                semantics: nyar_types::NominalValueSemantics::Value,
                fields: Vec::new(),
            },
        );
        let error = lower_type(&program, alias).expect_err("无布局必须失败关闭");
        assert!(error.contains("缺少目标 layout"), "{error}");
    }

    #[test]
    fn generic_declaration_without_layout_keeps_constructor_spelling() {
        let declaration = type_id(0);
        let integer = type_id(1);
        let instance = type_id(2);
        let mut program = program_with_types(vec![
            (declaration, CanonicalTypeKind::Nominal { declaration, arguments: Vec::new() }),
            (integer, CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Integer { bits: 64, signed: true })),
            (instance, CanonicalTypeKind::Nominal { declaration, arguments: vec![integer] }),
        ]);
        program.linked.type_surface_spellings.insert(declaration, nyar_types::Identifier::new("ArrayList"));
        program.linked.nominal_instances.insert(
            nyar_types::NominalInstanceId::from_index(0).expect("测试名义实例身份"),
            nyar_types::NominalInstanceRecord {
                declaration,
                ty: declaration,
                semantics: nyar_types::NominalValueSemantics::Reference,
                fields: Vec::new(),
            },
        );
        assert_eq!(lower_type(&program, declaration).expect("泛型声明拼写"), NyarType::Named(nyar_types::Identifier::new("ArrayList")));
    }

    /// 浮点宽度与目标载体一一对应：`Float{bits:32}` 必须登记为独立的 `NyarType::Float32`
    /// 与 `TargetOperandType::F32`，不得静默并入 64 位载体。
    #[test]
    fn float_primitive_registers_width_matched_target_contract() {
        let program = program_with_types(vec![
            (type_id(0), CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Float { bits: 64 })),
            (type_id(1), CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Float { bits: 32 })),
        ]);
        assert_eq!(lower_type(&program, type_id(0)).expect("f64 必须有目标合同"), NyarType::Float64);
        let float32 = lower_type(&program, type_id(1)).expect("f32 必须有目标合同");
        assert_eq!(float32, NyarType::Float32);
        assert_eq!(operand_carrier(&float32), Some(TargetOperandType::F32));
        assert_ne!(operand_carrier(&float32), operand_carrier(&NyarType::Float64), "f32 不得复用 f64 载体");
    }

    /// canonical 用 `u64` 位模式承载 float64，计划层必须按位重建：`-0.0` 的符号位
    /// 与 NaN payload 一旦被 `f64` 语义规范化，常量池就再也分不出 `-0.0` 与 `0.0`。
    #[test]
    fn float64_canonical_constant_preserves_sign_bit_and_nan_payload() {
        let negative_zero = ValkyrieMirConstant::Float64((-0.0f64).to_bits());
        let positive_zero = ValkyrieMirConstant::Float64(0.0f64.to_bits());
        match lower_constant(&negative_zero) {
            Constant::Float64(value) => {
                assert!(value.0.is_sign_negative(), "`-0.0` 的符号位不得在计划层丢失");
                assert_eq!(value.0.to_bits(), (-0.0f64).to_bits());
            }
            other => panic!("float64 常量必须降低为 `Constant::Float64`，实际 {other:?}"),
        }
        // 计划层的 `Constant::Float64` 用 `OrderedFloat`，其 `PartialEq` 仍是 f64 语义
        // （`-0.0 == 0.0`），因此相等性无法区分两者；canonical 选 `u64` 位模式正是为此。
        match (lower_constant(&negative_zero), lower_constant(&positive_zero)) {
            (Constant::Float64(negative), Constant::Float64(positive)) => {
                assert_ne!(negative.0.to_bits(), positive.0.to_bits(), "`-0.0` 与 `0.0` 的位模式必须不同");
            }
            other => panic!("两个零常量都必须保持 float64 载体，实际 {other:?}"),
        }
        // 带 payload 的 quiet NaN：位模式重建后 payload 必须逐位保真。
        let nan_payload = 0x7ff8_0000_dead_beefu64;
        match lower_constant(&ValkyrieMirConstant::Float64(nan_payload)) {
            Constant::Float64(value) => assert_eq!(value.0.to_bits(), nan_payload, "NaN payload 不得被规范化"),
            other => panic!("NaN 常量必须保持 float64 载体，实际 {other:?}"),
        }
    }

    /// `Intersection` 必须在 frontend 擦除；平台 lowering 静默降成 `Union` 会改变语义，
    /// 属错误成功路径，因此必须结构化失败闭合。
    #[test]
    fn intersection_type_fails_closed_instead_of_widening_to_union() {
        let program = program_with_types(vec![
            (type_id(0), CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Integer { bits: 32, signed: true })),
            (type_id(1), CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Bool)),
            (type_id(2), CanonicalTypeKind::Intersection(vec![type_id(0), type_id(1)])),
        ]);
        let error = lower_type(&program, type_id(2)).expect_err("Intersection 不得被降成 Union");
        assert!(error.contains("Intersection"), "诊断必须指明未擦除的 Intersection，实际：{error}");
        assert!(error.contains("frontend"), "诊断必须指向 frontend 擦除责任，实际：{error}");
    }

    /// 文本常量进入 executable 计划时必须携带由编码变体决定的类型身份；
    /// canonical 层不保存名称，不能在这里回落 `None` 让后端再猜。
    #[test]
    fn text_constants_carry_their_encoding_identity() {
        assert_eq!(constant_encoding_type(&ValkyrieMirConstant::Utf8("文本".to_string())), Some(NyarType::Utf8));
        assert_eq!(constant_encoding_type(&ValkyrieMirConstant::Utf16("文本".to_string())), Some(NyarType::Utf16));
        assert_eq!(constant_encoding_type(&ValkyrieMirConstant::Int(1)), None, "整数宽度不由常量变体单独声称");
        assert_eq!(constant_encoding_type(&ValkyrieMirConstant::Unit), None, "Unit 不声称文本编码");
    }

    /// `Raise` 是唯一进入目标合同的 canonical effect：映射为 executable `PerformEffect`，
    /// payload 与 resume 目标原样保留，由各后端决定自己的目标语义（wasm trap / ovm 分派）。
    #[test]
    fn raise_terminator_enters_the_target_contract() {
        let payload = nyar_types::MirValueId::from_index(7).expect("测试值身份");
        let lowered = lower_terminator(ValkyrieMirTerminator::PerformEffect {
            effect: ValkyrieMirEffectKind::Raise,
            payload: Some(payload),
            resume_target: ValkyrieMirBlockId(3),
        })
        .expect("`Raise` 必须有目标合同");
        assert_eq!(
            lowered,
            Terminator::PerformEffect {
                effect: nyar_types::EffectKind::Raise,
                payload: Some(Operand::Value(ValueRef(7))),
                resume_target: BlockRef(3)
            }
        );
    }

    /// 反例：`Yield` 等其余 effect 没有目标合同，必须在计划层失败闭合，
    /// 不得静默降级为普通终止符或 `Unreachable`。
    #[test]
    fn non_raise_effect_terminators_still_fail_closed() {
        let error = lower_terminator(ValkyrieMirTerminator::PerformEffect {
            effect: ValkyrieMirEffectKind::Yield,
            payload: None,
            resume_target: ValkyrieMirBlockId(1),
        })
        .expect_err("`Yield` 不得进入目标合同");
        assert!(error.contains("尚无目标合同"), "诊断必须保持失败闭合语义，实际：{error}");
    }

}

fn lower_terminator(terminator: ValkyrieMirTerminator) -> Result<Terminator> {
    let value = |id: nyar_types::MirValueId| Operand::Value(ValueRef(id.index()));
    Ok(match terminator {
        ValkyrieMirTerminator::Return { value: result } => Terminator::Return { value: result.map(value) },
        ValkyrieMirTerminator::Jump { target, arguments } => {
            Terminator::Jump { target: BlockRef(target.0), arguments: arguments.into_iter().map(value).collect() }
        }
        ValkyrieMirTerminator::Branch { condition, then_target, else_target } => {
            Terminator::Branch { condition: value(condition), then_target: BlockRef(then_target.0), else_target: BlockRef(else_target.0) }
        }
        ValkyrieMirTerminator::Unreachable => Terminator::Unreachable,
        // `Raise` 是唯一进入目标合同的 canonical effect：wasm 以 trap（`unreachable`）
        // 终止，ovm 以原生 `PerformEffect` 分派；托管 lane 在 emit 侧结构化失败。
        // 其余 effect（yield/await/…）需要续体运行时或状态机重写，本层尚无合同，继续失败闭合。
        ValkyrieMirTerminator::PerformEffect { effect: ValkyrieMirEffectKind::Raise, payload, resume_target } => {
            Terminator::PerformEffect {
                effect: nyar_types::EffectKind::Raise,
                payload: payload.map(value),
                resume_target: BlockRef(resume_target.0),
            }
        }
        unsupported => {
            return Err(format!("canonical effect terminator 尚无目标合同: {unsupported:?}"));
        }
    })
}

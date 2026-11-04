//! Compiler 成功载荷到目标私有执行计划的唯一边界。

use std::collections::BTreeMap;

use crate::{EmitterError, Result};
use nyar::{ExternalImportLink, QualifiedName};
use nyar_types::{
    CanonicalArrayInitialization, CanonicalCallee, CanonicalConstant, CanonicalOperation, CanonicalProgram, CanonicalTerminator,
    CanonicalTypeKind, CompiledProgram, Constant, Instruction, InstructionKind, ItemInstanceId, NyarFunctionType, NyarType, Operand,
    Terminator, Value, ValueOrigin,
    layout_choice::{InvokeLowering, ValueRepresentation},
};

use crate::{
    backend_plan_views::{ExecutableFunction, FunctionView, SuspendMetadataView},
    contracts::{Block, BlockRef, ValueRef},
    target_operator::{
        TargetOperandStep, TargetOperandType, TargetOperatorBinding, TargetOperatorPlan, TargetOperatorRecipe, WasmGcArrayOperation,
        WasmGcArrayOperatorPlan,
    },
};

/// 已完成 callable、类型、CFG 和表示合同绑定的目标私有计划。
#[derive(Debug, Clone, Default)]
pub struct TargetExecutionPlan {
    module_name: String,
    fragment_id: nyar::Identifier,
    exported_operations: Vec<ItemInstanceId>,
    required_capabilities: Vec<nyar::CapabilityTag>,
    theory_bundle: nyar::TheoryBundle,
    entry_operation: Option<ItemInstanceId>,
    wasm_export_names: BTreeMap<ItemInstanceId, String>,
    external_import_links: BTreeMap<ItemInstanceId, nyar::ExternalImportLink>,
    external_call_edges: Vec<nyar_types::CanonicalExternalCallEdge>,
    internal_call_edges: Vec<nyar_types::CanonicalCallEdge>,
    aggregate_layouts: nyar_types::AggregateLayoutPlan,
    aggregate_layout_by_nominal: BTreeMap<nyar_types::NominalInstanceId, nyar_types::LayoutId>,
    aggregate_layout_by_field: BTreeMap<nyar_types::FieldId, (nyar_types::LayoutId, u32)>,
    aggregate_layout_by_type: BTreeMap<nyar_types::TypeId, nyar_types::LayoutId>,
    /// 以结构化 `NyarType` 为键布局索引。
    ///
    /// `NyarType::Apply`（如 `ArrayList<i64>`）不携带 `LayoutId`；库导出 ABI 的
    /// 字段/容器桥需要从类型拼写回查冻结布局。索引只在 canonical 计划构建时
    /// 由 `aggregate_layout_by_type` 逐项 `lower_type` 生成，不按名字猜布局。
    aggregate_layout_by_nyar_type: BTreeMap<NyarType, nyar_types::LayoutId>,
    flags_types: Vec<nyar_types::FlagsLayout>,
    singleton_instances: Vec<nyar_types::SingletonInstancePlan>,
    functions: BTreeMap<ItemInstanceId, ExecutableFunction>,
    library_export_abis: BTreeMap<ItemInstanceId, LibraryExportAbi>,
    abi_names: BTreeMap<ItemInstanceId, QualifiedName>,
    imports: BTreeMap<ItemInstanceId, BackendImport>,
    sum_reps: BTreeMap<nyar_types::NominalInstanceId, nyar_types::layout_choice::SumRepresentation>,
}

/// 已由 Compiler 绑定身份、签名和链接合同的导入。
#[derive(Debug, Clone)]
pub struct BackendImport {
    /// Canonical 导入索引。
    pub index: nyar_types::ImportIndex,
    /// 外部链接合同。
    pub link: nyar_types::ExternalImportLink,
    /// 参数类型列表。
    pub parameter_types: Vec<NyarType>,
    /// 返回类型。
    pub return_type: NyarType,
    /// 计划冻结的目标 operator 绑定；`None` 表示该导入不是目标 operator 绑定。
    pub target_operator: Option<TargetOperatorBinding>,
}

/// 库导出参数或返回值的 Wasm 物理载体。
///
/// 载体是目标执行计划的冻结事实；backend 只能序列化这里的值，不得再从
/// `WasmAbiType` schema 现场推导载体。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbiCarrier {
    /// `i32` 槽位。
    I32,
    /// `i64` 槽位。
    I64,
    /// `f32` 槽位。
    F32,
    /// `f64` 槽位。
    F64,
    /// 宿主句柄（数组 / Option / Tuple / 具名聚合）。
    Handle,
    /// 无返回值。
    Void,
}

impl AbiCarrier {
    /// 返回载体名称（lowercase）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Handle => "handle",
            Self::Void => "void",
        }
    }
}

/// 一个库导出实例的冻结 ABI 载体合同。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryExportAbi {
    /// 按声明顺序排列的参数载体。
    pub params: Vec<AbiCarrier>,
    /// 返回值载体；无返回值时为 [`AbiCarrier::Void`]。
    pub returns: AbiCarrier,
}

impl TargetExecutionPlan {
    pub(crate) fn from_entry_function(function: ExecutableFunction) -> Result<Self> {
        let entry = ItemInstanceId::from_index(0).expect("测试入口身份");
        let name = QualifiedName::new(vec![nyar::Identifier::new("main")]);
        let abi = freeze_library_export_abi(&function)?;
        Ok(Self {
            entry_operation: Some(entry),
            functions: BTreeMap::from([(entry, function)]),
            library_export_abis: BTreeMap::from([(entry, abi)]),
            abi_names: BTreeMap::from([(entry, name)]),
            ..Self::default()
        })
    }

    /// 从 ABI 名称表构造测试用目标私有计划。
    ///
    /// 导出名尚未登记，因此库导出 ABI 载体合同也尚未冻结；测试在
    /// [`Self::testing_wasm_export_names_mut`] 登记导出名后必须调用
    /// [`Self::testing_freeze_library_export_abis`] 补齐冻结，不能依赖后端猜测载体。
    #[doc(hidden)]
    pub fn from_functions(functions: BTreeMap<QualifiedName, ExecutableFunction>) -> Self {
        let mut plan = Self::default();
        for (index, (name, function)) in functions.into_iter().enumerate() {
            let instance = ItemInstanceId::from_index(u32::try_from(index).expect("测试函数身份溢出")).expect("测试函数身份溢出");
            plan.abi_names.insert(instance, name);
            plan.functions.insert(instance, function);
        }
        plan
    }

    /// 返回全部可执行函数表（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_functions(&self) -> &BTreeMap<ItemInstanceId, ExecutableFunction> {
        &self.functions
    }

    /// 返回可写函数表（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_functions_mut(&mut self) -> &mut BTreeMap<ItemInstanceId, ExecutableFunction> {
        &mut self.functions
    }

    /// 返回可写 wasm 导出名表（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_wasm_export_names_mut(&mut self) -> &mut BTreeMap<ItemInstanceId, String> {
        &mut self.wasm_export_names
    }

    /// 返回可写导出实例表（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_exported_operations_mut(&mut self) -> &mut Vec<ItemInstanceId> {
        &mut self.exported_operations
    }

    /// 设置模块名（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_module_name(&mut self, name: impl Into<String>) -> &mut Self {
        self.module_name = name.into();
        self
    }

    /// 设置入口操作（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_entry_operation(&mut self, name: QualifiedName) -> &mut Self {
        if let Some((instance, _)) = self.abi_names.iter().find(|(_, abi)| **abi == name) {
            self.entry_operation = Some(*instance);
        }
        self
    }

    /// 设置聚合布局计划（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_aggregate_layouts(&mut self, layouts: nyar_types::AggregateLayoutPlan) -> &mut Self {
        self.aggregate_layouts = layouts;
        self
    }

    /// 登记 nominal → layout，并写入 `aggregate_layouts`（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_register_nominal_layout(
        &mut self,
        nominal: nyar_types::NominalInstanceId,
        layout: nyar_types::AggregateLayout,
    ) -> &mut Self {
        let layout_id = layout.id;
        if !self.aggregate_layouts.layouts.iter().any(|item| item.id == layout_id) {
            self.aggregate_layouts.layouts.push(layout);
        }
        self.aggregate_layout_by_nominal.insert(nominal, layout_id);
        self
    }

    /// 登记字段布局索引（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_register_field(&mut self, field: nyar_types::FieldId, layout_id: nyar_types::LayoutId, slot: u32) -> &mut Self {
        self.aggregate_layout_by_field.insert(field, (layout_id, slot));
        self
    }

    /// 登记结构化 `NyarType` → layout，并写入 `aggregate_layouts`（集成测试专用）。
    ///
    /// 生产路径由 `from_compiled_program` 从 `aggregate_layout_by_type` 逐项
    /// `lower_type` 生成；手工计划没有 canonical，必须显式登记同一事实，
    /// 否则泛型实例（`Apply`）只能退回按名字猜测布局。
    #[doc(hidden)]
    pub fn testing_register_type_layout(&mut self, ty: nyar_types::NyarType, layout: nyar_types::AggregateLayout) -> &mut Self {
        let layout_id = layout.id;
        if !self.aggregate_layouts.layouts.iter().any(|item| item.id == layout_id) {
            self.aggregate_layouts.layouts.push(layout);
        }
        self.aggregate_layout_by_nyar_type.insert(ty, layout_id);
        self
    }

    /// 登记外部导入 callable（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_register_import(&mut self, instance: ItemInstanceId, import: BackendImport) -> &mut Self {
        self.imports.insert(instance, import);
        self
    }

    /// 登记 nominal sum 的冻结表示合同（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_register_sum_representation(
        &mut self,
        nominal: nyar_types::NominalInstanceId,
        sum: nyar_types::layout_choice::SumRepresentation,
    ) -> &mut Self {
        self.sum_reps.insert(nominal, sum);
        self
    }

    /// 从完整 `CompiledProgram` 生成闭包；任何无法无损投影的语义都失败。
    pub fn from_compiled_program(
        program: &CompiledProgram,
        fragment_id: &nyar::Identifier,
        theory_bundle: nyar::TheoryBundle,
        roots: &[ItemInstanceId],
    ) -> Result<Self> {
        let canonical = program.canonical();
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
                let target_operator = freeze_target_operator(&import.link);
                if let Some(binding) = target_operator {
                    let symbol = canonical.linked.callable_names.get(&instance).map(|name| name.to_string());
                    match binding {
                        TargetOperatorBinding::Frozen(plan) => {
                            validate_frozen_operator(instance, symbol.as_deref(), &import.link, &parameter_types, &return_type, &plan)?;
                        }
                        TargetOperatorBinding::FrozenWasmGcArray(plan) => {
                            validate_frozen_array_operator(instance, symbol.as_deref(), &import.link, &parameter_types, &return_type, &plan)?;
                        }
                        TargetOperatorBinding::Unfrozen => {}
                    }
                }
                imports.insert(
                    instance,
                    BackendImport { index: *index, link: import.link.clone(), parameter_types, return_type, target_operator },
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
        for (type_id, layout_id) in &canonical.linked.aggregate_layout_by_type {
            // lower_type 只对“可执行类型”有结果；函数签名等非值类型跳过即可，
            // 它们不会成为容器元素或导出位置。
            if let Ok(ty) = lower_type(canonical, *type_id) {
                aggregate_layout_by_nyar_type.insert(ty, *layout_id);
            }
        }
        let mut plan = Self {
            module_name: canonical.linked.module_name.clone(),
            fragment_id: fragment.id.clone(),
            exported_operations: fragment.exported_operations.clone(),
            required_capabilities: fragment.required_capabilities.clone(),
            theory_bundle,
            entry_operation: fragment.entry_operation,
            wasm_export_names: fragment.wasm_export_names.clone(),
            external_import_links: fragment.external_imports.clone(),
            external_call_edges: fragment.external_call_edges.clone(),
            internal_call_edges: fragment.internal_call_edges.clone(),
            aggregate_layouts: canonical.linked.aggregate_layouts.clone(),
            aggregate_layout_by_nominal: canonical.linked.aggregate_layout_by_nominal.clone(),
            aggregate_layout_by_field: canonical.linked.aggregate_layout_by_field.clone(),
            aggregate_layout_by_type: canonical.linked.aggregate_layout_by_type.clone(),
            aggregate_layout_by_nyar_type,
            flags_types: canonical.linked.flags_types.clone(),
            singleton_instances: canonical.linked.singleton_instances.clone(),
            functions,
            library_export_abis: BTreeMap::new(),
            abi_names,
            imports,
            sum_reps: program.representation().sum_reps.clone(),
        };
        plan.freeze_library_export_abis()?;
        Ok(plan)
    }

    /// 按实例查询已冻结的库导出 ABI 载体合同。
    pub fn library_export_abi(&self, instance: &ItemInstanceId) -> Option<&LibraryExportAbi> {
        self.library_export_abis.get(instance)
    }

    /// 在测试登记导出名后补齐库导出 ABI 载体冻结（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_freeze_library_export_abis(&mut self) -> Result<()> {
        self.freeze_library_export_abis()
    }

    /// 为所有已声明导出名的实例冻结参数/返回载体。
    ///
    /// 聚合 helper 的 producer 也在 `wasm_export_names` 中，因此同样被冻结；缺少
    /// 函数体或类型没有稳定载体时结构化失败。
    fn freeze_library_export_abis(&mut self) -> Result<()> {
        let mut abis = BTreeMap::new();
        for (instance, export_name) in &self.wasm_export_names {
            let function = self
                .functions
                .get(instance)
                .ok_or_else(|| format!("库导出 `{instance}`（export_name={export_name}）缺少 Compiler 函数体，无法冻结 ABI 载体合同"))?;
            let abi = freeze_library_export_abi(function)
                .map_err(|reason| format!("库导出 `{instance}`（export_name={export_name}）冻结 ABI 载体合同失败: {reason}"))?;
            abis.insert(*instance, abi);
        }
        self.library_export_abis = abis;
        Ok(())
    }

    /// 返回模块名。
    pub fn module_name(&self) -> &str {
        &self.module_name
    }

    /// 返回片段标识。
    pub fn fragment_id(&self) -> &nyar::Identifier {
        &self.fragment_id
    }

    /// 返回已导出的 callable 实例列表。
    pub fn exported_operations(&self) -> &[ItemInstanceId] {
        &self.exported_operations
    }

    /// 返回片段所需 capability 标签。
    pub fn required_capabilities(&self) -> &[nyar::CapabilityTag] {
        &self.required_capabilities
    }

    /// 返回理论束合同。
    pub fn theory_bundle(&self) -> &nyar::TheoryBundle {
        &self.theory_bundle
    }

    /// 返回入口 callable 实例（若有）。
    pub fn entry_operation(&self) -> Option<ItemInstanceId> {
        self.entry_operation
    }

    /// 返回 wasm 导出名映射。
    pub fn wasm_export_names(&self) -> &BTreeMap<ItemInstanceId, String> {
        &self.wasm_export_names
    }

    /// 返回外部导入链接映射。
    pub fn external_import_links(&self) -> &BTreeMap<ItemInstanceId, nyar::ExternalImportLink> {
        &self.external_import_links
    }

    /// 返回外部调用边列表。
    pub fn external_call_edges(&self) -> &[nyar_types::CanonicalExternalCallEdge] {
        &self.external_call_edges
    }

    /// 返回内部调用边列表。
    pub fn internal_call_edges(&self) -> &[nyar_types::CanonicalCallEdge] {
        &self.internal_call_edges
    }

    /// 返回聚合布局计划。
    pub fn aggregate_layouts(&self) -> &nyar_types::AggregateLayoutPlan {
        &self.aggregate_layouts
    }

    /// 返回 nominal 实例到布局 ID 的映射。
    pub fn aggregate_layout_by_nominal(&self) -> &BTreeMap<nyar_types::NominalInstanceId, nyar_types::LayoutId> {
        &self.aggregate_layout_by_nominal
    }

    /// 返回字段 ID 到布局槽位的映射。
    pub fn aggregate_layout_by_field(&self) -> &BTreeMap<nyar_types::FieldId, (nyar_types::LayoutId, u32)> {
        &self.aggregate_layout_by_field
    }

    /// 返回类型 ID 到布局 ID 的映射。
    pub fn aggregate_layout_by_type(&self) -> &BTreeMap<nyar_types::TypeId, nyar_types::LayoutId> {
        &self.aggregate_layout_by_type
    }

    /// 返回结构化 `NyarType` 到布局 ID 的映射。
    ///
    /// 这是 `Apply(Named, args)` 唯一可用的布局身份来源；按名字回查布局被禁止。
    pub fn aggregate_layout_by_nyar_type(&self) -> &BTreeMap<NyarType, nyar_types::LayoutId> {
        &self.aggregate_layout_by_nyar_type
    }

    /// 返回 flags 类型布局列表。
    pub fn flags_types(&self) -> &[nyar_types::FlagsLayout] {
        &self.flags_types
    }

    /// 返回单例实例计划列表。
    pub fn singleton_instances(&self) -> &[nyar_types::SingletonInstancePlan] {
        &self.singleton_instances
    }

    /// 返回已声明导入映射。
    pub fn imports(&self) -> &BTreeMap<ItemInstanceId, BackendImport> {
        &self.imports
    }

    /// 返回全部 callable 实例 ID。
    pub fn instances(&self) -> Vec<ItemInstanceId> {
        self.functions.keys().copied().collect()
    }

    /// 按实例 ID 查询函数视图。
    pub fn get_function(&self, instance: &ItemInstanceId) -> Option<FunctionView> {
        self.functions.get(instance).cloned().map(|function| FunctionView { function })
    }

    /// 按实例 ID 查询 ABI 名称。
    pub fn abi_name_for_instance(&self, instance: ItemInstanceId) -> Option<QualifiedName> {
        self.abi_names.get(&instance).cloned()
    }

    /// 按实例 ID 查询挂起元数据视图。
    pub fn suspend_metadata(&self, instance: ItemInstanceId) -> Option<SuspendMetadataView> {
        self.functions.get(&instance).and_then(SuspendMetadataView::from_function)
    }

    /// 返回 sum 类型表示合同映射。
    pub fn sum_representations(&self) -> &BTreeMap<nyar_types::NominalInstanceId, nyar_types::layout_choice::SumRepresentation> {
        &self.sum_reps
    }
}

/// 冻结一个库导出函数的参数与返回载体。
///
/// 参数按 `param_types` 声明顺序逐个映射；返回值按 `return_type` 映射。映射语义
/// 必须与 `nyar.library_invoke` section 序列化消费的 `WasmAbiType` 载体保持一一对应。没有稳定 Wasm
/// 载体的类型（`Integer128` / `Utf16` / `Bottom` / `Function` / `TraitObject` /
/// `Union`）在此结构化失败，不得回落为默认载体。
fn freeze_library_export_abi(function: &ExecutableFunction) -> Result<LibraryExportAbi> {
    let mut params = Vec::with_capacity(function.param_types.len());
    for (index, ty) in function.param_types.iter().enumerate() {
        let carrier = freeze_abi_carrier(ty)
            .map_err(|reason| format!("函数 `{}` 参数 {index} 类型 `{ty}` 没有库导出 ABI 载体: {reason}", function.symbol))?;
        if carrier == AbiCarrier::Void {
            return Err(format!("函数 `{}` 参数 {index} 类型 `{ty}` 映射为 Void 载体，Void 只允许出现在返回位", function.symbol));
        }
        params.push(carrier);
    }
    let returns = freeze_abi_carrier(&function.return_type)
        .map_err(|reason| format!("函数 `{}` 返回类型 `{}` 没有库导出 ABI 载体: {reason}", function.symbol, function.return_type))?;
    Ok(LibraryExportAbi { params, returns })
}

/// 把单个 `NyarType` 映射为 Wasm 物理载体。
///
/// 语义与 `abi_schema_for_type` + `abi_carrier_name` 的组合结果一致：标量按 Wasm
/// 原生槽位，容器与具名聚合统一为宿主句柄。
fn freeze_abi_carrier(ty: &NyarType) -> Result<AbiCarrier> {
    Ok(match ty {
        NyarType::Unit => AbiCarrier::Void,
        NyarType::Boolean | NyarType::Character => AbiCarrier::I32,
        NyarType::Integer8 { .. } | NyarType::Integer16 { .. } | NyarType::Integer32 { .. } => AbiCarrier::I32,
        NyarType::Integer64 { .. } => AbiCarrier::I64,
        NyarType::Float32 => AbiCarrier::F32,
        NyarType::Float64 => AbiCarrier::F64,
        NyarType::Utf8 => AbiCarrier::I32,
        NyarType::Array(_) | NyarType::FixedArray { .. } | NyarType::Nullable(_) | NyarType::Tuple(_) => AbiCarrier::Handle,
        NyarType::Named(_) | NyarType::Nominal { .. } | NyarType::Apply(_, _) => AbiCarrier::Handle,
        NyarType::Integer128 { .. } => return Err("Integer128 没有原生 Wasm 载体，需要显式拆分合同".to_string()),
        NyarType::Utf16 => return Err("Utf16 在当前 JS glue 轨没有稳定的宿主 ABI 合同".to_string()),
        NyarType::Bottom => return Err("`void` / bottom 类型不能出现在库导出 ABI 位置".to_string()),
        NyarType::Function(_) => return Err("函数类型不能出现在库导出 ABI 位置".to_string()),
        NyarType::TraitObject(_) => return Err("TraitObject（dynamic dispatch）没有稳定的 Wasm 载体".to_string()),
        NyarType::Union(_) => return Err("匿名 union 的 Wasm ABI 合同尚未定义".to_string()),
    })
}

/// 计划构造时冻结 `[wasm_operator]` / `[ovm_operator]` / `[jvm_operator]` /
/// `[clr_operator]` 的完整 operator 计划。
///
/// 助记符只在此处解析一次；backend 之后只消费 [`TargetOperatorBinding`]，不得再从
/// 助记符字符串推断 arity、操作数改写或结果类型。未解析的绑定保留为
/// [`TargetOperatorBinding::Unfrozen`]，由 emit 侧结构化失败。
fn freeze_target_operator(link: &ExternalImportLink) -> Option<TargetOperatorBinding> {
    if let Some(mnemonic) = crate::lowering::shared::interop::wasm_operator_target(link) {
        if let Some(plan) = crate::lowering::backends::wasm::mir::numeric::freeze_wasm_numeric_binding(mnemonic) {
            return Some(TargetOperatorBinding::Frozen(plan));
        }
        // 数值表未命中时进入数组助记符表：两者都是 wasm backend 私有的冻结配方表，
        // 未知助记符统一落 `Unfrozen`，由 emit 结构化失败。
        if let Some(plan) = crate::lowering::backends::wasm::mir::array::freeze_wasm_array_binding(mnemonic) {
            return Some(TargetOperatorBinding::FrozenWasmGcArray(plan));
        }
        return Some(TargetOperatorBinding::Unfrozen);
    }
    let vm_mnemonic = crate::lowering::shared::interop::ovm_operator_target(link);
    #[cfg(feature = "ovm-lane")]
    if let Some(mnemonic) = vm_mnemonic {
        let resolved = crate::lowering::backends::ovm_mir::freeze_ovm_operator_binding(mnemonic);
        return Some(frozen_operator_binding(resolved));
    }
    #[cfg(not(feature = "ovm-lane"))]
    if vm_mnemonic.is_some() {
        return Some(TargetOperatorBinding::Unfrozen);
    }
    if let Some(mnemonic) = crate::lowering::shared::interop::jvm_operator_target(link) {
        // JVM 配方表是纯静态后端私有表，无需 feature 门。
        let resolved = crate::lowering::backends::jvm::recipe::freeze_jvm_operator_binding(mnemonic);
        return Some(frozen_operator_binding(resolved));
    }
    if let Some(mnemonic) = crate::lowering::shared::interop::clr_operator_target(link) {
        // CLR 配方表同形：纯静态后端私有表。
        let resolved = crate::lowering::backends::clr::mir::freeze_clr_operator_binding(mnemonic);
        return Some(frozen_operator_binding(resolved));
    }
    None
}

fn frozen_operator_binding(resolved: Option<TargetOperatorPlan>) -> TargetOperatorBinding {
    match resolved {
        Some(plan) => TargetOperatorBinding::Frozen(plan),
        None => TargetOperatorBinding::Unfrozen,
    }
}

/// 校验冻结计划与导入声明在 arity、操作数载体和结果槽载体上一致。
///
/// 导入声明是 Compiler 已绑定的调用合同，冻结计划是目标 operator 的物理形状；二者不一致
/// 时计划不成立，必须在此结构化失败，不得让后端按声明槽位发射后再改写或补默认值。
fn validate_frozen_operator(
    instance: ItemInstanceId,
    symbol: Option<&str>,
    link: &ExternalImportLink,
    parameter_types: &[NyarType],
    return_type: &NyarType,
    plan: &TargetOperatorPlan,
) -> Result<()> {
    let mnemonic = crate::lowering::shared::interop::wasm_operator_target(link)
        .or_else(|| crate::lowering::shared::interop::ovm_operator_target(link))
        .or_else(|| crate::lowering::shared::interop::jvm_operator_target(link))
        .or_else(|| crate::lowering::shared::interop::clr_operator_target(link))
        .unwrap_or("<unknown>");
    let function = symbol.unwrap_or("<unknown>");
    if parameter_types.len() != plan.arity {
        return Err(format!(
            "目标 operator 绑定 `{mnemonic}` 的声明 arity 与冻结计划不符: function={function} expected={} actual={} instance={instance:?} link={link}",
            plan.arity,
            parameter_types.len()
        ));
    }
    for (index, ty) in parameter_types.iter().enumerate() {
        let Some(carrier) = operand_carrier(ty)
        else {
            return Err(format!(
                "目标 operator 绑定 `{mnemonic}` 的参数 {index} 类型没有目标载体: function={function} type={ty:?} instance={instance:?} link={link}"
            ));
        };
        if carrier != plan.operand_type {
            return Err(format!(
                "目标 operator 绑定 `{mnemonic}` 的参数 {index} 操作数载体与冻结计划不符: function={function} expected={:?} actual={:?} instance={instance:?} link={link}",
                plan.operand_type, carrier
            ));
        }
    }
    let Some(result_carrier) = result_carrier(return_type)
    else {
        return Err(format!(
            "目标 operator 绑定 `{mnemonic}` 的返回类型没有结果槽载体: function={function} type={return_type:?} instance={instance:?} link={link}"
        ));
    };
    if result_carrier != plan.result_type {
        return Err(format!(
            "目标 operator 绑定 `{mnemonic}` 的结果槽载体与冻结计划不符: function={function} expected={:?} actual={:?} instance={instance:?} link={link}",
            plan.result_type, result_carrier
        ));
    }
    if let TargetOperatorRecipe::Steps(steps) = plan.recipe {
        for step in steps {
            if let TargetOperandStep::IntegerConstant { operand_type: constant_type, .. } = step {
                if *constant_type != plan.operand_type {
                    return Err(format!(
                        "目标 operator 绑定 `{mnemonic}` 的常量配方载体与冻结操作数载体不符: function={function} expected={:?} actual={:?} instance={instance:?} link={link}",
                        plan.operand_type, constant_type
                    ));
                }
            }
        }
    }
    Ok(())
}

/// 校验冻结的 wasm GC 数组计划与导入声明的实例化签名一致。
///
/// 数组 operator 的物理编码是多步的（`ref.cast` + `array.*`），arraytype 索引属于
/// 模块装配期 RepresentationPlan 注册表事实，不在计划层冻结；此处校验的是声明签名
/// 与冻结 arity/操作身份的语义形状：聚合参数必须是堆数组 `[T]`，长度/索引参数必须
/// 携带 i32 载体，`get`/`set` 的元素槽必须与数组元素类型逐点一致。形状不符即计划
/// 不成立，结构化失败，不得让后端发射后再改写。
fn validate_frozen_array_operator(
    instance: ItemInstanceId,
    symbol: Option<&str>,
    link: &ExternalImportLink,
    parameter_types: &[NyarType],
    return_type: &NyarType,
    plan: &WasmGcArrayOperatorPlan,
) -> Result<()> {
    use WasmGcArrayOperation::{Get, Length, New, Set};
    let mnemonic = crate::lowering::shared::interop::wasm_operator_target(link).unwrap_or("<unknown>");
    let function = symbol.unwrap_or("<unknown>");
    let fail = |reason: String| -> Result<()> {
        Err(format!(
            "目标数组 operator 绑定 `{mnemonic}` 的声明签名与冻结计划不符: function={function} {reason} instance={instance:?} link={link}"
        ))
    };
    if parameter_types.len() != plan.arity {
        return fail(format!("expected arity={} actual={}", plan.arity, parameter_types.len()));
    }
    fn array_element(ty: &NyarType) -> Option<&NyarType> {
        match ty {
            NyarType::Array(element) => Some(element.as_ref()),
            _ => None,
        }
    }
    let index_carrier = |ty: &NyarType| operand_carrier(ty) == Some(TargetOperandType::I32);
    match plan.operation {
        New => {
            let Some(length) = parameter_types.first()
            else {
                return fail("arity 事实缺失".to_owned());
            };
            if !index_carrier(length) {
                return fail(format!("`array.new` 的长度参数必须携带 i32 载体: actual={length:?}"));
            }
            if array_element(return_type).is_none() {
                return fail(format!("`array.new` 的返回类型必须是堆数组 `[T]`: actual={return_type:?}"));
            }
        }
        Length => {
            let Some(array) = parameter_types.first()
            else {
                return fail("arity 事实缺失".to_owned());
            };
            if array_element(array).is_none() {
                return fail(format!("`array.length` 的聚合参数必须是堆数组 `[T]`: actual={array:?}"));
            }
            if !index_carrier(return_type) {
                return fail(format!("`array.length` 的返回类型必须携带 i32 载体: actual={return_type:?}"));
            }
        }
        Get => {
            let (Some(array), Some(index)) = (parameter_types.first(), parameter_types.get(1))
            else {
                return fail("arity 事实缺失".to_owned());
            };
            let Some(element) = array_element(array)
            else {
                return fail(format!("`array.get` 的聚合参数必须是堆数组 `[T]`: actual={array:?}"));
            };
            if !index_carrier(index) {
                return fail(format!("`array.get` 的索引参数必须携带 i32 载体: actual={index:?}"));
            }
            if return_type != element {
                return fail(format!("`array.get` 的返回类型必须与数组元素类型一致: element={element:?} actual={return_type:?}"));
            }
        }
        Set => {
            let (Some(array), Some(index), Some(value)) = (parameter_types.first(), parameter_types.get(1), parameter_types.get(2))
            else {
                return fail("arity 事实缺失".to_owned());
            };
            let Some(element) = array_element(array)
            else {
                return fail(format!("`array.set` 的聚合参数必须是堆数组 `[T]`: actual={array:?}"));
            };
            if !index_carrier(index) {
                return fail(format!("`array.set` 的索引参数必须携带 i32 载体: actual={index:?}"));
            }
            if value != element {
                return fail(format!("`array.set` 的值参数必须与数组元素类型一致: element={element:?} actual={value:?}"));
            }
            if *return_type != NyarType::Unit {
                return fail(format!("`array.set` 的返回类型必须是 unit: actual={return_type:?}"));
            }
        }
    }
    Ok(())
}

fn operand_carrier(ty: &NyarType) -> Option<TargetOperandType> {
    match ty {
        NyarType::Integer32 { .. } => Some(TargetOperandType::I32),
        // `char`（u16 code unit）在这些 lane 的物理载体是 i32（ABI 层 `AbiCarrier::I32` 同构）；
        // 等值/比较配方按零扩展后的码位判定，不引入截断语义。
        NyarType::Character => Some(TargetOperandType::I32),
        NyarType::Integer64 { .. } => Some(TargetOperandType::I64),
        NyarType::Float64 => Some(TargetOperandType::F64),
        _ => None,
    }
}

fn result_carrier(ty: &NyarType) -> Option<TargetOperandType> {
    match ty {
        NyarType::Boolean => Some(TargetOperandType::I32),
        _ => operand_carrier(ty),
    }
}

fn lower_function(program: &CompiledProgram, function: &nyar_types::CanonicalFunction) -> Result<(ExecutableFunction, Vec<ItemInstanceId>)> {
    let canonical = program.canonical();
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
                let Some(sum) = program.representation().sum_reps.get(&nominal)
                else {
                    return Err(format!("sum `{nominal:?}` 缺少 RepresentationPlan 布局合同"));
                };
                if !sum.variants.contains_key(&variant) {
                    return Err(format!("sum variant `{nominal:?}/{variant:?}` 缺少 RepresentationPlan 布局合同"));
                }
            }
            if let CanonicalOperation::Invoke { callee, .. } = &instruction.operation {
                match (callee, program.representation().invoke_lowerings.get(&instruction.id)) {
                    (CanonicalCallee::Item(_), Some(InvokeLowering::Direct)) => {}
                    (CanonicalCallee::Witness(_), Some(InvokeLowering::TypedWitness)) => {
                        // 放行！在 lower_operation 里尝试消去 Witness 为具体 Item 调用。
                        // self_type 仍开放时（`None`）由 lower_operation 返回结构化失败。
                    }
                    (CanonicalCallee::Value(callee), Some(InvokeLowering::TypedReference)) => {
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
fn has_reference_instance_layout(program: &CanonicalProgram, type_id: nyar_types::TypeId) -> bool {
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

fn canonical_sum_operations(instruction: &nyar_types::CanonicalInstruction) -> Vec<(nyar_types::NominalInstanceId, nyar_types::VariantId)> {
    match instruction.operation {
        CanonicalOperation::SumNew { nominal, variant, .. }
        | CanonicalOperation::SumPayloadGet { nominal, variant, .. }
        | CanonicalOperation::SumVariantIs { nominal, variant, .. } => vec![(nominal, variant)],
        _ => Vec::new(),
    }
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
fn resolve_witness_to_item(program: &CanonicalProgram, witness: &nyar_types::CanonicalWitness) -> Result<ItemInstanceId> {
    let Some(self_type) = witness.self_type
    else {
        return Err(format!(
            "trait witness self_type 仍开放泛型，无法在 target_execution_plan 消去: trait={:?} method={:?}",
            witness.trait_declaration, witness.method_declaration
        ));
    };
    let mut matching = program
        .linked
        .item_instances
        .iter()
        .filter(|(_, record)| record.declaration == witness.method_declaration)
        .filter(|(_, record)| record.parameter_types.first() == Some(&self_type));
    let Some((instance, _)) = matching.next()
    else {
        return Err(format!(
            "trait witness 缺少 declaration={:?} self_type={self_type:?} 的实例: trait={:?}",
            witness.method_declaration, witness.trait_declaration
        ));
    };
    if matching.next().is_some() {
        return Err(format!(
            "trait witness declaration={:?} self_type={self_type:?} 对应多个实例；实例身份必须由前端冻结",
            witness.method_declaration
        ));
    }
    Ok(*instance)
}

fn lower_operation(
    program: &CanonicalProgram,
    function: &nyar_types::CanonicalFunction,
    instruction: &nyar_types::CanonicalInstruction,
    callees: &mut Vec<ItemInstanceId>,
) -> Result<InstructionKind> {
    let value = |id: nyar_types::MirValueId| Operand::Value(ValueRef(id.index()));
    // 捕获环境必须携带 SSA 身份；常量环境没有身份，禁止目标侧重造。
    let operand = |operand: &nyar_types::CanonicalOperand| -> Result<Operand> {
        match operand {
            nyar_types::CanonicalOperand::Value(id) => Ok(Operand::Value(ValueRef(id.index()))),
            nyar_types::CanonicalOperand::Constant(_) => Err("闭包捕获环境必须是已定义的 SSA 值".to_string()),
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
        CanonicalOperation::Invoke { callee: CanonicalCallee::Item(instance), arguments } => {
            if !program.linked.item_instances.contains_key(instance) {
                return Err(format!("调用 identity `{instance:?}` 未解析"));
            }
            callees.push(*instance);
            InstructionKind::Call { callee: Operand::Item(*instance), arguments: arguments.iter().map(|id| value(*id)).collect() }
        }
        CanonicalOperation::Invoke { callee: CanonicalCallee::Witness(witness), arguments } => {
            let instance = resolve_witness_to_item(program, &witness)?;
            callees.push(instance);
            InstructionKind::Call { callee: Operand::Item(instance), arguments: arguments.iter().map(|id| value(*id)).collect() }
        }
        CanonicalOperation::Invoke { callee: CanonicalCallee::Value(callee), arguments } => {
            InstructionKind::Call { callee: value(*callee), arguments: arguments.iter().map(|id| value(*id)).collect() }
        }
        // 具名函数作值：实例身份由前端冻结，目标层只校验链接事实并保留身份。
        // 被作值引用的函数不出现在任何直接调用边上，可达性必须由本指令登记，
        // 否则单态化后的实现函数体会被可执行闭包漏掉。
        CanonicalOperation::LoadFunctionRef { instance, environment, captured } => {
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
        CanonicalOperation::Copy { source } => InstructionKind::Copy { source: value(*source) },
        CanonicalOperation::StoreVar { name, value: stored, ty } => {
            InstructionKind::StoreVar { name: name.clone(), value: value(*stored), ty: ty.map(|id| lower_type(program, id)).transpose()? }
        }
        CanonicalOperation::AggregateCopy { source, destination } => {
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
        CanonicalOperation::LoadConstant { constant } => {
            InstructionKind::LoadConstant { constant: lower_constant(constant), ty: constant_encoding_type(constant) }
        }
        // 结构操作已由 frontend 依据自身内建结构操作身份判定完毕，事实所有权在此结束；
        // backend-private 计划只做载体映射，wasm backend 负责编码成 wasm GC 指令。
        // 在此重新拒绝会把已确认的结构语义降级成字符串错误，属禁止的向上推断。
        CanonicalOperation::ArrayGet { array, index } => InstructionKind::ArrayGet { array: value(*array), index: value(*index) },
        CanonicalOperation::ArraySet { array, index, value: stored } => {
            InstructionKind::ArraySet { array: value(*array), index: value(*index), value: value(*stored) }
        }
        CanonicalOperation::ArrayLength { array } => InstructionKind::ArrayLength { array: value(*array) },
        CanonicalOperation::NullIsNull { value: operand } => InstructionKind::NullIsNull { value: value(*operand) },
        CanonicalOperation::NullUnwrap { value: operand } => InstructionKind::NullUnwrap { value: value(*operand) },
        CanonicalOperation::ArrayNew { array_type, length, initialization } => InstructionKind::ArrayNew {
            array_type: lower_type(program, *array_type)?,
            length: value(*length),
            initialization: match initialization {
                CanonicalArrayInitialization::Default => nyar_types::ArrayInitialization::Default,
                CanonicalArrayInitialization::Fill(fill) => nyar_types::ArrayInitialization::Fill(value(*fill)),
            },
        },
        CanonicalOperation::ArrayFromElements { array_type, elements } => {
            // 只有 FixedArray 才需要 aggregate layout；普通 Array 由 Wasm GC array.new_fixed 处理。
            let need_layout = matches!(
                program.linked.types.get(array_type).map(|row| &row.kind),
                Some(nyar_types::CanonicalTypeKind::Array { length: Some(_), .. })
            );
            let layout_id = if need_layout {
                layout_for_type(*array_type, "ArrayFromElements")?
            }
            else {
                // 普通 Array 不需要 aggregate layout；用 dummy 0 占位，wasm backend 会跳过。
                0u32
            };
            InstructionKind::ArrayFromElements {
                layout_id,
                array_type: lower_type(program, *array_type)?,
                elements: elements.iter().map(|id| value(*id)).collect(),
            }
        }
        CanonicalOperation::TupleNew { fields, element_types } => {
            let result = instruction.results.first().copied().ok_or_else(|| format!("TupleNew 缺少结果值"))?;
            let result_type = value_type(result)?;
            if !matches!(program.linked.types.get(&result_type).map(|row| &row.kind), Some(nyar_types::CanonicalTypeKind::Tuple(_))) {
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
        CanonicalOperation::TupleGet { object, tuple_type, index } => {
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
        CanonicalOperation::StructNew { nominal, fields } => {
            InstructionKind::StructNew { nominal: *nominal, fields: fields.iter().map(|(field, id)| (*field, value(*id))).collect() }
        }
        CanonicalOperation::FieldGet { object, field } => InstructionKind::FieldGet { object: value(*object), field: *field },
        CanonicalOperation::FieldSet { object, field, value: stored } => {
            InstructionKind::FieldSet { object: value(*object), field: *field, value: value(*stored) }
        }
        CanonicalOperation::SumNew { nominal, variant, payload } => {
            InstructionKind::SumNew { nominal: *nominal, variant: *variant, payload: payload.map(value) }
        }
        CanonicalOperation::SumPayloadGet { nominal, variant, object } => {
            InstructionKind::SumPayloadGet { nominal: *nominal, variant: *variant, object: value(*object) }
        }
        CanonicalOperation::SumVariantIs { nominal, variant, object } => {
            InstructionKind::SumVariantIs { nominal: *nominal, variant: *variant, object: value(*object) }
        }
        // 语言层显式 `as`：目标类型是后端必须编码的事实，不能丢成 Copy。
        CanonicalOperation::Cast { source, ty } => InstructionKind::Cast { source: value(*source), ty: lower_type(program, *ty)? },
        unsupported => return Err(format!("canonical 操作尚无目标私有合同: {unsupported:?}")),
    })
}

/// 文本常量的编码身份由常量变体自身决定；整数/浮点/空值没有在这里声称的编码身份。
fn constant_encoding_type(constant: &CanonicalConstant) -> Option<NyarType> {
    match constant {
        CanonicalConstant::Utf8(_) => Some(NyarType::Utf8),
        CanonicalConstant::Utf16(_) => Some(NyarType::Utf16),
        _ => None,
    }
}

fn lower_constant(constant: &CanonicalConstant) -> Constant {
    match constant {
        CanonicalConstant::Int(v) => Constant::Int(*v),
        // 位模式原样重建：`-0.0` 与 NaN payload 不在计划层被规范化。
        CanonicalConstant::Float64(bits) => Constant::from(f64::from_bits(*bits)),
        CanonicalConstant::Bool(v) => Constant::Bool(*v),
        CanonicalConstant::Utf8(v) => Constant::Utf8(v.clone()),
        CanonicalConstant::Utf16(v) => Constant::Utf16(v.clone()),
        CanonicalConstant::Unit => Constant::Unit,
        CanonicalConstant::Null => Constant::Null,
    }
}

/// 函数声明的签名（参数与返回类型）是否仍含未闭合的 Generic。
///
/// 签名含 Generic 说明该实例是声明模板而非单态化后的闭合实例；目标层无法编码这种
/// 签名，因此它只能作为失败原因，不能作为「可以跳过」的判据。
fn signature_contains_generic(program: &CanonicalProgram, function: &nyar_types::CanonicalFunction) -> bool {
    function.parameters.iter().any(|(_, ty)| type_contains_generic_deep(program, *ty))
        || type_contains_generic_deep(program, function.return_type)
}

/// 递归检查 canonical 类型（含嵌套）是否包含 Generic。
fn type_contains_generic_deep(program: &CanonicalProgram, id: nyar_types::TypeId) -> bool {
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
        nyar_types::CanonicalTypeKind::LabeledTuple { fields } => {
            fields.iter().any(|(_, ty)| type_contains_generic_deep(program, *ty))
        }
        nyar_types::CanonicalTypeKind::Array { element, .. } => type_contains_generic_deep(program, *element),
        nyar_types::CanonicalTypeKind::Nullable(element) => type_contains_generic_deep(program, *element),
        nyar_types::CanonicalTypeKind::Function { parameters, return_type } => {
            parameters.iter().any(|a| type_contains_generic_deep(program, *a)) || type_contains_generic_deep(program, *return_type)
        }
        _ => false,
    }
}

fn lower_type(program: &CanonicalProgram, id: nyar_types::TypeId) -> Result<NyarType> {
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
            // `Float64` 与整数同构：`Value` 已有独立 `F64` 载体，不降成 `i64` 位模式。
            // f32 仍无目标合同（不属于本轮），继续走 `unsupported` 分支。
            nyar_types::CanonicalPrimitiveType::Float { bits: 64 } => Ok(NyarType::Float64),
            nyar_types::CanonicalPrimitiveType::Character => Ok(NyarType::Character),
            nyar_types::CanonicalPrimitiveType::Utf8 => Ok(NyarType::Utf8),
            nyar_types::CanonicalPrimitiveType::Utf16 => Ok(NyarType::Utf16),
            unsupported => Err(format!("primitive 尚无目标合同: {unsupported:?}")),
        },
        CanonicalTypeKind::Tuple(items) => Ok(NyarType::Tuple(items.iter().map(|id| lower_type(program, *id)).collect::<Result<_>>()?)),
        // 唯一标签擦除点：标签只属于 canonical 身份，目标层按位置布局，`NyarType`
        // 与 `LayoutKey` 保持无标签。类型身份不等但布局相同的元组在 ABI 上允许。
        CanonicalTypeKind::LabeledTuple { fields } => Ok(NyarType::Tuple(
            fields.iter().map(|(_, ty)| lower_type(program, *ty)).collect::<Result<_>>()?,
        )),
        CanonicalTypeKind::Array { element, length: None } => Ok(NyarType::Array(Box::new(lower_type(program, *element)?))),
        CanonicalTypeKind::Array { element, length: Some(length) } => Ok(NyarType::FixedArray {
            element: Box::new(lower_type(program, *element)?),
            length: usize::try_from(*length).map_err(|_| format!("固定数组长度溢出"))?,
        }),
        CanonicalTypeKind::Nullable(element) => Ok(NyarType::Nullable(Box::new(lower_type(program, *element)?))),
        CanonicalTypeKind::Nominal { declaration, arguments } => {
            if !arguments.is_empty() {
                return Ok(NyarType::Apply(
                    Box::new(lower_type(program, *declaration)?),
                    arguments.iter().map(|argument| lower_type(program, *argument)).collect::<Result<Vec<_>>>()?,
                ));
            }
            let spelling = program
                .linked
                .type_surface_spellings
                .get(if *declaration == id { &id } else { declaration })
                .or_else(|| program.linked.type_surface_spellings.get(&id));
            if let Some(name) = spelling {
                if let Some(primitive) = NyarType::from_surface_primitive(name.as_str()) {
                    return Ok(primitive);
                }
                // 已冻结实例级布局的零实参 nominal 铸造为携带身份的 `Nominal`：
                // 存储类/GC 字段/ABI 由 `layout` 身份解析，拼写仅诊断显示。
                // 未登记布局仍产出裸 `Named` 残差，由预发射门禁结构化拒绝。
                if let Some(layout) = program.linked.aggregate_layout_by_type.get(&id).copied() {
                    return Ok(NyarType::Nominal { layout, spelling: name.clone() });
                }
                if program.linked.nominal_instances.values().any(|record| record.ty == id) {
                    return Ok(NyarType::Named(name.clone()));
                }
                return Err(format!("nominal type `{name}` 缺少目标 layout 或 carrier 合同"));
            }
            if *declaration != id {
                return lower_type(program, *declaration);
            }
            Err(format!("canonical 类型尚无目标合同: Nominal {{ declaration: {declaration:?}, arguments: [] }}"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::target_operator::TargetOperatorEncoding;
    use nyar_types::{CanonicalBlockId, CanonicalFunction, CanonicalPrimitiveType, ItemId, TypeRecord};

    fn type_id(index: u32) -> nyar_types::TypeId {
        nyar_types::TypeId::from_index(index).expect("测试类型身份")
    }

    fn program_with_types(rows: Vec<(nyar_types::TypeId, CanonicalTypeKind)>) -> CanonicalProgram {
        let mut program = CanonicalProgram::default();
        for (id, kind) in rows {
            program.linked.types.insert(id, TypeRecord { declaration: id, kind });
        }
        program
    }

    fn function_with_signature(parameters: Vec<nyar_types::TypeId>, return_type: nyar_types::TypeId) -> nyar_types::CanonicalFunction {
        CanonicalFunction {
            instance: ItemInstanceId::from_index(0).expect("测试实例身份"),
            parameters: parameters
                .into_iter()
                .enumerate()
                .map(|(index, ty)| (nyar_types::MirValueId::from_index(u32::try_from(index).expect("测试值下标")).expect("测试值身份"), ty))
                .collect(),
            parameter_names: Vec::new(),
            return_type,
            value_types: BTreeMap::new(),
            entry: CanonicalBlockId(0),
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

    /// `Float{bits:64}` 必须有目标合同：f64 的宿主符号（算术/比较/转换）都以它为前提。
    /// `Float{bits:32}` 仍未登记，必须继续失败闭合——本测试只锁定事实，不替 f32 开合同。
    #[test]
    fn float64_primitive_has_target_contract_while_float32_stays_unsupported() {
        let program = program_with_types(vec![
            (type_id(0), CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Float { bits: 64 })),
            (type_id(1), CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Float { bits: 32 })),
        ]);
        assert_eq!(lower_type(&program, type_id(0)).expect("f64 必须有目标合同"), NyarType::Float64);
        let error = lower_type(&program, type_id(1)).expect_err("f32 本轮仍无目标合同");
        assert!(error.contains("Float { bits: 32 }"), "诊断必须点名未登记的 f32，实际：{error}");
    }

    /// canonical 用 `u64` 位模式承载 float64，计划层必须按位重建：`-0.0` 的符号位
    /// 与 NaN payload 一旦被 `f64` 语义规范化，常量池就再也分不出 `-0.0` 与 `0.0`。
    #[test]
    fn float64_canonical_constant_preserves_sign_bit_and_nan_payload() {
        let negative_zero = CanonicalConstant::Float64((-0.0f64).to_bits());
        let positive_zero = CanonicalConstant::Float64(0.0f64.to_bits());
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
        match lower_constant(&CanonicalConstant::Float64(nan_payload)) {
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
        assert_eq!(constant_encoding_type(&CanonicalConstant::Utf8("文本".to_string())), Some(NyarType::Utf8));
        assert_eq!(constant_encoding_type(&CanonicalConstant::Utf16("文本".to_string())), Some(NyarType::Utf16));
        assert_eq!(constant_encoding_type(&CanonicalConstant::Int(1)), None, "整数宽度不由常量变体单独声称");
        assert_eq!(constant_encoding_type(&CanonicalConstant::Unit), None, "Unit 不声称文本编码");
    }

    /// `Raise` 是唯一进入目标合同的 canonical effect：映射为 executable `PerformEffect`，
    /// payload 与 resume 目标原样保留，由各后端决定自己的目标语义（wasm trap / ovm 分派）。
    #[test]
    fn raise_terminator_enters_the_target_contract() {
        let payload = nyar_types::MirValueId::from_index(7).expect("测试值身份");
        let lowered = lower_terminator(CanonicalTerminator::PerformEffect {
            effect: nyar_types::canonical_program::CanonicalEffectKind::Raise,
            payload: Some(payload),
            resume_target: CanonicalBlockId(3),
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
        let error = lower_terminator(CanonicalTerminator::PerformEffect {
            effect: nyar_types::canonical_program::CanonicalEffectKind::Yield,
            payload: None,
            resume_target: CanonicalBlockId(1),
        })
        .expect_err("`Yield` 不得进入目标合同");
        assert!(error.contains("尚无目标合同"), "诊断必须保持失败闭合语义，实际：{error}");
    }

    /// `u64` 无符号小于的托管目标配方：CLR 冻结为原生 `clt.un`，JVM 冻结为符号位
    /// 翻转展开码；操作数载体 `I64`、结果槽 `I32`，与 `__u64_lt` 的声明签名一致。
    #[test]
    fn unsigned_u64_less_than_freezes_managed_operator_plans() {
        let clr_link = nyar::ExternalImportLink::host(Some(nyar::Identifier::new("clr-operator")), vec!["i64.lt.un".to_string()]);
        let Some(TargetOperatorBinding::Frozen(clr_plan)) = freeze_target_operator(&clr_link)
        else {
            panic!("`i64.lt.un` 必须冻结")
        };
        assert_eq!(clr_plan.arity, 2);
        assert_eq!(clr_plan.operand_type, TargetOperandType::I64);
        assert_eq!(clr_plan.result_type, TargetOperandType::I32);
        assert_eq!(clr_plan.encoding, TargetOperatorEncoding::Clr(crate::transitional::msil::MsilOpcode::CltUn));

        let jvm_link =
            nyar::ExternalImportLink::host(Some(nyar::Identifier::new("jvm-operator")), vec!["long.compare_less_than_unsigned".to_string()]);
        let Some(TargetOperatorBinding::Frozen(jvm_plan)) = freeze_target_operator(&jvm_link)
        else {
            panic!("`long.compare_less_than_unsigned` 必须冻结")
        };
        assert_eq!(jvm_plan.arity, 2);
        assert_eq!(jvm_plan.operand_type, TargetOperandType::I64);
        assert_eq!(jvm_plan.result_type, TargetOperandType::I32);
        assert!(matches!(
            jvm_plan.encoding,
            TargetOperatorEncoding::Jvm(crate::lowering::backends::jvm::JvmOperatorCode::LongCompareLessThanUnsigned)
        ));
    }

    /// `u64`/`u32` 无符号除、余、逻辑右移的托管目标配方：
    /// CLR 冻结为原生 `div.un`/`rem.un`/`shr.un`；JVM 冻结为 `lushr`/`iushr` 或
    /// `Integer/Long.divideUnsigned/remainderUnsigned` 静态调用。
    #[test]
    fn unsigned_div_rem_shift_freeze_managed_operator_plans() {
        use crate::lowering::backends::jvm::JvmOperatorCode;
        use crate::transitional::msil::MsilOpcode;

        for (mnemonic, operand, result, opcode) in [
            ("i32.div.un", TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::DivUn),
            ("i32.rem.un", TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::RemUn),
            ("i32.shr.un", TargetOperandType::I32, TargetOperandType::I32, MsilOpcode::ShrUn),
            ("i64.div.un", TargetOperandType::I64, TargetOperandType::I64, MsilOpcode::DivUn),
            ("i64.rem.un", TargetOperandType::I64, TargetOperandType::I64, MsilOpcode::RemUn),
            ("i64.shr.un", TargetOperandType::I64, TargetOperandType::I64, MsilOpcode::ShrUn),
        ] {
            let link = nyar::ExternalImportLink::host(Some(nyar::Identifier::new("clr-operator")), vec![mnemonic.to_string()]);
            let Some(TargetOperatorBinding::Frozen(plan)) = freeze_target_operator(&link)
            else {
                panic!("`{mnemonic}` 必须冻结")
            };
            assert_eq!(plan.arity, 2);
            assert_eq!(plan.operand_type, operand);
            assert_eq!(plan.result_type, result);
            assert_eq!(plan.encoding, TargetOperatorEncoding::Clr(opcode), "`{mnemonic}` 编码");
        }

        for (mnemonic, operand, result, code) in [
            ("integer.divide_unsigned", TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerDivideUnsigned),
            ("integer.remainder_unsigned", TargetOperandType::I32, TargetOperandType::I32, JvmOperatorCode::IntegerRemainderUnsigned),
            ("long.divide_unsigned", TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongDivideUnsigned),
            ("long.remainder_unsigned", TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongRemainderUnsigned),
            ("long.shift_right_unsigned", TargetOperandType::I64, TargetOperandType::I64, JvmOperatorCode::LongShiftRightUnsigned),
        ] {
            let link = nyar::ExternalImportLink::host(Some(nyar::Identifier::new("jvm-operator")), vec![mnemonic.to_string()]);
            let Some(TargetOperatorBinding::Frozen(plan)) = freeze_target_operator(&link)
            else {
                panic!("`{mnemonic}` 必须冻结")
            };
            assert_eq!(plan.arity, 2);
            assert_eq!(plan.operand_type, operand);
            assert_eq!(plan.result_type, result);
            assert_eq!(plan.encoding, TargetOperatorEncoding::Jvm(code), "`{mnemonic}` 编码");
        }
    }

    /// 反例：近似但未冻结的助记符必须保持 `Unfrozen`，由 emit 侧结构化失败，
    /// 不得按名字前缀猜默认指令。
    #[test]
    fn near_miss_unsigned_mnemonics_stay_unfrozen() {
        for (lane, mnemonic) in [
            ("clr-operator", "i64.div_unsigned"),
            ("clr-operator", "i64.shr.un.extra"),
            ("jvm-operator", "long.div.un"),
            ("jvm-operator", "long.shift_right_unsigned.extra"),
        ] {
            let link = nyar::ExternalImportLink::host(Some(nyar::Identifier::new(lane)), vec![mnemonic.to_string()]);
            assert_eq!(freeze_target_operator(&link), Some(TargetOperatorBinding::Unfrozen), "`{mnemonic}` 不得冻结");
        }
    }
}

fn lower_terminator(terminator: CanonicalTerminator) -> Result<Terminator> {
    let value = |id: nyar_types::MirValueId| Operand::Value(ValueRef(id.index()));
    Ok(match terminator {
        CanonicalTerminator::Return { value: result } => Terminator::Return { value: result.map(value) },
        CanonicalTerminator::Jump { target, arguments } => {
            Terminator::Jump { target: BlockRef(target.0), arguments: arguments.into_iter().map(value).collect() }
        }
        CanonicalTerminator::Branch { condition, then_target, else_target } => {
            Terminator::Branch { condition: value(condition), then_target: BlockRef(then_target.0), else_target: BlockRef(else_target.0) }
        }
        CanonicalTerminator::Unreachable => Terminator::Unreachable,
        // `Raise` 是唯一进入目标合同的 canonical effect：wasm 以 trap（`unreachable`）
        // 终止，ovm 以原生 `PerformEffect` 分派；托管 lane 在 emit 侧结构化失败。
        // 其余 effect（yield/await/…）需要续体运行时或状态机重写，本层尚无合同，继续失败闭合。
        CanonicalTerminator::PerformEffect { effect: nyar_types::canonical_program::CanonicalEffectKind::Raise, payload, resume_target } => {
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

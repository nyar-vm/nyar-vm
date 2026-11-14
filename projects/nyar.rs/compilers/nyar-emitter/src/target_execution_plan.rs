//! Compiler 成功载荷到目标私有执行计划的唯一边界。

use std::collections::BTreeMap;

use crate::Result;
use nyar::QualifiedName;
use nyar_types::{ItemInstanceId, NyarType};

use crate::{
    backend_plan_views::{ExecutableFunction, FunctionView, SuspendMetadataView},
    contracts::Block,
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
    export_names: BTreeMap<ItemInstanceId, String>,
    external_import_links: BTreeMap<ItemInstanceId, nyar::ExternalImportLink>,
    external_call_edges: Vec<nyar_types::CanonicalExternalCallEdge>,
    internal_call_edges: Vec<nyar_types::CanonicalCallEdge>,
    aggregate_layouts: nyar_types::AggregateLayoutPlan,
    aggregate_layout_by_nominal: BTreeMap<nyar_types::NominalInstanceId, nyar_types::LayoutId>,
    aggregate_layout_by_field: BTreeMap<nyar_types::FieldId, (nyar_types::LayoutId, u32)>,
    /// 字段声明值类型（canonical `TypeId`）已冻结的布局身份。
    ///
    /// 布局计划字段记录的是**构造器形态**（`Apply(Named(declaration), args)`），
    /// 而 SSA 槽是**结算形态**（`Nominal { layout }`）。字段合同以声明侧冻结的
    /// 布局身份判等，不比较拼写或结构形态；声明值类型不是聚合时无此项。
    field_type_layouts: BTreeMap<nyar_types::FieldId, nyar_types::LayoutId>,
    aggregate_layout_by_type: BTreeMap<nyar_types::TypeId, nyar_types::LayoutId>,
    /// 以结构化 `NyarType` 为键布局索引。
    ///
    /// `NyarType::Apply`（如 `ArrayList<i64>`）不携带 `LayoutId`；库导出 ABI 的
    /// 字段/容器桥需要从类型拼写回查冻结布局。索引只在 canonical 计划构建时
    /// 由 `aggregate_layout_by_type` 逐项 `lower_type` 生成，不按名字猜布局。
    aggregate_layout_by_nyar_type: BTreeMap<NyarType, nyar_types::LayoutId>,
    /// 以结构化 `NyarType` 为键的 unite 布局索引。
    ///
    /// unite 没有 `LayoutId`，但库导出 ABI 需要按变体身份读回值。索引只在
    /// canonical 计划构建时由 `sum_types` 的已冻结名义身份 `lower_type` 生成，
    /// 不按名字猜布局。
    sum_layout_by_nyar_type: BTreeMap<NyarType, nyar_types::SumTypeLayout>,
    flags_types: Vec<nyar_types::FlagsLayout>,
    singleton_instances: Vec<nyar_types::SingletonInstancePlan>,
    functions: BTreeMap<ItemInstanceId, ExecutableFunction>,
    abi_names: BTreeMap<ItemInstanceId, QualifiedName>,
    imports: BTreeMap<ItemInstanceId, BackendImport>,
    sum_reps: BTreeMap<nyar_types::NominalInstanceId, SumRepresentation>,
    /// 程序级文本表示模式；`None` 表示计划生产者尚未声明该事实，
    /// 后端按托管语义 fail-closed，绝不由布局注册表为空回落成 raw。
    program_text_representation: Option<ProgramTextRepresentation>,
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
}

/// 程序级文本载体表示模式：表示计划的正式事实。
///
/// raw/managed 的选择必须由计划生产者显式声明并冻结进目标计划，发射期与
/// ABI 合同只认这份声明；禁止由「布局注册表里没有 `utf8` 托管布局」反推
/// 程序选择了 raw——上游漏登记整类布局时，missing 会被吸收成合法表示。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramTextRepresentation {
    /// 程序显式选择 raw 宿主串表示；未逐槽冻结托管布局的文本槽才是合法 raw。
    Raw,
    /// 程序选择托管文本表示；没有逐值/逐槽载体事实的文本槽一律按 missing 拒收。
    Managed,
}

/// 平台 sum 表示计划：variant 身份由语言冻结，tag 宽度与槽位由平台决定。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SumRepresentation {
    /// 完整的 sum 名义实例身份。
    pub nominal: nyar_types::NominalInstanceId,
    /// 本平台选择的 tag 宽度（字节），必须落在该 lane 的合同范围内。
    pub tag_width: u32,
    /// variant 身份到 tag/payload 合同的稀疏表。
    pub variants: BTreeMap<nyar_types::VariantId, SumVariantRepresentation>,
}

/// 平台 sum variant 合同。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SumVariantRepresentation {
    /// 语言稳定 variant 身份。
    pub id: nyar_types::VariantId,
    /// 平台 discriminant；不是语义身份。
    pub tag: u32,
    /// 已代入 payload 类型；无 payload 时为空。
    pub payload_type: Option<nyar_types::TypeId>,
}

/// 语言 × 目标适配器准备完成的目标输入。
///
/// 编排层只接受这份已经按目标合同闭合的结构；它不读取语言 MIR，也不从类型
/// 形状补猜布局或调用策略。语言适配器负责把它自己的已验证事实映射到这里。
#[derive(Debug, Clone, Default)]
pub struct TargetPlanInput {
    /// 模块名。
    pub module_name: String,
    /// 片段身份。
    pub fragment_id: nyar::Identifier,
    /// 已导出的 callable 实例。
    pub exported_operations: Vec<ItemInstanceId>,
    /// 片段要求的能力标签。
    pub required_capabilities: Vec<nyar::CapabilityTag>,
    /// 片段选定的优化理论。
    pub theory_bundle: nyar::TheoryBundle,
    /// 入口 callable 实例。
    pub entry_operation: Option<ItemInstanceId>,
    /// callable 到公开导出名的映射。
    pub export_names: BTreeMap<ItemInstanceId, String>,
    /// 外部导入链接映射。
    pub external_import_links: BTreeMap<ItemInstanceId, nyar::ExternalImportLink>,
    /// 外部调用边。
    pub external_call_edges: Vec<nyar_types::CanonicalExternalCallEdge>,
    /// 内部调用边。
    pub internal_call_edges: Vec<nyar_types::CanonicalCallEdge>,
    /// 聚合布局计划。
    pub aggregate_layouts: nyar_types::AggregateLayoutPlan,
    /// nominal → 布局身份。
    pub aggregate_layout_by_nominal: BTreeMap<nyar_types::NominalInstanceId, nyar_types::LayoutId>,
    /// 字段 → (布局身份, 槽位)。
    pub aggregate_layout_by_field: BTreeMap<nyar_types::FieldId, (nyar_types::LayoutId, u32)>,
    /// 字段声明值类型 → 冻结布局身份。
    pub field_type_layouts: BTreeMap<nyar_types::FieldId, nyar_types::LayoutId>,
    /// 完整类型身份 → 布局身份。
    pub aggregate_layout_by_type: BTreeMap<nyar_types::TypeId, nyar_types::LayoutId>,
    /// 结构化类型 → 布局身份。
    pub aggregate_layout_by_nyar_type: BTreeMap<NyarType, nyar_types::LayoutId>,
    /// 结构化类型 → unite 布局。
    pub sum_layout_by_nyar_type: BTreeMap<NyarType, nyar_types::SumTypeLayout>,
    /// flags 类型布局。
    pub flags_types: Vec<nyar_types::FlagsLayout>,
    /// 单例实例计划。
    pub singleton_instances: Vec<nyar_types::SingletonInstancePlan>,
    /// 已降低的目标函数。
    pub functions: BTreeMap<ItemInstanceId, ExecutableFunction>,
    /// callable 到 ABI 名称的映射。
    pub abi_names: BTreeMap<ItemInstanceId, QualifiedName>,
    /// 已声明导入。
    pub imports: BTreeMap<ItemInstanceId, BackendImport>,
    /// nominal sum → 表示合同。
    pub sum_reps: BTreeMap<nyar_types::NominalInstanceId, SumRepresentation>,
    /// 程序级文本表示；`None` 表示生产者未声明该事实。
    pub program_text_representation: Option<ProgramTextRepresentation>,
}

impl TargetExecutionPlan {
    /// 接受语言 × 目标适配器准备完成的目标输入。
    ///
    /// 这里不做语义推断，只把已冻结的目标事实收进计划；缺少的事实由适配器
    /// 在其自己的验证门里失败。
    pub fn from_prepared_input(input: TargetPlanInput) -> Self {
        Self {
            module_name: input.module_name,
            fragment_id: input.fragment_id,
            exported_operations: input.exported_operations,
            required_capabilities: input.required_capabilities,
            theory_bundle: input.theory_bundle,
            entry_operation: input.entry_operation,
            export_names: input.export_names,
            external_import_links: input.external_import_links,
            external_call_edges: input.external_call_edges,
            internal_call_edges: input.internal_call_edges,
            aggregate_layouts: input.aggregate_layouts,
            aggregate_layout_by_nominal: input.aggregate_layout_by_nominal,
            aggregate_layout_by_field: input.aggregate_layout_by_field,
            field_type_layouts: input.field_type_layouts,
            aggregate_layout_by_type: input.aggregate_layout_by_type,
            aggregate_layout_by_nyar_type: input.aggregate_layout_by_nyar_type,
            sum_layout_by_nyar_type: input.sum_layout_by_nyar_type,
            flags_types: input.flags_types,
            singleton_instances: input.singleton_instances,
            functions: input.functions,
            abi_names: input.abi_names,
            imports: input.imports,
            sum_reps: input.sum_reps,
            program_text_representation: input.program_text_representation,
        }
    }

    pub(crate) fn from_entry_function(function: ExecutableFunction) -> Result<Self> {
        let entry = ItemInstanceId::from_index(0).expect("测试入口身份");
        let name = QualifiedName::new(vec![nyar::Identifier::new("main")]);
        Ok(Self {
            entry_operation: Some(entry),
            functions: BTreeMap::from([(entry, function)]),
            abi_names: BTreeMap::from([(entry, name)]),
            ..Self::default()
        })
    }

    /// 从 ABI 名称表构造测试用目标私有计划。
    ///
    /// 计划只携带逻辑导出身份与签名；各 lane 在自己的边界按签名冻结物理载体，
    /// 不把任何平台的载体缓存写回共享计划。
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

    /// 返回可写导出名表（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_export_names_mut(&mut self) -> &mut BTreeMap<ItemInstanceId, String> {
        &mut self.export_names
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

    /// 登记 unite 的冻结 sum 布局索引（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_register_sum_layout(&mut self, ty: NyarType, sum: nyar_types::SumTypeLayout) -> &mut Self {
        self.sum_layout_by_nyar_type.insert(ty, sum);
        self
    }

    /// 登记 nominal sum 的冻结表示合同（集成测试专用）。
    #[doc(hidden)]
    pub fn testing_register_sum_representation(
        &mut self,
        nominal: nyar_types::NominalInstanceId,
        sum: SumRepresentation,
    ) -> &mut Self {
        self.sum_reps.insert(nominal, sum);
        self
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

    /// 返回公开导出名映射。
    pub fn export_names(&self) -> &BTreeMap<ItemInstanceId, String> {
        &self.export_names
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

    /// 返回已冻结的 unite 布局。
    pub fn sum_layouts(&self) -> impl Iterator<Item = &nyar_types::SumTypeLayout> + '_ {
        self.sum_layout_by_nyar_type.values()
    }

    /// 返回 nominal 实例到布局 ID 的映射。
    pub fn aggregate_layout_by_nominal(&self) -> &BTreeMap<nyar_types::NominalInstanceId, nyar_types::LayoutId> {
        &self.aggregate_layout_by_nominal
    }

    /// 返回字段 ID 到布局槽位的映射。
    pub fn aggregate_layout_by_field(&self) -> &BTreeMap<nyar_types::FieldId, (nyar_types::LayoutId, u32)> {
        &self.aggregate_layout_by_field
    }

    /// 返回字段声明值类型到冻结布局身份的映射。
    ///
    /// 只登记声明值类型本身是聚合的字段；标量/合成容器字段没有布局身份，
    /// 由结构化形状比较处理。
    pub fn field_type_layouts(&self) -> &BTreeMap<nyar_types::FieldId, nyar_types::LayoutId> {
        &self.field_type_layouts
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

    /// 程序级文本表示模式；`None` 表示计划未声明该事实，后端按托管语义 fail-closed。
    pub fn program_text_representation(&self) -> Option<ProgramTextRepresentation> {
        self.program_text_representation
    }

    /// 冻结程序级文本表示模式：正式表示计划事实的唯一写入通道，
    /// 由计划生产者（Compiler 管线 / 测试夹具）在构造期显式声明；
    /// 发射期只读，禁止由布局注册表为空反推 raw。
    pub fn freeze_program_text_representation(&mut self, representation: ProgramTextRepresentation) -> &mut Self {
        self.program_text_representation = Some(representation);
        self
    }

    /// 清除程序级文本表示事实（集成测试专用）。
    ///
    /// 只用于验证「未声明该事实时后端必须失败闭合」；生产管线不得调用。
    #[doc(hidden)]
    pub fn testing_clear_program_text_representation(&mut self) -> &mut Self {
        self.program_text_representation = None;
        self
    }

    /// 返回结构化 `NyarType` 到 unite 布局的映射。
    ///
    /// unite 没有 `LayoutId`；库导出 ABI 的变体身份合同只能来自这份冻结索引。
    pub fn sum_layout_by_nyar_type(&self) -> &BTreeMap<NyarType, nyar_types::SumTypeLayout> {
        &self.sum_layout_by_nyar_type
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
    pub fn sum_representations(&self) -> &BTreeMap<nyar_types::NominalInstanceId, SumRepresentation> {
        &self.sum_reps
    }
}

//! Shared executable lowering utilities for backend drivers.

pub mod slots;

use nyar::SemanticType;

/// Identity helper: executable payloads already carry platform [`SemanticType`].
pub fn platform_type(ty: &SemanticType) -> SemanticType {
    ty.clone()
}
use std::borrow::Cow;

use nyar_types::{AggregateLayoutPlan, FieldId, LayoutId, NominalInstanceId, TypeId};

use crate::{
    FragmentSubmission,
    backend_plan_views::{ExecutableBlockRef, ExecutableFunction, ExecutableStorageKind, ExecutableTerminator},
    contracts::InstructionKind,
    lowering::shared::physical_layout::{
        LayoutPolicy, ResolvedAggregateLayout, ResolvedFieldLayout, ResolvedLayoutPlan, resolve_physical_layouts,
    },
};

/// Cross-backend lowering context attached to a fragment submission.
///
/// 上下文只提供**目标已解析**的物理布局：偏移、宽度与对齐必须由调用方选择
/// [`LayoutPolicy`] 后解析得到。语言语义布局（无物理 extent）不能作为布局查询
/// 来源，因此不再提供未解析构造。
pub struct ExecutableLoweringContext<'a> {
    pub submission: &'a FragmentSubmission,
    pub layouts: Cow<'a, ResolvedLayoutPlan>,
}

impl<'a> ExecutableLoweringContext<'a> {
    /// 按目标数据布局策略解析物理布局。
    pub fn new(submission: &'a FragmentSubmission, policy: LayoutPolicy) -> crate::Result<Self> {
        let layouts = resolve_physical_layouts(submission.backend_plan.aggregate_layouts(), policy)?;
        Ok(Self { submission, layouts: Cow::Owned(layouts) })
    }

    pub fn require_layout_id(layout_id: Option<LayoutId>, context: &str) -> LayoutId {
        layout_id.unwrap_or_else(|| panic!("missing layout_id for {context}; executable instructions must carry layout metadata"))
    }

    pub fn layout_by_id(&self, id: LayoutId) -> Option<&ResolvedAggregateLayout> {
        self.layouts.layout(id)
    }

    pub fn layout_by_nominal(&self, nominal: NominalInstanceId) -> Option<&ResolvedAggregateLayout> {
        self.submission.backend_plan.aggregate_layout_by_nominal().get(&nominal).and_then(|id| self.layout_by_id(*id))
    }

    pub fn layout_by_type_id(&self, ty: TypeId) -> Option<&ResolvedAggregateLayout> {
        self.submission.backend_plan.aggregate_layout_by_type().get(&ty).and_then(|id| self.layout_by_id(*id))
    }

    /// 按结构化类型查冻结布局。
    ///
    /// `Nominal` 自带 `LayoutId`；`Apply`（泛型实例）从计划冻结的
    /// `SemanticType → LayoutId` 索引解析。两者之外不做名字回查。
    pub fn layout_for_nyar_type(&self, ty: &SemanticType) -> Option<&ResolvedAggregateLayout> {
        let layout_id = match ty {
            SemanticType::Nominal { layout, .. } => *layout,
            _ => *self.submission.backend_plan.aggregate_layout_by_nyar_type().get(ty)?,
        };
        self.layout_by_id(layout_id)
    }

    pub fn field_layout_by_id(&self, field: FieldId) -> Option<(&ResolvedAggregateLayout, &ResolvedFieldLayout, u32)> {
        let (layout_id, slot) = self.submission.backend_plan.aggregate_layout_by_field().get(&field)?;
        let layout = self.layout_by_id(*layout_id)?;
        let field_layout = layout.fields.get(*slot as usize)?;
        Some((layout, field_layout, *slot))
    }

    pub fn field_layout(&self, layout_id: LayoutId, field: &str) -> Option<&ResolvedFieldLayout> {
        self.layout_by_id(layout_id).and_then(|layout| layout.fields.iter().find(|item| item.name == field))
    }

    /// 类型的存储类；每一条分支都来自类型自身的结构形状或已绑定身份。
    ///
    /// 这里既不做 layout 名称回查，也没有 `Reference` 默认值：`Named` 残差
    /// 不携带 declaration / `TypeId`，不存在可按身份解析的名义 layout。
    /// [`validate_storage_identities`] 已在 lowering 前拒绝该形状。
    pub fn storage_for_type(&self, ty: &SemanticType) -> ExecutableStorageKind {
        match ty {
            SemanticType::Bottom
            | SemanticType::Unit
            | SemanticType::Boolean
            | SemanticType::Integer8 { .. }
            | SemanticType::Integer16 { .. }
            | SemanticType::Integer32 { .. }
            | SemanticType::Integer64 { .. }
            | SemanticType::Integer128 { .. }
            | SemanticType::Float32
            | SemanticType::Float64
            | SemanticType::Character
            | SemanticType::Utf8
            | SemanticType::Utf16 => ExecutableStorageKind::Value,
            SemanticType::Tuple(_) | SemanticType::FixedArray { .. } => ExecutableStorageKind::Value,
            SemanticType::Array(_)
            | SemanticType::TraitObject(_)
            | SemanticType::Union(_)
            | SemanticType::Nullable(_)
            | SemanticType::Apply(..)
            | SemanticType::Function(_) => ExecutableStorageKind::Reference,
            // `Nominal` 携带 canonical 冻结的实例级布局身份：存储类按 `layout` 解析，
            // 不按拼写匹配。布局缺失属合同破坏（预发射门禁已拒绝该形状）。
            SemanticType::Nominal { layout, spelling } => match self.layout_by_id(*layout) {
                Some(layout) => layout.storage,
                None => panic!(
                    "executable lowering contract violation: `Nominal({spelling})` 的 layout `{layout:?}` 未注册，\
身份无法解析存储类（预编码校验见 `validate_storage_identities`）"
                ),
            },
            SemanticType::Named(name) => panic!(
                "executable lowering contract violation: `Named({name})` 无 declaration / `TypeId` 身份，\
禁止按短名解析名义 layout 或回落 `Reference`（预编码校验见 `validate_storage_identities`）"
            ),
        }
    }
}

/// 可执行合同里无法按身份解析存储类的名义类型残差。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedStorageIdentity {
    /// 出现该残差的函数符号；layout 字段来源时为空。
    pub function: String,
    /// 出现位置（参数 / 返回值 / SSA 值 / 指令 / layout 字段）。
    pub site: String,
    /// 残差类型。
    pub ty: SemanticType,
}

/// 预编码校验：可执行合同不得携带无身份的名义类型残差。
///
/// `SemanticType::Named` 只有短名，没有 declaration / `TypeId`。一旦它进入可执行合同，
/// 存储类、参数 ABI 与 GC 字段字节都只能靠名称回查或默认值猜实现；
/// 该形状必须在生成任何字节之前结构化失败，而不是由 backend 认亲。
pub fn validate_storage_identities(submission: &FragmentSubmission) -> Result<(), UnresolvedStorageIdentity> {
    let plan = &submission.backend_plan;
    let registered_layouts: std::collections::BTreeSet<LayoutId> = plan.aggregate_layouts().layouts.iter().map(|layout| layout.id).collect();
    for instance in plan.instances() {
        let Some(view) = plan.get_function(&instance)
        else {
            continue;
        };
        let function = &view.function;
        let mut sites = vec![("return type".to_string(), &function.return_type)];
        sites.extend(function.param_types.iter().enumerate().map(|(index, ty)| (format!("parameter {index}"), ty)));
        sites.extend(function.value_types.iter().map(|(value, ty)| (format!("SSA value {}", value.0), ty)));
        for block in &function.blocks {
            for (index, instruction) in block.instructions.iter().enumerate() {
                let location = format!("block {} instruction {index}", block.id.0);
                match &instruction.kind {
                    InstructionKind::LoadConstant { ty: Some(ty), .. } => sites.push((format!("{location} LoadConstant"), ty)),
                    InstructionKind::StoreVar { ty: Some(ty), .. } => sites.push((format!("{location} StoreVar"), ty)),
                    InstructionKind::ArrayNew { array_type, .. } => sites.push((format!("{location} ArrayNew"), array_type)),
                    InstructionKind::ArrayFromElements { array_type, .. } => sites.push((format!("{location} ArrayFromElements"), array_type)),
                    _ => {}
                }
            }
        }
        for (site, ty) in sites {
            if let Some(ty) = named_residual(ty, &registered_layouts) {
                return Err(UnresolvedStorageIdentity { function: function.symbol.clone(), site, ty });
            }
        }
    }
    for layout in plan.aggregate_layouts().layouts.iter() {
        for field in &layout.fields {
            if let Some(ty) = named_residual(&field.ty, &registered_layouts) {
                return Err(UnresolvedStorageIdentity {
                    function: String::new(),
                    site: format!("aggregate layout `{}::{}` field `{}`", layout.namespace, layout.name, field.name),
                    ty,
                });
            }
        }
    }
    for sum in plan.sum_layouts() {
        for variant in &sum.variants {
            if let Some(ty) = variant.payload_type.as_ref().and_then(|payload| named_residual(payload, &registered_layouts)) {
                return Err(UnresolvedStorageIdentity {
                    function: String::new(),
                    site: format!("unite layout `{}` variant `{}` payload", sum.name, variant.name),
                    ty,
                });
            }
        }
    }
    Ok(())
}

/// 递归查找无法按身份解析存储类的名义形状：
/// 裸 `Named` 短名残差，或 `Nominal` 携带的 `layout` 未在计划中注册。
fn named_residual(ty: &SemanticType, registered_layouts: &std::collections::BTreeSet<LayoutId>) -> Option<SemanticType> {
    match ty {
        SemanticType::Named(_) => Some(ty.clone()),
        SemanticType::Nominal { layout, .. } => {
            if registered_layouts.contains(layout) {
                None
            }
            else {
                Some(ty.clone())
            }
        }
        SemanticType::Apply(_base, arguments) => arguments.iter().find_map(|argument| named_residual(argument, registered_layouts)),
        SemanticType::Function(function) => named_residual(&function.return_type, registered_layouts)
            .or_else(|| function.params.iter().find_map(|parameter| named_residual(parameter, registered_layouts))),
        SemanticType::Tuple(elements) | SemanticType::Union(elements) => {
            elements.iter().find_map(|element| named_residual(element, registered_layouts))
        }
        SemanticType::Array(element) | SemanticType::FixedArray { element, .. } | SemanticType::Nullable(element) => {
            named_residual(element, registered_layouts)
        }
        SemanticType::TraitObject(object) => object.type_arguments.iter().find_map(|argument| named_residual(argument, registered_layouts)),
        _ => None,
    }
}

pub fn block_label(id: ExecutableBlockRef) -> String {
    format!("block_{}", id.0)
}

pub fn collect_reachable_blocks(function: &ExecutableFunction) -> Vec<ExecutableBlockRef> {
    let mut order = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut done = std::collections::BTreeSet::new();
    let mut stack = vec![function.entry];
    while let Some(block_id) = stack.pop() {
        if done.contains(&block_id) {
            continue;
        }
        if seen.contains(&block_id) {
            order.push(block_id);
            done.insert(block_id);
            continue;
        }
        seen.insert(block_id);
        stack.push(block_id);
        let Some(block) = function.blocks.get(block_id.0 as usize)
        else {
            continue;
        };
        match &block.terminator {
            ExecutableTerminator::Return { .. } => {}
            ExecutableTerminator::Jump { target, .. } => stack.push(*target),
            ExecutableTerminator::Branch { then_target, else_target, .. } => {
                stack.push(*else_target);
                stack.push(*then_target);
            }
            ExecutableTerminator::Raise { resume_target, .. } => stack.push(*resume_target),
            ExecutableTerminator::Unreachable => {}
        }
    }
    order.reverse();
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TargetExecutionPlan;
    use nyar_types::{AggregateLayout, FieldLayout, Identifier, StorageKind};

    /// 构造仅含返回类型与参数类型的最小可执行函数。
    fn identity_function(return_type: SemanticType, param_types: Vec<SemanticType>) -> ExecutableFunction {
        ExecutableFunction {
            symbol: "probe".to_string(),
            return_type,
            param_types,
            value_types: std::collections::BTreeMap::new(),
            value_layouts: std::collections::BTreeMap::new(),
            return_layout: None,
            entry: crate::contracts::BlockRef(0),
            values: Vec::new(),
            #[allow(deprecated)]
            blocks: vec![crate::contracts::Block {
                id: crate::contracts::BlockRef(0),
                label: "entry".to_string(),
                parameters: Vec::new(),
                instructions: Vec::new(),
                terminator: crate::contracts::Terminator::Return { value: None },
            }],
        }
    }

    fn submission(return_type: SemanticType, layouts: AggregateLayoutPlan) -> FragmentSubmission {
        let function = identity_function(return_type, Vec::new());
        let name = nyar::QualifiedName::new(vec![nyar::Identifier::new("probe")]);
        let mut plan = TargetExecutionPlan::from_functions(std::collections::BTreeMap::from([(name, function)]));
        plan.testing_aggregate_layouts(layouts);
        FragmentSubmission::from_backend_plan(plan)
    }

    fn layout_with_field(layout_id: LayoutId, field_ty: SemanticType) -> AggregateLayoutPlan {
        AggregateLayoutPlan {
            layouts: vec![AggregateLayout {
                id: layout_id,
                name: "Probe".to_string(),
                namespace: "app".to_string(),
                key: None,
                storage: StorageKind::Value,
                fields: vec![FieldLayout { name: "payload".to_string(), ty: field_ty }],
            }],
            ..AggregateLayoutPlan::default()
        }
    }

    #[test]
    fn named_residual_in_return_type_is_rejected() {
        let submission = submission(SemanticType::Named(Identifier::new("Config")), AggregateLayoutPlan::default());
        let error = validate_storage_identities(&submission).expect_err("裸 Named 缺声明身份必须被拒绝");
        assert_eq!(error.site, "return type");
        assert_eq!(error.ty, SemanticType::Named(Identifier::new("Config")));
    }

    #[test]
    fn named_residual_in_layout_field_is_rejected() {
        let layouts = layout_with_field(1, SemanticType::Named(Identifier::new("EngineSpec")));
        let submission = submission(nyar_types::SemanticType::Unit, layouts);
        let error = validate_storage_identities(&submission).expect_err("layout 字段裸 Named 必须被拒绝");
        assert!(error.site.contains("field `payload`"), "site must identify the layout field: {}", error.site);
    }

    #[test]
    fn nominal_with_unregistered_layout_is_rejected() {
        let nominal = SemanticType::Nominal { layout: 7, spelling: Identifier::new("app::Config") };
        let submission = submission(nominal.clone(), AggregateLayoutPlan::default());
        let error = validate_storage_identities(&submission).expect_err("未注册布局的 Nominal 必须被拒绝");
        assert_eq!(error.ty, nominal);
    }

    #[test]
    fn nominal_with_registered_layout_passes() {
        let layouts = layout_with_field(1, nyar_types::SemanticType::Integer32 { signed: true });
        let nominal = SemanticType::Nominal { layout: 1, spelling: Identifier::new("app::Probe") };
        let submission = submission(nominal, layouts);
        validate_storage_identities(&submission).expect("已注册布局的 Nominal 必须通过");
    }
}

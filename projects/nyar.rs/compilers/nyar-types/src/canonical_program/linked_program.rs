//! 链接完成的语义程序与稳定身份记录。
//!
//! 本模块拥有跨包链接后的成功类型：item instance、callable identity、
//! 名义实例、字段、variant、evidence、import、导出、入口、类型表与布局绑定。
//! 装配与 backend 只能消费这里冻结的身份，不得从旧 MIR 或名称重建。

use crate::{
    AggregateLayoutPlan, CapabilityTag, ExternalCallArgument, ExternalImportLink, FlagsLayout, Identifier, QualifiedName, SumTypeLayout,
    semantic_ids::{
        EvidenceId, FieldId, ImportCapability, ImportIndex, ItemId, ItemInstanceId, NominalInstanceId, TypeId, TypeInstanceId, VariantId,
    },
};
use std::collections::{BTreeMap, BTreeSet};

use super::CanonicalMirError;

/// 完成 adaptor 选择与跨包闭包后的链接程序。
///
/// 这是进入已校验 Semantic MIR 的**成功**类型——不是并行的
/// `FrontendNeutralPlan` / `FragmentSubmission` 权威。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkedSemanticProgram {
    /// Module / package identity key.
    pub module_name: String,
    /// Closed item instances (bodies + evidence reachable).
    pub item_instances: BTreeMap<ItemInstanceId, ItemInstanceRecord>,
    /// Compiler 已解析的 callable identity 到限定 ABI 名称映射；后端不得从 MIR 文本反查。
    pub callable_names: BTreeMap<ItemInstanceId, QualifiedName>,
    /// Closed nominal ADT instances.
    pub nominal_instances: BTreeMap<NominalInstanceId, NominalInstanceRecord>,
    /// 已解析字段身份及其 owner/类型合同。
    pub fields: BTreeMap<FieldId, FieldRecord>,
    /// 声明 variant 在具体名义实例中的 payload 合同；不同实例不得覆盖同一声明身份。
    pub variants: BTreeMap<(NominalInstanceId, VariantId), VariantRecord>,
    /// Selected evidence bindings.
    pub evidence: BTreeMap<EvidenceId, EvidenceRecord>,
    /// 已绑定的外部导入槽；执行层只消费 `ImportIndex`。
    pub imports: BTreeMap<ImportIndex, ImportRecord>,
    /// 已解析的公开导出合同，键为 Compiler 分配的 callable identity。
    pub exports: BTreeMap<ItemInstanceId, ExportRecord>,
    /// 已解析的程序入口合同。
    pub entries: BTreeMap<ItemInstanceId, EntryRecord>,
    /// Semantic type table.
    pub types: BTreeMap<TypeId, TypeRecord>,
    /// Frozen surface spelling for nominal declaration identities; used when lowering to executable types.
    pub type_surface_spellings: BTreeMap<TypeId, Identifier>,
    /// 可作为数组下标 / `ArrayLength` 结果的 TypeId（含 `Nominal{usize}`）；禁止把名义整数改写成 Primitive。
    pub array_index_types: BTreeSet<TypeId>,
    /// 文本载体名义声明到语言文本编码基元的显式映射。
    ///
    /// 字符串常量在 MIR 中只携带编码基元（`Utf8` / `Utf16`），而 `utf8` / `utf16`
    /// 是 std 中指向 `Utf8Text` / `Utf16Text` 的别名声明。前端把载体声明的 TypeId
    /// 登记在这里；canonical 只认这份身份，不按拼写猜语言类型。
    pub text_encoding_types: BTreeMap<TypeId, CanonicalPrimitiveType>,
    /// Compiler 解析出的聚合布局；装配与 backend 不得从旧 MIR 重建。
    pub aggregate_layouts: AggregateLayoutPlan,
    /// 已由 Compiler 绑定的名义身份到物理布局身份映射。
    pub aggregate_layout_by_nominal: BTreeMap<NominalInstanceId, crate::layout::LayoutId>,
    /// 已由 Compiler 绑定的字段身份到布局字段槽位映射。
    pub aggregate_layout_by_field: BTreeMap<FieldId, (crate::layout::LayoutId, u32)>,
    /// 已由 Compiler 绑定的完整类型身份到物理布局身份映射。
    pub aggregate_layout_by_type: BTreeMap<TypeId, crate::layout::LayoutId>,
    /// Compiler 解析出的 sum 布局；装配与 backend 不得从名称猜测。
    pub sum_types: Vec<SumTypeLayout>,
    /// Compiler 解析出的 flags 布局。
    pub flags_types: Vec<FlagsLayout>,
    /// Compiler 解析出的 singleton 生命周期合同。
    pub singleton_instances: Vec<crate::SingletonInstancePlan>,
    /// 已完成稳定身份绑定的语义片段；装配只能消费该表。
    pub fragments: BTreeMap<Identifier, CanonicalFragment>,
    /// Compiler 已确定的模块级引用管理合同。
    pub reference_management: Option<crate::ReferenceManagement>,
    /// Compiler 已确定的模块级运行时合同需求。
    pub runtime_requirements: Vec<crate::RuntimeRequirement>,
}

/// Canonical 程序中的稳定语义片段合同。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CanonicalFragment {
    /// 片段身份。
    pub id: Identifier,
    /// 片段公开及闭包内操作的 callable identity。
    pub exported_operations: Vec<ItemInstanceId>,
    /// 片段要求的能力。
    pub required_capabilities: Vec<CapabilityTag>,
    /// 片段入口 callable identity。
    pub entry_operation: Option<ItemInstanceId>,
    /// 片段使用的外部导入合同，键为外部 callable identity。
    pub external_imports: BTreeMap<ItemInstanceId, ExternalImportLink>,
    /// 已绑定身份的外部调用边。
    pub external_call_edges: Vec<CanonicalExternalCallEdge>,
    /// 已绑定身份的内部调用边。
    pub internal_call_edges: Vec<CanonicalCallEdge>,
    /// callable identity 到公开 Wasm 名称的映射。
    pub wasm_export_names: BTreeMap<ItemInstanceId, String>,
}

/// Canonical 片段内的内部调用边。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalCallEdge {
    /// 调用方 callable identity。
    pub caller: ItemInstanceId,
    /// 被调用方 callable identity。
    pub callee: ItemInstanceId,
}

/// Canonical 片段内的外部调用边。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CanonicalExternalCallEdge {
    /// 调用方 callable identity。
    pub caller: ItemInstanceId,
    /// 外部导入槽 identity。
    pub import: ImportIndex,
    /// 编译器已绑定的最小调用参数合同。
    pub arguments: Vec<ExternalCallArgument>,
}

/// 一个已链接 callable 的公开导出合同。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportRecord {
    /// 公开 ABI 名称，仅用于互操作边界。
    pub exported_name: String,
}

/// 一个已链接 callable 的程序入口合同。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntryRecord;

/// Placeholder item instance row (filled by linker / adaptor selection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemInstanceRecord {
    /// 已声明的 callable identity。
    pub declaration: ItemId,
    /// 完成代入后的参数类型身份，顺序与调用 ABI 一致。
    pub parameter_types: Vec<TypeId>,
    /// 完成代入后的返回类型身份。
    pub return_type: TypeId,
}

/// 名义类型声明的值语义；由语言前端决定，不是目标物理布局。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NominalValueSemantics {
    /// 复制聚合值，不共享对象身份。
    Value,
    /// 复制引用，保留对象身份。
    Reference,
}

/// 完成实例化的名义类型合同。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NominalInstanceRecord {
    /// 名义类型声明身份。
    pub declaration: TypeId,
    /// 完成泛型代入的完整实例类型；不同实参不得共用字段或 variant 合同。
    pub ty: TypeId,
    /// 声明决定的值/引用语义，不能由字段数量或目标布局推断。
    pub semantics: NominalValueSemantics,
    /// 该实例声明的全部字段，顺序为语言声明顺序。
    pub fields: Vec<FieldId>,
}

/// 已解析字段的 owner 与值类型合同。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldRecord {
    /// 字段所属名义实例。
    pub owner: NominalInstanceId,
    /// 字段值类型。
    pub ty: TypeId,
}

/// 已解析 sum variant 的 owner 与 payload 合同。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantRecord {
    /// 单 payload 类型；无 payload 时为空。
    pub payload_type: Option<TypeId>,
}

/// Placeholder evidence row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceRecord {
    /// trait 或 adaptor 声明身份。
    pub trait_id: ItemId,
    /// 实现方的具体类型实例身份。
    pub implementing_type: TypeInstanceId,
}

/// 已完成签名绑定的外部导入记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRecord {
    /// Compiler 已选择并验证的外部链接合同。
    pub link: crate::ExternalImportLink,
    /// 互操作链接能力，仅用于链接和诊断，不作为执行分派键。
    pub capability: ImportCapability,
    /// 对应的外部 callable 实例。
    pub callee: ItemInstanceId,
    /// 已代入的参数类型。
    pub parameter_types: Vec<TypeId>,
    /// 已代入的返回类型。
    pub return_type: TypeId,
}

/// Canonical 类型的结构事实。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalTypeKind {
    /// 语言原生标量或单元类型。
    Primitive(CanonicalPrimitiveType),
    /// 名义类型及其已代入参数。
    Nominal {
        /// 名义声明类型身份。
        declaration: TypeId,
        /// 已代入的类型实参列表。
        arguments: Vec<TypeId>,
    },
    /// 元组类型。
    Tuple(Vec<TypeId>),
    /// 带标签元组：字段标签是类型身份的一部分，字段顺序即位置身份。
    ///
    /// 与 [`CanonicalTypeKind::Tuple`] 是两个不同类型，二者之间没有隐式兼容；
    /// `NyarType` / `LayoutKey` 保持无标签，目标布局按位置擦除标签（`Ord` 不受影响）。
    LabeledTuple {
        /// 按源码顺序的 `(标签, 字段类型)`。
        fields: Vec<(Identifier, TypeId)>,
    },
    /// 数组类型。
    Array {
        /// 元素类型。
        element: TypeId,
        /// 固定长度；`None` 表示运行时可增长数组。
        length: Option<u64>,
    },
    /// 语言的显式可空类型；名义 Option 使用 Nominal 记录。
    Nullable(TypeId),
    /// 匿名联合类型。
    Union(Vec<TypeId>),
    /// 匿名交集类型。
    Intersection(Vec<TypeId>),
    /// 语言函数值类型。
    Function {
        /// 参数类型列表。
        parameters: Vec<TypeId>,
        /// 返回类型。
        return_type: TypeId,
    },
    /// 泛型形参：`binder` 声明身份 + 声明内序号。
    ///
    /// 形参是前端冻结的参数化 item 事实，不是按名字查表的占位符。canonical 只保留
    /// 该稳定身份；形参如何被代入、字典/vtable 如何传递由 target 合同决定。
    Generic {
        /// 形参所属声明身份（函数、名义类型、trait、imply 等）。
        owner: ItemId,
        /// 形参在所属声明内的源码顺序。
        ordinal: u32,
    },
}

/// Canonical 原生类型种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalPrimitiveType {
    /// 无返回值。
    Void,
    /// 布尔。
    Bool,
    /// 带宽度和符号的整数。
    Integer {
        /// 位宽。
        bits: u16,
        /// 是否为有符号整数。
        signed: bool,
    },
    /// 带宽度的浮点。
    Float {
        /// 位宽。
        bits: u16,
    },
    /// Unicode 字符。
    Character,
    /// 文本。
    Utf8,
    /// UTF-16 文本。
    Utf16,
    /// 单元。
    Unit,
}

/// Canonical 类型表中的结构化记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeRecord {
    /// 类型表中的声明身份。
    pub declaration: TypeId,
    /// 类型的完整语义形状。
    pub kind: CanonicalTypeKind,
}

/// 判断一个具体类型是否属于给定的名义实例。
///
/// `NominalInstanceRecord::ty` 保存声明形状，而调用点可能携带已经代入的
/// 泛型参数。两者的参数列表不应被当成不同的 owner；名义 declaration
/// identity 才是 ADT/字段/variant 所属关系的事实来源。
pub(crate) fn canonical_nominal_owner_matches(linked: &LinkedSemanticProgram, actual: TypeId, owner: TypeId) -> bool {
    if actual == owner {
        return true;
    }
    let Some(actual_kind) = linked.types.get(&actual).map(|record| &record.kind)
    else {
        return false;
    };
    let Some(owner_kind) = linked.types.get(&owner).map(|record| &record.kind)
    else {
        return false;
    };
    match (actual_kind, owner_kind) {
        (
            CanonicalTypeKind::Nominal { declaration: actual_declaration, arguments: actual_arguments },
            CanonicalTypeKind::Nominal { declaration: owner_declaration, arguments: owner_arguments },
        ) => {
            actual_declaration == owner_declaration
                && (actual_arguments == owner_arguments || actual_arguments.is_empty() || owner_arguments.is_empty())
        }
        _ => false,
    }
}

pub(crate) fn canonical_types_compatible(linked: &LinkedSemanticProgram, actual: TypeId, expected: TypeId) -> bool {
    if actual == expected || canonical_text_encoding_compatible(linked, actual, expected) {
        return true;
    }
    let (Some(actual_kind), Some(expected_kind)) =
        (linked.types.get(&actual).map(|record| &record.kind), linked.types.get(&expected).map(|record| &record.kind))
    else {
        return false;
    };
    match (actual_kind, expected_kind) {
        (
            CanonicalTypeKind::Nominal { declaration: actual_declaration, arguments: actual_arguments },
            CanonicalTypeKind::Nominal { declaration: expected_declaration, arguments: expected_arguments },
        ) => {
            actual_declaration == expected_declaration
                && actual_arguments.len() == expected_arguments.len()
                && actual_arguments
                    .iter()
                    .zip(expected_arguments)
                    .all(|(actual, expected)| canonical_types_compatible(linked, *actual, *expected))
        }
        (
            CanonicalTypeKind::Array { element: actual, length: actual_length },
            CanonicalTypeKind::Array { element: expected, length: expected_length },
        ) => actual_length == expected_length && canonical_types_compatible(linked, *actual, *expected),
        (CanonicalTypeKind::Nullable(actual), CanonicalTypeKind::Nullable(expected)) => canonical_types_compatible(linked, *actual, *expected),
        // `T` 可隐式提升为 `T?`：非空值就是同一引用的 Some 侧，不是名义 Option 变体。
        (_, CanonicalTypeKind::Nullable(inner)) => canonical_types_compatible(linked, actual, *inner),
        (CanonicalTypeKind::Tuple(actual), CanonicalTypeKind::Tuple(expected)) if actual.len() == expected.len() => {
            actual.iter().zip(expected).all(|(actual, expected)| canonical_types_compatible(linked, *actual, *expected))
        }
        // 标签是类型身份的一部分：同名同型同长度才兼容；带标签 ↔ 无标签没有隐式兼容，
        // 跨形态必须由语言层的显式重构造（或未来单独设计的 `as` 结构转换）完成。
        (CanonicalTypeKind::LabeledTuple { fields: actual }, CanonicalTypeKind::LabeledTuple { fields: expected })
            if actual.len() == expected.len() =>
        {
            actual.iter().zip(expected).all(|((actual_name, actual_ty), (expected_name, expected_ty))| {
                actual_name == expected_name && canonical_types_compatible(linked, *actual_ty, *expected_ty)
            })
        }
        // 形参身份由 `binder`(owner, ordinal) 决定：同一 binder 才是同一形参。
        (
            CanonicalTypeKind::Generic { owner: actual_owner, ordinal: actual_ordinal },
            CanonicalTypeKind::Generic { owner: expected_owner, ordinal: expected_ordinal },
        ) => actual_owner == expected_owner && actual_ordinal == expected_ordinal,
        _ => false,
    }
}

/// 文本编码基元与文本载体名义类型之间是否等价。
///
/// 只有前端显式登记进 [`LinkedSemanticProgram::text_encoding_types`] 的名义 TypeId
/// 才参与判定；两种形态必须映射到同一编码（`Utf8` 或 `Utf16`），不同编码之间
/// 没有隐式兼容。
pub(crate) fn canonical_text_encoding_compatible(linked: &LinkedSemanticProgram, left: TypeId, right: TypeId) -> bool {
    fn encoding(linked: &LinkedSemanticProgram, ty: TypeId) -> Option<CanonicalPrimitiveType> {
        match linked.types.get(&ty).map(|record| &record.kind) {
            Some(CanonicalTypeKind::Primitive(primitive @ (CanonicalPrimitiveType::Utf8 | CanonicalPrimitiveType::Utf16))) => {
                Some(*primitive)
            }
            _ => linked.text_encoding_types.get(&ty).copied(),
        }
    }
    match (encoding(linked, left), encoding(linked, right)) {
        (Some(left), Some(right)) => left == right,
        _ => false,
    }
}

impl LinkedSemanticProgram {
    /// 数组下标 / 长度结果是否为已登记整数形状（原生 Integer 或 `array_index_types` 中的名义整数）。
    pub fn is_array_index_type(&self, ty: TypeId) -> bool {
        if self.array_index_types.contains(&ty) {
            return true;
        }
        matches!(self.types.get(&ty).map(|row| &row.kind), Some(CanonicalTypeKind::Primitive(CanonicalPrimitiveType::Integer { .. })))
    }

    /// 拒绝「已有冻结布局或载体身份，却仍以 `Named` 短名残留」的类型。
    ///
    /// `Named` 只允许表示尚无身份的名义拼写。同一声明一旦已有
    /// `aggregate_layout_by_type`、文本载体登记或原生标量身份，残留就会让下游
    /// 只能按名字猜实现。命中即结构化失败，并点名阶段、产生函数与 owner，
    /// 不降级为告警。
    pub fn validate_named_residues(&self) -> Result<(), CanonicalMirError> {
        for layout in &self.aggregate_layouts.layouts {
            for field in &layout.fields {
                if let Some(name) = self.shadowed_named_residue(&field.ty) {
                    return Err(CanonicalMirError::ShadowedNamedResidue {
                        stage: "canonical::aggregate_layouts".into(),
                        function: "value_semantics::layout_for_struct / value_semantics::instance_layout_for_type".into(),
                        owner: format!("{}::{}.{}", layout.namespace, layout.name, field.name),
                        name: name.as_str().into(),
                    });
                }
            }
        }
        for sum in &self.sum_types {
            for variant in &sum.variants {
                if let Some(name) = variant.payload_type.as_ref().and_then(|payload| self.shadowed_named_residue(payload)) {
                    return Err(CanonicalMirError::ShadowedNamedResidue {
                        stage: "canonical::sum_layouts".into(),
                        function: "canonical::identities::sum_payload_layout / canonical::identities::record_payload_layout".into(),
                        owner: format!("{}.{}", sum.name, variant.name),
                        name: name.as_str().into(),
                    });
                }
            }
        }
        Ok(())
    }

    /// 递归返回第一个与已登记身份同名的 `Named` 残留。
    fn shadowed_named_residue<'a>(&self, ty: &'a crate::NyarType) -> Option<&'a Identifier> {
        use crate::NyarType;
        match ty {
            NyarType::Named(name) if self.shadows_registered_identity(name) => Some(name),
            NyarType::Named(_) => None,
            // `Apply` 的基是泛型构造器拼写（声明名），不是值位类型；身份由参数与整体
            // 应用承载。只检查实参，避免把合法的 `Apply(Named("Option"), …)` 误判为残留。
            NyarType::Apply(_, arguments) => arguments.iter().find_map(|argument| self.shadowed_named_residue(argument)),
            NyarType::Function(function) => function
                .params
                .iter()
                .find_map(|parameter| self.shadowed_named_residue(parameter))
                .or_else(|| self.shadowed_named_residue(&function.return_type)),
            NyarType::Tuple(elements) | NyarType::Union(elements) => elements.iter().find_map(|element| self.shadowed_named_residue(element)),
            NyarType::Array(element) | NyarType::Nullable(element) => self.shadowed_named_residue(element),
            NyarType::FixedArray { element, .. } => self.shadowed_named_residue(element),
            NyarType::TraitObject(object) => object.type_arguments.iter().find_map(|argument| self.shadowed_named_residue(argument)),
            NyarType::Bottom
            | NyarType::Unit
            | NyarType::Boolean
            | NyarType::Integer8 { .. }
            | NyarType::Integer16 { .. }
            | NyarType::Integer32 { .. }
            | NyarType::Integer64 { .. }
            | NyarType::Integer128 { .. }
            | NyarType::Float32
            | NyarType::Float64
            | NyarType::Character
            | NyarType::Utf8
            | NyarType::Utf16
            | NyarType::Nominal { .. } => None,
        }
    }

    /// 该拼写是否属于已登记身份（布局 / 文本载体 / 原生标量）。
    ///
    /// 只比对**声明级**身份：泛型实例的 `TypeId` 同样带拼写与布局，但裸 `Named`
    /// 指向的是声明拼写（如 `Apply(Named("Option"), …)` 的基），把它按实例匹配会
    /// 把合法的泛型基误判为残留。
    fn shadows_registered_identity(&self, name: &Identifier) -> bool {
        let spelling = name.as_str();
        let short = spelling.rsplit("::").next().unwrap_or(spelling);
        self.types.iter().any(|(id, record)| {
            let registered = self.aggregate_layout_by_type.contains_key(id)
                || self.text_encoding_types.contains_key(id)
                || matches!(record.kind, CanonicalTypeKind::Primitive(_));
            let declaration_identity = match &record.kind {
                CanonicalTypeKind::Nominal { arguments, .. } => arguments.is_empty(),
                CanonicalTypeKind::Primitive(_) => true,
                _ => false,
            };
            registered
                && declaration_identity
                && self.type_surface_spellings.get(id).is_some_and(|registered_spelling| {
                    let registered_spelling = registered_spelling.as_str();
                    registered_spelling == spelling || registered_spelling.rsplit("::").next() == Some(short)
                })
        })
    }

    /// 验证整个类型闭包，不限于当前函数引用到的行。
    pub fn validate_types(&self) -> Result<(), CanonicalMirError> {
        for (owner, record) in &self.types {
            let references = match &record.kind {
                CanonicalTypeKind::Primitive(primitive) => {
                    let valid = match primitive {
                        CanonicalPrimitiveType::Integer { bits, .. } => {
                            matches!(bits, 8 | 16 | 32 | 64 | 128)
                        }
                        CanonicalPrimitiveType::Float { bits } => matches!(bits, 32 | 64),
                        _ => true,
                    };
                    if !valid {
                        return Err(CanonicalMirError::InvalidPrimitiveType { ty: *owner });
                    }
                    Vec::new()
                }
                CanonicalTypeKind::Nominal { declaration, arguments } => {
                    std::iter::once(*declaration).chain(arguments.iter().copied()).collect()
                }
                CanonicalTypeKind::Tuple(members) | CanonicalTypeKind::Union(members) | CanonicalTypeKind::Intersection(members) => {
                    members.clone()
                }
                CanonicalTypeKind::LabeledTuple { fields } => fields.iter().map(|(_, ty)| *ty).collect(),
                CanonicalTypeKind::Array { element, .. } | CanonicalTypeKind::Nullable(element) => {
                    vec![*element]
                }
                CanonicalTypeKind::Function { parameters, return_type } => {
                    parameters.iter().copied().chain(std::iter::once(*return_type)).collect()
                }
                // 形参不引用其他类型身份：binder(owner, ordinal) 本身就是稳定身份。
                CanonicalTypeKind::Generic { .. } => Vec::new(),
            };
            for referenced in references {
                if !self.types.contains_key(&referenced) {
                    return Err(CanonicalMirError::UnknownTypeReference { owner: *owner, referenced });
                }
            }
        }
        let mut nominal_semantics = BTreeMap::new();
        let mut nominal_types = std::collections::BTreeSet::new();
        let mut declared_fields = std::collections::BTreeSet::new();
        for (nominal, record) in &self.nominal_instances {
            if !matches!(self.types.get(&record.declaration).map(|ty| &ty.kind), Some(CanonicalTypeKind::Nominal { declaration, .. }) if *declaration == record.declaration)
            {
                return Err(CanonicalMirError::InvalidNominalDeclaration { nominal: *nominal, declaration: record.declaration });
            }
            if !matches!(self.types.get(&record.ty).map(|ty| &ty.kind), Some(CanonicalTypeKind::Nominal { declaration, .. }) if *declaration == record.declaration)
            {
                return Err(CanonicalMirError::InvalidNominalInstanceType { nominal: *nominal, ty: record.ty });
            }
            if !nominal_types.insert(record.ty) {
                return Err(CanonicalMirError::DuplicateNominalInstanceType { ty: record.ty });
            }
            if let Some(previous) = nominal_semantics.insert(record.declaration, record.semantics) {
                if previous != record.semantics {
                    return Err(CanonicalMirError::ConflictingNominalSemantics { declaration: record.declaration });
                }
            }
            for field in &record.fields {
                if !declared_fields.insert(*field)
                    || !self.fields.get(field).is_some_and(|row| row.owner == *nominal && self.types.contains_key(&row.ty))
                {
                    return Err(CanonicalMirError::InvalidNominalField { nominal: *nominal, field: *field });
                }
            }
        }
        for (field, record) in &self.fields {
            if !declared_fields.contains(field) {
                return Err(CanonicalMirError::InvalidNominalField { nominal: record.owner, field: *field });
            }
        }
        for record in self.types.values() {
            if let CanonicalTypeKind::Nominal { declaration, .. } = &record.kind {
                if !nominal_semantics.contains_key(declaration) {
                    return Err(CanonicalMirError::MissingNominalSemantics { declaration: *declaration });
                }
            }
        }
        for (ty, record) in &self.types {
            let references = match &record.kind {
                CanonicalTypeKind::Nominal { declaration, arguments } => {
                    if ty != declaration || !arguments.is_empty() {
                        self.validate_nominal_value_type(*ty, &nominal_types)?;
                    }
                    arguments.clone()
                }
                CanonicalTypeKind::Tuple(members) | CanonicalTypeKind::Union(members) | CanonicalTypeKind::Intersection(members) => {
                    members.clone()
                }
                CanonicalTypeKind::LabeledTuple { fields } => fields.iter().map(|(_, ty)| *ty).collect(),
                CanonicalTypeKind::Array { element, .. } | CanonicalTypeKind::Nullable(element) => {
                    vec![*element]
                }
                CanonicalTypeKind::Function { parameters, return_type } => {
                    parameters.iter().copied().chain(std::iter::once(*return_type)).collect()
                }
                CanonicalTypeKind::Primitive(_) => Vec::new(),
                // 形参身份即 binder(owner, ordinal)，不引用其他类型身份。
                CanonicalTypeKind::Generic { .. } => Vec::new(),
            };
            for referenced in references {
                self.validate_nominal_value_type(referenced, &nominal_types)?;
            }
        }
        for record in self.item_instances.values() {
            for ty in record.parameter_types.iter().chain(std::iter::once(&record.return_type)) {
                self.validate_nominal_value_type(*ty, &nominal_types)?;
            }
        }
        for field in self.fields.values() {
            self.validate_nominal_value_type(field.ty, &nominal_types)?;
        }
        for ((nominal, variant_id), variant) in &self.variants {
            if !self.nominal_instances.contains_key(nominal) {
                return Err(CanonicalMirError::InvalidVariantOwner { nominal: *nominal, variant: *variant_id });
            }
            if let Some(ty) = variant.payload_type {
                self.validate_nominal_value_type(ty, &nominal_types)?;
            }
        }
        for import in self.imports.values() {
            let Some(callee) = self.item_instances.get(&import.callee)
            else {
                return Err(CanonicalMirError::UnknownFunction { function: import.callee });
            };
            if callee.parameter_types != import.parameter_types || callee.return_type != import.return_type {
                return Err(CanonicalMirError::ImportSignatureMismatch { function: import.callee });
            }
            for ty in import.parameter_types.iter().chain(std::iter::once(&import.return_type)) {
                if !self.types.contains_key(ty) {
                    return Err(CanonicalMirError::UnknownType { function: import.callee, ty: *ty });
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate_nominal_value_type(
        &self,
        ty: TypeId,
        nominal_types: &std::collections::BTreeSet<TypeId>,
    ) -> Result<(), CanonicalMirError> {
        if matches!(self.types.get(&ty).map(|record| &record.kind), Some(CanonicalTypeKind::Nominal { .. })) && !nominal_types.contains(&ty) {
            return Err(CanonicalMirError::MissingNominalInstance { ty });
        }
        Ok(())
    }
}

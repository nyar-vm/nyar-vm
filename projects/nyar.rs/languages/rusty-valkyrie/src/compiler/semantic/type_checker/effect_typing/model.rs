use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use crate::compiler::types::{
    Identifier, NamePath, SourceSpan,
    hir::{AssociatedType, FunctionType, HirEnum, ValkyrieType as HirType},
};

use crate::compiler::semantic::type_checker::display_path;

/// 约束求解中的类型变量标识，显示为 `?Tn`。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeVar(pub usize);

impl fmt::Display for TypeVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?T{}", self.0)
    }
}

/// 生命周期标注名（如 `'a`、`'static`）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Lifetime(String);

impl Lifetime {
    /// 由名称构造生命周期。
    pub fn new(name: &str) -> Self {
        Self(name.to_string())
    }

    /// 返回 `'static` 生命周期。
    pub fn static_lifetime() -> Self {
        Self("static".to_string())
    }
}

impl fmt::Display for Lifetime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "'{}", self.0)
    }
}

/// 约束求解器接受的单一约束形式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeConstraint {
    /// 要求类型变量满足指定 trait 约束。
    TraitBound {
        /// 被约束的类型变量。
        var: TypeVar,
        /// 要求满足的 trait 路径。
        trait_name: NamePath,
        /// 约束出现的位置。
        span: Option<SourceSpan>,
    },
    /// 要求左右两侧类型相等。
    Equality {
        /// 等式左侧类型。
        left: HirType,
        /// 等式右侧类型。
        right: HirType,
        /// 约束出现的位置。
        span: Option<SourceSpan>,
    },
    /// 要求 `sub` 是 `sup` 的子类型。
    Subtype {
        /// 子类型一侧。
        sub: HirType,
        /// 父类型一侧。
        sup: HirType,
        /// 约束出现的位置。
        span: Option<SourceSpan>,
    },
}

impl TypeConstraint {
    /// 构造 trait 约束。
    pub fn trait_bound(var: TypeVar, trait_name: NamePath, span: Option<SourceSpan>) -> Self {
        Self::TraitBound { var, trait_name, span }
    }

    /// 构造类型相等约束。
    pub fn equality(left: HirType, right: HirType, span: Option<SourceSpan>) -> Self {
        Self::Equality { left, right, span }
    }

    /// 构造子类型约束。
    pub fn subtype(sub: HirType, sup: HirType, span: Option<SourceSpan>) -> Self {
        Self::Subtype { sub, sup, span }
    }
}

/// 一条已登记的 trait 实现记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraitImpl {
    /// 实现所在的 target 类型。
    pub ty: HirType,
    /// 被实现的 trait 路径。
    pub trait_name: NamePath,
    /// 泛型 trait 的类型实参。
    pub type_args: Vec<HirType>,
}

/// 同一类型变量上的多个 trait 约束集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiTraitBound {
    /// 被约束的类型变量。
    pub var: TypeVar,
    /// 要求同时满足的全部 trait 路径。
    pub traits: Vec<NamePath>,
    /// 约束出现的位置。
    pub span: Option<SourceSpan>,
}

impl MultiTraitBound {
    /// 构造多 trait 约束。
    pub fn new(var: TypeVar, traits: Vec<NamePath>, span: Option<SourceSpan>) -> Self {
        Self { var, traits, span }
    }

    /// 判断是否不含任何 trait 约束。
    pub fn is_empty(&self) -> bool {
        self.traits.is_empty()
    }
}

/// 一条 `where` 约束：某类型必须满足的下界 trait 集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhereBound {
    /// 被约束的目标类型。
    pub ty: HirType,
    /// 该类型必须满足的 trait 路径集合。
    pub traits: Vec<NamePath>,
    /// 约束出现的位置。
    pub span: Option<SourceSpan>,
}

/// 一组 `where` 约束及其源码位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhereClause {
    /// 该 where 子句中的全部约束。
    pub bounds: Vec<WhereBound>,
    /// where 子句出现的位置。
    pub span: Option<SourceSpan>,
}

impl WhereClause {
    /// 由约束列表与位置构造 where 子句。
    pub fn new(bounds: Vec<WhereBound>, span: Option<SourceSpan>) -> Self {
        Self { bounds, span }
    }

    /// 构造不含任何约束的 where 子句。
    pub fn empty() -> Self {
        Self { bounds: vec![], span: None }
    }

    /// 追加一条约束。
    pub fn add_bound(&mut self, bound: WhereBound) {
        self.bounds.push(bound);
    }

    /// 判断是否不含任何约束。
    pub fn is_empty(&self) -> bool {
        self.bounds.is_empty()
    }
}

/// 生命周期约束的两种形式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifetimeConstraintKind {
    /// `longer` 必须活得比 `shorter` 更久。
    Outlives {
        /// 更长的一侧生命周期。
        longer: Lifetime,
        /// 更短的一侧生命周期。
        shorter: Lifetime,
    },
    /// 两个生命周期必须相等。
    Equality {
        /// 等式左侧生命周期。
        left: Lifetime,
        /// 等式右侧生命周期。
        right: Lifetime,
    },
}

/// 单条生命周期约束：形式与源码位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifetimeConstraint {
    /// 约束形式。
    pub kind: LifetimeConstraintKind,
    /// 约束出现的位置。
    pub span: Option<SourceSpan>,
}

impl LifetimeConstraint {
    /// 构造 `longer: shorter` 形式的生命周期约束。
    pub fn outlives(longer: Lifetime, shorter: Lifetime, span: Option<SourceSpan>) -> Self {
        Self { kind: LifetimeConstraintKind::Outlives { longer, shorter }, span }
    }

    /// 构造生命周期相等约束。
    pub fn equality(left: Lifetime, right: Lifetime, span: Option<SourceSpan>) -> Self {
        Self { kind: LifetimeConstraintKind::Equality { left, right }, span }
    }
}

/// 一条关联类型等式约束：`base_type` 实现 `trait_name` 时 `assoc_name` 等于 `expected`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociatedTypeConstraint {
    /// 关联类型所属的基类型。
    pub base_type: HirType,
    /// 提供关联类型的 trait 路径。
    pub trait_name: NamePath,
    /// 关联类型名称。
    pub assoc_name: Identifier,
    /// 期望关联类型相等的具体类型。
    pub expected: HirType,
    /// 约束出现的位置。
    pub span: Option<SourceSpan>,
}

impl AssociatedTypeConstraint {
    /// 构造关联类型约束。
    pub fn new(base_type: HirType, trait_name: NamePath, assoc_name: Identifier, expected: HirType, span: Option<SourceSpan>) -> Self {
        Self { base_type, trait_name, assoc_name, expected, span }
    }
}

/// 约束求解失败的类别。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstraintErrorKind {
    /// 类型未实现要求的 trait。
    TraitNotImplemented {
        /// 未实现 trait 的类型渲染名。
        type_name: String,
        /// 未被满足的 trait 渲染名。
        trait_name: String,
    },
    /// 期望类型与实际类型不一致。
    TypeMismatch {
        /// 期望的类型。
        expected: HirType,
        /// 实际得到的类型。
        found: HirType,
    },
    /// 出现无限类型（类型变量出现在自身约束中）。
    InfiniteType {
        /// 参与自引用的类型变量。
        var: TypeVar,
        /// 递归包含该变量的类型。
        ty: HirType,
    },
    /// 类型变量的取值无法唯一确定。
    AmbiguousType {
        /// 无法确定的类型变量。
        var: TypeVar,
    },
    /// 未找到要求的关联类型绑定。
    AssociatedTypeNotFound {
        /// 基类型渲染名。
        type_name: String,
        /// trait 路径渲染名。
        trait_name: String,
        /// 缺失的关联类型名。
        assoc_name: String,
    },
    /// 约束传播失败。
    PropagationFailed {
        /// 失败原因。
        reason: String,
    },
}

/// 单条约束错误：类别、渲染后的消息与源码位置。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintError {
    /// 错误类别。
    pub kind: ConstraintErrorKind,
    /// 人类可读的错误信息。
    pub message: String,
    /// 出错位置；不可定位时为 None。
    pub span: Option<SourceSpan>,
}

impl ConstraintError {
    /// 构造「类型未实现 trait」错误。
    pub fn trait_not_implemented(type_name: &str, trait_name: &str, span: Option<SourceSpan>) -> Self {
        Self {
            kind: ConstraintErrorKind::TraitNotImplemented { type_name: type_name.to_string(), trait_name: trait_name.to_string() },
            message: format!("类型 {} 未实现 trait {}", type_name, trait_name),
            span,
        }
    }

    /// 构造「类型不匹配」错误。
    pub fn type_mismatch(expected: HirType, found: HirType, span: Option<SourceSpan>) -> Self {
        Self {
            kind: ConstraintErrorKind::TypeMismatch { expected: expected.clone(), found: found.clone() },
            message: format!("类型不匹配: expected {:?}, found {:?}", expected, found),
            span,
        }
    }

    /// 构造「无限类型」错误。
    pub fn infinite_type(var: TypeVar, ty: HirType, span: Option<SourceSpan>) -> Self {
        Self {
            kind: ConstraintErrorKind::InfiniteType { var: var.clone(), ty: ty.clone() },
            message: format!("无限类型: {} occurs in {:?}", var, ty),
            span,
        }
    }

    /// 构造「类型歧义」错误。
    pub fn ambiguous_type(var: TypeVar, span: Option<SourceSpan>) -> Self {
        Self { kind: ConstraintErrorKind::AmbiguousType { var: var.clone() }, message: format!("类型歧义: {}", var), span }
    }

    /// 构造「关联类型未找到」错误。
    pub fn associated_type_not_found(base_type: HirType, trait_name: NamePath, assoc_name: Identifier, span: Option<SourceSpan>) -> Self {
        Self {
            kind: ConstraintErrorKind::AssociatedTypeNotFound {
                type_name: format!("{:?}", base_type),
                trait_name: display_path(&trait_name),
                assoc_name: assoc_name.to_string(),
            },
            message: "关联类型未找到".to_string(),
            span,
        }
    }

    /// 构造「约束传播失败」错误。
    pub fn propagation_failed(reason: &str, span: Option<SourceSpan>) -> Self {
        Self {
            kind: ConstraintErrorKind::PropagationFailed { reason: reason.to_string() }, message: format!("约束传播失败: {}", reason), span
        }
    }
}

impl fmt::Display for ConstraintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ConstraintError {}

/// 面向用户的修复建议：文案、优先级与可选示例代码。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixSuggestion {
    /// 建议内容。
    pub message: String,
    /// 建议优先级（数值越小越优先）。
    pub priority: i32,
    /// 可选的示例代码。
    pub code_example: Option<String>,
}

impl FixSuggestion {
    /// 构造指定优先级、无示例代码的建议。
    pub fn new(message: impl Into<String>, priority: i32) -> Self {
        Self { message: message.into(), priority, code_example: None }
    }

    /// 附加示例代码并返回自身。
    pub fn with_code_example(mut self, code_example: impl Into<String>) -> Self {
        self.code_example = Some(code_example.into());
        self
    }
}

/// 约束推导链上的一个节点，用于追溯约束来源。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintChainNode {
    /// 该节点的说明文本。
    pub description: String,
    /// 该节点对应的源码位置。
    pub span: Option<SourceSpan>,
    /// 该节点的来源描述。
    pub origin: Option<String>,
}

impl ConstraintChainNode {
    /// 构造约束链节点。
    pub fn new(description: String, span: Option<SourceSpan>, origin: Option<String>) -> Self {
        Self { description, span, origin }
    }
}

/// 约束错误的完整报告：主信息、建议、相关类型与推导链。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintReport {
    /// 错误类别。
    pub kind: ConstraintErrorKind,
    /// 出错位置。
    pub span: Option<SourceSpan>,
    /// 面向用户的主错误信息。
    pub primary_message: String,
    /// 给出的修复建议列表。
    pub suggestions: Vec<FixSuggestion>,
    /// 报告涉及的相关类型渲染名。
    pub related_types: Vec<String>,
    /// 约束推导链节点列表。
    pub chain: Vec<ConstraintChainNode>,
}

impl ConstraintReport {
    /// 由错误类别、位置与主信息构造报告（建议/相关类型/推导链为空）。
    pub fn new(kind: ConstraintErrorKind, span: Option<SourceSpan>, primary_message: impl Into<String>) -> Self {
        Self { kind, span, primary_message: primary_message.into(), suggestions: vec![], related_types: vec![], chain: vec![] }
    }

    /// 在推导链末尾追加一个节点。
    pub fn add_chain_node(&mut self, node: ConstraintChainNode) {
        self.chain.push(node);
    }

    /// 追加一条修复建议。
    pub fn add_suggestion(&mut self, suggestion: FixSuggestion) {
        self.suggestions.push(suggestion);
    }

    /// 渲染包含推导链与建议条数的详细错误文本。
    pub fn to_detailed_message(&self) -> String {
        format!(
            r#"错误: {}
约束追溯: {}
建议修复: {}"#,
            self.primary_message,
            self.chain.len(),
            self.suggestions.len()
        )
    }
}

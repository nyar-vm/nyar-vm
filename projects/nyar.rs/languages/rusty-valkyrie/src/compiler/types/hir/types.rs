use super::{HirExpr, HirIdentifier, HirResolvedCall};
use crate::compiler::types::{Identifier, SourceSpan};
use std::cmp::Ordering;

/// Access level for HIR items.
///
/// This enum represents the four access levels in Valkyrie:
/// - `Public`: Visible to all code
/// - `Protected`: Visible to the current class and its subclasses
/// - `Internal`: Visible within the current module
/// - `Private`: Visible only within the current class/file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AccessLevel {
    /// Public access - visible to all code.
    ///
    /// Items declared with `public` modifier are accessible from any module or class.
    /// This is the most permissive access level.
    Public,

    /// Protected access - visible to current class and subclasses.
    ///
    /// Items declared with `protected` modifier are accessible within:
    /// - The class that defines them
    /// - All subclasses (direct and indirect)
    ///
    /// This is useful for implementation details that should be hidden
    /// from external code but accessible to subclasses.
    Protected,

    /// Internal access - visible within the current module.
    ///
    /// Items declared with `internal` modifier are accessible from any code
    /// within the same module, but not from other modules.
    ///
    /// This is useful for module-internal APIs that should not be exposed
    /// to external consumers.
    Internal,

    /// Private access - visible only within the current class/file.
    ///
    /// Items declared with `private` modifier are accessible only within:
    /// - The class that defines them (for class members)
    /// - The current file (for top-level items)
    ///
    /// This is the most restrictive access level.
    #[default]
    Private,
}

impl AccessLevel {
    fn rank(&self) -> u8 {
        match self {
            AccessLevel::Private => 0,
            AccessLevel::Internal => 1,
            AccessLevel::Protected => 2,
            AccessLevel::Public => 3,
        }
    }

    /// Returns true if this is public access.
    pub fn is_public(&self) -> bool {
        matches!(self, AccessLevel::Public)
    }

    /// Returns true if this is protected access.
    pub fn is_protected(&self) -> bool {
        matches!(self, AccessLevel::Protected)
    }

    /// Returns true if this is internal access.
    pub fn is_internal(&self) -> bool {
        matches!(self, AccessLevel::Internal)
    }

    /// Returns true if this is private access.
    pub fn is_private(&self) -> bool {
        matches!(self, AccessLevel::Private)
    }

    /// Returns the access level as a string for display purposes.
    pub fn as_str(&self) -> &'static str {
        match self {
            AccessLevel::Public => "public",
            AccessLevel::Protected => "protected",
            AccessLevel::Internal => "internal",
            AccessLevel::Private => "private",
        }
    }
}

impl PartialOrd for AccessLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AccessLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank().cmp(&other.rank())
    }
}

/// Visibility of HIR items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirVisibility {
    /// The access level of the item.
    pub access: AccessLevel,
}

impl HirVisibility {
    /// Creates a new visibility with the given access level.
    pub fn new(access: AccessLevel) -> Self {
        Self { access }
    }

    /// Creates a public visibility.
    pub fn public() -> Self {
        Self { access: AccessLevel::Public }
    }

    /// Creates a protected visibility.
    pub fn protected() -> Self {
        Self { access: AccessLevel::Protected }
    }

    /// Creates an internal visibility.
    pub fn internal() -> Self {
        Self { access: AccessLevel::Internal }
    }

    /// Creates a private visibility.
    pub fn private() -> Self {
        Self { access: AccessLevel::Private }
    }

    /// Returns true if the item is public.
    pub fn is_public(&self) -> bool {
        self.access.is_public()
    }

    /// Returns true if the item is protected.
    pub fn is_protected(&self) -> bool {
        self.access.is_protected()
    }

    /// Returns true if the item is internal.
    pub fn is_internal(&self) -> bool {
        self.access.is_internal()
    }

    /// Returns true if the item is private.
    pub fn is_private(&self) -> bool {
        self.access.is_private()
    }

    /// Returns the access level.
    pub fn access_level(&self) -> AccessLevel {
        self.access
    }
}

/// 泛型形参种类（类型或函数种类）。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HirKind {
    /// 类型形参。
    Type,
    /// 函数种类 `domain -> codomain`。
    Function(Box<HirKind>, Box<HirKind>),
}

/// An associated type projection like `Self::Item` or `<T as Trait>::Item`.
///
/// This represents accessing an associated type from a trait implementation.
/// For Generic Associated Types (GATs), the `type_args` field contains the
/// type arguments passed to the GAT's type parameters.
///
/// # Examples
///
/// Simple associated type:
/// ```v
/// trait Iterator {
///     type Item
///     micro next(self) -> Self::Item?
/// }
///
/// micro process<T: Iterator>(iter: T) -> T::Item {
///     iter.next()?
/// }
/// ```
///
/// Generic Associated Type (GAT):
/// ```v
/// trait LendingIterator {
///     type Item<'a> where Self: 'a
///     micro next<'a>(&'a mut self) -> Option<Self::Item<'a>>
/// }
///
/// // Here Self::Item<'a> would have type_args = [HirType::Lifetime("'a")]
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AssociatedType {
    /// The base type (e.g., `Self` or a type parameter).
    pub base: ValkyrieType,
    /// The name of the associated type.
    pub name: Identifier,
    /// Type arguments for Generic Associated Types (GATs).
    ///
    /// For example, in `T::Item<'a>`, this would contain the lifetime
    /// argument `'a`. For non-GAT associated types, this is empty.
    pub type_arguments: Vec<ValkyrieType>,
}

/// A trait object type representing dynamic dispatch.
///
/// Trait objects use witness tables for dynamic method dispatch.
/// At runtime, a trait object is represented as a fat pointer:
/// - data pointer: points to the actual data
/// - witness table pointer: points to the witness table for the trait implementation
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TraitObject {
    /// The trait being implemented.
    pub trait_path: Identifier,
    /// Type parameters for the trait (if any).
    pub type_arguments: Vec<ValkyrieType>,
}

/// 结构类型（row）中的方法签名。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RowMethodType {
    /// 方法名。
    pub name: Identifier,
    /// 参数类型列表。
    pub params: Vec<ValkyrieType>,
    /// 返回类型。
    pub return_type: ValkyrieType,
}

/// 结构类型（row）的方法集合。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RowType {
    /// 方法签名列表。
    pub methods: Vec<RowMethodType>,
}

/// 函数类型签名。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FunctionType {
    /// 参数类型列表。
    pub params: Vec<ValkyrieType>,
    /// 返回类型。
    pub return_type: ValkyrieType,
}

/// 类型 lambda（`micro(T) -> U`）的内部表示。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TypeLambda {
    /// 绑定的泛型形参。
    pub params: Vec<GenericType>,
    /// lambda 体类型。
    pub body: ValkyrieType,
}

/// 带标签元组的字段：标签是类型身份的一部分，位置仍按源码顺序。
///
/// `name` 使用无 shadow 的 [`Identifier`]（只含文本），避免成员可见性身份混进类型身份。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirTupleField {
    /// 字段标签。
    pub name: Identifier,
    /// 字段类型。
    pub ty: ValkyrieType,
}

/// Type model of Valkyrie language.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ValkyrieType {
    /// `void`, the zero type in ADT model
    Void,
    /// `unit`, the one type in ADT model
    Unit,
    /// `bool`
    Boolean,
    /// 8 位整数。
    Integer8 {
        /// 是否为有符号整数。
        signed: bool,
    },
    /// 16 位整数。
    Integer16 {
        /// 是否为有符号整数。
        signed: bool,
    },
    /// 32 位整数。
    Integer32 {
        /// 是否为有符号整数。
        signed: bool,
    },
    /// 64 位整数。
    Integer64 {
        /// 是否为有符号整数。
        signed: bool,
    },
    /// 128 位整数。
    Integer128 {
        /// 是否为有符号整数。
        signed: bool,
    },
    /// `f32`
    Float32,
    /// `f64`
    Float64,
    /// `char`, the Unicode character
    Character,
    /// UTF-8 字符串类型。
    Utf8,
    /// UTF-16 字符串类型。
    Utf16,
    /// 未绑定声明的表面名（泛型形参、尚未解析符号）。不得单独充当名义身份。
    Named(Identifier),
    /// 待绑定的限定类型路径；绑定阶段按 `namespace` + `name` 冻结为 [`Nominal`]，禁止压成点号字符串。
    QualifiedPath {
        /// 类型路径前缀（不含最终短名）。
        namespace: Vec<String>,
        /// 类型短名。
        name: Identifier,
    },
    /// 已绑定 `ItemId` 的名义类型；`arguments` 为完成代入的类型实参。
    /// Resolver/HIR 成功路径必须使用本变体，禁止把短名改写成限定名再塞回 [`Self::Named`]。
    Nominal {
        /// 名义声明身份。
        declaration: nyar_types::ItemId,
        /// 类型实参列表。
        arguments: Vec<ValkyrieType>,
        /// 声明携带的 `[primitive("…")]` 自举身份（规范身份，不是表面短名）。
        ///
        /// 由绑定期从声明的注解冻结写入；仅携带该注解的声明为 `Some`。
        /// 下游的类型识别、carrier 布局与数值/字面量判定必须读这个字段，
        /// 不得再按短名或路径字符串猜。
        #[cfg_attr(feature = "serde", serde(default))]
        primitive_identity: Option<Box<str>>,
    },
    /// 类型应用 `F<A, B, ...>`。
    Apply(Box<ValkyrieType>, Vec<ValkyrieType>),
    /// `T<X>`
    Generic(GenericType),
    /// 函数类型。
    Function(Box<FunctionType>),
    /// 元组类型。
    Tuple(Vec<ValkyrieType>),
    /// 结构 row 类型。
    Row(RowType),
    /// `[T]` 堆数组。
    Array(Box<ValkyrieType>),
    /// `[T; N]` 定长数组。
    FixedArray {
        /// 元素类型。
        element: Box<ValkyrieType>,
        /// 固定长度。
        length: usize,
    },
    /// `micro(T) -> U`
    TypeLambda(Box<TypeLambda>),
    /// trait 对象类型。
    TraitObject(TraitObject),
    /// 关联类型投影。
    Associated(Box<AssociatedType>),
    /// `auto`, automatic infer type
    AutoType,
    /// `Self`, the type slot
    SelfType,
    /// `T?`, an explicit nullable type. It is not an anonymous union and is
    /// never represented by a nominal type named `null`.
    Nullable(Box<ValkyrieType>),
    /// `A | B | C` anonymous union type
    Union(Vec<ValkyrieType>),
    /// `A & B & C` anonymous intersection type
    Intersection(Vec<ValkyrieType>),
    /// 带标签元组：标签参与类型身份；无标签元组仍走 [`Self::Tuple`]，两者不互相兼容。
    ///
    /// 新增变体必须追加在枚举末尾，保证既有变体的 `Ord` 判别顺序与 `TypeId` 分配不变。
    LabeledTuple(Vec<HirTupleField>),
}

impl Default for ValkyrieType {
    fn default() -> Self {
        ValkyrieType::Unit
    }
}

impl ValkyrieType {
    /// 诊断用限定路径显示；只投影 `QualifiedPath`，不参与语义匹配。
    pub fn qualified_path_label(namespace: &[String], name: &Identifier) -> String {
        if namespace.is_empty() { name.to_string() } else { format!("{}::{}", namespace.join("::"), name) }
    }

    /// 名义声明身份；仅 [`Self::Nominal`]（及历史上仍包在 `Apply` 里的名义基）携带。
    pub fn nominal_declaration(&self) -> Option<nyar_types::ItemId> {
        match self {
            Self::Nominal { declaration, .. } => Some(*declaration),
            Self::Apply(base, _) => base.nominal_declaration(),
            _ => None,
        }
    }

    /// 声明携带的 `[primitive("…")]` 自举身份；未绑定或声明无该注解时为 `None`。
    ///
    /// 这是下游识别 primitive 类型的唯一事实来源：返回 `None` 时不得回退短名匹配。
    pub fn primitive_identity(&self) -> Option<&str> {
        match self {
            Self::Nominal { primitive_identity, .. } => primitive_identity.as_deref(),
            _ => None,
        }
    }

    /// 名义类型实参；非名义形状返回空切片。
    pub fn nominal_arguments(&self) -> &[ValkyrieType] {
        match self {
            Self::Nominal { arguments, .. } => arguments.as_slice(),
            Self::Apply(_, arguments) => arguments.as_slice(),
            _ => &[],
        }
    }
}

/// 数值自举身份的形状：整数（位宽、有符号）或浮点（位宽）。
///
/// 这是 `[primitive("std::primitive::…")]` 身份的类型层面事实，不是 carrier 决定；
/// carrier 由表示层按同一身份授予。`usize`/`isize` 的位宽是目标指针宽度合同。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveNumericShape {
    /// 整数形状。
    Integer {
        /// 位宽。
        bits: u16,
        /// 是否有符号。
        signed: bool,
    },
    /// 浮点形状。
    Float {
        /// 位宽。
        bits: u16,
    },
}

/// 按 `[primitive]` 规范身份（`std::primitive::i32` 等，不是表面短名）给出数值形状。
///
/// 身份表外一律 `None`：调用方不得回退到短名匹配。数值族覆盖 std 已声明的
/// 全部定宽整数与浮点身份；`bool`/`unit`/`void`/`any`/`null` 不是数值。
/// `usize`/`isize` 不在本表：它们是指针宽度别名，由 std 按目标架构的
/// `<% match %>`（`std::primitive::pointer_width`）结算成定宽整数后再进入编译器，
/// 编译器不得把它们当内建量化形状。
pub fn numeric_primitive_identity_shape(identity: &str) -> Option<PrimitiveNumericShape> {
    use PrimitiveNumericShape::{Float, Integer};
    Some(match identity {
        "std::primitive::i8" => Integer { bits: 8, signed: true },
        "std::primitive::i16" => Integer { bits: 16, signed: true },
        "std::primitive::i32" => Integer { bits: 32, signed: true },
        "std::primitive::i64" => Integer { bits: 64, signed: true },
        "std::primitive::i128" => Integer { bits: 128, signed: true },
        "std::primitive::u8" => Integer { bits: 8, signed: false },
        "std::primitive::u16" => Integer { bits: 16, signed: false },
        "std::primitive::u32" => Integer { bits: 32, signed: false },
        "std::primitive::u64" => Integer { bits: 64, signed: false },
        "std::primitive::u128" => Integer { bits: 128, signed: false },
        "std::primitive::f32" => Float { bits: 32 },
        "std::primitive::f64" => Float { bits: 64 },
        "std::primitive::f128" => Float { bits: 128 },
        _ => return None,
    })
}

/// 把 `[primitive("…")]` 规范身份折叠成该内建标量的唯一类型身份。
///
/// 内建标量在语言里只有一个 TypeId；std 的 `[primitive] structure` 只是宿主绑定
/// 声明，不是第二个名义类型。因此绑定阶段必须走本表，命中即产出内建载体，
/// 不得生成 [`ValkyrieType::Nominal`]。
///
/// 表外身份一律 `None`，调用方保留既有形态并在缺口清单登记：
/// `std::primitive::f128`（无 128 位浮点载体）、`std::primitive::any` / `null`
/// （顶/空类型，不是宿主绑定的标量载体）、`std::text::text`（Utf8/Utf16 双载体）。
/// `usize`/`isize` 不是 `[primitive]` 声明：std 用 `type usize = u32|u64` 别名，
/// 别名展开后自然落到对应内建整数，不进入本表。
pub fn primitive_identity_builtin_type(identity: &str) -> Option<ValkyrieType> {
    Some(match identity {
        "std::primitive::bool" => ValkyrieType::Boolean,
        "std::primitive::unit" => ValkyrieType::Unit,
        "std::primitive::void" => ValkyrieType::Void,
        "std::text::char" => ValkyrieType::Character,
        "std::primitive::f32" => ValkyrieType::Float32,
        "std::primitive::f64" => ValkyrieType::Float64,
        "std::primitive::i8" => ValkyrieType::Integer8 { signed: true },
        "std::primitive::i16" => ValkyrieType::Integer16 { signed: true },
        "std::primitive::i32" => ValkyrieType::Integer32 { signed: true },
        "std::primitive::i64" => ValkyrieType::Integer64 { signed: true },
        "std::primitive::i128" => ValkyrieType::Integer128 { signed: true },
        "std::primitive::u8" => ValkyrieType::Integer8 { signed: false },
        "std::primitive::u16" => ValkyrieType::Integer16 { signed: false },
        "std::primitive::u32" => ValkyrieType::Integer32 { signed: false },
        "std::primitive::u64" => ValkyrieType::Integer64 { signed: false },
        "std::primitive::u128" => ValkyrieType::Integer128 { signed: false },
        _ => return None,
    })
}

/// 泛型参数由所属声明和源码参数序号唯一标识，拼写只供解析与诊断。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GenericBinderId {
    /// 声明身份（函数、名义类型、trait、imply 或局部泛型声明）。
    pub owner: nyar_types::ItemId,
    /// 在所属声明内的源码顺序。
    pub ordinal: u32,
}

/// 泛型形参表面表示。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GenericType {
    /// 泛型声明的身份；仅解析中的表面节点可以为空。
    pub binder: Option<GenericBinderId>,
    /// 形参名。
    pub name: Identifier,
    /// 形参种类。
    pub kind: HirKind,
    /// trait 约束列表。
    pub bounds: Vec<Identifier>,
}

/// An associated type implementation in an impl block.
///
/// When implementing a trait with associated types, the concrete type
/// for each associated type must be specified in the impl block.
///
/// # Example
///
/// ```v
/// impl Iterator for Counter {
///     type Item = i32
///
///     micro next(self) -> Self::Item? {
///         // implementation
///     }
/// }
/// ```
///
/// In this example, `HirAssociatedTypeImpl` would represent the
/// `type Item = i32` declaration within the impl block.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirAssociatedTypeImpl {
    /// The name of the associated type being implemented.
    pub name: Identifier,
    /// The concrete type that the associated type is bound to.
    pub concrete_type: ValkyrieType,
    /// Type arguments for Generic Associated Types (GATs).
    ///
    /// For example, in `type Item<'a> = &'a T`, this would contain the
    /// lifetime argument `'a`. For non-GAT associated types, this is empty.
    pub type_args: Vec<ValkyrieType>,
    /// Source span for error reporting.
    pub span: SourceSpan,
}

impl HirAssociatedTypeImpl {
    /// Creates a new associated type implementation.
    pub fn new(name: Identifier, concrete_type: ValkyrieType, span: SourceSpan) -> Self {
        Self { name, concrete_type, type_args: Vec::new(), span }
    }

    /// Creates a GAT implementation with type arguments.
    pub fn with_type_args(name: Identifier, concrete_type: ValkyrieType, type_args: Vec<ValkyrieType>, span: SourceSpan) -> Self {
        Self { name, concrete_type, type_args, span }
    }

    /// Returns true if this is a GAT implementation with type arguments.
    pub fn is_gat_imply(&self) -> bool {
        !self.type_args.is_empty()
    }
}

/// An associated constant implementation in an impl block.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirAssociatedConstImpl {
    /// The name of the associated constant being implemented.
    pub name: Identifier,
    /// Optional explicit constant type annotation.
    pub const_type: Option<ValkyrieType>,
    /// The concrete value assigned to the associated constant.
    pub value: HirExpr,
    /// Source span for error reporting.
    pub span: SourceSpan,
}

/// Call-site binding mode for a parameter (`<` / `>` markers).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HirParameterBindingKind {
    /// May only be passed positionally (before `<`).
    PositionalOnly,
    /// May be passed positionally or by name (default when no markers).
    #[default]
    PositionalOrKeyword,
    /// May only be passed by name (after `>`).
    KeywordOnly,
}

/// Variadic parameter kind (`..` positional rest, `...` keyword rest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HirVariadicKind {
    #[default]
    /// 非可变参数。
    None,
    /// `..name` ? absorbs extra positional arguments.
    PositionalRest,
    /// `...name` ? absorbs extra keyword arguments.
    KeywordRest,
}

/// 函数或方法形参。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirParam {
    /// 参数标识符。
    pub name: HirIdentifier,
    /// 参数类型。
    pub ty: ValkyrieType,
    /// 调用点绑定模式。
    pub binding_kind: HirParameterBindingKind,
    /// Whether the parameter uses `mut` (including `mut self`).
    #[cfg_attr(feature = "serde", serde(default))]
    pub is_mutable: bool,
    /// 默认值表达式（若有）。
    pub default: Option<HirExpr>,
    /// 可变参数种类。
    pub variadic: HirVariadicKind,
}

impl Default for HirParam {
    fn default() -> Self {
        Self {
            name: HirIdentifier {
                name: Identifier::new(""),
                shadow_index: 0,
                span: crate::compiler::types::SourceSpan::new(crate::compiler::types::SourceID::default(), 0, 0),
            },
            ty: ValkyrieType::Unit,
            binding_kind: HirParameterBindingKind::default(),
            is_mutable: false,
            default: None,
            variadic: HirVariadicKind::default(),
        }
    }
}

/// 提取器模式（构造器或数组切片）。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HirExtractorPattern {
    /// 构造器提取器。
    Constructor {
        /// 构造器路径。
        name: crate::compiler::types::NamePath,
        /// 规范化被调路径。
        canonical_callee: crate::compiler::types::NamePath,
        /// 字段子模式。
        fields: Vec<HirPattern>,
        /// 解析后的调用信息。
        resolved: Option<HirResolvedCall>,
    },
    /// 数组切片提取器。
    Array {
        /// 规范化被调路径。
        canonical_callee: crate::compiler::types::NamePath,
        /// 前缀元素模式。
        prefix: Vec<HirPattern>,
        /// 剩余绑定（若有）。
        rest: Option<HirIdentifier>,
        /// 后缀元素模式。
        suffix: Vec<HirPattern>,
        /// 解析后的调用信息。
        resolved: Option<HirResolvedCall>,
    },
}

/// 模式静态可反驳性分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PatternRefutability {
    /// Pattern always matches (irrefutable) - safe for `let` bindings
    Irrefutable,
    /// Pattern may fail to match (refutable) - requires `if let`, `match`, etc.
    Refutable,
    /// Cannot determine statically (e.g., extractor with unknown semantics)
    Unknown,
}

impl PatternRefutability {
    /// 判断模式是否可能匹配失败。
    pub fn is_refutable(self) -> bool {
        matches!(self, PatternRefutability::Refutable | PatternRefutability::Unknown)
    }
}

/// HIR 模式匹配节点。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HirPattern {
    /// 通配符 `_`。
    Wildcard,
    /// 变量绑定。
    Variable(HirIdentifier),
    /// 元组模式。
    Tuple(Vec<HirPattern>),
    /// 字面量模式。
    Literal(HirLiteral),
    /// 范围模式。
    Range {
        /// 起始字面量（可选）。
        start: Option<HirLiteral>,
        /// 结束字面量（可选）。
        end: Option<HirLiteral>,
        /// 结束边界是否包含。
        inclusive_end: bool,
    },
    /// 提取器模式。
    Extractor(HirExtractorPattern),
    /// 或模式 `a | b`。
    Or(Vec<HirPattern>),
    /// 名称路径模式。
    Name(crate::compiler::types::NamePath),
    /// 类型测试模式。
    Type {
        /// 类型路径（仅保留源码拼写）。
        path: crate::compiler::types::NamePath,
        /// HIR 解析出的名义声明身份；缺失即结构化失败，MIR 不得按名称重建类型关系。
        #[cfg_attr(feature = "serde", serde(default))]
        declaration: Option<nyar_types::ItemId>,
    },
    /// 带类型注解的绑定。
    TypedBind {
        /// 绑定标识符。
        identifier: HirIdentifier,
        /// 类型路径。
        ty: crate::compiler::types::NamePath,
    },
    /// 对象/结构体字段模式。
    Object {
        /// 类型路径（可选）。
        name: Option<crate::compiler::types::NamePath>,
        /// HIR 解析出的期望类型声明身份；缺失即结构化失败。
        #[cfg_attr(feature = "serde", serde(default))]
        type_declaration: Option<nyar_types::ItemId>,
        /// 命名字段子模式。
        fields: Vec<(Identifier, HirPattern)>,
        /// 与 `fields` 一一对应的字段身份；HIR 绑定阶段冻结，MIR 只按 `FieldId` 读取。
        #[cfg_attr(feature = "serde", serde(default))]
        field_ids: Vec<Option<nyar_types::FieldId>>,
        /// 字段名来自源码花括号写法（真名字段，按声明字段名绑定）；
        /// `false` 表示位置解构（按书写槽位绑定）。
        #[cfg_attr(feature = "serde", serde(default))]
        named_fields: bool,
        /// 剩余字段绑定（若有）。
        rest: Option<HirIdentifier>,
    },
    /// `else` 兜底分支。
    Else,
    /// `whole @ subpat` ? bind whole scrutinee slice and destructure sub-pattern.
    Bind {
        /// 整体绑定标识符。
        identifier: HirIdentifier,
        /// 子模式。
        pattern: Box<HirPattern>,
    },
    /// `mut pat` ? mutable reference binding.
    Mut(Box<HirPattern>),
    /// `pin [mut] pat` ? pinned mutable reference binding.
    Pin {
        /// 是否为 `mut pin`。
        mutable: bool,
        /// 子模式。
        pattern: Box<HirPattern>,
    },
}

impl HirPattern {
    /// 静态推断模式的可反驳性。
    pub fn refutability(&self) -> PatternRefutability {
        use PatternRefutability::*;
        match self {
            HirPattern::Wildcard | HirPattern::Variable(_) | HirPattern::Bind { .. } | HirPattern::Mut(_) | HirPattern::Pin { .. } => {
                Irrefutable
            }
            HirPattern::Tuple(items) => {
                if items.iter().any(|item| item.refutability().is_refutable()) {
                    Refutable
                }
                else {
                    Irrefutable
                }
            }
            HirPattern::Literal(_) | HirPattern::Range { .. } | HirPattern::Extractor(_) | HirPattern::Name(_) | HirPattern::Type { .. } => {
                Refutable
            }
            HirPattern::Or(items) => {
                if items.iter().any(|item| item.refutability().is_refutable()) {
                    Refutable
                }
                else {
                    Irrefutable
                }
            }
            HirPattern::TypedBind { .. } => Refutable,
            HirPattern::Object { fields, rest, .. } => {
                let fields_refutable = fields.iter().any(|(_, p)| p.refutability().is_refutable());
                let rest_refutable = rest.is_some();
                if fields_refutable || rest_refutable { Refutable } else { Irrefutable }
            }
            HirPattern::Else => Refutable,
        }
    }
}

/// HIR 字符串片段。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HirStringSegment {
    /// 纯文本。
    Text(String),
    /// 插值片段。
    Interpolation {
        /// 插值表达式。
        expr: HirExpr,
        /// 是否为 Fluent 插值。
        is_fluent: bool,
    },
}

/// 引号捕获的引号种类（§4.10 A：语法元数据，不是已定型值；只在无约束默认时使用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HirQuoteKind {
    /// `'` 捕获。
    Single,
    /// `"` 捕获。
    Double,
}

/// HIR 字符串字面量。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct HirStringLiteral {
    /// DSL 前缀。
    pub prefix: Option<Identifier>,
    /// 引号个数。
    pub quote_count: u8,
    /// 引号种类。
    pub quote: HirQuoteKind,
    /// 字符串片段列表。
    pub segments: Vec<HirStringSegment>,
}

impl HirStringLiteral {
    /// 是否为无插值的纯文本捕获。
    pub fn is_plain_text(&self) -> bool {
        self.segments.iter().all(|segment| matches!(segment, HirStringSegment::Text(_)))
    }

    /// 恰好一个 BMP 字符的纯文本捕获返回其码元；含插值、空、多字符或超 BMP 返回 `None`。
    ///
    /// 用于上下文定型：`char` 期望只接受这一形态，其余按文本处理。
    pub fn as_single_bmp_char(&self) -> Option<u16> {
        let [HirStringSegment::Text(text)] = self.segments.as_slice() else {
            return None;
        };
        let mut characters = text.chars();
        match (characters.next(), characters.next()) {
            (Some(single), None) => u16::try_from(u32::from(single)).ok(),
            _ => None,
        }
    }
}

/// HIR 字面量值。
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum HirLiteral {
    /// 64 位整数。
    Integer64(i64),
    /// 64 位浮点数。
    Float64(ordered_float::OrderedFloat<f64>),
    /// 字符字面量（`'x'`）：`char` 是 u16 code unit，值已在 lowering 校验于 BMP 内。
    Character(u16),
    /// 字节字面量（`b'x'`）：值域 `u8`。
    Byte(u8),
    /// 字符串字面量。
    String(HirStringLiteral),
    /// 布尔值。
    Bool(bool),
    /// 可空空值 `null`（`T?` 的空引用），不是名义 `Option::None` 变体。
    Null,
    /// 单元值 `()`。
    Unit,
}

impl HirLiteral {
    /// 常量文本渲染：纯文本捕获逐段拼接，单字符 `char` 字面量按单字符文本处理
    /// （§4.10 B/E：引号不预定类型，插值段不得混入符号身份）。
    ///
    /// 含插值或非文本字面量返回 `None`，调用方据此拒绝常量资格。
    pub fn as_constant_text(&self) -> Option<String> {
        match self {
            HirLiteral::String(literal) => {
                if !literal.is_plain_text() {
                    return None;
                }
                let mut rendered = String::new();
                for segment in &literal.segments {
                    let HirStringSegment::Text(text) = segment else {
                        return None;
                    };
                    rendered.push_str(text);
                }
                Some(rendered)
            }
            HirLiteral::Character(value) => char::from_u32(u32::from(*value)).map(|character| character.to_string()),
            _ => None,
        }
    }
}

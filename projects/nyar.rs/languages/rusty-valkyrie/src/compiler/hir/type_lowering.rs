//! Oak `AST` 类型表达式到 `HIR` 类型的 lowering 与预检查。
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

use crate::compiler::{
    hir::{HirError, type_relation::is_numeric_type_name},
    syntax::oak::{
        self,
        ast::{
            NamePath as AstNamePath, TermExpression, TypeExpression,
            type_nodes::{FunctionType as AstFunctionType, OptionalType, TupleType},
        },
    },
    types::{
        Identifier,
        hir::{FunctionType, GenericType, HirKind, HirTupleField, RowMethodType, RowType, ValkyrieType},
    },
};
use nyar_types::DiagnosticSpan;
use oak_valkyrie::lexer::token_type::ValkyrieTokenType;

use super::lowering::errors::{ArrayExtentFailure, TupleLabelingFailure};

thread_local! {
    static MODULE_TYPE_ALIASES: RefCell<Vec<ModuleTypeAliasFrame>> = RefCell::new(Vec::new());
    /// 当前 lowering 词法作用域声明的泛型形参与 trait 关联类型短名。
    ///
    /// 只有声明域本身拥有的名字进入这里；引用 `T` 或 `Item` 时不得再按拼写猜测。
    static GENERIC_BINDER_SCOPE: RefCell<Vec<GenericBinderFrame>> = RefCell::new(Vec::new());
    /// 当前 lowering 作用域可见的名义类型声明短名（structure/class/enums/variant）。
    ///
    /// 只登记实际声明过的名字，用于产出待冻结的 `Named`；不构成未知名字的 fallback。
    /// 每条记录同时携带声明 namespace，限定路径必须按完整身份匹配。
    static DECLARED_NOMINAL_TYPES: RefCell<Vec<Vec<VisibleNominalType>>> = RefCell::new(Vec::new());
}

/// 一条可见名义类型声明：短名 + 声明 namespace（不含短名本身）。
#[derive(Debug, Clone)]
pub(crate) struct VisibleNominalType {
    pub(crate) namespace: Vec<String>,
    pub(crate) name: String,
}

/// 进入一次 lowering 作用域时可见的单条类型别名。
///
/// `target` 为 `None` 表示该别名声明在本组，目标尚未降低：名字已经可见（因此遮蔽同名内建），
/// 展开则要等目标填入后再进行。可见依赖组导出的别名携带已冻结的目标。
#[derive(Debug, Clone)]
pub(crate) struct VisibleTypeAlias {
    /// 别名短名。
    pub name: String,
    /// 声明该别名的 namespace（不含别名本身），用于限定路径的身份匹配。
    pub namespace: Vec<String>,
    /// 泛型形参名。
    pub params: Vec<String>,
    /// 已冻结的目标类型；本组声明为 `None`。
    pub target: Option<ValkyrieType>,
}

/// 模块类型别名条目：可携带泛型形参；目标未填入时为 `None`。
#[derive(Debug, Clone)]
struct ModuleTypeAliasEntry {
    namespace: Vec<String>,
    params: Vec<String>,
    target: Option<ValkyrieType>,
    /// 该条目是否来自依赖组导出的别名（`VisibleTypeAlias::target: Some`）。
    ///
    /// 本组（或本根）自身的 `type` 声明在 `enter` 时目标尚未填入，显式记为 `false`；
    /// 之后 `register_alias` 填目标也保持 `false`。生成组据此区分「依赖包别名」与「自己的声明」。
    imported: bool,
}

/// 一层模块类型别名作用域：可见别名表 + 该层是否收窄「依赖导出内建同名别名的短名遮蔽」。
#[derive(Debug)]
struct ModuleTypeAliasFrame {
    aliases: BTreeMap<String, ModuleTypeAliasEntry>,
    /// 仅生成源码组（`GeneratedOnly` 入口）启用。
    ///
    /// 生成单元的类型位置是它自己生成的宿主 ABI 面，不该被依赖包别名改写；
    /// 普通应用组仍经依赖可见性拿到别名，不受影响。
    suppress_imported_builtin_short_names: bool,
}

/// 管理当前 lowering 过程中的模块 `type` 别名作用域。
///
/// 作用域内容就是「本组（或本根）的 `type` 声明 + 可见依赖导出的别名」，与语义闭包的
/// 可见性合同一致：不可见组的别名不进入作用域，内建遮蔽同样只认作用域内的名字。
#[derive(Debug)]
pub(crate) struct ModuleTypeAliasScope;

impl ModuleTypeAliasScope {
    /// 进入一次 lowering 作用域并注册可见别名。
    ///
    /// 调用方把依赖导出（`target: Some`）与本组待填目标的声明（`target: None`）一起传入；
    /// 本组声明在依赖导出之后插入，故同名时以本组声明为准。
    ///
    /// `suppress_imported_builtin_short_names` 只由生成源码组（`GeneratedOnly` 入口）传入
    /// `true`：依赖导出的别名若与既有内建同名，则不参与短名遮蔽（判据见
    /// [`short_alias_entry`]）。普通应用组传 `false`，保持既有的依赖可见性合同。
    pub(crate) fn enter(visible: Vec<VisibleTypeAlias>, suppress_imported_builtin_short_names: bool) -> Self {
        let mut aliases = BTreeMap::new();
        for alias in visible {
            let imported = alias.target.is_some();
            let entry = ModuleTypeAliasEntry { namespace: alias.namespace, params: alias.params, target: alias.target, imported };
            aliases.insert(alias.name, entry);
        }
        MODULE_TYPE_ALIASES.with(|stack| {
            stack.borrow_mut().push(ModuleTypeAliasFrame { aliases, suppress_imported_builtin_short_names });
        });
        Self
    }

    /// 填入本组别名的目标类型；名字必须已由 [`ModuleTypeAliasScope::enter`] 注册。
    pub(crate) fn register_alias(name: &str, namespace: Vec<String>, params: Vec<String>, target: ValkyrieType) {
        MODULE_TYPE_ALIASES.with(|stack| {
            if let Some(top) = stack.borrow_mut().last_mut() {
                top.aliases.insert(name.to_string(), ModuleTypeAliasEntry { namespace, params, target: Some(target), imported: false });
            }
        });
    }
}

impl Drop for ModuleTypeAliasScope {
    fn drop(&mut self) {
        MODULE_TYPE_ALIASES.with(|stack| {
            let _ = stack.borrow_mut().pop();
        });
    }
}

/// 一层声明域的泛型形参与 trait 关联类型短名。
#[derive(Debug, Default)]
struct GenericBinderFrame {
    /// 本域声明的泛型形参短名。
    type_params: BTreeSet<String>,
    /// 本域 trait 声明的关联类型短名。
    associated: BTreeSet<String>,
}

/// 管理泛型形参与 trait 关联类型名字的 lowering 作用域。
///
/// 与 [`ModuleTypeAliasScope`] 同构：进入时压入一帧，`Drop` 时出栈；嵌套声明域
/// （例如 `structure` 里的 method）逐层向外查找，因此内层域可以看到外层的形参。
#[derive(Debug)]
pub(crate) struct GenericBinderScope;

impl GenericBinderScope {
    /// 进入一次声明域作用域并注册该域拥有的名字。
    pub(crate) fn enter(type_params: &[String], associated: &[String]) -> Self {
        GENERIC_BINDER_SCOPE.with(|stack| {
            stack.borrow_mut().push(GenericBinderFrame {
                type_params: type_params.iter().cloned().collect(),
                associated: associated.iter().cloned().collect(),
            });
        });
        Self
    }
}

impl Drop for GenericBinderScope {
    fn drop(&mut self) {
        GENERIC_BINDER_SCOPE.with(|stack| {
            let _ = stack.borrow_mut().pop();
        });
    }
}

/// 管理当前 lowering 作用域内已声明名义类型的短名集合。
///
/// 与 [`ModuleTypeAliasScope`] 同构：进入时压栈，`Drop` 时出栈。名义声明与类型别名
/// 是两份独立事实，不能塞进别名表：别名要展开目标，名义名只产出待 `bind_declared_nominal_types`
/// 冻结声明身份的 `Named`。
#[derive(Debug)]
pub(crate) struct NominalTypeScope;

impl NominalTypeScope {
    /// 进入一次 lowering 作用域并注册可见的名义类型声明身份。
    pub(crate) fn enter(names: Vec<VisibleNominalType>) -> Self {
        DECLARED_NOMINAL_TYPES.with(|stack| {
            stack.borrow_mut().push(names);
        });
        Self
    }
}

impl Drop for NominalTypeScope {
    fn drop(&mut self) {
        DECLARED_NOMINAL_TYPES.with(|stack| {
            let _ = stack.borrow_mut().pop();
        });
    }
}

/// 当前词法作用域（含外层声明域）是否声明了该短名的名义类型。
fn is_declared_nominal_type(name: &str) -> bool {
    DECLARED_NOMINAL_TYPES.with(|stack| stack.borrow().iter().rev().any(|names| names.iter().any(|item| item.name == name)))
}

/// 限定路径是否命中作用域内某条名义声明：namespace + 短名必须与路径逐段相等。
///
/// 末段同名但 namespace 不同不构成可见性，禁止把限定路径折叠成短名匹配。
fn is_declared_qualified_nominal_type(parts: &[&str]) -> bool {
    let Some((name, namespace)) = parts.split_last()
    else {
        return false;
    };
    DECLARED_NOMINAL_TYPES.with(|stack| {
        stack
            .borrow()
            .iter()
            .rev()
            .any(|names| names.iter().any(|item| item.name == *name && item.namespace.iter().map(String::as_str).eq(namespace.iter().copied())))
    })
}

/// 该短名是否已被别名、名义声明、内建或数值声明事实占据。
///
/// 供 `imply Option<T>` 这类把形参写在目标类型实参位置的声明入口筛选真正的形参名；
/// 已占据的名字不得再被登记为新形参。
pub(crate) fn is_bound_type_name(name: &str) -> bool {
    has_module_type_alias(name) || is_declared_nominal_type(name) || canonical_builtin_type(name).is_some() || is_numeric_type_name(name)
}

/// 当前词法作用域（含外层声明域）是否声明了该泛型形参短名。
fn is_generic_type_param(name: &str) -> bool {
    GENERIC_BINDER_SCOPE.with(|stack| stack.borrow().iter().rev().any(|frame| frame.type_params.contains(name)))
}

/// 当前词法作用域（含外层声明域）是否声明了该 trait 关联类型短名。
fn is_trait_associated_name(name: &str) -> bool {
    GENERIC_BINDER_SCOPE.with(|stack| stack.borrow().iter().rev().any(|frame| frame.associated.contains(name)))
}

/// 校验 Oak `AST` 类型表达式是否满足当前 `HIR` lowering 前提。
#[doc(hidden)]
pub fn validate_type_expression(ty: &TypeExpression) -> Result<(), HirError> {
    match ty {
        TypeExpression::Namepath(path) => {
            if let Some(part) = path.parts.last() {
                validate_source_text_type_name(&part.name)?;
                if let Some(canonical_name) = legacy_builtin_type_alias(&part.name) {
                    return Err(HirError::legacy_builtin_type_alias(&part.name, canonical_name));
                }
            }
        }
        TypeExpression::Generic(generic) => {
            validate_source_text_type_name(&generic.name.name)?;
        }
        TypeExpression::Tuple(tuple) => {
            for element in &tuple.elements {
                validate_type_expression(&element.ty)?;
            }
        }
        TypeExpression::Function(function) => {
            for param in &function.params {
                validate_type_expression(param)?;
            }
            validate_type_expression(&function.return_type)?;
        }
        TypeExpression::Optional(optional) => validate_type_expression(optional.inner.as_ref())?,
        TypeExpression::AssociatedType(associated) => {
            validate_source_text_type_name(&associated.name.name)?;
            validate_source_text_type_name(&associated.base.name)?;
        }
        TypeExpression::QualifiedAssociatedType(qualified) => {
            validate_type_expression(qualified.ty.as_ref())?;
            validate_source_text_type_name(&qualified.name.name)?;
        }
        TypeExpression::Binary(node) => {
            validate_type_expression(&node.lhs)?;
            validate_type_expression(&node.rhs)?;
        }
        TypeExpression::Unary(node) => validate_type_expression(&node.base)?,
        TypeExpression::Apply(apply) => {
            validate_type_expression(&apply.base)?;
            for argument in &apply.arguments {
                validate_type_expression(argument)?;
            }
        }
    }
    Ok(())
}

/// Reject source-level text aliases that do not identify a language encoding.
pub fn validate_source_text_type_name(name: &str) -> Result<(), HirError> {
    if is_legacy_text_type_name(name) {
        return Err(HirError::ambiguous_text_type_name(name));
    }
    Ok(())
}

/// 将 Oak `AST` 类型表达式降到最小 `HIR` 类型表示。
///
/// 无法在当前作用域解析的类型短名会以 `HirError::TypeNotFound` 返回，由调用方
/// 逐层传播；不再静默 fallback 成 `ValkyrieType::Named` 传递到 canonical 阶段。
#[doc(hidden)]
pub fn lower_type_expression(ty: &TypeExpression) -> Result<ValkyrieType, HirError> {
    Ok(match ty {
        TypeExpression::Namepath(path) => lower_type_namepath(path)?,
        TypeExpression::Generic(generic) => ValkyrieType::Generic(GenericType {
            binder: None,
            name: Identifier::new(&generic.name.name),
            kind: HirKind::Type,
            bounds: Vec::new(),
        }),
        TypeExpression::Tuple(tuple) => {
            if tuple.elements.is_empty() {
                ValkyrieType::Unit
            }
            else {
                let lowered = tuple
                    .elements
                    .iter()
                    .map(|element| Ok((element.name.as_ref().map(|name| Identifier::new(&name.name)), lower_type_expression(&element.ty)?)))
                    .collect::<Result<Vec<_>, HirError>>()?;
                let named = lowered.iter().filter(|(name, _)| name.is_some()).count();
                if named == 0 {
                    ValkyrieType::Tuple(lowered.into_iter().map(|(_, ty)| ty).collect())
                }
                else if named != lowered.len() {
                    return Err(tuple_labeling_error(TupleLabelingFailure::Mixed));
                }
                else {
                    let mut seen = BTreeSet::new();
                    let mut fields = Vec::with_capacity(lowered.len());
                    for (name, ty) in lowered {
                        let name = name.expect("all elements named by the count check");
                        if !seen.insert(name.as_str().to_string()) {
                            return Err(tuple_labeling_error(TupleLabelingFailure::DuplicateLabel { name: name.as_str().to_string() }));
                        }
                        fields.push(HirTupleField { name, ty });
                    }
                    ValkyrieType::LabeledTuple(fields)
                }
            }
        }
        TypeExpression::Function(function) => ValkyrieType::Function(Box::new(FunctionType {
            params: function.params.iter().map(lower_type_expression).collect::<Result<Vec<_>, _>>()?,
            return_type: lower_type_expression(function.return_type.as_ref())?,
        })),
        TypeExpression::Optional(optional) => flatten_nullable_type(lower_type_expression(optional.inner.as_ref())?),
        TypeExpression::AssociatedType(associated) => ValkyrieType::Associated(Box::new(crate::compiler::types::hir::AssociatedType {
            base: if associated.base.name == "Self" {
                ValkyrieType::SelfType
            }
            else {
                ValkyrieType::Named(Identifier::new(&associated.base.name))
            },
            name: Identifier::new(&associated.name.name),
            type_arguments: Vec::new(),
        })),
        TypeExpression::QualifiedAssociatedType(qualified) => {
            let base = lower_type_expression(qualified.ty.as_ref())?;
            ValkyrieType::Associated(Box::new(crate::compiler::types::hir::AssociatedType {
                base,
                name: Identifier::new(&qualified.name.name),
                type_arguments: qualified.trait_path.parts.iter().map(|part| ValkyrieType::Named(Identifier::new(&part.name))).collect(),
            }))
        }
        TypeExpression::Binary(node) => match node.operator {
            ValkyrieTokenType::Pipe => ValkyrieType::Union(vec![lower_type_expression(&node.lhs)?, lower_type_expression(&node.rhs)?]),
            ValkyrieTokenType::Ampersand => {
                ValkyrieType::Intersection(vec![lower_type_expression(&node.lhs)?, lower_type_expression(&node.rhs)?])
            }
            _ => lower_type_expression(&node.lhs)?,
        },
        TypeExpression::Unary(node) => match node.operator {
            ValkyrieTokenType::BracketL => {
                let element = lower_type_expression(&node.base)?;
                match &node.length {
                    Some(extent) => lower_array_extent(extent, element)?,
                    None => ValkyrieType::Array(Box::new(element)),
                }
            }
            _ => lower_type_expression(&node.base)?,
        },
        TypeExpression::Apply(apply) => {
            let base = lower_type_expression(&apply.base)?;
            let arguments = apply.arguments.iter().map(lower_type_expression).collect::<Result<Vec<_>, _>>()?;
            ValkyrieType::Apply(Box::new(base), arguments)
        }
    })
}

fn flatten_nullable_type(inner: ValkyrieType) -> ValkyrieType {
    match inner {
        ValkyrieType::Nullable(payload) => ValkyrieType::Nullable(payload),
        payload => ValkyrieType::Nullable(Box::new(payload)),
    }
}

/// 渲染类型表达式，供错误消息与回退路径使用。
pub(crate) fn render_type_expression(ty: &TypeExpression) -> String {
    match ty {
        TypeExpression::Namepath(path) => path.parts.iter().map(|part| part.name.as_str()).collect::<Vec<_>>().join("::"),
        TypeExpression::Generic(generic) => generic.name.name.clone(),
        TypeExpression::Tuple(tuple) => {
            if tuple.elements.is_empty() {
                return "()".to_string();
            }
            let inner = tuple.elements.iter().map(|e| render_type_expression(&e.ty)).collect::<Vec<_>>().join(", ");
            format!("({inner})")
        }
        TypeExpression::Function(function) => {
            let params_str = function.params.iter().map(render_type_expression).collect::<Vec<_>>().join(", ");
            format!("micro({params_str}) -> {}", render_type_expression(function.return_type.as_ref()))
        }
        TypeExpression::Optional(optional) => {
            format!("{}?", render_type_expression(optional.inner.as_ref()))
        }
        TypeExpression::AssociatedType(associated) => {
            format!("{}::{}", associated.base.name, associated.name.name)
        }
        TypeExpression::QualifiedAssociatedType(qualified) => {
            format!(
                "<{} as {}>::{}",
                render_type_expression(qualified.ty.as_ref()),
                render_type_expression(&TypeExpression::Namepath(Box::new(qualified.trait_path.clone()))),
                qualified.name.name
            )
        }
        TypeExpression::Binary(node) => {
            let op = match node.operator {
                ValkyrieTokenType::Pipe => " | ",
                ValkyrieTokenType::Ampersand => " & ",
                _ => " ",
            };
            format!("{}{}{}", render_type_expression(&node.lhs), op, render_type_expression(&node.rhs))
        }
        TypeExpression::Unary(node) => match node.operator {
            ValkyrieTokenType::BracketL => {
                if let Some(length_expr) = &node.length {
                    let length = render_array_extent(length_expr);
                    format!("[{}; {}]", render_type_expression(&node.base), length)
                }
                else {
                    format!("[{}]", render_type_expression(&node.base))
                }
            }
            _ => render_type_expression(&node.base),
        },
        TypeExpression::Apply(apply) => {
            let args = apply.arguments.iter().map(render_type_expression).collect::<Vec<_>>().join(", ");
            format!("{}<{}>", render_type_expression(&apply.base), args)
        }
    }
}

fn is_legacy_text_type_name(name: &str) -> bool {
    matches!(name, "string" | "str" | "String")
}

fn legacy_builtin_type_alias(name: &str) -> Option<&'static str> {
    match name {
        "sbyte" => Some("i8"),
        "short" => Some("i16"),
        "int" => Some("i32"),
        "long" => Some("i64"),
        "byte" => Some("u8"),
        "ushort" => Some("u16"),
        "uint" => Some("u32"),
        "ulong" => Some("u64"),
        "float" => Some("f32"),
        "double" => Some("f64"),
        "boolean" => Some("bool"),
        _ => None,
    }
}

/// 将内建类型名映射到 `HIR` 类型；未知名字返回 `None`。
///
/// 这里只保留**没有源码声明身份**的编码型内建（文本编码、`bool`、`char`、`unit`/`void`
/// 形态名）；数值原始类型名不是内建：`i8`…`f64` 与 `usize` 一样指 `std/primitive`
/// 的 `structure` 声明身份，由 [`resolve_named_type`] 产出 `Named` 后交给
/// `bind_declared_nominal_types` 冻结成 [`ValkyrieType::Nominal`]。
#[doc(hidden)]
pub fn canonical_builtin_type(name: &str) -> Option<ValkyrieType> {
    match name {
        "bool" => Some(ValkyrieType::Boolean),
        "char" => Some(ValkyrieType::Character),
        "utf8" => Some(ValkyrieType::Utf8),
        "utf16" => Some(ValkyrieType::Utf16),
        "c_str" => Some(ValkyrieType::Named(Identifier::new("c_str"))),
        "unit" => Some(ValkyrieType::Unit),
        "void" => Some(ValkyrieType::Void),
        _ => None,
    }
}

/// 数值 primitive 的拼写到唯一内建变体的映射。
///
/// 这是**唯一**的数值身份表：`HIR` 签名的 `Named` 拼写、`MIR` 归一、canonical 表回退
/// 都必须经 [`primitive_surface_type`] 查询，不得再各自建别名表。
pub fn primitive_numeric_type(name: &str) -> Option<ValkyrieType> {
    match name {
        "i8" => Some(ValkyrieType::Integer8 { signed: true }),
        "u8" => Some(ValkyrieType::Integer8 { signed: false }),
        "i16" => Some(ValkyrieType::Integer16 { signed: true }),
        "u16" => Some(ValkyrieType::Integer16 { signed: false }),
        "i32" => Some(ValkyrieType::Integer32 { signed: true }),
        "u32" => Some(ValkyrieType::Integer32 { signed: false }),
        "i64" => Some(ValkyrieType::Integer64 { signed: true }),
        "u64" => Some(ValkyrieType::Integer64 { signed: false }),
        "i128" => Some(ValkyrieType::Integer128 { signed: true }),
        "u128" => Some(ValkyrieType::Integer128 { signed: false }),
        "f32" => Some(ValkyrieType::Float32),
        "f64" => Some(ValkyrieType::Float64),
        _ => None,
    }
}

/// 把 primitive 的 surface 拼写（内建变体本身、legacy 别名、或规范拼写）归一到
/// 唯一内建变体；非 primitive 返回 `None`。
///
/// 唯一产生点：数值身份 = [`primitive_numeric_type`]，编码/bool/char 身份 =
/// [`canonical_builtin_type`]，legacy 拼写 = [`legacy_builtin_type_alias`]。
/// 下游（`HIR` 证据、`MIR` 归一、canonical 表回退）只查询本函数，不再持有别名表。
pub fn primitive_surface_type(ty: &ValkyrieType) -> Option<ValkyrieType> {
    match ty {
        ValkyrieType::Boolean
        | ValkyrieType::Character
        | ValkyrieType::Integer8 { .. }
        | ValkyrieType::Integer16 { .. }
        | ValkyrieType::Integer32 { .. }
        | ValkyrieType::Integer64 { .. }
        | ValkyrieType::Integer128 { .. }
        | ValkyrieType::Float32
        | ValkyrieType::Float64 => Some(ty.clone()),
        ValkyrieType::Named(name) => {
            let spelling = name.as_str();
            let canonical = legacy_builtin_type_alias(spelling).or_else(|| seed_primitive_alias(spelling)).unwrap_or(spelling);
            canonical_builtin_type(canonical).or_else(|| primitive_numeric_type(canonical))
        }
        _ => None,
    }
}

/// 种子签名里的 primitive surface 拼写（大写首字母一类），与源码 legacy 拼写分开：
/// 源码侧这些名字仍按未声明类型报错，不能借本表悄悄接受。
fn seed_primitive_alias(name: &str) -> Option<&'static str> {
    match name {
        "Bool" => Some("bool"),
        "Int" => Some("i32"),
        "UInt" => Some("u32"),
        "Long" => Some("i64"),
        "ULong" => Some("u64"),
        "Float" => Some("f32"),
        "Double" => Some("f64"),
        "float32" => Some("f32"),
        _ => None,
    }
}

/// 该名字是否为**语言编码型内建**：语言层已有固定 carrier，源码里的同名
/// `[primitive(…)] structure` 只是编码声明的载体，**不建立名义身份**。
///
/// 当前有两个成员，理由不同但结论一致：
/// - `unit` / `void`：语义是「没有值」，`return;` 不带操作数、调用点不产生结果；
///   源码里没有任何可判别的实例状态，冻结成零字段名义类型只会让返回类型是否携带值
///   依赖于「本编译单元是否看得见该声明」。
/// - `bool`：语言层以 `ValkyrieType::Boolean` 承载（字面量、比较内建、分支条件都按它对齐），
///   而 `imply bool` 的成员派发走短名桥接（`hir::overload::trait::impl_nominal_type_name`
///   与 `mir::ssa::hir_identity::nominal_type_name` 都把 `Boolean` 映射为 `bool`），
///   不依赖声明身份；因此冻结成 `Nominal{bool}` 会让同一份 `: bool` 在声明侧与使用侧
///   得到两种 identity，分支条件在 `Semantic MIR` 合同处被拒。
///
/// 数值原始类型（`i8`…`f64`、`usize`、`isize`）**不在**此列：`imply usize` 之类按
/// 声明名派发，而 carrier 会把 `usize` 折叠成 `u64`，抹掉同名原始类型之间的区分；
/// 它们的名义身份必须保留，由 `bind_declared_nominal_types` 冻结。
///
/// 名字解析必须让编码形态优先于同名声明，否则同一类型在「依赖 core 的编译单元」
/// 与「看不到该声明的编译单元」里得到互相矛盾的身份。
fn encoding_builtin_type(name: &str) -> Option<ValkyrieType> {
    match canonical_builtin_type(name) {
        builtin @ Some(ValkyrieType::Boolean | ValkyrieType::Unit | ValkyrieType::Void) => builtin,
        _ => None,
    }
}

/// `i64(x)` / `f64(x)` 等单实参类型名调用：仅数值 builtin，不含 utf8/bool 等（那些走显式转换函数）。
///
/// 这里是**显式转换**的目标形态，与类型名的声明身份无关：源码写出 `i32` 仍指
/// `std/primitive` 的 `structure` 声明，只有转换表达式才需要低层 carrier。
pub(crate) fn numeric_builtin_cast_type(name: &str) -> Option<ValkyrieType> {
    match name {
        "i8" => Some(ValkyrieType::Integer8 { signed: true }),
        "i16" => Some(ValkyrieType::Integer16 { signed: true }),
        "i32" => Some(ValkyrieType::Integer32 { signed: true }),
        "i64" => Some(ValkyrieType::Integer64 { signed: true }),
        "u8" => Some(ValkyrieType::Integer8 { signed: false }),
        "u16" => Some(ValkyrieType::Integer16 { signed: false }),
        "u32" => Some(ValkyrieType::Integer32 { signed: false }),
        "u64" => Some(ValkyrieType::Integer64 { signed: false }),
        "f32" => Some(ValkyrieType::Float32),
        "f64" => Some(ValkyrieType::Float64),
        _ => None,
    }
}

/// 该短名在既有内建表中有身份（`bool`/`char`/`utf8`/`utf16`/`c_str`/`unit`/`void`）。
///
/// 两张表都查：`encoding_builtin_type` 覆盖 `bool`/`unit`/`void` 的编码载体，
/// `canonical_builtin_type` 覆盖文本编码与 `char`/`c_str`。这里只问「有没有内建身份」，
/// 不列名字清单；`usize`/`isize` 等无内建身份的声明族不在此列。
fn has_builtin_type_identity(name: &str) -> bool {
    canonical_builtin_type(name).is_some() || encoding_builtin_type(name).is_some()
}

/// 当前帧里该短名的可见别名条目。
///
/// 生成源码组内，来自依赖导出的别名若与既有内建同名（如 `std.text` 的
/// `type utf8 = Utf8Text`），不参与短名遮蔽：该短名回落到内建事实。只收窄短名解析；
/// 本组自身声明（`imported: false`）照常遮蔽，显式限定路径仍按声明身份解析。
fn short_alias_entry(frame: &ModuleTypeAliasFrame, name: &str) -> Option<ModuleTypeAliasEntry> {
    let entry = frame.aliases.get(name)?.clone();
    if frame.suppress_imported_builtin_short_names && entry.imported && has_builtin_type_identity(name) {
        return None;
    }
    Some(entry)
}

/// 当前作用域内是否声明了该短名的 `type` 别名（含目标尚未填入的本组声明）。
///
/// 这是内建遮蔽的唯一事实来源：源码声明 `type utf8 = Utf8Text` 之后，`utf8` 就不再是内建编码类型。
/// 生成组内依赖导出的同名内建别名按 [`short_alias_entry`] 的判据不参与遮蔽。
fn has_module_type_alias(name: &str) -> bool {
    MODULE_TYPE_ALIASES.with(|stack| stack.borrow().last().and_then(|frame| short_alias_entry(frame, name)).is_some())
}

/// 单段类型名的唯一解析顺序：作用域别名 → 当前声明域泛型形参 → 当前 trait 作用域关联名
/// → 作用域内的名义声明 → 内建类型 → 数值声明引用 → 失败（结构化 TypeNotFound）。
///
/// 别名目标尚未填入（或形参个数不匹配）时保留别名身份为 `ValkyrieType::Named`，
/// 不折算成内建类型：别名是声明身份，不能被同名的内建类型顶替。
///
/// 泛型形参只按声明域事实命中，产出待回填的 `Generic`（`binder: None`），由既有的
/// `propagate_declared_generic_binders` 补上声明 binder；trait 关联名产出以 `Self`
/// 为基的 `Associated`。两者都只匹配**单段名**：多段路径的末段同名不构成可见性，
/// 由调用方通过 `single_segment` 拒绝放宽，保持 namespace/owner 隔离。
///
/// 数值原始类型名（`i8`…`f64`、`usize`、`isize` 与 128 位宽度）属于 `std/primitive`
/// 的 `structure` 声明族：这里只产出待绑定的 `Named`，不按名字生产低层 carrier；
/// 声明身份由 `bind_declared_nominal_types` 冻结，声明不可见时残留 `Named` 由 canonical
/// 以结构化诊断拒绝（CAN003），不会默认成 `Integer32`/`Float64`。
///
/// 无法在作用域内找到别名、也不匹配任何语言内建或数值声明族的名字，以前被错误地 fallback 成
/// `Named` 传递到 canonical 阶段。现在这里直接返回结构化 `TypeNotFound`，由上层
/// 逐层用 `?` 传播到 lowering 主入口。
fn resolve_named_type(name: &str, single_segment: bool) -> Result<ValkyrieType, HirError> {
    if let Some(expanded) = expand_module_type_alias(name, &[], &mut BTreeSet::new()) {
        return Ok(expanded);
    }
    if has_module_type_alias(name) {
        return Ok(ValkyrieType::Named(Identifier::new(name)));
    }
    if single_segment {
        if is_generic_type_param(name) {
            return Ok(ValkyrieType::Generic(GenericType {
                binder: None,
                name: Identifier::new(name),
                kind: HirKind::Type,
                bounds: Vec::new(),
            }));
        }
        if is_trait_associated_name(name) {
            return Ok(ValkyrieType::Associated(Box::new(crate::compiler::types::hir::AssociatedType {
                base: ValkyrieType::SelfType,
                name: Identifier::new(name),
                type_arguments: Vec::new(),
            })));
        }
    }
    // 语言编码型内建优先于同名声明：`unit` / `void` / `bool` 的
    // `[primitive] structure` 只是编码载体，不建立名义身份（详见
    // [`encoding_builtin_type`]）。别名仍然优先，别名是声明身份。
    if let Some(builtin) = encoding_builtin_type(name) {
        return Ok(builtin);
    }
    // 已声明名义类型的短名保留为待冻结的 `Named`；声明身份由 `bind_declared_nominal_types`
    // 按实际结构/枚举冻结，未知名字仍走结构化失败。多段路径的末段同名不构成可见性，
    // 不允许借作用域名义名把限定名折叠成短名。
    if single_segment && is_declared_nominal_type(name) {
        return Ok(ValkyrieType::Named(Identifier::new(name)));
    }
    if let Some(builtin) = canonical_builtin_type(name) {
        return Ok(builtin);
    }
    if is_numeric_type_name(name) {
        return Ok(ValkyrieType::Named(Identifier::new(name)));
    }
    Err(HirError::TypeNotFound {
        name: name.to_string(),
        source: nyar_types::DiagnosticSource { id: "<type_lowering>".into(), revision: None },
        span: DiagnosticSpan { start: 0, end: 0 },
    })
}

fn tuple_labeling_error(failure: TupleLabelingFailure) -> HirError {
    HirError::InvalidTupleLabeling {
        failure,
        source: nyar_types::DiagnosticSource { id: "<type_lowering>".into(), revision: None },
        span: DiagnosticSpan { start: 0, end: 0 },
    }
}

fn array_extent_error(failure: ArrayExtentFailure) -> HirError {
    HirError::InvalidArrayExtent {
        failure,
        source: nyar_types::DiagnosticSource { id: "<type_lowering>".into(), revision: None },
        span: DiagnosticSpan { start: 0, end: 0 },
    }
}

/// 将 `[T; extent]` 的类型位 extent 折成唯一的两种形态。
///
/// 裸整数字面量是栈 `FixedArray<T, N>`；整数元组字面量是堆包装
/// `Array1D<T>` / `Array2D<T>` / `Array3D<T>`（括号即堆），与
/// `Array2D<T, (W, H)>` 落在同一个声明身份上。
///
/// 形状元组只逐维校验为正，不参与类型身份：语言不做 const generic，
/// 元素总数与行主序线性化由包装实例的运行时形状给出。其它 extent 一律
/// 结构化拒绝，不再静默折叠成 0。
fn lower_array_extent(extent: &TermExpression, element: ValkyrieType) -> Result<ValkyrieType, HirError> {
    match extent {
        TermExpression::IntegerLiteral { value, .. } => {
            let length = parse_array_extent_length(value)?;
            Ok(ValkyrieType::FixedArray { element: Box::new(element), length })
        }
        TermExpression::TupleLiteral { items, .. } => {
            let mut dimensions = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    TermExpression::IntegerLiteral { value, .. } => dimensions.push(parse_array_extent_length(value)?),
                    _ => return Err(array_extent_error(ArrayExtentFailure::NotLiteral)),
                }
            }
            if dimensions.contains(&0) {
                return Err(array_extent_error(ArrayExtentFailure::ZeroDimension));
            }
            let name = match dimensions.len() {
                1 => "Array1D",
                2 => "Array2D",
                3 => "Array3D",
                count => return Err(array_extent_error(ArrayExtentFailure::UnsupportedDimensionCount { count })),
            };
            Ok(ValkyrieType::Apply(Box::new(ValkyrieType::Named(Identifier::new(name))), vec![element]))
        }
        TermExpression::Unary(unary)
            if unary.operator == ValkyrieTokenType::Minus && matches!(&unary.base, TermExpression::IntegerLiteral { .. }) =>
        {
            Err(array_extent_error(ArrayExtentFailure::NegativeLength))
        }
        _ => Err(array_extent_error(ArrayExtentFailure::NotLiteral)),
    }
}

fn parse_array_extent_length(value: &str) -> Result<usize, HirError> {
    value.parse::<usize>().map_err(|_| array_extent_error(ArrayExtentFailure::LengthNotRepresentable))
}

fn render_array_extent(extent: &TermExpression) -> String {
    match extent {
        TermExpression::IntegerLiteral { value, .. } => value.clone(),
        TermExpression::TupleLiteral { items, .. } => {
            let rendered: Vec<String> = items.iter().map(render_array_extent).collect();
            format!("({})", rendered.join(", "))
        }
        _ => "?".to_string(),
    }
}

fn expand_module_type_alias(name: &str, arguments: &[ValkyrieType], visiting: &mut BTreeSet<String>) -> Option<ValkyrieType> {
    if visiting.contains(name) {
        return None;
    }
    MODULE_TYPE_ALIASES.with(|stack| {
        let entry = short_alias_entry(stack.borrow().last()?, name)?;
        if entry.params.len() != arguments.len() {
            return None;
        }
        let target = entry.target?;
        visiting.insert(name.to_string());
        let substituted = if entry.params.is_empty() {
            target
        }
        else {
            let mapping: BTreeMap<String, ValkyrieType> = entry.params.into_iter().zip(arguments.iter().cloned()).collect();
            substitute_named_type_params(&target, &mapping)
        };
        let expanded = expand_type_aliases(&substituted, visiting);
        visiting.remove(name);
        Some(expanded)
    })
}

/// 限定路径的别名展开：namespace 段 + 短名必须与条目的声明身份逐段相等。
///
/// 末段同名但 namespace 不同的条目不算命中；环检测用完整路径作 key。
fn expand_qualified_module_type_alias(parts: &[&str], arguments: &[ValkyrieType], visiting: &mut BTreeSet<String>) -> Option<ValkyrieType> {
    let (name, namespace) = parts.split_last()?;
    let key = parts.join(".");
    if visiting.contains(&key) {
        return None;
    }
    MODULE_TYPE_ALIASES.with(|stack| {
        let entry = stack.borrow().iter().rev().find_map(|frame| {
            frame.aliases.get(*name).filter(|entry| entry.namespace.iter().map(String::as_str).eq(namespace.iter().copied())).cloned()
        })?;
        if entry.params.len() != arguments.len() {
            return None;
        }
        let target = entry.target?;
        visiting.insert(key.clone());
        let substituted = if entry.params.is_empty() {
            target
        }
        else {
            let mapping: BTreeMap<String, ValkyrieType> = entry.params.into_iter().zip(arguments.iter().cloned()).collect();
            substitute_named_type_params(&target, &mapping)
        };
        let expanded = expand_type_aliases(&substituted, visiting);
        visiting.remove(&key);
        Some(expanded)
    })
}

fn substitute_named_type_params(ty: &ValkyrieType, mapping: &BTreeMap<String, ValkyrieType>) -> ValkyrieType {
    match ty {
        ValkyrieType::Named(name) => mapping.get(name.as_str()).cloned().unwrap_or_else(|| ty.clone()),
        ValkyrieType::Apply(base, args) => ValkyrieType::Apply(
            Box::new(substitute_named_type_params(base, mapping)),
            args.iter().map(|arg| substitute_named_type_params(arg, mapping)).collect(),
        ),
        ValkyrieType::Array(inner) => ValkyrieType::Array(Box::new(substitute_named_type_params(inner, mapping))),
        ValkyrieType::Nullable(inner) => ValkyrieType::Nullable(Box::new(substitute_named_type_params(inner, mapping))),
        ValkyrieType::Tuple(items) => ValkyrieType::Tuple(items.iter().map(|item| substitute_named_type_params(item, mapping)).collect()),
        ValkyrieType::Union(items) => ValkyrieType::Union(items.iter().map(|item| substitute_named_type_params(item, mapping)).collect()),
        ValkyrieType::Intersection(items) => {
            ValkyrieType::Intersection(items.iter().map(|item| substitute_named_type_params(item, mapping)).collect())
        }
        ValkyrieType::Function(function) => ValkyrieType::Function(Box::new(crate::compiler::types::hir::FunctionType {
            params: function.params.iter().map(|param| substitute_named_type_params(param, mapping)).collect(),
            return_type: substitute_named_type_params(&function.return_type, mapping),
        })),
        _ => ty.clone(),
    }
}

fn expand_type_aliases(ty: &ValkyrieType, visiting: &mut BTreeSet<String>) -> ValkyrieType {
    match ty {
        ValkyrieType::Named(name) => {
            if let Some(expanded) = expand_module_type_alias(name.as_str(), &[], visiting) {
                expanded
            }
            else {
                ty.clone()
            }
        }
        ValkyrieType::Apply(base, args) => {
            let expanded_args: Vec<ValkyrieType> = args.iter().map(|arg| expand_type_aliases(arg, visiting)).collect();
            if let ValkyrieType::Named(name) = base.as_ref() {
                if let Some(expanded) = expand_module_type_alias(name.as_str(), &expanded_args, visiting) {
                    return expanded;
                }
            }
            ValkyrieType::Apply(Box::new(expand_type_aliases(base, visiting)), expanded_args)
        }
        ValkyrieType::Array(inner) => ValkyrieType::Array(Box::new(expand_type_aliases(inner, visiting))),
        ValkyrieType::Nullable(inner) => ValkyrieType::Nullable(Box::new(expand_type_aliases(inner, visiting))),
        ValkyrieType::Tuple(items) => ValkyrieType::Tuple(items.iter().map(|item| expand_type_aliases(item, visiting)).collect()),
        ValkyrieType::Union(items) => ValkyrieType::Union(items.iter().map(|item| expand_type_aliases(item, visiting)).collect()),
        ValkyrieType::Intersection(items) => ValkyrieType::Intersection(items.iter().map(|item| expand_type_aliases(item, visiting)).collect()),
        ValkyrieType::Function(function) => ValkyrieType::Function(Box::new(crate::compiler::types::hir::FunctionType {
            params: function.params.iter().map(|param| expand_type_aliases(param, visiting)).collect(),
            return_type: expand_type_aliases(&function.return_type, visiting),
        })),
        ValkyrieType::LabeledTuple(fields) => ValkyrieType::LabeledTuple(
            fields.iter().map(|field| HirTupleField { name: field.name.clone(), ty: expand_type_aliases(&field.ty, visiting) }).collect(),
        ),
        _ => ty.clone(),
    }
}

fn lower_type_namepath(path: &AstNamePath) -> Result<ValkyrieType, HirError> {
    // 只消费 Oak 产出的结构化 NamePath；禁止按 span 切片重解析源码。
    let parts = path.parts.iter().map(|part| part.name.as_str()).collect::<Vec<_>>();
    let last = parts.last().copied().unwrap_or_default();
    if parts.len() == 1 && last == "Self" {
        return Ok(ValkyrieType::SelfType);
    }
    if parts.first().is_some_and(|part| *part == "Self") {
        let mut base = ValkyrieType::SelfType;
        for name in parts.iter().skip(1) {
            base = ValkyrieType::Associated(Box::new(crate::compiler::types::hir::AssociatedType {
                base,
                name: Identifier::new(name),
                type_arguments: Vec::new(),
            }));
        }
        return Ok(base);
    }
    // 限定类型名按声明身份解析：先展开同名 namespace 的别名，再匹配同名 namespace 的
    // 名义声明。两者都要求 namespace 段逐段相等，禁止只按末段短名命中。
    if parts.len() > 1 {
        if let Some(expanded) = expand_qualified_module_type_alias(&parts, &[], &mut BTreeSet::new()) {
            return Ok(expanded);
        }
        // `std::primitive::unit` / `std::primitive::bool` 等编码形态的限定名，
        // 与短名同规则：声明可见也不建立名义身份。
        if is_known_builtin_type_namespace(&parts, last) {
            if let Some(builtin) = encoding_builtin_type(last) {
                return Ok(builtin);
            }
        }
        if is_declared_qualified_nominal_type(&parts) {
            return Ok(ValkyrieType::Named(Identifier::new(last)));
        }
    }
    // `Type::Assoc`：两段且非已知命名空间前缀时保留 Associated，禁止丢前缀只留短名。
    // `std::…` 等限定路径仍走下方短名/builtin 规则，供 bind 按声明冻结。
    if parts.len() == 2 && !is_known_builtin_type_namespace(&parts, last) && !is_module_root_prefix(parts[0]) {
        let head = parts[0];
        return Ok(ValkyrieType::Associated(Box::new(crate::compiler::types::hir::AssociatedType {
            base: resolve_named_type(head, true)?,
            name: Identifier::new(last),
            type_arguments: Vec::new(),
        })));
    }
    // 停线清理：删除 QualifiedPath 作为语法类型进入 HIR 的路径。多段限定类型名
    // 在 frontend 绑定阶段必须已完全解析为声明身份；无法解析时必须返回前端
    // TypeError，不得带着 QualifiedPath 旁路进入后续 canonical / representation。
    //
    // 当前所有非内建、非 Self、非两段 Associated 的多段名直接按最后一段短名交给
    // resolve_named_type 处理；无法在当前作用域绑定则返回 TypeNotFound，
    // 不再静默 fallback 成 Named。
    // 只有单段名属于当前声明域的形参/关联名可见范围；多段路径的末段同名不放宽。
    Ok(resolve_named_type(last, parts.len() == 1)?)
}

fn is_module_root_prefix(name: &str) -> bool {
    matches!(name, "std" | "self")
}

fn is_known_builtin_type_namespace(parts: &[&str], last: &str) -> bool {
    let prefix = &parts[..parts.len().saturating_sub(1)];
    match prefix {
        ["std", "primitive"] => canonical_builtin_type(last).is_some(),
        ["std", "text"] => matches!(last, "char" | "utf8" | "utf16" | "c_str"),
        _ => false,
    }
}

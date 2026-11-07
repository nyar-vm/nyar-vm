//! Unified parameter matching entry points for the current `HIR` layer.
//!
//! This module centralizes the settled matching categories used by overload
//! resolution and future call checking:
//! - nominal parameter matching
//! - named trait satisfaction
//! - anonymous row requirement satisfaction

use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::{
    hir::{
        nominal::{NominalModuleError, NominalModuleView},
        row::{RowMethodSignature, RowRequirement, RowRequirementError},
        trait_system::{NamedTraitWitness, TraitModuleError, TraitModuleView},
    },
    types::{
        Identifier, NamePath,
        hir::{GenericBinderId, HirAssociatedTypeBinding, HirEnum, HirModule, HirStruct, HirTraitBound, HirWhereConstraint, RowType, TraitObject, ValkyrieType, primitive_identity_builtin_type},
    },
};

/// 类型关系判定失败时携带的诊断种子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeRelationDiagnosticSeed {
    /// 两侧类型互不相容。
    IncompatibleTypes {
        /// 实参类型。
        actual: ValkyrieType,
        /// 期望类型。
        expected: ValkyrieType,
    },
    /// 实参类型形态不受支持。
    UnsupportedActualType {
        /// 不受支持的实参类型。
        actual: ValkyrieType,
    },
    /// 期望类型形态不受支持。
    UnsupportedExpectedType {
        /// 不受支持的期望类型。
        expected: ValkyrieType,
    },
    /// 名义类型判定失败。
    Nominal {
        /// 名义模块层错误。
        error: NominalModuleError,
    },
    /// 具名 trait 判定失败。
    Trait {
        /// trait 模块层错误。
        error: TraitModuleError,
    },
    /// 匿名 row 判定失败。
    Row {
        /// 全部 row 校验失败原因。
        errors: Vec<RowRequirementError>,
    },
}

/// 单个形参的匹配结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParameterMatchResult {
    /// 名义类型完全一致。
    NominalExact,
    /// 名义子类型，距离越小越接近。
    NominalSubtype {
        /// 继承距离。
        distance: usize,
    },
    /// 通过具名 trait 满足，携带 witness。
    Trait {
        /// 生成的 trait witness。
        witness: NamedTraitWitness,
    },
    /// 通过匿名 row 形状满足。
    Row,
    /// 不匹配，携带诊断种子。
    NoMatch {
        /// 失败诊断种子。
        diagnostic_seed: TypeRelationDiagnosticSeed,
    },
}

/// 一条模块级导入绑定：`using module::{imported as local}` 或 glob 导入。
///
/// 自由函数调用在无限定形式下必须能命中显式导入的声明身份；只按
/// `enclosing_module + name` 相等匹配会丢掉 `using!` 导入的事实。
#[derive(Debug, Clone)]
struct ImportedSymbol {
    module: NamePath,
    glob: bool,
    local: Identifier,
    imported: Identifier,
}

/// 类型关系判定上下文：模块的名义、trait 与可调用约束视图。
#[derive(Debug, Clone)]
pub struct TypeRelationContext {
    nominal: NominalModuleView,
    traits: TraitModuleView,
    structs: BTreeMap<Identifier, HirStruct>,
    structs_by_declaration: BTreeMap<nyar_types::ItemId, HirStruct>,
    enums: BTreeMap<Identifier, HirEnum>,
    enums_by_declaration: BTreeMap<nyar_types::ItemId, HirEnum>,
    callable_constraints: Vec<HirWhereConstraint>,
    /// 当前正在解析调用的模块路径；无限定自由函数按结构化路径匹配，禁止末段回退。
    enclosing_module: NamePath,
    /// 模块级 `using` 导入绑定：无限定调用只有命中这些绑定才接受导入目标。
    imported_symbols: Vec<ImportedSymbol>,
    /// 同一源码组内的命名空间清单：组内各命名空间共享文件作用域。
    ///
    /// 该清单只供无限定自由函数通道使用，**不**并入 `imported_symbols`：成员方法派发
    /// 只认类型自身方法、trait 方法与显式导入/依赖导出的扩展 micro，组作用域不得把
    /// 同组其它命名空间的同名函数拉进方法候选池。
    group_scope_namespaces: Vec<NamePath>,
    /// 当前正在解析的函数所属 impl 的目标类型；`let` 声明里写 `Self` 时按此代入。
    impl_owner: Option<ValkyrieType>,
    /// 当前正在解析的函数所属 trait 声明身份；默认体里的 `self` 只能是本 trait 的
    /// 方法槽，不许回退到全局同名候选池。
    trait_self_scope: Option<nyar_types::ItemId>,
}

impl TypeRelationContext {
    /// 由模块（含子模块与导入语义导出）构建判定上下文。
    pub fn from_module(module: &HirModule) -> Self {
        let mut structs = BTreeMap::new();
        let mut structs_by_declaration = BTreeMap::new();
        let mut enums = BTreeMap::new();
        let mut enums_by_declaration = BTreeMap::new();
        collect_relation_structs(module, &mut structs, &mut structs_by_declaration);
        collect_relation_enums(module, &mut enums, &mut enums_by_declaration);
        let mut imported_symbols = Vec::new();
        collect_relation_imports(module, &mut imported_symbols);
        let mut group_scope_namespaces = Vec::new();
        collect_group_scope_namespaces(module, &mut group_scope_namespaces);
        Self {
            nominal: NominalModuleView::from_module(module),
            traits: TraitModuleView::from_module(module),
            structs,
            structs_by_declaration,
            enums,
            enums_by_declaration,
            callable_constraints: Vec::new(),
            enclosing_module: module.name.clone(),
            imported_symbols,
            group_scope_namespaces,
            impl_owner: None,
            trait_self_scope: None,
        }
    }

    /// 记录当前 impl 的目标类型，供方法体内 `Self` 声明代入。
    pub(crate) fn with_impl_owner(&self, owner: ValkyrieType) -> Self {
        Self { impl_owner: Some(owner), ..self.clone() }
    }

    /// 当前 impl 的目标类型。
    pub(crate) fn impl_owner(&self) -> Option<&ValkyrieType> {
        self.impl_owner.as_ref()
    }

    /// 记录当前 trait 声明作用域；默认体里的 `self` 绑定与成员查找据此收敛到本 trait。
    pub(crate) fn with_trait_self_scope(&self, declaration: nyar_types::ItemId) -> Self {
        Self { trait_self_scope: Some(declaration), ..self.clone() }
    }

    /// 候选的 trait 归属是否正是当前声明作用域。
    pub(crate) fn is_trait_self_scope(&self, trait_owner: Option<nyar_types::ItemId>) -> bool {
        self.trait_self_scope.is_some() && self.trait_self_scope == trait_owner
    }

    /// 语言级迭代协议 trait 的声明身份（`std.iterator::{IntoIterator, Iterator}`）。
    pub(crate) fn protocol_trait_declaration(&self, name: &str) -> Option<nyar_types::ItemId> {
        self.traits.declaration_at_namespace(&crate::compiler::semantic::operator_members::FOR_IN_PROTOCOL_NAMESPACE, name)
    }

    /// 无限定自由函数是否命中一条显式 `using` 导入绑定。
    ///
    /// - 选择性导入：候选符号必须等于 `module + imported`，且调用名等于本地名；
    /// - glob 导入：候选符号以导入模块为前缀，调用名比较末段。
    ///
    /// 这里只承认模块声明的导入事实，不做跨命名空间的末段回退。
    pub(crate) fn symbol_matches_import(&self, symbol: &NamePath, callee_name: &Identifier) -> bool {
        self.imported_symbols.iter().any(|imported| {
            if imported.glob {
                let module_parts = imported.module.parts();
                !module_parts.is_empty()
                    && symbol.parts().len() > module_parts.len()
                    && symbol.parts().get(..module_parts.len()) == Some(module_parts)
                    && symbol.parts().last() == Some(callee_name)
            }
            else {
                let module_parts = imported.module.parts();
                &imported.local == callee_name
                    && symbol.parts().len() == module_parts.len() + 1
                    && symbol.parts().get(..module_parts.len()) == Some(module_parts)
                    && symbol.parts().last() == Some(&imported.imported)
            }
        })
    }

    /// 无限定自由函数是否命中同一源码组内的文件作用域。
    ///
    /// 一个源码组就是一次 package 编译单元：组内任一命名空间的顶层声明都能被组内
    /// 其它模块以单段短名引用。候选符号必须落在组作用域前缀之下、末段等于调用名；
    /// 跨组路径不会命中，不做末段回退。
    ///
    /// 该谓词只服务自由函数通道，**不**参与成员方法派发：`x.method()` 的第三层扩展
    /// micro 只认显式导入与依赖导出。
    pub(crate) fn symbol_matches_group_scope(&self, symbol: &NamePath, callee_name: &Identifier) -> bool {
        self.group_scope_namespaces.iter().any(|namespace| {
            let namespace_parts = namespace.parts();
            !namespace_parts.is_empty()
                && symbol.parts().len() > namespace_parts.len()
                && symbol.parts().get(..namespace_parts.len()) == Some(namespace_parts)
                && symbol.parts().last() == Some(callee_name)
        })
    }

    /// 限定当前模块作用域，供无限定自由函数 overload 解析使用。
    pub(crate) fn with_enclosing_module(&self, module: &NamePath) -> Self {
        let mut context = self.clone();
        context.enclosing_module = module.clone();
        context
    }

    pub(crate) fn enclosing_module(&self) -> &NamePath {
        &self.enclosing_module
    }

    /// 短名 → 已冻结的 `Nominal{ItemId}`（有 declaration 时）；供 Self/AutoType 回收，禁止只写回 Named。
    /// `[primitive]` 声明折叠回唯一内建身份，与注解路径产出同一类型。
    pub fn nominal_type_for_name(&self, name: &Identifier) -> Option<ValkyrieType> {
        if let Some(item) = self.structs.get(name) {
            if let Some(builtin) = item.primitive_identity.as_deref().and_then(primitive_identity_builtin_type) {
                return Some(builtin);
            }
            return Some(ValkyrieType::Nominal {
                declaration: item.constructor_declaration?,
                arguments: Vec::new(),
                primitive_identity: item.primitive_identity.clone(),
            });
        }
        let item = self.enums.get(name)?;
        Some(ValkyrieType::Nominal { declaration: item.declaration?, arguments: Vec::new(), primitive_identity: None })
    }

    /// owner 名与 `Nominal` declaration 的身份核对：`impl_nominal_type_name` 对
    /// `Nominal` 无表面名，receiver 已冻结为 `Nominal{ItemId}` 时必须按 declaration
    /// 反查 owner，否则泛型方法调用的 owner 实参无从播种。
    pub fn owner_name_matches_declaration(&self, owner: &Identifier, declaration: nyar_types::ItemId) -> bool {
        self.structs_by_declaration.get(&declaration).is_some_and(|item| item.name == *owner)
            || self.enums_by_declaration.get(&declaration).is_some_and(|item| item.name == *owner)
    }

    /// 形参槽位是否为「声明含同名无载荷变体」的具体 `unite` 类型。
    ///
    /// 只认已冻结的声明身份与已解析的短名：`Generic` / `AutoType` 等开放槽位没有
    /// 可判定的变体清单，必须返回 `false`，由调用方保持结构化失败。
    pub fn is_unit_variant_of(&self, expected: &ValkyrieType, name: &Identifier) -> bool {
        let item = match expected {
            ValkyrieType::Nominal { declaration, .. } => self.enums_by_declaration.get(declaration),
            ValkyrieType::Named(type_name) => self.enums.get(type_name),
            ValkyrieType::Apply(base, _) => return self.is_unit_variant_of(base, name),
            _ => return false,
        };
        item.is_some_and(|item| item.variants.iter().any(|variant| &variant.name == name && variant.fields.is_empty()))
    }

    /// 字面量特化用：数值形参只认已冻结的 `[primitive]` 身份。
    ///
    /// 短名不参与判定：名叫 `i32` 的普通 class 不是数值类型；未结算别名的
    /// `Named("usize")` 同样不算，留给别名展开或失败关闭，禁止按名字猜。
    pub fn is_numeric_type(&self, ty: &ValkyrieType) -> bool {
        match ty {
            ValkyrieType::Nominal { declaration, .. } => self
                .structs_by_declaration
                .get(declaration)
                .and_then(|item| item.primitive_identity.as_deref())
                .and_then(primitive_identity_builtin_type)
                .is_some_and(|builtin| is_numeric_type_name_of(&builtin)),
            other => is_numeric_type_name_of(other),
        }
    }

    /// 字符串字面量可特化到的形参类型：`Utf8`/`Utf16` 原语、`Named` 文本拼写，
    /// 以及经别名解析后的 `Nominal{ItemId}`（`type utf8 = Utf8Text` 的目标声明）。
    /// 与 `is_numeric_type` 同源：名字集合按 declaration 冻结身份核对，不按拼写猜。
    pub fn is_text_formal(&self, ty: &ValkyrieType) -> bool {
        match ty {
            ValkyrieType::Nominal { declaration, .. } => {
                self.structs_by_declaration.get(declaration).is_some_and(|item| is_text_type_name(item.name.as_str()))
            }
            other => crate::compiler::hir::overload::types::is_text_type(other),
        }
    }

    /// 内建文本编码 carrier 与源码别名锚定的文本名义声明之间的身份归一。
    ///
    /// 源码 `type utf8 = Utf8Text` 把编码锚定到名义声明上；生成组按 §4 规则不回填
    /// 该别名，`utf8` 位置回落到内建 carrier `Utf8`（`utf16` 同理）。两者是同一值空间，
    /// 与 MIR 的 `text_encoding_matches_nominal` 同源。只归一同一编码：`Utf8` 不接受
    /// `Utf16Text`，反之亦然；带泛型实参的名义声明不在此列。
    pub(crate) fn text_encoding_matches_nominal(&self, actual: &ValkyrieType, expected: &ValkyrieType) -> bool {
        let ValkyrieType::Nominal { declaration, arguments, .. } = expected else {
            return false;
        };
        if !arguments.is_empty() {
            return false;
        }
        let Some(item) = self.structs_by_declaration.get(declaration) else {
            return false;
        };
        let name = item.name.as_str();
        match actual {
            // 与 MIR `text_encoding_matches_nominal` 同一名集：`text` 是两种编码共享的载体。
            ValkyrieType::Utf8 => matches!(name, "text" | "utf8" | "Utf8Text" | "std::text::text" | "std.text.text"),
            ValkyrieType::Utf16 => matches!(name, "text" | "utf16" | "Utf16Text" | "std::text::text" | "std.text.text"),
            _ => false,
        }
    }

    /// 字符字面量可特化到的形参类型：`char` 自身，或任意数值形参（码位值参与比较/算术）。
    ///
    /// 与整数字面量同源：值域校验（码位 ≤ 0xFFFF）已在 HIR lowering 完成，
    /// 窄形参（如 `u8`）是否容得下具体值沿用整数字面量同一缺口，不在此特判。
    pub fn is_character_literal_formal(&self, ty: &ValkyrieType) -> bool {
        is_character_type_name_of(ty) || self.is_numeric_type(ty)
    }

    /// 字面量特化用：只有浮点形参接受浮点字面量。
    ///
    /// 整数形参不是浮点类型，浮点字面量不得按“同属数值”静默塞入整数类型；与
    /// [`Self::is_numeric_type`] 同源：carrier、声明名与源码写的声明引用共用一份浮点名集合。
    pub fn is_float_type(&self, ty: &ValkyrieType) -> bool {
        match ty {
            ValkyrieType::Nominal { declaration, .. } => {
                self.structs_by_declaration.get(declaration).is_some_and(|item| is_float_type_name(item.name.as_str()))
            }
            ValkyrieType::Float32 | ValkyrieType::Float64 => true,
            ValkyrieType::Named(name) => is_float_type_name(name.as_str()),
            _ => false,
        }
    }

    /// 派生一份带可调用声明级 where 约束的上下文副本。
    pub fn with_callable_constraints(&self, constraints: &[HirWhereConstraint]) -> Self {
        let mut context = self.clone();
        context.callable_constraints = constraints.to_vec();
        context
    }

    /// 当前上下文携带的可调用 where 约束。
    pub fn callable_constraints(&self) -> &[HirWhereConstraint] {
        &self.callable_constraints
    }

    /// 统一形参匹配入口：按 nominal、trait、row 的既定优先级返回结果。
    pub fn match_parameter(&self, actual: &ValkyrieType, expected: &ValkyrieType) -> ParameterMatchResult {
        if trivially_compatible(actual, expected) {
            return ParameterMatchResult::NominalExact;
        }

        // 生成组的内建编码 carrier 与依赖侧 `type utf8 = Utf8Text` 锚定的名义声明同一值空间。
        if self.text_encoding_matches_nominal(actual, expected) {
            return ParameterMatchResult::NominalExact;
        }

        // `Nominal{ItemId}` 只认 declaration；同 declaration + 实参相容即为 exact。
        if let (
            ValkyrieType::Nominal { declaration: actual_decl, arguments: actual_args, .. },
            ValkyrieType::Nominal { declaration: expected_decl, arguments: expected_args, .. },
        ) = (actual, expected)
        {
            if actual_decl == expected_decl
                && actual_args.len() == expected_args.len()
                && actual_args.iter().zip(expected_args.iter()).all(|(left, right)| self.matches_non_error(left, right))
            {
                return ParameterMatchResult::NominalExact;
            }
        }

        if let (ValkyrieType::Generic(actual), ValkyrieType::Generic(expected)) = (actual, expected) {
            if actual.binder.is_some() && actual.binder == expected.binder {
                return ParameterMatchResult::NominalExact;
            }
        }

        // Nominal applications retain their type arguments at the semantic
        // boundary.  Match the constructor nominally and each argument using
        // the same parameter relation, so exported `Choice<T, E>` can accept
        // a consumer-side `Choice<i32, utf8>` without erasing either generic
        // argument or falling back to a backend representation.
        if let (ValkyrieType::Apply(actual_base, actual_args), ValkyrieType::Apply(expected_base, expected_args)) = (actual, expected) {
            if actual_args.len() == expected_args.len()
                && matches!(
                    self.match_parameter(actual_base, expected_base),
                    ParameterMatchResult::NominalExact | ParameterMatchResult::NominalSubtype { .. }
                )
                && actual_args.iter().zip(expected_args).all(|(actual_arg, expected_arg)| {
                    !matches!(self.match_parameter(actual_arg, expected_arg), ParameterMatchResult::NoMatch { .. })
                })
            {
                return ParameterMatchResult::NominalExact;
            }
        }

        match expected {
            ValkyrieType::Union(expected_items) => {
                if expected_items.iter().any(|item| self.matches_non_error(actual, item)) {
                    return ParameterMatchResult::NominalExact;
                }
                if let ValkyrieType::Union(actual_items) = actual {
                    let all_assignable = actual_items
                        .iter()
                        .all(|actual_item| expected_items.iter().any(|expected_item| self.matches_non_error(actual_item, expected_item)));
                    if all_assignable {
                        return ParameterMatchResult::NominalExact;
                    }
                }
            }
            ValkyrieType::Intersection(expected_items) => {
                let all_assignable = expected_items.iter().all(|item| self.matches_non_error(actual, item));
                if all_assignable {
                    return ParameterMatchResult::NominalExact;
                }
            }
            ValkyrieType::Named(expected_name) => {
                if let Some(actual_name) = named_type_name(actual) {
                    let nominal = self.match_nominal_parameter(&actual_name, expected_name);
                    match &nominal {
                        ParameterMatchResult::NominalExact | ParameterMatchResult::NominalSubtype { .. } => return nominal,
                        ParameterMatchResult::NoMatch {
                            diagnostic_seed: TypeRelationDiagnosticSeed::Nominal { error: NominalModuleError::UnknownType { .. } },
                        } => {
                            if let Some(result) = self.match_named_trait_parameter(&actual_name, expected_name) {
                                return result;
                            }
                        }
                        ParameterMatchResult::NoMatch { diagnostic_seed: TypeRelationDiagnosticSeed::Nominal { .. } } => return nominal,
                        ParameterMatchResult::NoMatch { .. } => {
                            if let Some(result) = self.match_named_trait_parameter(&actual_name, expected_name) {
                                return result;
                            }
                            return nominal;
                        }
                        ParameterMatchResult::Trait { .. } | ParameterMatchResult::Row => {
                            unreachable!("nominal matching should not yield trait/row")
                        }
                    }
                }
            }
            ValkyrieType::TraitObject(TraitObject { trait_path, .. }) => {
                if let Some(actual_name) = named_type_name(actual) {
                    return self.match_named_trait_parameter(&actual_name, trait_path).unwrap_or(ParameterMatchResult::NoMatch {
                        diagnostic_seed: TypeRelationDiagnosticSeed::IncompatibleTypes { actual: actual.clone(), expected: expected.clone() },
                    });
                }
            }
            ValkyrieType::Row(row) => {
                if let Some(actual_name) = named_type_name(actual) {
                    return self.match_row_requirement(&actual_name, &row_type_to_requirement(row));
                }
            }
            _ => {}
        }

        ParameterMatchResult::NoMatch {
            diagnostic_seed: TypeRelationDiagnosticSeed::IncompatibleTypes { actual: actual.clone(), expected: expected.clone() },
        }
    }

    /// 按名义继承关系匹配两个类型短名。
    pub fn match_nominal_parameter(&self, actual_name: &Identifier, expected_name: &Identifier) -> ParameterMatchResult {
        match self.nominal.nominal_match_distance(actual_name, expected_name) {
            Ok(Some(0)) => ParameterMatchResult::NominalExact,
            Ok(Some(distance)) => ParameterMatchResult::NominalSubtype { distance },
            Ok(None) => ParameterMatchResult::NoMatch {
                diagnostic_seed: TypeRelationDiagnosticSeed::IncompatibleTypes {
                    actual: ValkyrieType::Named(actual_name.clone()),
                    expected: ValkyrieType::Named(expected_name.clone()),
                },
            },
            Err(error) => ParameterMatchResult::NoMatch { diagnostic_seed: TypeRelationDiagnosticSeed::Nominal { error } },
        }
    }

    /// 按具名 trait 满足关系匹配；trait 未知时返回 `None`。
    pub fn match_named_trait_parameter(&self, actual_name: &Identifier, trait_name: &Identifier) -> Option<ParameterMatchResult> {
        match self.traits.satisfy_named_trait(actual_name, trait_name) {
            Ok(witness) => Some(ParameterMatchResult::Trait { witness }),
            Err(TraitModuleError::UnknownTrait { .. }) => None,
            Err(error) => Some(ParameterMatchResult::NoMatch { diagnostic_seed: TypeRelationDiagnosticSeed::Trait { error } }),
        }
    }

    /// 在泛型实参完成代入后验证 callable 的声明级 where 合同。
    ///
    /// 约束按 `where` 目标分三类处理：
    /// - 调用方上下文已有同一目标的约束：必须由该约束提供 evidence（不足即拒绝）；
    /// - 目标已代入为具体类型：必须存在显式 impl evidence，否则拒绝；
    /// - 目标仍含调用方的刚性泛型参数（未代入）：这是调用方**尚未履行**的义务，
    ///   只能在该实例按具体类型实例化时结算，解析阶段不得据此把调用降成未解析。
    pub fn satisfies_where_constraints(
        &self,
        constraints: &[HirWhereConstraint],
        substitutions: &BTreeMap<GenericBinderId, ValkyrieType>,
    ) -> bool {
        constraints.iter().all(|constraint| {
            let target = substitute_relation_type(&constraint.target, substitutions);
            constraint.bounds.iter().all(|bound| {
                if let Some(context) = self.callable_constraints.iter().find(|context| {
                    let context_target = substitute_relation_type(&context.target, substitutions);
                    context_target == target
                }) {
                    if bound_is_satisfied_by_context(bound, context, &self.callable_constraints, substitutions) {
                        return true;
                    }
                    // 同一目标已有调用方约束、但不提供本 bound 时，不能就此拒绝：
                    // 若目标仍是开放刚性泛型参数，本 bound 是调用方尚未履行的义务，
                    // 必须留到具体类型实例化时结算（见 `instantiate_generic_callables`）。
                    return relation_type_is_open_obligation(&target);
                }
                // witness 结算必须看到调用侧的实参代入：`where I: Iterator<Item = T>`
                // 的关联等式右侧是调用方 binder，未代入时 witness 只能看到开放 binder，
                // 与 impl 冻结的具体关联类型永远不等。先把 bound 按当前 substitutions
                // 落成目标形态再取 evidence，证据链与 impl 声明仍然逐项核对。
                let substituted_bound = HirTraitBound {
                    declaration: bound.declaration,
                    trait_path: bound.trait_path.clone(),
                    type_arguments: bound.type_arguments.iter().map(|ty| substitute_relation_type(ty, substitutions)).collect(),
                    associated_types: bound
                        .associated_types
                        .iter()
                        .map(|equation| HirAssociatedTypeBinding {
                            name: equation.name.clone(),
                            ty: substitute_relation_type(&equation.ty, substitutions),
                        })
                        .collect(),
                };
                let Ok(witness) = self.traits.satisfy_explicit_type(&target, &substituted_bound)
                else {
                    return relation_type_is_open_obligation(&target);
                };
                bound.associated_types.iter().all(|equation| {
                    let expected = self.traits.canonical_member_type(&substitute_relation_type(&equation.ty, substitutions));
                    let actual = witness.associated_types.get(&equation.name).map(|actual| self.traits.canonical_member_type(actual));
                    actual.as_ref() == Some(&expected)
                })
            })
        })
    }

    /// 关联类型驱动的 binder 推断：`where I: Iterator<Item = T>` 中 `I` 已由接收者
    /// 确定、`T` 只出现在关联等式右侧时，用 impl witness 冻结的关联类型补出 `T`。
    /// 目标仍是开放刚性参数时，只从调用方已声明且携带关联等式的 where 约束取解；
    /// 没有这条声明依据时不做（那是调用方尚未履行的义务，留给实例化结算）。
    pub fn infer_associated_binders(
        &self,
        constraints: &[HirWhereConstraint],
        substitutions: &mut BTreeMap<GenericBinderId, ValkyrieType>,
        binders: &BTreeSet<GenericBinderId>,
    ) {
        for constraint in constraints {
            let target = substitute_relation_type(&constraint.target, substitutions);
            let open = relation_type_is_open_obligation(&target);
            for bound in &constraint.bounds {
                if bound.associated_types.is_empty() {
                    continue;
                }
                let bound_without_equations = HirTraitBound {
                    declaration: bound.declaration,
                    trait_path: bound.trait_path.clone(),
                    type_arguments: bound.type_arguments.iter().map(|ty| substitute_relation_type(ty, substitutions)).collect(),
                    associated_types: Vec::new(),
                };
                // 目标头部已是具体构造（`Nominal`/元组/固定数组等）时，即使实参里还含
                // 调用方刚性 binder，impl 选择仍是结构性的，witness 的关联类型就是投影
                // 结果（`ArrayIterator<K>::Item` 即 `K`）。只有目标是裸刚性参数或关联
                // 投影时，才没有可选的 impl。
                let witness_target = !matches!(&target, ValkyrieType::Generic(_) | ValkyrieType::Associated(_));
                if witness_target {
                    if let Ok(witness) = self.traits.satisfy_explicit_type(&target, &bound_without_equations) {
                        for equation in &bound.associated_types {
                            let expected = substitute_relation_type(&equation.ty, substitutions);
                            let ValkyrieType::Generic(generic) = &expected
                            else {
                                continue;
                            };
                            let Some(binder) = generic.binder
                            else {
                                continue;
                            };
                            if !binders.contains(&binder) || substitutions.contains_key(&binder) {
                                continue;
                            }
                            if let Some(actual) = witness.associated_types.get(&equation.name) {
                                substitutions.insert(binder, actual.clone());
                            }
                        }
                        continue;
                    }
                }
                if open {
                    // 目标仍是调用方的刚性泛型参数时，impl witness 不存在；但若调用方
                    // 已声明的 where 约束对同一目标携带同一 trait 的关联等式，投影结果
                    // 已由该声明唯一确定（`where A: Iterator<Item = char>` 下
                    // `<A as Iterator>::Item` 就是 `char`），据此补出关联 binder。
                    self.infer_from_declared_equation(&target, bound, substitutions, binders);
                }
            }
        }
    }

    /// 从调用方已声明的 where 约束取关联等式补解。
    ///
    /// 只接受「同一目标的已声明约束 + 同一 trait 声明 + 实参逐项一致 + 同名关联等式」
    /// 这一条来源；同一 binder 已有绑定时不覆盖，由后续合并按 fail-closed 判定冲突。
    fn infer_from_declared_equation(
        &self,
        target: &ValkyrieType,
        required: &HirTraitBound,
        substitutions: &mut BTreeMap<GenericBinderId, ValkyrieType>,
        binders: &BTreeSet<GenericBinderId>,
    ) {
        let Some(context) =
            self.callable_constraints.iter().find(|context| substitute_relation_type(&context.target, substitutions) == *target)
        else {
            return;
        };
        for provided in &context.bounds {
            if provided.declaration.is_none() || provided.declaration != required.declaration {
                continue;
            }
            if provided.type_arguments.len() != required.type_arguments.len() {
                continue;
            }
            if !provided.type_arguments.iter().zip(&required.type_arguments).all(|(provided, required)| {
                normalize_context_type(provided, &self.callable_constraints, substitutions)
                    == normalize_context_type(required, &self.callable_constraints, substitutions)
            }) {
                continue;
            }
            for required_equation in &required.associated_types {
                let Some(provided_equation) = provided.associated_types.iter().find(|equation| equation.name == required_equation.name) else {
                    continue;
                };
                let expected = substitute_relation_type(&required_equation.ty, substitutions);
                let ValkyrieType::Generic(generic) = &expected
                else {
                    continue;
                };
                let Some(binder) = generic.binder
                else {
                    continue;
                };
                if !binders.contains(&binder) || substitutions.contains_key(&binder) {
                    continue;
                }
                let solution = normalize_context_type(&provided_equation.ty, &self.callable_constraints, substitutions);
                substitutions.insert(binder, solution);
            }
        }
    }

    /// 形参/返回槽位上的关联投影解析：`L::Settings` 这类投影在 `L` 已由实参实例化为
    /// 具体类型后，由候选自身 `where` 约束声明的 trait 实现的关联类型见证给出具体值。
    ///
    /// 只接受「同一目标 + 满足该 bound 的唯一 explicit impl 见证」这一条来源；基类型仍是
    /// 开放 binder、没有见证、或不同 bound 给出不同解时保持投影原样，交由匹配阶段淘汰，
    /// 不在解析阶段挑 impl 猜测。
    pub fn resolve_associated_projections(
        &self,
        ty: &ValkyrieType,
        constraints: &[HirWhereConstraint],
        substitutions: &BTreeMap<GenericBinderId, ValkyrieType>,
    ) -> ValkyrieType {
        let ty = substitute_relation_type(ty, substitutions);
        let mut resolve = |associated: &crate::compiler::types::hir::AssociatedType| {
            let base = substitute_relation_type(&associated.base, substitutions);
            if relation_type_is_open_obligation(&base) {
                return None;
            }
            let mut solutions = Vec::new();
            for constraint in constraints {
                if substitute_relation_type(&constraint.target, substitutions) != base {
                    continue;
                }
                for bound in &constraint.bounds {
                    let bound = HirTraitBound {
                        declaration: bound.declaration,
                        trait_path: bound.trait_path.clone(),
                        type_arguments: bound.type_arguments.iter().map(|ty| substitute_relation_type(ty, substitutions)).collect(),
                        associated_types: Vec::new(),
                    };
                    let Ok(witness) = self.traits.satisfy_explicit_type(&base, &bound)
                    else {
                        continue;
                    };
                    let Some(solution) = witness.associated_types.get(&associated.name) else {
                        continue;
                    };
                    let solution = substitute_relation_type(solution, substitutions);
                    if !solutions.contains(&solution) {
                        solutions.push(solution);
                    }
                }
            }
            (solutions.len() == 1).then(|| solutions.remove(0))
        };
        super::overload::substitution::rewrite_associated_projections(&ty, &mut resolve)
    }

    /// 校验具名类型是否满足匿名 row 的方法要求。
    pub fn match_row_requirement(&self, actual_name: &Identifier, requirement: &RowRequirement) -> ParameterMatchResult {
        let Some(candidate) = self.structs.get(actual_name)
        else {
            return ParameterMatchResult::NoMatch {
                diagnostic_seed: TypeRelationDiagnosticSeed::UnsupportedActualType { actual: ValkyrieType::Named(actual_name.clone()) },
            };
        };

        match requirement.check_struct(candidate) {
            Ok(()) => ParameterMatchResult::Row,
            Err(errors) => ParameterMatchResult::NoMatch { diagnostic_seed: TypeRelationDiagnosticSeed::Row { errors } },
        }
    }

    fn matches_non_error(&self, actual: &ValkyrieType, expected: &ValkyrieType) -> bool {
        !matches!(self.match_parameter(actual, expected), ParameterMatchResult::NoMatch { .. })
    }
}

fn collect_relation_enums(
    module: &HirModule,
    by_name: &mut BTreeMap<Identifier, HirEnum>,
    by_declaration: &mut BTreeMap<nyar_types::ItemId, HirEnum>,
) {
    for item in &module.enums {
        by_name.entry(item.name.clone()).or_insert_with(|| item.clone());
        if let Some(declaration) = item.declaration {
            by_declaration.entry(declaration).or_insert_with(|| item.clone());
        }
    }
    for export in &module.imported_semantic_exports {
        for item in &export.enums {
            by_name.entry(item.name.clone()).or_insert_with(|| item.clone());
            if let Some(declaration) = item.declaration {
                by_declaration.entry(declaration).or_insert_with(|| item.clone());
            }
        }
    }
    for submodule in &module.submodules {
        collect_relation_enums(submodule, by_name, by_declaration);
    }
}

/// 收集模块（含子模块）声明的 `using` 导入绑定。
///
/// 导入归属由文件作用域承载：逐文件局部导入 + unit 共享的主空间 `using!`。
/// P1 阶段两者仍合并成同一候选池，匹配语义与改造前一致。
fn collect_relation_imports(module: &HirModule, out: &mut Vec<ImportedSymbol>) {
    for scope in &module.files {
        collect_import_bindings(&scope.imports, out);
    }
    collect_import_bindings(&module.primary_imports, out);
    for submodule in &module.submodules {
        collect_relation_imports(submodule, out);
    }
}

fn collect_import_bindings(imports: &[crate::compiler::types::hir::HirImport], out: &mut Vec<ImportedSymbol>) {
    for import in imports {
        if import.glob {
            let local = import.alias.clone().or_else(|| import.path.parts().last().cloned()).unwrap_or_else(|| Identifier::new(""));
            out.push(ImportedSymbol { module: import.path.clone(), glob: true, imported: local.clone(), local });
            continue;
        }
        // `using a::b::c`（无花括号）是单符号导入：末段是导入名，前缀是模块。
        if import.bindings.is_empty() {
            let parts = import.path.parts();
            let Some(imported) = parts.last().cloned()
            else {
                continue;
            };
            let module = NamePath::new(parts[..parts.len().saturating_sub(1)].to_vec());
            let local = import.alias.clone().unwrap_or_else(|| imported.clone());
            out.push(ImportedSymbol { module, glob: false, imported, local });
            continue;
        }
        for binding in &import.bindings {
            out.push(ImportedSymbol {
                module: import.path.clone(),
                glob: false,
                local: binding.alias.clone().unwrap_or_else(|| binding.name.clone()),
                imported: binding.name.clone(),
            });
        }
    }
}

/// 同一源码组内的模块共享一个文件作用域：组内任一命名空间的顶层声明都能被组内
/// 其它模块以短名无限定引用。
///
/// 这与组内名义类型的短名解析保持一致（`NominalModuleView` 同样按组内声明集合建索引），
/// 也符合「Package 拥有 Module、声明及包内可见性」的合同。跨组仍然只承认显式 `using`
/// 绑定与依赖组导出，不允许以末段回退跨越组边界；本清单只服务自由函数通道，不进入
/// HIR 序列化面，也不并入 `imported_symbols`（成员方法派发不认组作用域）。
fn collect_group_scope_namespaces(module: &HirModule, out: &mut Vec<NamePath>) {
    if !module.name.parts().is_empty() {
        out.push(module.name.clone());
    }
    for submodule in &module.submodules {
        collect_group_scope_namespaces(submodule, out);
    }
}

fn collect_relation_structs(
    module: &HirModule,
    by_name: &mut BTreeMap<Identifier, HirStruct>,
    by_declaration: &mut BTreeMap<nyar_types::ItemId, HirStruct>,
) {
    for item in &module.structs {
        by_name.entry(item.name.clone()).or_insert_with(|| item.clone());
        if let Some(declaration) = item.constructor_declaration {
            by_declaration.entry(declaration).or_insert_with(|| item.clone());
        }
    }
    for export in &module.imported_semantic_exports {
        for item in &export.structs {
            by_name.entry(item.name.clone()).or_insert_with(|| item.clone());
            if let Some(declaration) = item.constructor_declaration {
                by_declaration.entry(declaration).or_insert_with(|| item.clone());
            }
        }
    }
    for submodule in &module.submodules {
        collect_relation_structs(submodule, by_name, by_declaration);
    }
}

/// 数值面拼写集合：仅用于 HIR 阶段识别“这个未结算拼写可能是数值类型名”。
///
/// 不参与数值身份判定（`TypeRelation::is_numeric_type` 只认 `[primitive]` 身份）；
/// 这里不生产任何低层 carrier，carrier 由表示阶段拥有。
pub(crate) fn is_numeric_type_name(name: &str) -> bool {
    matches!(
        name,
        "usize"
            | "isize"
            | "u8"
            | "u16"
            | "u32"
            | "u64"
            | "u128"
            | "i8"
            | "i16"
            | "i32"
            | "i64"
            | "i128"
            | "f32"
            | "f64"
            | "f128"
            | "std.primitive.usize"
            | "std.primitive.isize"
            | "std::primitive::usize"
            | "std::primitive::isize"
    )
}

/// 文本名集合：字符串字面量特化只认这些名字（含 `type utf8 = Utf8Text` 的别名目标）。
pub(crate) fn is_text_type_name(name: &str) -> bool {
    matches!(name.as_ref(), "text" | "utf8" | "Utf8Text" | "utf16" | "Utf16Text" | "std::text::text" | "std.text.text")
}

/// 浮点名集合：浮点字面量只认这几个名字，整数名不在其中。
fn is_float_type_name(name: &str) -> bool {
    matches!(name, "f32" | "f64" | "f128")
}

/// 数值类型判定：只认已结算的平台数值形状。
///
/// `Named("i32")` 这类未绑定声明的拼写不再算数值——按名字猜身份会让同名普通
/// class 通过字面量特化；缺别名/缺身份必须失败关闭。
pub(crate) fn is_numeric_type_name_of(ty: &ValkyrieType) -> bool {
    matches!(
        ty,
        ValkyrieType::Integer8 { .. }
            | ValkyrieType::Integer16 { .. }
            | ValkyrieType::Integer32 { .. }
            | ValkyrieType::Integer64 { .. }
            | ValkyrieType::Integer128 { .. }
            | ValkyrieType::Float32
            | ValkyrieType::Float64
    )
}

/// 字符类型判定（字符字面量特化与值位置绑定共用同一份口径）。
///
/// `char` 是 u16 code unit 的编码型内建；声明面已折叠成 `Character`，未绑定声明
/// 只可能以源码短名出现。
pub(crate) fn is_character_type_name_of(ty: &ValkyrieType) -> bool {
    match ty {
        ValkyrieType::Character => true,
        ValkyrieType::Named(name) => matches!(name.as_str(), "char" | "std::text::char"),
        _ => false,
    }
}

fn bound_is_satisfied_by_context(
    required: &crate::compiler::types::hir::HirTraitBound,
    context: &HirWhereConstraint,
    all_contexts: &[HirWhereConstraint],
    substitutions: &BTreeMap<GenericBinderId, ValkyrieType>,
) -> bool {
    context.bounds.iter().any(|provided| {
        provided.declaration.is_some()
            && provided.declaration == required.declaration
            && provided.type_arguments.len() == required.type_arguments.len()
            && provided.type_arguments.iter().zip(&required.type_arguments).all(|(provided, required)| {
                normalize_context_type(provided, all_contexts, substitutions) == normalize_context_type(required, all_contexts, substitutions)
            })
            && required.associated_types.iter().all(|required_equation| {
                let expected = normalize_context_type(&required_equation.ty, all_contexts, substitutions);
                provided.associated_types.iter().any(|provided_equation| {
                    provided_equation.name == required_equation.name
                        && normalize_context_type(&provided_equation.ty, all_contexts, substitutions) == expected
                })
            })
    })
}

fn normalize_context_type(
    ty: &ValkyrieType,
    contexts: &[HirWhereConstraint],
    substitutions: &BTreeMap<GenericBinderId, ValkyrieType>,
) -> ValkyrieType {
    let ty = substitute_relation_type(ty, substitutions);
    let ValkyrieType::Associated(associated) = &ty
    else {
        return ty;
    };
    let base = &associated.base;
    contexts
        .iter()
        .filter(|context| substitute_relation_type(&context.target, substitutions) == *base)
        .flat_map(|context| &context.bounds)
        .flat_map(|bound| &bound.associated_types)
        .find(|equation| equation.name == associated.name)
        .map(|equation| normalize_context_type(&equation.ty, contexts, substitutions))
        .unwrap_or(ty)
}

fn substitute_relation_type(ty: &ValkyrieType, substitutions: &BTreeMap<GenericBinderId, ValkyrieType>) -> ValkyrieType {
    super::overload::substitute_type_vars(ty, substitutions)
}

/// 目标类型是否仍然“开放”：代入后仍含调用方尚未结算的刚性泛型参数。
///
/// 这类目标上的 `where` 约束属于调用方**尚未履行**的义务：解析阶段没有 evidence 可查，
/// 只能在该实例按具体类型结算，因此不得据此把调用降成未解析。
/// 注意 `binder` 为空的 [`ValkyrieType::Generic`] 只是 lowering 未冻结的残留名字，
/// 不属于开放义务——那是 producer 缺陷，必须继续报错。
fn relation_type_is_open_obligation(ty: &ValkyrieType) -> bool {
    match ty {
        ValkyrieType::Generic(generic) => generic.binder.is_some(),
        ValkyrieType::Nominal { arguments, .. }
        | ValkyrieType::Tuple(arguments)
        | ValkyrieType::Union(arguments)
        | ValkyrieType::Intersection(arguments) => arguments.iter().any(relation_type_is_open_obligation),
        ValkyrieType::Apply(base, arguments) => {
            relation_type_is_open_obligation(base) || arguments.iter().any(relation_type_is_open_obligation)
        }
        ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => relation_type_is_open_obligation(element),
        ValkyrieType::FixedArray { element, .. } => relation_type_is_open_obligation(element),
        ValkyrieType::Function(function) => {
            function.params.iter().any(relation_type_is_open_obligation) || relation_type_is_open_obligation(&function.return_type)
        }
        ValkyrieType::TypeLambda(lambda) => relation_type_is_open_obligation(&lambda.body),
        ValkyrieType::TraitObject(object) => object.type_arguments.iter().any(relation_type_is_open_obligation),
        ValkyrieType::Associated(associated) => {
            relation_type_is_open_obligation(&associated.base) || associated.type_arguments.iter().any(relation_type_is_open_obligation)
        }
        ValkyrieType::Row(row) => row
            .methods
            .iter()
            .any(|method| method.params.iter().any(relation_type_is_open_obligation) || relation_type_is_open_obligation(&method.return_type)),
        _ => false,
    }
}

fn row_type_to_requirement(row: &RowType) -> RowRequirement {
    RowRequirement::from_methods(
        row.methods
            .iter()
            .map(|method| RowMethodSignature {
                name: method.name.clone(),
                params: method.params.clone(),
                return_type: method.return_type.clone(),
            })
            .collect(),
    )
}

fn named_type_name(ty: &ValkyrieType) -> Option<Identifier> {
    match ty {
        ValkyrieType::Named(name) => Some(name.clone()),

        ValkyrieType::Apply(base, _) => named_type_name(base),
        _ => None,
    }
}

fn trivially_compatible(actual: &ValkyrieType, expected: &ValkyrieType) -> bool {
    match (actual, expected) {
        (ValkyrieType::Generic(actual), ValkyrieType::Generic(expected)) if actual.binder.is_some() && actual.binder == expected.binder => true,
        // `Self` is only a placeholder inside an imply body; overload matching must
        // substitute it with the owner before calling this (see `substitute_self_type`).
        // Keep identity for unresolved Self-vs-Self only — never "Self matches anything".
        (ValkyrieType::r#SelfType, ValkyrieType::r#SelfType) => true,
        (ValkyrieType::Array(actual), ValkyrieType::Array(expected)) => trivially_compatible(actual, expected),
        (ValkyrieType::Tuple(actual), ValkyrieType::Tuple(expected)) if actual.len() == expected.len() => {
            actual.iter().zip(expected).all(|(actual, expected)| trivially_compatible(actual, expected))
        }
        (ValkyrieType::Intersection(actual), ValkyrieType::Intersection(expected)) if actual.len() == expected.len() => {
            actual.iter().zip(expected).all(|(actual, expected)| trivially_compatible(actual, expected))
        }
        _ => actual == expected || matches!(actual, ValkyrieType::AutoType) || matches!(expected, ValkyrieType::AutoType),
    }
}

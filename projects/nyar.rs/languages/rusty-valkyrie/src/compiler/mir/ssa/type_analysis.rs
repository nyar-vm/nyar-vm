//! 类型占位、未解析引用与类型身份表分析。

use super::{MirSumDeclaration, model::*};
use crate::compiler::types::hir::ValkyrieType;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn type_contains_placeholder(ty: &ValkyrieType, include_generic_binders: bool) -> bool {
    match ty {
        ValkyrieType::SelfType | ValkyrieType::Associated(_) | ValkyrieType::AutoType => true,
        // 已带 `binder` 的泛型形参是前端冻结的声明身份，可以作为函数签名的一部分
        // 保留（调用方用 `include_generic_binders=false` 时放行）；但不能进 type_identities。
        ValkyrieType::Generic(generic) => include_generic_binders && generic.binder.is_none(),
        ValkyrieType::Nominal { arguments, .. } => arguments.iter().any(|arg| type_contains_placeholder(arg, include_generic_binders)),
        ValkyrieType::Apply(base, args) => {
            type_contains_placeholder(base, include_generic_binders)
                || args.iter().any(|arg| type_contains_placeholder(arg, include_generic_binders))
        }
        ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => {
            items.iter().any(|item| type_contains_placeholder(item, include_generic_binders))
        }
        ValkyrieType::Function(function) => {
            function.params.iter().any(|param| type_contains_placeholder(param, include_generic_binders))
                || type_contains_placeholder(&function.return_type, include_generic_binders)
        }
        ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => type_contains_placeholder(element, include_generic_binders),
        ValkyrieType::FixedArray { element, .. } => type_contains_placeholder(element, include_generic_binders),
        _ => false,
    }
}

/// Canonical 类型表只收录可无损 lower 的形状；泛型形参与 `Self`/关联类型保留在 MIR 局部语义中。
pub(crate) fn type_contains_unresolved_reference(ty: &ValkyrieType) -> bool {
    type_contains_placeholder(ty, true)
}

/// 函数签名允许携带泛型形参，但不允许 `Self`/关联类型占位。
pub(crate) fn type_contains_non_monomorphic_signature_reference(ty: &ValkyrieType) -> bool {
    type_contains_placeholder(ty, false)
}

/// 为模块函数、外部导入、结构体与 sum 声明建立类型事实表；外部测试入口。
#[doc(hidden)]
pub fn type_identity_table(
    functions: &[MirFunction],
    external_calls: &[MirExternalCallContract],
    structs: &[MirStruct],
    sums: &[MirSumDeclaration],
) -> BTreeMap<ValkyrieType, nyar_types::TypeId> {
    /// 把 primitive numeric 的 Named surface name 归一到底层 primitive 变体。
    /// 其他类型原样返回。
    fn normalize_named_primitive(ty: ValkyrieType) -> ValkyrieType {
        match &ty {
            ValkyrieType::Named(name) => match name.as_str() {
                "boolean" | "Bool" | "bool" => ValkyrieType::Boolean,
                "i8" => ValkyrieType::Integer8 { signed: true },
                "u8" | "byte" => ValkyrieType::Integer8 { signed: false },
                "i16" => ValkyrieType::Integer16 { signed: true },
                "u16" => ValkyrieType::Integer16 { signed: false },
                "i32" | "int" | "Int" => ValkyrieType::Integer32 { signed: true },
                "u32" | "uint" | "UInt" => ValkyrieType::Integer32 { signed: false },
                "i64" | "long" | "Long" => ValkyrieType::Integer64 { signed: true },
                "u64" | "ulong" => ValkyrieType::Integer64 { signed: false },
                "i128" => ValkyrieType::Integer128 { signed: true },
                "u128" => ValkyrieType::Integer128 { signed: false },
                "f32" | "float32" => ValkyrieType::Float32,
                "f64" | "float" | "Float" | "double" | "Double" => ValkyrieType::Float64,
                _ => ty,
            },
            _ => ty,
        }
    }

    /// 递归检查类型内部是否含声明期占位（Generic、SelfType、AutoType）。
    /// 无论 binder 是否存在都算占位。
    fn contains_deferred_type_param(ty: &ValkyrieType) -> bool {
        match ty {
            ValkyrieType::Generic(_) | ValkyrieType::SelfType | ValkyrieType::AutoType => true,
            ValkyrieType::Nominal { arguments, .. } => arguments.iter().any(contains_deferred_type_param),
            ValkyrieType::Apply(base, args) => {
                contains_deferred_type_param(base) || args.iter().any(contains_deferred_type_param)
            }
            ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => {
                items.iter().any(contains_deferred_type_param)
            }
            ValkyrieType::Function(function) => {
                function.params.iter().any(contains_deferred_type_param)
                    || contains_deferred_type_param(&function.return_type)
            }
            ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => contains_deferred_type_param(element),
            ValkyrieType::FixedArray { element, .. } => contains_deferred_type_param(element),
            ValkyrieType::Associated(associated) => {
                contains_deferred_type_param(&associated.base)
                    || associated.type_arguments.iter().any(contains_deferred_type_param)
            }
            _ => false,
        }
    }

    /// 递归检查类型内部是否含"开放"的泛型形参（Generic{binder=None}）。
    /// 带 binder 的泛型形参是声明身份，不算开放。
    fn contains_open_generic(ty: &ValkyrieType) -> bool {
        match ty {
            ValkyrieType::Generic(generic) => generic.binder.is_none(),
            ValkyrieType::Nominal { arguments, .. } => arguments.iter().any(contains_open_generic),
            ValkyrieType::Apply(base, args) => contains_open_generic(base) || args.iter().any(contains_open_generic),
            ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => {
                items.iter().any(contains_open_generic)
            }
            ValkyrieType::Function(function) => {
                function.params.iter().any(contains_open_generic) || contains_open_generic(&function.return_type)
            }
            ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => contains_open_generic(element),
            ValkyrieType::FixedArray { element, .. } => contains_open_generic(element),
            ValkyrieType::Associated(associated) => {
                contains_open_generic(&associated.base) || associated.type_arguments.iter().any(contains_open_generic)
            }
            _ => false,
        }
    }

    /// 收集类型到 set。
    /// - `strict=true`：严格模式，禁止任何 deferred param（Generic{binder=Some}、SelfType、AutoType）。
    ///   用于值位置（value_types）——这些类型必须完全闭合。
    /// - `strict=false`：宽松模式，允许带 binder 的 Generic（声明身份）。用于函数签名。
    fn collect(set: &mut BTreeSet<ValkyrieType>, ty: &ValkyrieType, strict: bool) {
        // strict 模式下，声明期占位类型不得进入 type_identities。
        if strict {
            if contains_deferred_type_param(ty) {
                // 仍需递归下钻实参，让子类型（如 Nominal 的 arguments 里的具体类型）也能被收集。
                // 但 ty 本身不 insert。
            }
        }
        match ty {
            ValkyrieType::Nominal { arguments, .. } => {
                for arg in arguments {
                    collect(set, arg, strict);
                }
            }
            ValkyrieType::Apply(base, args) => {
                collect(set, base, strict);
                for arg in args {
                    collect(set, arg, strict);
                }
            }
            ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => {
                for item in items {
                    collect(set, item, strict);
                }
            }
            ValkyrieType::Function(function) => {
                for param in &function.params {
                    collect(set, param, strict);
                }
                collect(set, &function.return_type, strict);
            }
            ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => collect(set, element, strict),
            ValkyrieType::FixedArray { element, .. } => collect(set, element, strict),
            ValkyrieType::Associated(associated) => {
                collect(set, &associated.base, strict);
                for argument in &associated.type_arguments {
                    collect(set, argument, strict);
                }
            }
            _ => {}
        }
        // insert 条件：
        // 1. 不含未解析引用（Generic{binder=None}）
        // 2. strict 模式下还不能含任何 deferred param（Generic{binder=Some}、SelfType、AutoType）
        let no_unresolved = !contains_open_generic(ty) && !matches!(ty, ValkyrieType::SelfType | ValkyrieType::AutoType);
        let passes_strict = !strict || !contains_deferred_type_param(ty);
        if no_unresolved && passes_strict {
            set.insert(normalize_named_primitive(ty.clone()));
        }
    }

    /// 聚合声明的规范类型键：已绑定 ItemId 的无参 Nominal。
    fn aggregate_type_key(structure: &MirStruct) -> Option<ValkyrieType> {
        structure.declaration.map(|declaration| ValkyrieType::Nominal { declaration, arguments: Vec::new() })
    }

    let mut types = BTreeSet::new();
    for function in functions {
        // 函数签名允许泛型形参 binder（声明身份），用宽松模式收集。
        collect(&mut types, &function.return_type, false);
        for ty in &function.param_types {
            collect(&mut types, ty, false);
        }
        // 值位置必须完全闭合，严格模式过滤任何 deferred param。
        for ty in function.value_types.values() {
            collect(&mut types, ty, true);
        }
    }
    for contract in external_calls {
        for ty in &contract.parameter_types {
            collect(&mut types, ty, true);
        }
        collect(&mut types, &contract.return_type, true);
    }
    for structure in structs {
        if let Some(key) = aggregate_type_key(structure) {
            collect(&mut types, &key, true);
        }
        // 泛型声明的字段类型含 Generic{binder} 占位，只有具体实例（由
        // register_aggregate_nominal_instances 代入后）才应进入类型事实表。
        if structure.generics.is_empty() {
            for field in &structure.fields {
                collect(&mut types, &field.ty, true);
            }
        }
    }
    for sum in sums {
        if let Some(declaration) = sum.declaration {
            collect(&mut types, &ValkyrieType::Nominal { declaration, arguments: Vec::new() }, true);
        }
        for variant in &sum.variants {
            for field in &variant.fields {
                collect(&mut types, &field.ty, true);
            }
            if let Some(result) = &variant.result_type {
                collect(&mut types, result, true);
            }
        }
    }

    types
        .into_iter()
        .enumerate()
        .map(|(index, ty)| (ty, nyar_types::TypeId::from_index(index as u32).expect("type identity overflow")))
        .collect()
}

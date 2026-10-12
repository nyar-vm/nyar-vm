//! 将泛型种子调用点物化为具体签名的 `ItemInstanceId`。
//!
//! Semantic MIR 允许调用点 SSA 携带已代入返回类型，而种子 `MirFunction` 仍保留 binder。
//! Canonical `CallResultTypeMismatch` 要求 Invoke callee 的 `ItemInstanceRecord.return_type`
//! 与结果 SSA 的 `TypeId` 精确相等，因此必须在进入 Canonical 前为每次具体代入分配实例并改写 callee。

use std::collections::BTreeMap;

use nyar_types::ItemInstanceId;

use crate::types::{Identifier, hir::ValkyrieType};

use super::{MirFunction, MirOperand, MirOperation};

/// 若 `concrete` 实例化 `template`，返回 binder → 实参代入表。
pub(crate) fn collect_generic_substitutions(
    template: &ValkyrieType,
    concrete: &ValkyrieType,
) -> Option<BTreeMap<Identifier, ValkyrieType>> {
    let mut substitutions = BTreeMap::new();
    if unify_generic_instantiation(template, concrete, &mut substitutions) {
        Some(substitutions)
    }
    else {
        None
    }
}

/// 判断 `concrete` 是否为 `template` 在某一组 binder 代入下的实例。
pub(crate) fn type_instantiates_generic_signature(template: &ValkyrieType, concrete: &ValkyrieType) -> bool {
    collect_generic_substitutions(template, concrete).is_some()
}

/// 将 binder 代入应用到类型形状；未出现在表中的 `Named` 保持原样。
pub(crate) fn apply_type_substitution(ty: &ValkyrieType, substitutions: &BTreeMap<Identifier, ValkyrieType>) -> ValkyrieType {
    match ty {
        ValkyrieType::Named(name) => substitutions.get(name).cloned().unwrap_or_else(|| ty.clone()),
        ValkyrieType::Nominal { declaration, arguments } => ValkyrieType::Nominal {
            declaration: *declaration,
            arguments: arguments.iter().map(|argument| apply_type_substitution(argument, substitutions)).collect(),
        },
        ValkyrieType::Apply(base, arguments) => ValkyrieType::Apply(
            Box::new(apply_type_substitution(base, substitutions)),
            arguments.iter().map(|argument| apply_type_substitution(argument, substitutions)).collect(),
        ),
        ValkyrieType::Array(element) => ValkyrieType::Array(Box::new(apply_type_substitution(element, substitutions))),
        ValkyrieType::FixedArray { element, length } => ValkyrieType::FixedArray {
            element: Box::new(apply_type_substitution(element, substitutions)),
            length: *length,
        },
        ValkyrieType::Nullable(element) => ValkyrieType::Nullable(Box::new(apply_type_substitution(element, substitutions))),
        ValkyrieType::Tuple(items) => {
            ValkyrieType::Tuple(items.iter().map(|item| apply_type_substitution(item, substitutions)).collect())
        }
        ValkyrieType::Union(items) => {
            ValkyrieType::Union(items.iter().map(|item| apply_type_substitution(item, substitutions)).collect())
        }
        ValkyrieType::Intersection(items) => {
            ValkyrieType::Intersection(items.iter().map(|item| apply_type_substitution(item, substitutions)).collect())
        }
        ValkyrieType::Function(function) => {
            let mut function = function.as_ref().clone();
            function.params = function.params.iter().map(|param| apply_type_substitution(param, substitutions)).collect();
            function.return_type = apply_type_substitution(&function.return_type, substitutions);
            ValkyrieType::Function(Box::new(function))
        }
        _ => ty.clone(),
    }
}

pub(crate) fn unify_generic_instantiation(
    template: &ValkyrieType,
    concrete: &ValkyrieType,
    substitutions: &mut BTreeMap<Identifier, ValkyrieType>,
) -> bool {
    match (template, concrete) {
        (left, right) if left == right => true,
        (
            ValkyrieType::Nominal { declaration: template_decl, arguments: template_args },
            ValkyrieType::Nominal { declaration: concrete_decl, arguments: concrete_args },
        ) => {
            template_decl == concrete_decl
                && template_args.len() == concrete_args.len()
                && template_args
                    .iter()
                    .zip(concrete_args)
                    .all(|(template_arg, concrete_arg)| unify_generic_instantiation(template_arg, concrete_arg, substitutions))
        }
        (ValkyrieType::Apply(template_base, template_args), ValkyrieType::Apply(concrete_base, concrete_args)) => {
            template_args.len() == concrete_args.len()
                && unify_generic_instantiation(template_base, concrete_base, substitutions)
                && template_args
                    .iter()
                    .zip(concrete_args)
                    .all(|(template_arg, concrete_arg)| unify_generic_instantiation(template_arg, concrete_arg, substitutions))
        }
        (ValkyrieType::Named(template_name), concrete) => match substitutions.get(template_name) {
            Some(bound) => bound == concrete,
            None => {
                substitutions.insert(template_name.clone(), concrete.clone());
                true
            }
        },
        (ValkyrieType::Array(template_element), ValkyrieType::Array(concrete_element)) => {
            unify_generic_instantiation(template_element, concrete_element, substitutions)
        }
        (
            ValkyrieType::FixedArray { element: template_element, length: template_length },
            ValkyrieType::FixedArray { element: concrete_element, length: concrete_length },
        ) => template_length == concrete_length && unify_generic_instantiation(template_element, concrete_element, substitutions),
        (ValkyrieType::Function(template_fn), ValkyrieType::Function(concrete_fn)) => {
            template_fn.params.len() == concrete_fn.params.len()
                && template_fn
                    .params
                    .iter()
                    .zip(&concrete_fn.params)
                    .all(|(template_param, concrete_param)| unify_generic_instantiation(template_param, concrete_param, substitutions))
                && unify_generic_instantiation(&template_fn.return_type, &concrete_fn.return_type, substitutions)
        }
        _ => false,
    }
}

/// 扫描全部 Call 站点：若结果类型是种子返回类型的实例，则物化 monomorph 并改写 callee。
pub(super) fn materialize_generic_call_instances(functions: &mut Vec<MirFunction>) {
    let mut next_instance = functions
        .iter()
        .filter_map(|function| function.instance)
        .map(|instance| instance.index())
        .max()
        .map(|index| index.saturating_add(1))
        .unwrap_or(0);
    let mut cache: BTreeMap<(ItemInstanceId, ValkyrieType), ItemInstanceId> = BTreeMap::new();
    let mut cursor = 0usize;
    while cursor < functions.len() {
        let sites = collect_instantiation_sites(&functions[cursor], functions);
        for (block_index, instruction_index, seed, concrete_return, substitutions) in sites {
            let mono = if let Some(existing) = cache.get(&(seed, concrete_return.clone())).copied() {
                existing
            }
            else {
                let seed_function = functions
                    .iter()
                    .find(|function| function.instance == Some(seed))
                    .cloned()
                    .expect("seed callable must exist");
                let specialized = specialize_function(&seed_function, &concrete_return, &substitutions, next_instance);
                let instance = specialized.instance.expect("specialized callable must receive instance");
                next_instance = next_instance.saturating_add(1);
                cache.insert((seed, concrete_return), instance);
                functions.push(specialized);
                instance
            };
            if let MirOperation::Call { callee, .. } = &mut functions[cursor].blocks[block_index].instructions[instruction_index].kind
            {
                *callee = MirOperand::Callable(mono);
            }
        }
        cursor += 1;
    }
}

fn collect_instantiation_sites(
    function: &MirFunction,
    functions: &[MirFunction],
) -> Vec<(usize, usize, ItemInstanceId, ValkyrieType, BTreeMap<Identifier, ValkyrieType>)> {
    let mut sites = Vec::new();
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            let MirOperation::Call { callee: MirOperand::Callable(seed), .. } = &instruction.kind
            else {
                continue;
            };
            let Some(result) = instruction.results.first()
            else {
                continue;
            };
            let Some(concrete_return) = function.value_types.get(result).cloned()
            else {
                continue;
            };
            let Some(seed_function) = functions.iter().find(|candidate| candidate.instance == Some(*seed))
            else {
                continue;
            };
            if seed_function.return_type == concrete_return {
                continue;
            }
            let Some(substitutions) = collect_generic_substitutions(&seed_function.return_type, &concrete_return)
            else {
                continue;
            };
            if substitutions.is_empty() {
                continue;
            }
            sites.push((block_index, instruction_index, *seed, concrete_return, substitutions));
        }
    }
    sites
}

fn specialize_function(
    seed: &MirFunction,
    concrete_return: &ValkyrieType,
    substitutions: &BTreeMap<Identifier, ValkyrieType>,
    instance_index: u32,
) -> MirFunction {
    let instance = ItemInstanceId::from_index(instance_index).expect("ItemInstanceId overflow");
    let mut specialized = seed.clone();
    specialized.instance = Some(instance);
    // callable_identities 以符号为键；monomorph 必须使用独立符号，避免覆盖种子条目。
    specialized.symbol = format!("{}#{}", seed.symbol, instance_index);
    specialized.return_type = concrete_return.clone();
    specialized.param_types = seed.param_types.iter().map(|ty| apply_type_substitution(ty, substitutions)).collect();
    specialized.value_types =
        seed.value_types.iter().map(|(value, ty)| (*value, apply_type_substitution(ty, substitutions))).collect();
    for block in &mut specialized.blocks {
        for instruction in &mut block.instructions {
            specialize_operation(&mut instruction.kind, substitutions);
        }
    }
    specialized
}

fn specialize_operation(operation: &mut MirOperation, substitutions: &BTreeMap<Identifier, ValkyrieType>) {
    match operation {
        MirOperation::LoadConstant { ty: Some(ty), .. }
        | MirOperation::StoreVar { ty: Some(ty), .. }
        | MirOperation::Cast { ty, .. }
        | MirOperation::ArrayNew { array_type: ty, .. }
        | MirOperation::ArrayFromElements { array_type: ty, .. } => {
            *ty = apply_type_substitution(ty, substitutions);
        }
        MirOperation::SumNew { type_args, payload_type, .. } => {
            for argument in type_args.iter_mut() {
                *argument = apply_type_substitution(argument, substitutions);
            }
            if let Some(payload) = payload_type.as_mut() {
                *payload = apply_type_substitution(payload, substitutions);
            }
        }
        MirOperation::SumPayloadGet { type_args, payload_type, .. } => {
            for argument in type_args.iter_mut() {
                *argument = apply_type_substitution(argument, substitutions);
            }
            *payload_type = apply_type_substitution(payload_type, substitutions);
        }
        MirOperation::SumVariantIs { type_args, .. } => {
            for argument in type_args.iter_mut() {
                *argument = apply_type_substitution(argument, substitutions);
            }
        }
        _ => {}
    }
}

//! 按调用点的泛型 callable 单态化（instantiation 固定点）。
//!
//! 阶段位置：全部源码组的 `resolve_hir_calls` 完成之后，`validate_hir_semantic_contract`
//! 与 MIR lowering 之前。声明期开放签名不再分配 `ItemInstanceId`（见
//! `register_function_declarations`）；调用点的具体代入在这里被冻结成真正的 callable
//! 实例：克隆声明体、代入签名/局部/聚合类型，并递归处理体内调用，直到不再产生新实例。
//!
//! 唯一事实所有者是 frontend：禁止在 MIR/emitter 事后补做后置物化，禁止按短名特判，
//! 也禁止把开放实例擦成 `anyref`。调用点无法闭合时保持 `instance` 为空，
//! 由既有 `UnresolvedCallableIdentity` 结构化诊断失败。
//!
//! 泛型 trait 分派先以 witness（`HirTraitDispatch`）保留事实；当 `self` 类型在代入后
//! 完全具体化、且声明确实归属该 trait 时，本模块把它消去为真实实现实例；否则继续
//! 保留 witness，让后端按冻结的 (声明, self 类型) 精确失败，禁止在那里猜实现。

use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::{
    hir::{
        lowering::errors::HirError,
        overload::{
            has_unresolved_generic_arguments, owner_type_generic_binders, substitute_type_self, substitution::substitute_associated_equations,
        },
        trait_system::TraitModuleView,
        type_relation::TypeRelationContext,
    },
    types::{
        Identifier,
        hir::{
            AssociatedType, FunctionType, GenericBinderId, GenericType, HirBlock, HirExpr, HirExprKind, HirFunction,
            HirFunctionReference, HirInstantiatedCallable, HirLambdaIdentity, HirModule, HirParam, HirResolvedCall, HirStatementKind,
            HirTraitDispatch, HirWhereConstraint, ValkyrieType, export_spec::parse_library_glue_operation,
            parse_export_spec_from_annotations,
        },
    },
};

/// 未实例化声明的快照：声明体、owner 类型与 trait 归属都不再依赖模块借用。
#[derive(Debug, Clone)]
struct CallableDeclaration {
    /// 声明所在模块在本次闭包里的下标。
    module: usize,
    /// 声明自身的已冻结下标。
    index: u32,
    /// 声明原样（`instance` 为空，签名与体内仍含 binder）。
    function: HirFunction,
    /// impl owner 类型（含 owner 泛型 binder）；自由函数为空。
    owner_type: Option<ValkyrieType>,
    /// impl 块的 trait 声明身份；inherent impl 为空。
    trait_declaration: Option<nyar_types::ItemId>,
}

/// `GenericBinderId` 到具体类型的代入。`BTreeMap` 保证 `(声明, 代入)` 的稳定序。
type Substitution = BTreeMap<GenericBinderId, ValkyrieType>;

/// 声明是否真的没有泛型 binder：函数自身与 impl owner 都不带 binder 时，空代入
/// 才代表「所有类型本就具体」。impl 成员的泛型 binder 在 owner 上，不在
/// `function.generics` 里；只看后者会把 `ArrayList<T>::new` 这类成员的声明期空代入
/// 误判成具体实例，铸造出永远不会闭合的开放克隆。
fn declaration_has_no_generic_binders(declaration: &CallableDeclaration) -> bool {
    declaration.function.generics.is_empty()
        && declaration.owner_type.as_ref().is_none_or(|owner| owner_type_generic_binders(owner).is_empty())
}

/// 一个固定点实例请求的完整身份。
///
/// 普通声明由 `(声明, 代入)` 唯一确定；trait 默认方法没有 impl owner，同一默认方法
/// 在不同 self 类型上是不同实现，`self_type` 必须进入身份，否则不同接收者会撞实例。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct InstanceRequest {
    declaration: u32,
    substitution: Substitution,
    self_type: Option<ValkyrieType>,
}

/// 跨多次单态化调用保存的实例账本。
///
/// 物化门在每轮固定点之后才补进「实例已达」的声明；这些新声明必须先回到本固定点
/// 铸造体内调用点实例，否则 MIR 只能退化成 Unit 占位。账本按
/// `(声明, 代入, self)` 复用已铸造实例，重入不会重复铸造同一请求。
#[derive(Default)]
pub(crate) struct GenericInstantiationState {
    instances: BTreeMap<InstanceRequest, nyar_types::ItemInstanceId>,
    per_declaration: BTreeMap<u32, usize>,
    next_instance: Option<u32>,
}

/// 固定点需要闭合的可调用请求点：已解析调用合同，或具名函数作值的引用。
enum CallableRequest<'a> {
    /// 已解析调用合同。
    Call(&'a mut HirResolvedCall),
    /// 具名函数作值引用；只有非泛型声明会被改写生成。
    Reference(&'a mut HirFunctionReference),
    /// lambda 创建点：合成函数继承 owner 泛型，必须按环境类型实参解析到具体实例。
    Lambda { identity: &'a mut HirLambdaIdentity, params: &'a [HirParam], return_type: &'a ValkyrieType },
}

/// 单态化固定点允许铸造的最大 `(声明, 代入)` 实例数。
///
/// 每个新实例都 `instantiate_callable` 克隆整个函数体；递归泛型或开放克隆会让
/// 组合无界膨胀（`boom<T>` 体内 `boom::<Box<T>>` 这类更深自代入），上限处必须
/// fail-closed，不许静默截断实例表。正常全量闭包远低于该量级。
pub const MAX_GENERIC_INSTANCES: usize = 65_536;

/// 单态化固定点的最大轮次。
///
/// 每轮至少铸造一个新实例才继续；超过该轮次仍有增长即判定不收敛。
pub const MAX_FIXPOINT_ROUNDS: usize = 512;

/// 固定点超限诊断的 top-N 规模。
const TOP_GROWTH_REPORT: usize = 5;

/// 把声明身份投影为「限定名 + 冻结 span」供超限诊断使用。
fn declaration_label(declarations: &BTreeMap<u32, CallableDeclaration>, index: u32) -> (String, nyar_types::DiagnosticSpan) {
    let Some(declaration) = declarations.get(&index)
    else {
        return (format!("<declaration #{index}>"), nyar_types::DiagnosticSpan { start: 0, end: 0 });
    };
    let function = &declaration.function;
    let qualified = if function.declaring_namespace.parts().is_empty() {
        function.name.as_str().to_string()
    }
    else {
        let namespace = function.declaring_namespace.parts().iter().map(|part| part.as_str()).collect::<Vec<_>>().join(".");
        format!("{namespace}.{}", function.name.as_str())
    };
    (qualified, nyar_types::DiagnosticSpan { start: function.span.span.start as usize, end: function.span.span.end as usize })
}

/// 超限失败：实例数 top-N 按声明聚合，span 取增长最快的声明。
fn instantiation_limit_error(
    kind: super::errors::GenericInstantiationLimitKind,
    limit: usize,
    actual: usize,
    declarations: &BTreeMap<u32, CallableDeclaration>,
    per_declaration: &BTreeMap<u32, usize>,
) -> HirError {
    let mut ranked: Vec<(u32, usize)> = per_declaration.iter().map(|(index, count)| (*index, *count)).collect();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(&right.0)));
    let top_growth =
        ranked.iter().take(TOP_GROWTH_REPORT).map(|(index, count)| (declaration_label(declarations, *index).0, *count)).collect::<Vec<_>>();
    let span = ranked.first().map(|(index, _)| declaration_label(declarations, *index).1).unwrap_or(nyar_types::DiagnosticSpan { start: 0, end: 0 });
    HirError::GenericInstantiationLimitExceeded {
        kind,
        limit,
        actual,
        top_growth,
        source: nyar_types::DiagnosticSource { id: "<hir_lowering>".into(), revision: None },
        span,
    }
}

/// 把仍指向 trait 方法槽的 witness 换成具体 impl 成员的 (声明下标, 代入)。
///
/// witness 的 `method_declaration` 携带两种合法身份（解析期两处冻结点）：
/// 1. 具体接收者路径（`try_resolve_instance_method`）：重载决胜已经选定唯一 impl
///    成员，witness 冻结的就是该成员的声明——实现身份已定，固定点只剩代入闭合。
/// 2. 开放接收者/trait-bound 路径：实现尚不可知，witness 冻结的是 trait 方法槽
///    声明——这里按 (trait 声明, 成员名, impl owner 与接收者实例同一, 成员签名统一)
///    搜索唯一 impl 成员，候选不唯一或缺位都保持 witness。
/// 两种身份按 `declarations` 成员资格机械区分（槽位声明不在 impl 成员表里），
/// 不得回退首个候选或按名字猜一个「看起来合适」的实现（`Hash::hash` 被猜成
/// `i64.infix -` 就是这么来的）。
fn discharge_trait_witness(
    declarations: &BTreeMap<u32, CallableDeclaration>,
    trait_members: &BTreeMap<u32, String>,
    trait_views: &[TraitModuleView],
    call: &HirResolvedCall,
) -> Option<(u32, Substitution, Option<ValkyrieType>)> {
    let dispatch = call.trait_dispatch.as_ref()?;
    // 守卫：声明期 trait 默认体里的 `self` 仍是 `Self` 占位，不是实现类型。把
    // `self = Self` 铸成实例等于给单态化留一个永不闭合的开放身份；必须保持 witness，
    // 等默认体按具体 self 类型实例化、代入收敛后再结算。
    if matches!(&dispatch.self_type, ValkyrieType::r#SelfType | ValkyrieType::AutoType) {
        return None;
    }
    if has_unresolved_generic_arguments(&dispatch.self_type) {
        return None;
    }
    // 身份形态 1：witness 直接冻结了已选定的 impl 成员声明或 trait 默认方法声明。
    // trait 归属必须与 witness 冻结的 trait 一致（不一致即身份损坏，保持 witness
    // 由后端精确失败）；代入仍由 (owner, 成员签名) 与调用合同统一推出，不从调用点照搬。
    if let Some(declaration) = declarations.get(&dispatch.method_declaration.index()) {
        if declaration.trait_declaration != Some(dispatch.trait_declaration) {
            return None;
        }
        let equations = declaration_equations(declaration, &dispatch.self_type, trait_views);
        let substitution = derive_witness_substitution(declaration, call, &equations)?;
        // 默认方法没有 impl owner：具体 self 类型就是它的实现身份，必须进实例键。
        let self_type = if declaration.owner_type.is_none() { Some(dispatch.self_type.clone()) } else { None };
        return Some((declaration.index, substitution, self_type));
    }
    // 身份形态 2：witness 冻结的是 trait 方法槽声明，恢复成员名后搜索唯一实现。
    let member_name = trait_members.get(&dispatch.method_declaration.index())?;
    let mut matched: Option<(u32, Substitution, Option<ValkyrieType>)> = None;
    for declaration in declarations.values() {
        if declaration.trait_declaration != Some(dispatch.trait_declaration) {
            continue;
        }
        if declaration.function.name.as_str() != member_name.as_str() {
            continue;
        }
        let Some(substitution) = derive_witness_substitution(declaration, call, &BTreeMap::new())
        else {
            continue;
        };
        if matched.is_some() {
            return None;
        }
        matched = Some((declaration.index, substitution, None));
    }
    matched
}

/// 具体 self 类型上该声明所属 trait 的关联类型等式。
///
/// impl 成员与自由函数的 owner 已在声明里，关联投影由 owner 自身结算；只有 trait
/// 默认方法（`owner_type` 为空）需要按具体 self 类型去查它的唯一 impl。
fn declaration_equations(
    declaration: &CallableDeclaration,
    self_type: &ValkyrieType,
    trait_views: &[TraitModuleView],
) -> BTreeMap<Identifier, ValkyrieType> {
    let Some(trait_declaration) = declaration.trait_declaration
    else {
        return BTreeMap::new();
    };
    trait_views
        .get(declaration.module)
        .and_then(|view| view.associated_type_equations(trait_declaration, self_type, &[]))
        .unwrap_or_default()
}

/// witness 候选的精确代入：impl owner 必须与 witness 冻结的接收者实例结构化同一，
/// 成员签名必须与调用合同逐项等长统一，二者共用一张代入表以避免各自自洽。
///
/// 不做等长检查时，形参个数不同的候选会退化成「只按返回类型匹配」，这正是跨 owner
/// 错选 witness 的来源；因此这里对不等长候选直接判不合格。
fn derive_witness_substitution(
    declaration: &CallableDeclaration,
    call: &HirResolvedCall,
    equations: &BTreeMap<Identifier, ValkyrieType>,
) -> Option<Substitution> {
    let dispatch = call.trait_dispatch.as_ref()?;
    // trait 默认方法挂在 trait 上，没有 impl owner：具体 self 类型就是它的 owner。
    let owner = declaration.owner_type.as_ref().unwrap_or(&dispatch.self_type);
    let mut substitution = Substitution::new();
    if !unify_instantiation(owner, &dispatch.self_type, &mut substitution) {
        return None;
    }
    // owner 的每个 binder 都必须被接收者实例固定；否则该 impl 在实例级并不成立。
    if owner_type_generic_binders(owner).iter().any(|binder| !substitution.contains_key(binder)) {
        return None;
    }
    let declared_params = declaration
        .function
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| substitute_associated_equations(&declared_param_type(param, index, Some(owner)), owner, equations))
        .collect::<Vec<_>>();
    if declared_params.len() != call.parameter_types.len() {
        return None;
    }
    for (declared, actual) in declared_params.iter().zip(&call.parameter_types) {
        if !unify_instantiation(declared, actual, &mut substitution) {
            return None;
        }
    }
    let declared_return =
        substitute_associated_equations(&declared_type(&declaration.function.return_type, Some(owner)), owner, equations);
    if !unify_instantiation(&declared_return, &call.return_type, &mut substitution) {
        return None;
    }
    if substitution.values().any(has_unresolved_generic_arguments) {
        return None;
    }
    Some(substitution)
}

/// 从完整源码闭包出发，把开放泛型 impl 成员的调用点闭合成真实 callable 实例。
///
/// 实例下标从闭包内已冻结的最大 `ItemInstanceId` 续号，顺序只取决于
/// `(声明下标, 代入)` 的字典序，因此同一输入恒产生同一实例空间。
///
/// `gate` 为物化可达性门：不可达 callable 的体内调用点不得播种实例化请求，
/// 导出面 glue 种子同样只覆盖门内声明；`None` 保持全闭包物化（单源路径）。
///
/// `provider_seeds` 为 Resolver 绑定给出的「合同声明下标 → `[host_provider]`
/// 声明下标」映射：合同调用点的具体代入同时为 provider 声明插入同代入请求，
/// provider 的闭合副本在同一个固定点铸造，其体内宿主导入调用随之闭合；
/// 实例身份、签名与实现全部来自声明与 provider 本身，不存在名字表或空体兜底。
pub(crate) fn instantiate_generic_callables(
    modules: &mut [HirModule],
    gate: Option<&crate::compiler::hir::reachability::HirMaterializationGate>,
    provider_seeds: &BTreeMap<u32, u32>,
    state: &mut GenericInstantiationState,
) -> Result<(), HirError> {
    let (declarations, trait_members) = collect_impl_declarations(modules);
    if declarations.is_empty() {
        return Ok(());
    }
    // 每个声明模块的 trait 证据视图：实例化期用它结算解析期被延后的开放义务。
    let relations = modules.iter().map(TypeRelationContext::from_module).collect::<Vec<_>>();
    // trait 默认方法的关联类型等式按具体 self 类型查询，和上面的证据视图同源。
    let trait_views = modules.iter().map(TraitModuleView::from_module).collect::<Vec<_>>();
    let max_instance = modules.iter().filter_map(super::max_hir_callable_instance_index).max().map_or(0, |index| index.saturating_add(1));
    let next_instance = state.next_instance.get_or_insert(max_instance);
    if max_instance > *next_instance {
        *next_instance = max_instance;
    }
    let next_instance = &mut *next_instance;
    // lambda 环境聚合的泛型声明表：合成环境的类型实参是 owner 代入的冻结来源。
    let environment_generics = modules
        .iter()
        .flat_map(|module| module.structs.iter())
        .filter_map(|structure| {
            let declaration = structure.constructor_declaration?;
            Some((declaration.index(), structure.generics.clone()))
        })
        .collect::<BTreeMap<u32, Vec<GenericType>>>();
    let instances = &mut state.instances;
    let per_declaration = &mut state.per_declaration;
    let mut rounds = 0usize;
    loop {
        rounds = rounds.saturating_add(1);
        // 1. 收集仍指向开放声明的调用点。
        let mut requests: BTreeSet<InstanceRequest> = BTreeSet::new();
        for module in modules.iter_mut() {
            visit_module_calls(module, gate, &mut |request| {
                let call = match request {
                    CallableRequest::Call(call) => call,
                    // 具名函数作值：非泛型声明按空代入照常铸造实例；
                    // 声明不在闭包表里（如抽象无泛型 intrinsic）保持空，由后端 fail-closed。
                    CallableRequest::Reference(reference) => {
                        let Some(index) = reference.declaration.map(|declaration| declaration.index())
                        else {
                            return;
                        };
                        if declarations.get(&index).is_some_and(declaration_has_no_generic_binders) {
                            requests.insert(InstanceRequest { declaration: index, substitution: Substitution::new(), self_type: None });
                        }
                        return;
                    }
                    // lambda 创建点：合成函数若是泛型声明，必须按环境类型实参铸造具体
                    // 实例；单态合成函数的声明期实例已经冻结，无需请求。
                    CallableRequest::Lambda { identity, params, return_type } => {
                        let Some(index) = identity.declaration.map(|declaration| declaration.index())
                        else {
                            return;
                        };
                        if !declarations.get(&index).is_some_and(|declaration| !declaration.function.generics.is_empty()) {
                            return;
                        }
                        let Some(substitution) =
                            derive_lambda_substitution(&declarations, &environment_generics, index, identity, params, return_type)
                        else {
                            return;
                        };
                        if substitution.values().any(has_unresolved_generic_arguments) {
                            return;
                        }
                        requests.insert(InstanceRequest { declaration: index, substitution, self_type: None });
                        return;
                    }
                };
                let Some(index) = call.declaration.map(|declaration| declaration.index())
                else {
                    return;
                };
                // 非泛型 callable 的身份在声明期已冻结成唯一实例，固定点不得再铸造重复
                // 实例；泛型声明的声明期种子只是占位，调用点的具体代入仍必须铸造真实例，
                // 并在回填时覆盖它。witness 调用的 `instance` 恒为空，不受影响。
                if call.instance.is_some() && declarations.get(&index).is_some_and(declaration_has_no_generic_binders) {
                    return;
                }
                // witness 的 declaration 是 trait 方法槽，不在 impl 成员表里；
                // 必须先换成具体 impl 成员，再按 (声明下标, 代入) 铸造实例。
                if call.trait_dispatch.is_some() {
                    let Some((index, substitution, self_type)) = discharge_trait_witness(&declarations, &trait_members, &trait_views, call)
                    else {
                        return;
                    };
                    let Some(declaration) = declarations.get(&index)
                    else {
                        return;
                    };
                    let self_type_closed =
                        self_type.as_ref().is_none_or(|self_type| !has_unresolved_generic_arguments(self_type));
                    if (!substitution.is_empty() || declaration_has_no_generic_binders(declaration))
                        && self_type_closed
                        && !substitution.values().any(has_unresolved_generic_arguments)
                    {
                        requests.insert(InstanceRequest { declaration: index, substitution, self_type });
                    }
                    return;
                }
                let Some(declaration) = declarations.get(&index)
                else {
                    return;
                };
                if let Some(substitution) = derive_substitution(declaration, call) {
                    // 有泛型参数的函数：空 substitution 说明调用侧 binder 和声明侧是同一个
                    // （比如 imply 方法内部自调用），没有替换就不需要铸造实例。
                    // 无泛型函数：空 substitution 是合法的（所有类型本就具体），照常铸造。
                    // 代入仍指向开放 binder 时调用点处于声明模板上下文：开放克隆既不能
                    // 发射，也不能进依赖导入清单，必须 fail-closed 留给具体调用点收口。
                    if (!substitution.is_empty() || declaration_has_no_generic_binders(declaration))
                        && !substitution.values().any(has_unresolved_generic_arguments)
                    {
                        requests.insert(InstanceRequest { declaration: index, substitution, self_type: None });
                    }
                }
                // 绑定合同的 provider 同步播种：合同调用点的具体代入同时决定
                // provider 实例签名，provider 闭合副本在本固定点铸造，其体内
                // 宿主导入调用随之闭合；代入未闭合（调用方仍是泛型上下文）时
                // 不播种，留给后续具体调用点。
                if let Some(&provider_index) = provider_seeds.get(&index)
                    && let Some(provider_declaration) = declarations.get(&provider_index)
                    && !provider_declaration.function.generics.is_empty()
                    && let Some(substitution) = derive_substitution(provider_declaration, call)
                    && !substitution.is_empty()
                    && !substitution.values().any(has_unresolved_generic_arguments)
                {
                    requests.insert(InstanceRequest { declaration: provider_index, substitution, self_type: None });
                }
            });
        }
        // 2. 导出面种子：导出函数签名里的具体聚合实例，必须带上该聚合的全部
        //    `[library_glue]` 成员；这些成员可能没有任何调用点。
        seed_export_surface_requests(modules, gate, &declarations, &mut requests);
        if requests.is_empty() {
            return Ok(());
        }
        // 收敛守卫：递归泛型或开放克隆会让 `(声明, 代入)` 无界膨胀；
        // 上限处 fail-closed，诊断带增长最快的声明 top-N。
        if rounds > MAX_FIXPOINT_ROUNDS {
            return Err(instantiation_limit_error(
                super::errors::GenericInstantiationLimitKind::Rounds,
                MAX_FIXPOINT_ROUNDS,
                rounds,
                &declarations,
                &per_declaration,
            ));
        }
        // 3. 按稳定序铸造新实例。
        let before = instances.len();
        for request in &requests {
            if instances.contains_key(request) {
                continue;
            }
            let InstanceRequest { declaration: index, substitution, self_type } = request;
            let Some(declaration) = declarations.get(index)
            else {
                continue;
            };
            let equations = self_type.as_ref().map_or_else(BTreeMap::new, |self_type| declaration_equations(declaration, self_type, &trait_views));
            // 解析期对开放目标延后的义务必须在这里按具体代入结算：证据缺失则
            // 不铸造实例，调用点保持未闭合，由既有合同验证结构化失败。
            if !where_constraints_satisfied(declaration, substitution, self_type.as_ref(), &equations, relations.get(declaration.module)) {
                continue;
            }
            // 铸造前再守一次闭合：开放代入说明调用方仍是泛型上下文，克隆体签名会
            // 残留开放 binder，既不能发射也不能登记为合同；留给具体调用点收口。
            if substitution.values().any(has_unresolved_generic_arguments)
                || self_type.as_ref().is_some_and(has_unresolved_generic_arguments)
            {
                continue;
            }
            let Some(instance) = nyar_types::ItemInstanceId::from_index(*next_instance)
            else {
                continue;
            };
            *next_instance = (*next_instance).saturating_add(1);
            let callable = instantiate_callable(declaration, substitution, self_type.as_ref(), &equations, instance);
            instances.insert(request.clone(), instance);
            *per_declaration.entry(*index).or_insert(0) += 1;
            modules[declaration.module].instantiated_callables.push(callable);
            if instances.len() > MAX_GENERIC_INSTANCES {
                return Err(instantiation_limit_error(
                    super::errors::GenericInstantiationLimitKind::Instances,
                    MAX_GENERIC_INSTANCES,
                    instances.len(),
                    &declarations,
                    &per_declaration,
                ));
            }
        }
        // 4. 回填调用点。即使本轮没有新实例也必须回填：克隆体里的重复代入
        //    只能指向已存在的实例，不能永远悬空。
        for module in modules.iter_mut() {
            visit_module_calls(module, gate, &mut |request| {
                let call = match request {
                    CallableRequest::Call(call) => call,
                    CallableRequest::Reference(reference) => {
                        let Some(index) = reference.declaration.map(|declaration| declaration.index())
                        else {
                            return;
                        };
                        if let Some(instance) =
                            instances.get(&InstanceRequest { declaration: index, substitution: Substitution::new(), self_type: None })
                        {
                            reference.instance = Some(*instance);
                        }
                        return;
                    }
                    // lambda 创建点回填具体合成函数实例；开放代入保持声明期种子，
                    // 由 MIR/目标侧结构化失败，不静默使用未单态化的实例。
                    CallableRequest::Lambda { identity, params, return_type } => {
                        let Some(index) = identity.declaration.map(|declaration| declaration.index())
                        else {
                            return;
                        };
                        if !declarations.get(&index).is_some_and(|declaration| !declaration.function.generics.is_empty()) {
                            return;
                        }
                        let Some(substitution) =
                            derive_lambda_substitution(&declarations, &environment_generics, index, identity, params, return_type)
                        else {
                            return;
                        };
                        if substitution.values().any(has_unresolved_generic_arguments) {
                            return;
                        }
                        if let Some(instance) =
                            instances.get(&InstanceRequest { declaration: index, substitution, self_type: None })
                        {
                            identity.instance = Some(*instance);
                        }
                        return;
                    }
                };
                let Some(index) = call.declaration.map(|declaration| declaration.index())
                else {
                    return;
                };
                // 回填同样不得覆盖非泛型声明的冻结实例：与收集步保持同一守卫；
                // 泛型声明的种子实例必须让位给具体代入铸造出的实例。
                if call.instance.is_some() && declarations.get(&index).is_some_and(declaration_has_no_generic_binders) {
                    return;
                }
                if call.trait_dispatch.is_some() {
                    let Some((index, substitution, self_type)) = discharge_trait_witness(&declarations, &trait_members, &trait_views, call)
                    else {
                        return;
                    };
                    // 开放代入/开放 self 类型说明还处在声明模板上下文：witness 尚未
                    // 由具体实例偿清，保留 trait evidence 交给具体克隆闭合。
                    if substitution.values().any(has_unresolved_generic_arguments)
                        || self_type.as_ref().is_some_and(has_unresolved_generic_arguments)
                    {
                        return;
                    }
                    if let Some(instance) = instances.get(&InstanceRequest { declaration: index, substitution, self_type }) {
                        // witness 义务已由具体 impl 实例偿清：继续把 trait evidence 传到后端
                        // 只会诱发「按签名挑一个实现」的后置猜测。
                        call.instance = Some(*instance);
                        call.trait_dispatch = None;
                    }
                    return;
                }
                let Some(declaration) = declarations.get(&index)
                else {
                    return;
                };
                let Some(substitution) = derive_substitution(declaration, call)
                else {
                    return;
                };
                // 只有完全具体的代入才代表「按 where 证据结算后被拒绝」；空代入或
                // 仍含开放 binder 的代入是声明体内的自递归/跨结构占位，留给后续
                // 具体克隆闭合，不能在这里抹掉占位实例。
                let concrete_attempt = (!substitution.is_empty() || declaration_has_no_generic_binders(declaration))
                    && !substitution.values().any(has_unresolved_generic_arguments);
                match instances.get(&InstanceRequest { declaration: index, substitution, self_type: None }) {
                    // 开放代入铸造出的实例签名带开放 binder：不能把它回填进调用点，
                    // 否则调用方会被判成“已冻结”，而合同面因开放签名把该行剔掉。
                    Some(instance) if concrete_attempt => call.instance = Some(*instance),
                    Some(_) => {}
                    // 具体代入没有铸造出实例（where 证据不足被拒）时，调用点不得保留
                    // 解析期/声明种子留下的开放占位实例，否则缺失证据会被静默放行。
                    None if concrete_attempt => call.instance = None,
                    None => {}
                }
            });
        }
        if instances.len() == before {
            return Ok(());
        }
    }
}

/// 导出面种子：导出函数签名里出现了哪个具体聚合实例，就必须实例化该聚合声明的
/// 全部 `[library_glue]` 成员。
///
/// 这些成员由宿主 glue 按导出名直通调用，函数体里可能没有任何调用点，只靠调用点
/// 固定点永远到不了。归属仍只由「声明标注 + owner 类型合一」决定，不按成员名猜。
/// 幂等：同一 (声明, 代入) 重复插入由 `BTreeSet` 与实例表去重。
fn seed_export_surface_requests(
    modules: &[HirModule],
    gate: Option<&crate::compiler::hir::reachability::HirMaterializationGate>,
    declarations: &BTreeMap<u32, CallableDeclaration>,
    requests: &mut BTreeSet<InstanceRequest>,
) {
    let mut concretes = BTreeSet::new();
    for module in modules {
        collect_exported_concrete_nominals(module, &mut concretes);
    }
    if concretes.is_empty() {
        return;
    }
    for declaration in declarations.values() {
        // 只有声明标注给出角色的成员才是宿主直通操作。
        if parse_library_glue_operation(&declaration.function.annotations).is_none() {
            continue;
        }
        // 物化可达性门已按导出面标记 glue 根；门外声明不得铸造实例。
        if gate.is_some_and(|gate| !gate.permits_declaration(declaration.function.declaration)) {
            continue;
        }
        let Some(owner) = declaration.owner_type.as_ref()
        else {
            continue;
        };
        // 无泛型的聚合成员由声明本身闭合，不需要按导出面铸造实例。
        if !has_unresolved_generic_arguments(owner) {
            continue;
        }
        for concrete in &concretes {
            let mut substitution = Substitution::new();
            if !unify_instantiation(owner, concrete, &mut substitution) {
                continue;
            }
            // 代入必须闭合：owner 的每个 binder 都要落到具体类型。
            if owner_type_generic_binders(owner).iter().any(|binder| !substitution.contains_key(binder)) {
                continue;
            }
            if substitution.values().any(has_unresolved_generic_arguments) {
                continue;
            }
            requests.insert(InstanceRequest { declaration: declaration.index, substitution, self_type: None });
        }
    }
}

/// 收集导出函数签名里出现的完全具体名义实例；含嵌套实参（`ArrayList<ArrayList<i64>>` 收两层）。
fn collect_exported_concrete_nominals(module: &HirModule, out: &mut BTreeSet<ValkyrieType>) {
    for function in &module.functions {
        // 只有已实例化的导出函数才代表对外签名。
        if function.instance.is_none() || parse_export_spec_from_annotations(&function.annotations).is_none() {
            continue;
        }
        for param in &function.params {
            collect_concrete_nominals(&param.ty, out);
        }
        collect_concrete_nominals(&function.return_type, out);
    }
    for submodule in &module.submodules {
        collect_exported_concrete_nominals(submodule, out);
    }
}

/// 递归收集类型里所有完全具体的名义实例；仍含开放 binder 的形状一律跳过。
fn collect_concrete_nominals(ty: &ValkyrieType, out: &mut BTreeSet<ValkyrieType>) {
    match ty {
        ValkyrieType::Nominal { arguments, .. } => {
            if !arguments.is_empty() && !has_unresolved_generic_arguments(ty) {
                out.insert(ty.clone());
            }
            for argument in arguments {
                collect_concrete_nominals(argument, out);
            }
        }
        ValkyrieType::Apply(base, arguments) => {
            collect_concrete_nominals(base, out);
            for argument in arguments {
                collect_concrete_nominals(argument, out);
            }
        }
        ValkyrieType::Array(element) | ValkyrieType::Nullable(element) => collect_concrete_nominals(element, out),
        ValkyrieType::FixedArray { element, .. } => collect_concrete_nominals(element, out),
        ValkyrieType::Tuple(items) | ValkyrieType::Union(items) | ValkyrieType::Intersection(items) => {
            for item in items {
                collect_concrete_nominals(item, out);
            }
        }
        ValkyrieType::Function(function) => {
            for param in &function.params {
                collect_concrete_nominals(param, out);
            }
            collect_concrete_nominals(&function.return_type, out);
        }
        _ => {}
    }
}

/// 收集所有有 declaration 的 callable 声明——包含 impl 成员和 module-level free function。
/// 不再过滤 `instance`：lowering 阶段就给每个声明分配了一个 ItemInstanceId（等于 ItemId），
/// 但那个 instance 只是"声明身份"，不是特化实例。泛型函数的特化版本需要 generic_instantiation
/// 在这里铸造新的 ItemInstanceId。因此只要有 declaration 就收集。
fn collect_impl_declarations(modules: &[HirModule]) -> (BTreeMap<u32, CallableDeclaration>, BTreeMap<u32, String>) {
    let mut declarations = BTreeMap::new();
    // trait 方法槽声明 -> 成员名：witness 只冻结槽身份，成员名在这里恢复。
    let mut trait_members = BTreeMap::new();
    for (module_index, module) in modules.iter().enumerate() {
        for trait_definition in &module.traits {
            for method in &trait_definition.methods {
                if let Some(declaration) = method.declaration {
                    trait_members.entry(declaration.index()).or_insert_with(|| method.name.to_string());
                }
            }
            // trait 默认方法：方法体归 trait，impl 只提供 Self 与关联类型。默认方法
            // 没有 impl owner，具体 self 类型在 witness 消解时进入实例身份。
            for method in &trait_definition.default_methods {
                if method.is_abstract {
                    continue;
                }
                let Some(declaration) = method.declaration
                else {
                    continue;
                };
                trait_members.entry(declaration.index()).or_insert_with(|| method.name.to_string());
                declarations.entry(declaration.index()).or_insert_with(|| CallableDeclaration {
                    module: module_index,
                    index: declaration.index(),
                    function: method.clone(),
                    owner_type: None,
                    trait_declaration: trait_definition.declaration,
                });
            }
        }
        // impl 成员方法（owner_type 为 impl 目标类型）
        for implementation in &module.impls {
            for method in &implementation.methods {
                // is_abstract 的无泛型函数（如 __i32_eq host_contract）不需要实例化；
                // 有泛型的抽象声明仍要进入闭包表，因为泛型实参由调用点决定。
                if method.is_abstract && method.generics.is_empty() {
                    continue;
                }
                let Some(declaration) = method.declaration
                else {
                    continue;
                };
                declarations.entry(declaration.index()).or_insert_with(|| CallableDeclaration {
                    module: module_index,
                    index: declaration.index(),
                    function: method.clone(),
                    owner_type: Some(implementation.target.clone()),
                    trait_declaration: implementation.trait_declaration,
                });
            }
        }
        // module-level free function（无 owner_type，无 trait）
        for function in &module.functions {
            // is_abstract 的无泛型函数（如 __i32_eq host_contract）不需要实例化；
            // 有泛型的抽象声明仍要进入闭包表，因为泛型实参由调用点决定。
            if function.is_abstract && function.generics.is_empty() {
                continue;
            }
            let Some(declaration) = function.declaration
            else {
                continue;
            };
            declarations.entry(declaration.index()).or_insert_with(|| CallableDeclaration {
                module: module_index,
                index: declaration.index(),
                function: function.clone(),
                owner_type: None,
                trait_declaration: None,
            });
        }
    }
    (declarations, trait_members)
}

/// 声明侧形参类型：`self` 槽的 `AutoType`/`Self` 先落到 owner，再交给调用点统一。
fn declared_param_type(param: &HirParam, index: usize, owner: Option<&ValkyrieType>) -> ValkyrieType {
    let declared = match (&param.ty, index, owner) {
        (ValkyrieType::AutoType | ValkyrieType::r#SelfType, 0, Some(owner)) => owner.clone(),
        (ty, _, _) => ty.clone(),
    };
    declared_type(&declared, owner)
}

/// 声明侧 `Self` 代入 owner；owner 为空时原样返回。
fn declared_type(ty: &ValkyrieType, owner: Option<&ValkyrieType>) -> ValkyrieType {
    match owner {
        Some(owner) => substitute_type_self(ty, owner),
        None => ty.clone(),
    }
}

/// 用开放签名与调用点已特化合同反推 binder→具体类型。
///
/// 只有代入后签名完全闭合（不含未代入 binder）才返回；否则调用点保持开放，
/// 由后续固定点轮次（或 `UnresolvedCallableIdentity`）处理。
fn derive_substitution(declaration: &CallableDeclaration, call: &HirResolvedCall) -> Option<Substitution> {
    let owner = declaration.owner_type.as_ref();
    let declared_params =
        declaration.function.params.iter().enumerate().map(|(index, param)| declared_param_type(param, index, owner)).collect::<Vec<_>>();
    let declared_return = declared_type(&declaration.function.return_type, owner);
    let mut substitution = Substitution::new();
    if declared_params.len() == call.parameter_types.len() {
        for (declared, actual) in declared_params.iter().zip(&call.parameter_types) {
            if !unify_instantiation(declared, actual, &mut substitution) {
                return None;
            }
        }
    }
    if !unify_instantiation(&declared_return, &call.return_type, &mut substitution) {
        return None;
    }
    // 不再检查 substitution 是否为空。空 substitution 有两种合法情况：
    // (a) 声明侧和调用侧共享同一个 binder（同结构泛型调用，不需要跨 binder 替换）；
    // (b) 声明本身就不是泛型，所有类型完全具体。
    // 也不再检查 substitution.values() 是否闭合——跨结构委托调用第一次只会把声明侧 binder
    // 映射到调用方的 binder，后续轮次会继续闭合。只要 unify 全通过，就接受。
    Some(substitution)
}

/// lambda 合成函数的实例代入。
///
/// 合成函数继承 owner 泛型，而环境聚合的实参正是 owner 代入在创建点的冻结结果：
/// 先按环境结构泛型配对得到 owner binder 的代入，再用 lambda 自身签名统一剩余
/// binder（lambda 自定义泛型）。任一 owner/lambda binder 未闭合时不铸造实例，
/// `hoisted.instance` 保持声明期种子，由 MIR 目标侧结构化失败。
fn derive_lambda_substitution(
    declarations: &BTreeMap<u32, CallableDeclaration>,
    environment_generics: &BTreeMap<u32, Vec<GenericType>>,
    declaration_index: u32,
    identity: &HirLambdaIdentity,
    params: &[HirParam],
    return_type: &ValkyrieType,
) -> Option<Substitution> {
    let declaration = declarations.get(&declaration_index)?;
    if declaration.function.generics.is_empty() {
        return Some(Substitution::new());
    }
    let mut substitution = Substitution::new();
    if let Some(environment) = &identity.environment {
        let ValkyrieType::Nominal { declaration: environment_declaration, arguments, .. } = environment.ty.as_ref()?
        else {
            return None;
        };
        let generics = environment_generics.get(&environment_declaration.index())?;
        if generics.len() != arguments.len() {
            return None;
        }
        for (generic, argument) in generics.iter().zip(arguments) {
            if let Some(binder) = generic.binder {
                substitution.insert(binder, argument.clone());
            }
        }
    }
    // 合成函数形参 = lambda 形参 + 可选尾环境形参；模板与已具体化的实际类型统一。
    let environment_params = usize::from(identity.environment.is_some());
    if declaration.function.params.len() != params.len() + environment_params {
        return None;
    }
    for (declared, actual) in declaration.function.params.iter().zip(params) {
        if !unify_instantiation(&declared.ty, &actual.ty, &mut substitution) {
            return None;
        }
    }
    if !unify_instantiation(&declaration.function.return_type, return_type, &mut substitution) {
        return None;
    }
    // owner + lambda 的每个 binder 都必须有具体代入；开放 binder 保持实例空缺。
    declaration
        .function
        .generics
        .iter()
        .all(|generic| {
            generic.binder.is_some_and(|binder| {
                substitution.get(&binder).is_some_and(|bound| !has_unresolved_generic_arguments(bound))
            })
        })
        .then_some(substitution)
}

/// 结构化统一：只认已冻结的 binder 与名义声明身份，不做短名匹配。
pub(crate) fn unify_instantiation(template: &ValkyrieType, concrete: &ValkyrieType, substitution: &mut Substitution) -> bool {
    match template {
        ValkyrieType::Generic(generic) => {
            let Some(binder) = generic.binder
            else {
                return template == concrete;
            };
            match substitution.get(&binder) {
                Some(bound) => bound == concrete,
                None => {
                    // 自引用跳过：template binder == concrete binder 时不插入，避免 sub[X]=Generic(X) 循环。
                    // 同结构泛型调用（如 Array<T> → Array<T>）两边 binder 相同，空 substitution 是合法中间态。
                    if let ValkyrieType::Generic(other) = concrete {
                        if other.binder == Some(binder) {
                            return true;
                        }
                    }
                    substitution.insert(binder, concrete.clone());
                    true
                }
            }
        }
        ValkyrieType::Nominal { declaration, arguments, .. } => match concrete {
            ValkyrieType::Nominal { declaration: actual, arguments: actual_arguments, .. }
                if declaration == actual && arguments.len() == actual_arguments.len() =>
            {
                arguments.iter().zip(actual_arguments).all(|(template, concrete)| unify_instantiation(template, concrete, substitution))
            }
            _ => false,
        },
        ValkyrieType::Apply(base, arguments) => match concrete {
            ValkyrieType::Apply(actual, actual_arguments) if arguments.len() == actual_arguments.len() => {
                unify_instantiation(base, actual, substitution)
                    && arguments.iter().zip(actual_arguments).all(|(template, concrete)| unify_instantiation(template, concrete, substitution))
            }
            _ => false,
        },
        ValkyrieType::Array(element) => match concrete {
            ValkyrieType::Array(actual) => unify_instantiation(element, actual, substitution),
            _ => false,
        },
        ValkyrieType::Nullable(element) => match concrete {
            ValkyrieType::Nullable(actual) => unify_instantiation(element, actual, substitution),
            _ => false,
        },
        ValkyrieType::FixedArray { element, length } => match concrete {
            ValkyrieType::FixedArray { element: actual, length: actual_length } if length == actual_length => {
                unify_instantiation(element, actual, substitution)
            }
            _ => false,
        },
        ValkyrieType::Tuple(items) => match concrete {
            ValkyrieType::Tuple(actual) if items.len() == actual.len() => {
                items.iter().zip(actual).all(|(template, concrete)| unify_instantiation(template, concrete, substitution))
            }
            _ => false,
        },
        ValkyrieType::Union(items) => match concrete {
            ValkyrieType::Union(actual) if items.len() == actual.len() => {
                items.iter().zip(actual).all(|(template, concrete)| unify_instantiation(template, concrete, substitution))
            }
            _ => false,
        },
        ValkyrieType::Intersection(items) => match concrete {
            ValkyrieType::Intersection(actual) if items.len() == actual.len() => {
                items.iter().zip(actual).all(|(template, concrete)| unify_instantiation(template, concrete, substitution))
            }
            _ => false,
        },
        ValkyrieType::Function(function) => match concrete {
            ValkyrieType::Function(actual) if function.params.len() == actual.params.len() => {
                function.params.iter().zip(&actual.params).all(|(template, concrete)| unify_instantiation(template, concrete, substitution))
                    && unify_instantiation(&function.return_type, &actual.return_type, substitution)
            }
            _ => false,
        },
        _ => template == concrete,
    }
}

/// 铸造一个实例：克隆声明体并应用代入，实例身份与签名同时冻结。
fn instantiate_callable(
    declaration: &CallableDeclaration,
    substitution: &Substitution,
    self_type: Option<&ValkyrieType>,
    equations: &BTreeMap<Identifier, ValkyrieType>,
    instance: nyar_types::ItemInstanceId,
) -> HirInstantiatedCallable {
    // trait 默认方法没有 impl owner：调用点冻结的 self 类型就是它的 owner。
    let owner = self_type.or(declaration.owner_type.as_ref());
    let mut function = declaration.function.clone();
    function.instance = Some(instance);
    // 全部泛型已在签名与体内代入，实例不再携带自己的泛型参数。
    function.generics = Vec::new();
    for (index, param) in function.params.iter_mut().enumerate() {
        let declared = declared_param_type(&declaration.function.params[index], index, owner);
        param.ty = instantiate_declared_type(&declared, owner, substitution, equations);
    }
    function.return_type = instantiate_declared_type(&declaration.function.return_type, owner, substitution, equations);
    for constraint in &mut function.where_constraints {
        substitute_where_constraint(constraint, substitution, owner, equations);
    }
    substitute_block(&mut function.body, substitution, owner, equations);
    let owner_type = owner.map(|owner| substitute_type(owner, substitution));
    HirInstantiatedCallable { function, owner_type, trait_declaration: declaration.trait_declaration }
}

/// 声明侧类型 → 实例类型：先落 `Self`/owner，再应用 binder 代入，最后按 impl
/// 关联类型等式消去 `owner::Assoc` 投影。
fn instantiate_declared_type(
    ty: &ValkyrieType,
    owner: Option<&ValkyrieType>,
    substitution: &Substitution,
    equations: &BTreeMap<Identifier, ValkyrieType>,
) -> ValkyrieType {
    let substituted = substitute_type(&declared_type(ty, owner), substitution);
    match owner {
        Some(owner) => substitute_associated_equations(&substituted, owner, equations),
        None => substituted,
    }
}

/// 把声明侧类型里的 binder 换成代入结果。
/// Generic binder 可能指向另一个 Generic binder（如跨结构委托：67 → 109），
/// 这里用循环 resolve 到底，直到到达具体类型或无进一步代入。
pub(crate) fn substitute_type(ty: &ValkyrieType, substitution: &Substitution) -> ValkyrieType {
    match ty {
        ValkyrieType::Generic(generic) => {
            let Some(mut current) = generic.binder
            else {
                return ty.clone();
            };
            let mut visited = std::collections::HashSet::new();
            // 循环传递 resolve：sub[67]=Generic(109), sub[109]=i64 → resolve(67)=i64
            while let Some(replacement) = substitution.get(&current) {
                if !visited.insert(current) {
                    // 循环引用，停在这里
                    break;
                }
                match replacement {
                    ValkyrieType::Generic(g) => {
                        if let Some(next) = g.binder {
                            current = next;
                        }
                        else {
                            return replacement.clone();
                        }
                    }
                    _ => return replacement.clone(),
                }
            }
            ty.clone()
        }
        ValkyrieType::Nominal { declaration, arguments, primitive_identity } => ValkyrieType::Nominal {
            declaration: *declaration,
            arguments: arguments.iter().map(|argument| substitute_type(argument, substitution)).collect(),
            primitive_identity: primitive_identity.clone(),
        },
        ValkyrieType::Apply(base, arguments) => ValkyrieType::Apply(
            Box::new(substitute_type(base, substitution)),
            arguments.iter().map(|argument| substitute_type(argument, substitution)).collect(),
        ),
        ValkyrieType::Array(element) => ValkyrieType::Array(Box::new(substitute_type(element, substitution))),
        ValkyrieType::Nullable(element) => ValkyrieType::Nullable(Box::new(substitute_type(element, substitution))),
        ValkyrieType::FixedArray { element, length } => {
            ValkyrieType::FixedArray { element: Box::new(substitute_type(element, substitution)), length: *length }
        }
        ValkyrieType::Tuple(items) => ValkyrieType::Tuple(items.iter().map(|item| substitute_type(item, substitution)).collect()),
        ValkyrieType::Union(items) => ValkyrieType::Union(items.iter().map(|item| substitute_type(item, substitution)).collect()),
        ValkyrieType::Intersection(items) => ValkyrieType::Intersection(items.iter().map(|item| substitute_type(item, substitution)).collect()),
        ValkyrieType::Function(function) => ValkyrieType::Function(Box::new(FunctionType {
            params: function.params.iter().map(|param| substitute_type(param, substitution)).collect(),
            return_type: substitute_type(&function.return_type, substitution),
        })),
        ValkyrieType::Associated(associated) => ValkyrieType::Associated(Box::new(AssociatedType {
            base: substitute_type(&associated.base, substitution),
            name: associated.name.clone(),
            type_arguments: associated.type_arguments.iter().map(|argument| substitute_type(argument, substitution)).collect(),
        })),
        _ => ty.clone(),
    }
}

fn substitute_where_constraint(
    constraint: &mut HirWhereConstraint,
    substitution: &Substitution,
    owner: Option<&ValkyrieType>,
    equations: &BTreeMap<Identifier, ValkyrieType>,
) {
    constraint.target = instantiate_declared_type(&constraint.target, owner, substitution, equations);
    for bound in &mut constraint.bounds {
        for argument in &mut bound.type_arguments {
            *argument = instantiate_declared_type(argument, owner, substitution, equations);
        }
        for equation in &mut bound.associated_types {
            equation.ty = instantiate_declared_type(&equation.ty, owner, substitution, equations);
        }
    }
}

/// impl 体内：把声明槽位中的 `Self` 代入 impl 头部 target（含泛型实参）。
///
/// 与 [`substitute_function`] 的声明期代入同一口径，但只处理方法体，且发生在调用解析
/// 之前。方法体里 `let`/cast/lambda 等声明槽位若保留未代入的 `SelfType`，局部变量就会
/// 带上一个与 owner 不同的类型身份，成员调用只看到 owner 上的候选却拿 `SelfType` 去
/// 匹配，最终报 `unresolved_call`。
pub(crate) fn substitute_impl_self_declarations(block: &mut HirBlock, owner: &ValkyrieType) {
    let substitution = Substitution::new();
    let equations = BTreeMap::new();
    substitute_block(block, &substitution, Some(owner), &equations);
}

fn substitute_block(
    block: &mut HirBlock,
    substitution: &Substitution,
    owner: Option<&ValkyrieType>,
    equations: &BTreeMap<Identifier, ValkyrieType>,
) {
    for statement in &mut block.statements {
        match &mut statement.kind {
            HirStatementKind::Let { ty, initializer, .. } => {
                let expected = ty.as_mut().map(|ty| {
                    *ty = instantiate_declared_type(ty, owner, substitution, equations);
                    ty.clone()
                });
                if let Some(initializer) = initializer {
                    if let Some(expected) = expected {
                        // 聚合字面量在声明体里以被构造类型自身的 binder 记录类型参数，
                        // 外层声明代入覆盖不到这些跨结构 binder。用语句标注的具体类型
                        // 补齐它们的映射，否则字面量体内的泛型 callee 永远停在开放克隆。
                        let mut extended = substitution.clone();
                        extend_substitution_from_expected_type(&mut extended, &expected, initializer, owner, substitution, equations);
                        substitute_expr(initializer, &extended, owner, equations);
                    }
                    else {
                        substitute_expr(initializer, substitution, owner, equations);
                    }
                }
            }
            HirStatementKind::Expr(expr) => substitute_expr(expr, substitution, owner, equations),
        }
    }
    if let Some(expr) = &mut block.expr {
        substitute_expr(expr, substitution, owner, equations);
    }
}

/// 用语句标注的具体类型补齐聚合字面量/构造调用冻结签名里的跨结构 binder 映射。
///
/// `BTreeNode<K, V>` 这类字面量在声明体里被解析成「被构造类型自身的 binder」，
/// 外层声明（`BTreeMap<K, V>`）的代入键覆盖不到它们，于是字面量字段初始化的泛型
/// callee（`ArrayList::new`）在克隆体里始终带着开放 binder。标注类型已在代入后
/// 具体化，可与字面量的已代入签名统一，得到这些 binder 的具体映射；只补充缺失项，
/// 不覆盖外层代入已确定的映射。
fn extend_substitution_from_expected_type(
    extended: &mut Substitution,
    expected: &ValkyrieType,
    initializer: &HirExpr,
    owner: Option<&ValkyrieType>,
    substitution: &Substitution,
    equations: &BTreeMap<Identifier, ValkyrieType>,
) {
    let shape = match &initializer.kind {
        HirExprKind::Construct { resolved: Some(resolved), .. } => &resolved.return_type,
        HirExprKind::Call { resolved: Some(resolved), .. } => &resolved.return_type,
        _ => return,
    };
    let declared = instantiate_declared_type(shape, owner, substitution, equations);
    let mut extra = Substitution::new();
    if !unify_instantiation(&declared, expected, &mut extra) {
        return;
    }
    for (binder, ty) in extra {
        extended.entry(binder).or_insert(ty);
    }
}

fn substitute_resolved_call(
    call: &mut HirResolvedCall,
    substitution: &Substitution,
    owner: Option<&ValkyrieType>,
    equations: &BTreeMap<Identifier, ValkyrieType>,
) {
    for parameter in &mut call.parameter_types {
        *parameter = instantiate_declared_type(parameter, owner, substitution, equations);
    }
    call.return_type = instantiate_declared_type(&call.return_type, owner, substitution, equations);
    if let Some(payload) = &mut call.extractor_payload_type {
        *payload = instantiate_declared_type(payload, owner, substitution, equations);
    }
    if let Some(dispatch) = &mut call.trait_dispatch {
        let self_type = instantiate_declared_type(&dispatch.self_type, owner, substitution, equations);
        *dispatch =
            HirTraitDispatch { trait_declaration: dispatch.trait_declaration, method_declaration: dispatch.method_declaration, self_type };
    }
}

/// 实例化期的 where 证据结算：把声明约束按代入展开后交给模块的 trait 视图。
///
/// 仍含开放 binder 的约束由 `satisfies_where_constraints` 继续延后；目标已具体化却
/// 没有显式 impl evidence 时返回 false，调用点不得铸造实例。空 substitutions 是
/// 因为约束在进入本函数前已按实例代入完毕。
fn where_constraints_satisfied(
    declaration: &CallableDeclaration,
    substitution: &Substitution,
    self_type: Option<&ValkyrieType>,
    equations: &BTreeMap<Identifier, ValkyrieType>,
    relations: Option<&TypeRelationContext>,
) -> bool {
    let Some(relations) = relations
    else {
        return false;
    };
    let owner = self_type.or(declaration.owner_type.as_ref());
    let mut constraints = declaration.function.where_constraints.clone();
    for constraint in &mut constraints {
        substitute_where_constraint(constraint, substitution, owner, equations);
    }
    relations.satisfies_where_constraints(&constraints, &BTreeMap::new())
}

fn substitute_expr(
    expr: &mut HirExpr,
    substitution: &Substitution,
    owner: Option<&ValkyrieType>,
    equations: &BTreeMap<Identifier, ValkyrieType>,
) {
    match &mut expr.kind {
        HirExprKind::Cast { ty, expr: inner } => {
            *ty = instantiate_declared_type(ty, owner, substitution, equations);
            substitute_expr(inner, substitution, owner, equations);
        }
        HirExprKind::Lambda { params, return_type, body, captures, hoisted, .. } => {
            for param in params {
                param.ty = instantiate_declared_type(&param.ty, owner, substitution, equations);
            }
            *return_type = instantiate_declared_type(return_type, owner, substitution, equations);
            // 捕获事实与环境聚合随 owner 单态化一起代入：不同实例得到不同环境
            // 类型实参，不能共享同一份 `Nominal{arguments}`。
            for capture in captures {
                capture.ty = instantiate_declared_type(&capture.ty, owner, substitution, equations);
            }
            if let Some(environment) = hoisted.as_mut().and_then(|identity| identity.environment.as_mut()) {
                if let Some(ty) = environment.ty.as_mut() {
                    *ty = instantiate_declared_type(ty, owner, substitution, equations);
                }
            }
            substitute_block(body, substitution, owner, equations);
        }
        HirExprKind::TryScope { result_type, body, .. } => {
            if let Some(ty) = result_type {
                *ty = instantiate_declared_type(ty, owner, substitution, equations);
            }
            substitute_block(body, substitution, owner, equations);
        }
        HirExprKind::Block(block) => substitute_block(block, substitution, owner, equations),
        HirExprKind::If { condition, then_branch, else_branch, .. } => {
            substitute_expr(condition, substitution, owner, equations);
            substitute_block(then_branch, substitution, owner, equations);
            if let Some(branch) = else_branch {
                substitute_block(branch, substitution, owner, equations);
            }
        }
        HirExprKind::IfLet { scrutinee, then_branch, else_branch, .. } => {
            substitute_expr(scrutinee, substitution, owner, equations);
            substitute_block(then_branch, substitution, owner, equations);
            if let Some(branch) = else_branch {
                substitute_block(branch, substitution, owner, equations);
            }
        }
        HirExprKind::Return(Some(value))
        | HirExprKind::Yield(Some(value))
        | HirExprKind::Await(value)
        | HirExprKind::Awake(value)
        | HirExprKind::BlockOn(value)
        | HirExprKind::YieldFrom(value)
        | HirExprKind::Raise(value)
        | HirExprKind::Resume(value) => substitute_expr(value, substitution, owner, equations),
        HirExprKind::TryPropagate { inner, resolved_ok_variant } => {
            substitute_expr(inner, substitution, owner, equations);
            if let Some(variant) = resolved_ok_variant {
                substitute_resolved_call(variant, substitution, owner, equations);
            }
        }
        HirExprKind::Call { callee, args, resolved } => {
            substitute_expr(callee, substitution, owner, equations);
            for arg in args {
                substitute_expr(&mut arg.value, substitution, owner, equations);
            }
            if let Some(resolved) = resolved {
                substitute_resolved_call(resolved, substitution, owner, equations);
            }
        }
        HirExprKind::Construct { args, resolved, .. } => {
            for arg in args {
                substitute_expr(arg, substitution, owner, equations);
            }
            if let Some(resolved) = resolved {
                substitute_resolved_call(resolved, substitution, owner, equations);
            }
        }
        HirExprKind::FieldAccess { object, .. } | HirExprKind::FieldInit { value: object, .. } => {
            substitute_expr(object, substitution, owner, equations);
        }
        HirExprKind::StoreField { object, value, .. } => {
            substitute_expr(object, substitution, owner, equations);
            substitute_expr(value, substitution, owner, equations);
        }
        HirExprKind::GenericApply { callee, arguments } => {
            substitute_expr(callee, substitution, owner, equations);
            for argument in arguments {
                *argument = instantiate_declared_type(argument, owner, substitution, equations);
            }
        }
        HirExprKind::ArrayNew { element_type, length } => {
            *element_type = instantiate_declared_type(element_type, owner, substitution, equations);
            substitute_expr(length, substitution, owner, equations);
        }
        HirExprKind::ArrayLiteral { items } | HirExprKind::TupleLiteral { items } => {
            for item in items {
                substitute_expr(item, substitution, owner, equations);
            }
        }
        HirExprKind::Match { scrutinee, arms, .. } | HirExprKind::Case { scrutinee, arms, .. } => {
            substitute_expr(scrutinee, substitution, owner, equations);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    substitute_expr(guard, substitution, owner, equations);
                }
                substitute_expr(&mut arm.body, substitution, owner, equations);
            }
        }
        HirExprKind::Loop { iterator, body, condition, .. } => {
            if let Some(iterator) = iterator {
                substitute_expr(iterator, substitution, owner, equations);
            }
            if let Some(condition) = condition {
                substitute_expr(condition, substitution, owner, equations);
            }
            substitute_block(body, substitution, owner, equations);
        }
        HirExprKind::Catch { expr: inner, arms } => {
            substitute_expr(inner, substitution, owner, equations);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    substitute_expr(guard, substitution, owner, equations);
                }
                substitute_expr(&mut arm.body, substitution, owner, equations);
            }
        }
        HirExprKind::With { base, updates } => {
            substitute_expr(base, substitution, owner, equations);
            for (_, update) in updates {
                substitute_expr(update, substitution, owner, equations);
            }
        }
        HirExprKind::SuperCall { args, .. } => {
            for arg in args {
                substitute_expr(arg, substitution, owner, equations);
            }
        }
        HirExprKind::Assign { value, .. } | HirExprKind::Break { expr: Some(value), .. } => {
            substitute_expr(value, substitution, owner, equations);
        }
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            for (_, value) in fields {
                substitute_expr(value, substitution, owner, equations);
            }
            for method in methods {
                substitute_function(method, substitution, owner, equations);
            }
        }
        _ => {}
    }
}

fn substitute_function(
    function: &mut HirFunction,
    substitution: &Substitution,
    owner: Option<&ValkyrieType>,
    equations: &BTreeMap<Identifier, ValkyrieType>,
) {
    for param in &mut function.params {
        param.ty = instantiate_declared_type(&param.ty, owner, substitution, equations);
    }
    function.return_type = instantiate_declared_type(&function.return_type, owner, substitution, equations);
    for constraint in &mut function.where_constraints {
        substitute_where_constraint(constraint, substitution, owner, equations);
    }
    substitute_block(&mut function.body, substitution, owner, equations);
}

/// 遍历模块（含子模块）内全部 callable 体内的可调用请求点（调用合同与函数作值引用）。
///
/// `gate` 为物化可达性门：门不可达 callable 的体内调用点不参与请求播种与回填，
/// 不可达子图因此不会把泛型实例化拉进物化集合。
fn visit_module_calls(
    module: &mut HirModule,
    gate: Option<&crate::compiler::hir::reachability::HirMaterializationGate>,
    visit: &mut impl FnMut(CallableRequest<'_>),
) {
    let permits = |function: &HirFunction| {
        gate.is_none_or(|gate| gate.permits_declaration(function.declaration)) && declaration_body_is_closed(function)
    };
    for function in &mut module.functions {
        if permits(function) {
            visit_block(&mut function.body, visit);
        }
    }
    for structure in &mut module.structs {
        for method in &mut structure.methods {
            if permits(method) {
                visit_block(&mut method.body, visit);
            }
        }
    }
    for singleton in &mut module.singletons {
        for method in singleton
            .methods
            .iter_mut()
            .chain(singleton.constructor.iter_mut().map(Box::as_mut))
            .chain(singleton.finalizer.iter_mut().map(Box::as_mut))
        {
            if permits(method) {
                visit_block(&mut method.body, visit);
            }
        }
    }
    for trait_definition in &mut module.traits {
        for method in trait_definition.methods.iter_mut().chain(trait_definition.default_methods.iter_mut()) {
            if permits(method) {
                visit_block(&mut method.body, visit);
            }
        }
    }
    for implementation in &mut module.impls {
        for method in &mut implementation.methods {
            if permits(method) {
                visit_block(&mut method.body, visit);
            }
        }
    }
    for widget in &mut module.widgets {
        for method in &mut widget.methods {
            if permits(method) {
                visit_block(&mut method.body, visit);
            }
        }
    }
    // 已铸造克隆体的可达性由铸造它的调用点决定，而该调用点本身已过门：克隆体只可能
    // 从门内体内请求铸造，不能再按声明身份过滤。witness 消解出的 impl 成员声明在
    // 代入闭合前对静态可达闭包不可见，声明身份不在可达集；若按声明拒绝，这些具体
    // 克隆体（如 `ArrayIterator<i64>::has_next`）体内的嵌套调用永远拿不到实例。
    for callable in &mut module.instantiated_callables {
        visit_block(&mut callable.function.body, visit);
    }
    for submodule in &mut module.submodules {
        visit_module_calls(submodule, gate, visit);
    }
}

/// 声明级 callable 的签名是否闭合。
///
/// §4 的实例身份含完整泛型替换。开放 binder 只允许在克隆体上逐层结算；把实例写回
/// 仍含开放 binder 的声明体，等于把单态化中间态冻进声明——该实例随后会被声明可达性
/// 当作真实 callee 收进物化集，最终在链接期表现为「调用实例没有定义或显式导入合同」。
pub(crate) fn declaration_body_is_closed(function: &HirFunction) -> bool {
    if !function.generics.is_empty()
        || function.params.iter().any(|param| has_unresolved_generic_arguments(&param.ty))
        || has_unresolved_generic_arguments(&function.return_type)
    {
        return false;
    }
    true
}

fn visit_block(block: &mut HirBlock, visit: &mut impl FnMut(CallableRequest<'_>)) {
    for statement in &mut block.statements {
        match &mut statement.kind {
            HirStatementKind::Let { initializer, .. } => {
                if let Some(initializer) = initializer {
                    visit_expr(initializer, visit);
                }
            }
            HirStatementKind::Expr(expr) => visit_expr(expr, visit),
        }
    }
    if let Some(expr) = &mut block.expr {
        visit_expr(expr, visit);
    }
}

fn visit_expr(expr: &mut HirExpr, visit: &mut impl FnMut(CallableRequest<'_>)) {
    match &mut expr.kind {
        HirExprKind::Call { callee, args, resolved } => {
            visit_expr(callee, visit);
            for arg in args {
                visit_expr(&mut arg.value, visit);
            }
            if let Some(resolved) = resolved {
                visit(CallableRequest::Call(resolved));
            }
        }
        HirExprKind::Construct { args, resolved, .. } => {
            for arg in args {
                visit_expr(arg, visit);
            }
            if let Some(resolved) = resolved {
                visit(CallableRequest::Call(resolved));
            }
        }
        HirExprKind::FunctionReference(reference) => visit(CallableRequest::Reference(reference)),
        HirExprKind::Cast { expr: inner, .. }
        | HirExprKind::FieldAccess { object: inner, .. }
        | HirExprKind::FieldInit { value: inner, .. }
        | HirExprKind::Await(inner)
        | HirExprKind::Awake(inner)
        | HirExprKind::BlockOn(inner)
        | HirExprKind::YieldFrom(inner)
        | HirExprKind::Raise(inner)
        | HirExprKind::Resume(inner)
        | HirExprKind::Assign { value: inner, .. }
        | HirExprKind::Return(Some(inner))
        | HirExprKind::Yield(Some(inner))
        | HirExprKind::Break { expr: Some(inner), .. } => visit_expr(inner, visit),
        HirExprKind::TryPropagate { inner, resolved_ok_variant } => {
            visit_expr(inner, visit);
            if let Some(variant) = resolved_ok_variant {
                visit(CallableRequest::Call(variant));
            }
        }
        HirExprKind::StoreField { object, value, .. } => {
            visit_expr(object, visit);
            visit_expr(value, visit);
        }
        HirExprKind::GenericApply { callee, .. } => visit_expr(callee, visit),
        HirExprKind::ArrayNew { length, .. } => visit_expr(length, visit),
        HirExprKind::ArrayLiteral { items } | HirExprKind::TupleLiteral { items } => {
            for item in items {
                visit_expr(item, visit);
            }
        }
        // 集合字面量构造槽由 post-binding pass 冻结（声明身份已带、实例待铸造）：
        // 两槽交给固定点登记，字面量项照常下钻。
        HirExprKind::CollectionInit { with_length, append, items, .. } => {
            visit(CallableRequest::Call(with_length));
            visit(CallableRequest::Call(append));
            for item in items {
                visit_expr(item, visit);
            }
        }
        HirExprKind::Block(block) | HirExprKind::TryScope { body: block, .. } => visit_block(block, visit),
        HirExprKind::Lambda { params, return_type, body, hoisted, .. } => {
            if let Some(identity) = hoisted.as_mut() {
                visit(CallableRequest::Lambda { identity, params, return_type });
            }
            visit_block(body, visit);
        }
        HirExprKind::If { condition, then_branch, else_branch, .. } => {
            visit_expr(condition, visit);
            visit_block(then_branch, visit);
            if let Some(branch) = else_branch {
                visit_block(branch, visit);
            }
        }
        HirExprKind::IfLet { scrutinee, then_branch, else_branch, .. } => {
            visit_expr(scrutinee, visit);
            visit_block(then_branch, visit);
            if let Some(branch) = else_branch {
                visit_block(branch, visit);
            }
        }
        HirExprKind::Match { scrutinee, arms, .. } | HirExprKind::Case { scrutinee, arms, .. } => {
            visit_expr(scrutinee, visit);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    visit_expr(guard, visit);
                }
                visit_expr(&mut arm.body, visit);
            }
        }
        HirExprKind::Loop { iterator, condition, body, .. } => {
            if let Some(iterator) = iterator {
                visit_expr(iterator, visit);
            }
            if let Some(condition) = condition {
                visit_expr(condition, visit);
            }
            visit_block(body, visit);
        }
        HirExprKind::Catch { expr: inner, arms } => {
            visit_expr(inner, visit);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    visit_expr(guard, visit);
                }
                visit_expr(&mut arm.body, visit);
            }
        }
        HirExprKind::With { base, updates } => {
            visit_expr(base, visit);
            for (_, update) in updates {
                visit_expr(update, visit);
            }
        }
        HirExprKind::SuperCall { args, .. } => {
            for arg in args {
                visit_expr(arg, visit);
            }
        }
        HirExprKind::AnonymousClass { fields, methods, .. } => {
            for (_, value) in fields {
                visit_expr(value, visit);
            }
            for method in methods {
                visit_block(&mut method.body, visit);
            }
        }
        _ => {}
    }
}

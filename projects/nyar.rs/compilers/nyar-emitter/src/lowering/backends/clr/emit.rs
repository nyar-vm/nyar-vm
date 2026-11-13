//! CLR 最小 MIR → `MSIL` 模块降低。
//!
//! 只消费计划已经冻结的标量事实：整型/浮点常量、SSA local、冻结 operator、
//! 块标签与条件分支、同片段 `call` 以及 `Return`。聚合、数组、sum、宿主导入等
//! 形状在这里返回结构化错误，不补造指令、默认值或空体方法。
//!
//! 与 JVM 不同，CLR 的局部变量与方法形参是两套编号：方法入口必须先把 `ldarg`
//! 写入 entry 块的参数局部，否则 `ldloc` 读到未初始化槽。块跳转按目标块参数
//! 局部逆序写回，比较 operator 是单 opcode（`ceq`/`clt`/`cgt`），不展开分支。

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    FragmentSubmission, Result,
    backend_plan_views::{
        ExecutableBlock as MirBlock, ExecutableBlockRef as MirBlockRef, ExecutableFunction as MirFunction,
        ExecutableInstruction as MirInstruction, ExecutableValueRef as MirValueRef, NyarType,
    },
    contracts::{ArrayInitialization, Constant, InstructionKind, Operand, Terminator},
    lowering::shared::{
        executable::{block_label, collect_reachable_blocks, slots::ExecutableSlotPlan},
        value_copy::{MAX_VALUE_AGGREGATE_COPY_DEPTH, nested_value_aggregate_layout, value_aggregate_copy_depth},
    },
    managed_abi::{ManagedAbiContract, ManagedAbiEntry},
    target_operator::{
        TargetOperandStep, TargetOperandType, TargetOperatorBinding, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe,
    },
    transitional::msil::{
        MsilAssembly, MsilField, MsilInstruction, MsilInstructionOperand, MsilMethodBody, MsilMethodRef, MsilMethodSignature, MsilModule,
        MsilOpcode, MsilType, MsilTypeDef,
    },
};
use nyar_types::{AggregateLayout, AggregateLayoutPlan, FieldId, ItemInstanceId, LayoutId, NominalInstanceId, VariantId};

/// CLR 栈上标量载体类别：`int32` 覆盖 bool/char/窄整型，比较结果与布尔同槽。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClrValueKind {
    /// `int32` 载体。
    Int32,
    /// `int64` 载体。
    Int64,
    /// `float32` 载体。
    Float32,
    /// `float64` 载体。
    Float64,
    /// 引用载体（字符串等）。
    Reference,
}

impl ClrValueKind {
    fn of_operand_type(operand_type: TargetOperandType) -> Self {
        match operand_type {
            TargetOperandType::I32 => Self::Int32,
            TargetOperandType::I64 => Self::Int64,
            TargetOperandType::F32 => Self::Float32,
            TargetOperandType::F64 => Self::Float64,
        }
    }

    fn of_msil_type(ty: &MsilType) -> Result<Self> {
        Ok(match ty {
            MsilType::Void => return Err("`void` 不能作为局部变量或操作数载体".to_string()),
            MsilType::Bool
            | MsilType::Char
            | MsilType::Int8 { .. }
            | MsilType::Int16 { .. }
            | MsilType::Int32 { .. }
            | MsilType::IntPtr { .. } => Self::Int32,
            MsilType::Int64 { .. } => Self::Int64,
            MsilType::Float32 => Self::Float32,
            MsilType::Float64 => Self::Float64,
            MsilType::String | MsilType::Object | MsilType::SzArray(_) | MsilType::Named(_) => Self::Reference,
        })
    }
}

/// 函数级 CLR 方法计划：稳定名字与冻结签名。
#[derive(Debug, Clone)]
struct ClrMethodPlan {
    name: String,
    signature: MsilMethodSignature,
}

/// CLR lowering 结果：`MSIL` 模块与导出入口的 managed ABI 契约。
#[derive(Debug, Clone)]
pub(crate) struct ClrLoweredFragment {
    /// 片段 `MSIL` 模块。
    pub module: MsilModule,
    /// 导出入口 ABI 契约；没有可导出入口时为 `None`。
    pub managed_abi: Option<ManagedAbiContract>,
}

/// 将片段降低为单个 `MSIL` 模块，并为导出入口合成 argv 分发入口。
///
/// 只降低导出面（`wasm_export_names` / `exported_operations` / 入口操作）以及
/// 从它们出发、经同片段静态调用可达的函数；其余函数不进入模块，避免为不可达
/// 形状发明载体。
pub(crate) fn lower_fragment_to_clr_module(submission: &FragmentSubmission) -> Result<ClrLoweredFragment> {
    let plan = &submission.backend_plan;
    let operations = collect_clr_operations(submission);
    if operations.is_empty() {
        return Err("CLR 最小 lowering 没有可导出的可执行函数：片段既无 `wasm_export_names` 也无入口操作，拒绝凭空生成模块".to_string());
    }

    let mut used_names = BTreeMap::<String, ()>::new();
    let mut method_plans = BTreeMap::<ItemInstanceId, ClrMethodPlan>::new();
    for (ordinal, instance) in operations.iter().enumerate() {
        let view = plan.get_function(instance).ok_or_else(|| format!("CLR 最小 lowering：可达操作 `{instance:?}` 缺少函数体"))?;
        let base = plan
            .wasm_export_names()
            .get(instance)
            .cloned()
            .or_else(|| plan.abi_name_for_instance(*instance).map(|name| crate::lowering::sanitize_operation_symbol(&name)))
            .unwrap_or_else(|| format!("function_{ordinal}"));
        let base = crate::lowering::sanitize_symbol(&base);
        let mut name = base.clone();
        let mut suffix = 0usize;
        while used_names.contains_key(&name) {
            suffix += 1;
            name = format!("{base}__{suffix}");
        }
        used_names.insert(name.clone(), ());
        method_plans.insert(*instance, ClrMethodPlan { name, signature: function_signature(plan.aggregate_layouts(), &view.function)? });
    }

    let operator_by_instance = external_operator_by_instance(submission);
    let entry_operation = plan.entry_operation();
    let layout_types = build_layout_types(plan.aggregate_layouts())?;
    let sum_types = build_sum_types(submission)?;
    let mut module = MsilModule {
        assembly: MsilAssembly { name: crate::lowering::sanitize_symbol(plan.module_name()), externs: Vec::new() },
        types: Vec::new(),
        global_methods: Vec::new(),
    };
    module.types.extend(layout_types);
    module.types.extend(sum_types);
    reject_duplicate_type_names(&module.types)?;
    for instance in &operations {
        let Some(view) = plan.get_function(instance)
        else {
            continue;
        };
        let method = method_plans.get(instance).cloned().ok_or_else(|| format!("CLR 最小 lowering：可达操作 `{instance:?}` 缺少方法计划"))?;
        // 有导出面时 PE 入口由合成的 `managed_entry` 独占；单个导出方法不能再标入口。
        let is_entry_point = entry_operation == Some(*instance) && plan.wasm_export_names().is_empty();
        module.global_methods.push(lower_function(
            submission,
            &view.function,
            &method,
            &method_plans,
            &operator_by_instance,
            is_entry_point,
        )?);
    }

    let managed_abi = build_managed_abi_surface(&mut module, plan.wasm_export_names(), &method_plans)?;
    Ok(ClrLoweredFragment { module, managed_abi })
}

/// 为导出入口生成 managed ABI 契约，并合成唯一的 argv 分发入口 `managed_entry`。
///
/// 每个导出入口必须已作为全局方法落库；参数/返回必须是 `i64` / `i32` / `f64` /
/// `bool` / `void` 标量，其它载体返回结构化错误，不发明入口或默认值。
fn build_managed_abi_surface(
    module: &mut MsilModule,
    export_names: &BTreeMap<ItemInstanceId, String>,
    method_plans: &BTreeMap<ItemInstanceId, ClrMethodPlan>,
) -> Result<Option<ManagedAbiContract>> {
    if export_names.is_empty() {
        return Ok(None);
    }
    if let Some(existing) = module.global_methods.iter().find(|method| method.is_entry_point) {
        return Err(format!("CLR managed ABI：模块已有可执行入口 `{}`，拒绝再合成 `managed_entry` 造成双入口", existing.method.name));
    }
    if module.global_methods.iter().any(|method| method.method.name == MANAGED_ENTRY_METHOD) {
        return Err(format!("CLR managed ABI：已有全局方法 `{MANAGED_ENTRY_METHOD}`，拒绝复用该名字"));
    }

    let mut entries = Vec::with_capacity(export_names.len());
    let mut dispatch_targets = Vec::with_capacity(export_names.len());
    for (instance, logical_entry) in export_names {
        let method = method_plans
            .get(instance)
            .ok_or_else(|| format!("CLR managed ABI：导出入口 `{logical_entry}` 的实例 `{instance:?}` 没有已降低方法"))?;
        let body = module
            .global_methods
            .iter()
            .find(|candidate| candidate.method.name == method.name && candidate.method.signature == method.signature)
            .ok_or_else(|| format!("CLR managed ABI：导出入口 `{logical_entry}` 的方法 `{}` 不在模块中", method.name))?;
        let parameter_types = method.signature.parameter_types.iter().map(managed_scalar_type_name).collect::<Result<Vec<_>>>()?;
        let return_type = managed_scalar_type_name(&method.signature.return_type)?;
        entries.push(ManagedAbiEntry {
            logical_entry: logical_entry.clone(),
            owner: GLOBAL_METHODS_TYPE.to_string(),
            method: method.name.clone(),
            signature: method.signature.to_string(),
            parameter_types: parameter_types.clone(),
            return_type,
            launcher_class: MANAGED_ENTRY_METHOD.to_string(),
        });
        dispatch_targets.push((
            body.method.signature.clone(),
            parameter_types.clone(),
            managed_scalar_type_name(&method.signature.return_type)?,
        ));
    }

    let entry_method = build_managed_entry_method(&entries, &dispatch_targets)?;
    module.global_methods.push(entry_method);
    // 产物文件名由驱动层最终确定；这里先留空，桥接层负责补全并刷新摘要。
    Ok(Some(ManagedAbiContract::new("clr", String::new(), entries)))
}

/// CLI 分发入口名，同时作为 ABI 的 `launcher_class` 标识。
const MANAGED_ENTRY_METHOD: &str = "managed_entry";

/// PE 写入器为全局方法合成的类型名。
const GLOBAL_METHODS_TYPE: &str = "_GlobalMethods";

const SYSTEM_STRING: &str = "[mscorlib]System.String";
const SYSTEM_INT64: &str = "[mscorlib]System.Int64";
const SYSTEM_INT32: &str = "[mscorlib]System.Int32";
const SYSTEM_DOUBLE: &str = "[mscorlib]System.Double";
const SYSTEM_BOOLEAN: &str = "[mscorlib]System.Boolean";
const SYSTEM_CONSOLE: &str = "[mscorlib]System.Console";

/// managed ABI 只接受冻结的标量类型：`i64` / `i32` / `f64` / `bool` / `void`。
fn managed_scalar_type_name(ty: &MsilType) -> Result<String> {
    Ok(match ty {
        MsilType::Int64 { signed: true } => "i64",
        MsilType::Int32 { signed: true } => "i32",
        MsilType::Float64 => "f64",
        MsilType::Bool => "bool",
        MsilType::Void => "void",
        other => {
            return Err(format!("CLR managed ABI 不支持类型 `{other}`：managed 标量只接受 `i64`/`i32`/`f64`/`bool`/`void`"));
        }
    }
    .to_string())
}

/// 合成 `int32(string[])` argv 分发入口：`args[0]` 是逻辑入口名，其余按声明类型解析。
fn build_managed_entry_method(
    entries: &[ManagedAbiEntry],
    dispatch_targets: &[(MsilMethodSignature, Vec<String>, String)],
) -> Result<MsilMethodBody> {
    let mut instructions = Vec::new();
    for (index, (entry, (signature, parameter_types, return_type))) in entries.iter().zip(dispatch_targets).enumerate() {
        let next_label = format!("managed_entry_next_{index}");
        instructions.push(ldarg0());
        instructions.push(ldc_i4_0());
        instructions.push(opcode_instruction(MsilOpcode::LdelemRef));
        instructions.push(msil_instruction(MsilOpcode::Ldstr, Some(MsilInstructionOperand::StringLiteral(entry.logical_entry.clone()))));
        instructions.push(call_instruction(string_equality_reference()));
        instructions.push(branch_instruction(MsilOpcode::Brfalse, next_label.clone()));

        for (parameter_index, parameter_type) in parameter_types.iter().enumerate() {
            instructions.push(ldarg0());
            instructions
                .push(msil_instruction(MsilOpcode::LdcI4, Some(MsilInstructionOperand::Integer(i64::from(parameter_index as u32) + 1))));
            instructions.push(opcode_instruction(MsilOpcode::LdelemRef));
            instructions.push(call_instruction(argument_parse_reference(parameter_type)?));
        }
        instructions.push(call_instruction(MsilMethodRef { owner: None, name: entry.method.clone(), signature: signature.clone() }));
        if return_type != "void" {
            instructions.push(call_instruction(result_print_reference(return_type)?));
        }
        instructions.push(ldc_i4_0());
        instructions.push(opcode_instruction(MsilOpcode::Ret));
        instructions.push(MsilInstruction { label: Some(next_label), opcode: MsilOpcode::Nop, operand: None });
    }

    instructions.push(msil_instruction(MsilOpcode::Ldstr, Some(MsilInstructionOperand::StringLiteral("unknown entry".to_string()))));
    instructions.push(call_instruction(console_write_line_reference("string")?));
    instructions.push(ldarg0());
    instructions.push(ldc_i4_0());
    instructions.push(opcode_instruction(MsilOpcode::LdelemRef));
    instructions.push(call_instruction(console_write_line_reference("string")?));
    instructions.push(msil_instruction(MsilOpcode::LdcI4, Some(MsilInstructionOperand::Integer(1))));
    instructions.push(opcode_instruction(MsilOpcode::Ret));

    let max_stack = dispatch_targets
        .iter()
        .map(|(_, parameter_types, _)| u16::try_from(parameter_types.len()).unwrap_or(u16::MAX))
        .max()
        .unwrap_or(0)
        .saturating_add(2);
    Ok(MsilMethodBody {
        method: MsilMethodRef {
            owner: None,
            name: MANAGED_ENTRY_METHOD.to_string(),
            signature: MsilMethodSignature::new(MsilType::Int32 { signed: true }, vec![MsilType::sz_array(MsilType::String)]),
        },
        locals: Vec::new(),
        instructions,
        max_stack,
        is_entry_point: true,
        is_async: false,
    })
}

fn msil_instruction(opcode: MsilOpcode, operand: Option<MsilInstructionOperand>) -> MsilInstruction {
    MsilInstruction { label: None, opcode, operand }
}

fn opcode_instruction(opcode: MsilOpcode) -> MsilInstruction {
    msil_instruction(opcode, None)
}

fn ldarg0() -> MsilInstruction {
    opcode_instruction(MsilOpcode::Ldarg0)
}

fn ldc_i4_0() -> MsilInstruction {
    opcode_instruction(MsilOpcode::LdcI4_0)
}

fn call_instruction(method: MsilMethodRef) -> MsilInstruction {
    msil_instruction(MsilOpcode::Call, Some(MsilInstructionOperand::Method(method)))
}

fn branch_instruction(opcode: MsilOpcode, target: String) -> MsilInstruction {
    msil_instruction(opcode, Some(MsilInstructionOperand::BranchTarget(target)))
}

fn external_method(owner: &str, name: &str, return_type: MsilType, parameter_types: Vec<MsilType>) -> MsilMethodRef {
    MsilMethodRef { owner: Some(owner.to_string()), name: name.to_string(), signature: MsilMethodSignature::new(return_type, parameter_types) }
}

fn string_equality_reference() -> MsilMethodRef {
    external_method(SYSTEM_STRING, "op_Equality", MsilType::Bool, vec![MsilType::String, MsilType::String])
}

/// argv 解析调用：按声明标量类型选择 `Int64.Parse` / `Int32.Parse` /
/// `Double.Parse` / `Boolean.Parse`。
fn argument_parse_reference(managed_type: &str) -> Result<MsilMethodRef> {
    let (owner, return_type) = match managed_type {
        "i64" => (SYSTEM_INT64, MsilType::Int64 { signed: true }),
        "i32" => (SYSTEM_INT32, MsilType::Int32 { signed: true }),
        "f64" => (SYSTEM_DOUBLE, MsilType::Float64),
        "bool" => (SYSTEM_BOOLEAN, MsilType::Bool),
        other => return Err(format!("CLR managed ABI launcher 不支持形参标量 `{other}`")),
    };
    Ok(external_method(owner, "Parse", return_type, vec![MsilType::String]))
}

/// 结果输出调用：按返回值类型选择匹配的 `Console.WriteLine` 重载。
fn result_print_reference(managed_type: &str) -> Result<MsilMethodRef> {
    match managed_type {
        "i64" | "i32" | "f64" | "bool" => console_write_line_reference(managed_type),
        "void" => Err("CLR managed ABI launcher：`void` 结果不得生成输出调用".to_string()),
        other => Err(format!("CLR managed ABI launcher 不支持返回标量 `{other}`")),
    }
}

fn console_write_line_reference(managed_type: &str) -> Result<MsilMethodRef> {
    let parameter_type = match managed_type {
        "i64" => MsilType::Int64 { signed: true },
        "i32" => MsilType::Int32 { signed: true },
        "f64" => MsilType::Float64,
        "bool" => MsilType::Bool,
        "string" => MsilType::String,
        other => return Err(format!("CLR managed ABI launcher 不支持 `Console.WriteLine` 载体 `{other}`")),
    };
    Ok(external_method(SYSTEM_CONSOLE, "WriteLine", MsilType::Void, vec![parameter_type]))
}

/// 从导出面出发收集可达操作（按实例身份升序，保证稳定输出顺序）。
fn collect_clr_operations(submission: &FragmentSubmission) -> Vec<ItemInstanceId> {
    let plan = &submission.backend_plan;
    let mut roots = BTreeSet::new();
    roots.extend(plan.wasm_export_names().keys().copied());
    roots.extend(plan.exported_operations().iter().copied());
    if let Some(entry) = plan.entry_operation() {
        roots.insert(entry);
    }

    let mut visited = BTreeSet::new();
    let mut ordered = Vec::new();
    loop {
        let Some(instance) = roots.iter().copied().find(|instance| !visited.contains(instance))
        else {
            break;
        };
        visited.insert(instance);
        let Some(view) = plan.get_function(&instance)
        else {
            continue;
        };
        for block in &view.function.blocks {
            for instruction in &block.instructions {
                let InstructionKind::Call { callee: Operand::Item(callee), .. } = &instruction.kind
                else {
                    continue;
                };
                if plan.get_function(callee).is_some() && !plan.imports().contains_key(callee) {
                    roots.insert(*callee);
                }
            }
        }
        ordered.push(instance);
    }
    ordered
}

/// 冻结的 `[clr_operator("...")]` 绑定：助记符只用于诊断，配方来自计划。
fn external_operator_by_instance(submission: &FragmentSubmission) -> BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)> {
    submission
        .backend_plan
        .imports()
        .iter()
        .filter_map(|(instance, import)| {
            let mnemonic = crate::lowering::shared::interop::clr_operator_target(&import.link)?;
            Some((*instance, (mnemonic.to_string(), import.target_operator.unwrap_or(TargetOperatorBinding::Unfrozen))))
        })
        .collect()
}

fn function_signature(layouts: &AggregateLayoutPlan, function: &MirFunction) -> Result<MsilMethodSignature> {
    let entry =
        function.blocks.get(function.entry.0 as usize).ok_or_else(|| format!("CLR 最小 lowering：函数 `{}` 缺少入口块", function.symbol))?;
    let mut parameter_types = Vec::with_capacity(entry.parameters.len());
    for parameter in &entry.parameters {
        let ty =
            function.value_types.get(parameter).ok_or_else(|| format!("CLR 最小 lowering：函数 `{}` 入口形参缺少冻结类型", function.symbol))?;
        parameter_types.push(msil_parameter_type(layouts, ty)?);
    }
    Ok(MsilMethodSignature::new(msil_return_type(layouts, &function.return_type)?, parameter_types))
}

/// 局部 / 形参位置的 CLR 载体：`Unit`/`Bottom` 没有值槽。
fn msil_parameter_type(layouts: &AggregateLayoutPlan, ty: &NyarType) -> Result<MsilType> {
    let msil_type = msil_type(layouts, ty)?;
    if msil_type == MsilType::Void {
        return Err(format!("CLR 最小 lowering：`{ty}` 是 void 形态，不能作为方法形参或局部值"));
    }
    Ok(msil_type)
}

fn msil_return_type(layouts: &AggregateLayoutPlan, ty: &NyarType) -> Result<MsilType> {
    msil_type(layouts, ty)
}

/// 布局类型定义的稳定名字：`nyar::layout::<sanitized(name)>$<layout id>`。
///
/// CLR 名字用 `::` 层级；块 4 生成 `MsilTypeDef` 时复用同一函数，绝不另写第二套规则。
/// 同一 layout id 稳定、不同布局不撞名。
pub(crate) fn clr_layout_type_name(layout: &AggregateLayout) -> String {
    format!("nyar::layout::{}${}", crate::lowering::sanitize_symbol(&layout.name), layout.id)
}

/// 布局字段在生成类型中的唯一名字；构造与字段访问必须共用这一条规则。
fn clr_layout_field_name(slot: usize) -> String {
    format!("field{slot}")
}

/// sum 生成类型的稳定名字：只由 nominal 身份派生，禁止按 variant 名/短名推断。
fn clr_sum_type_name(nominal: NominalInstanceId) -> String {
    format!("nyar::sum::${}", nominal.index())
}

const CLR_SUM_TAG_FIELD: &str = "tag";
const CLR_SUM_PAYLOAD_FIELD: &str = "payload";
const SYSTEM_OBJECT: &str = "[mscorlib]System.Object";

/// 生成类型的实例默认构造器：`ldarg.0; call instance void [mscorlib]System.Object::.ctor(); ret`。
///
/// 构造函数体是 PE 写入器唯一要求的实例初始化路径；没有它，生成的类型无法加载。
fn clr_instance_constructor(type_name: &str) -> MsilMethodBody {
    MsilMethodBody {
        method: MsilMethodRef {
            owner: Some(type_name.to_string()),
            name: ".ctor".to_string(),
            signature: MsilMethodSignature::new_instance(MsilType::Void, Vec::new()),
        },
        locals: Vec::new(),
        instructions: vec![
            ldarg0(),
            call_instruction(MsilMethodRef {
                owner: Some(SYSTEM_OBJECT.to_string()),
                name: ".ctor".to_string(),
                signature: MsilMethodSignature::new_instance(MsilType::Void, Vec::new()),
            }),
            opcode_instruction(MsilOpcode::Ret),
        ],
        max_stack: 1,
        is_entry_point: false,
        is_async: false,
    }
}

/// 为所有需要堆载体的布局生成 `MsilTypeDef`。
///
/// 规则与块 2 的 `msil_type` 载体分类一致：引用存储或非零字段布局才需要类型定义；
/// 零字段值存储布局继续在载体层结构化拒绝。
fn build_layout_types(layouts: &AggregateLayoutPlan) -> Result<Vec<MsilTypeDef>> {
    let mut types = Vec::new();
    for layout in &layouts.layouts {
        if !(layout.storage.is_reference() || !layout.fields.is_empty()) {
            continue;
        }
        let type_name = clr_layout_type_name(layout);
        let mut fields = Vec::with_capacity(layout.fields.len());
        for (slot, field) in layout.fields.iter().enumerate() {
            fields.push(MsilField { name: clr_layout_field_name(slot), ty: msil_type(layouts, &field.ty)?, is_static: false });
        }
        types.push(MsilTypeDef {
            methods: vec![clr_instance_constructor(&type_name)],
            full_name: type_name,
            namespace: String::new(),
            fields,
            is_value_type: false,
        });
    }
    Ok(types)
}

/// 只为 MIR 中实际出现 sum 指令的 nominal 生成 tag/payload 类型定义。
fn build_sum_types(submission: &FragmentSubmission) -> Result<Vec<MsilTypeDef>> {
    let plan = &submission.backend_plan;
    let mut nominals = BTreeSet::new();
    for instance in plan.instances() {
        let Some(view) = plan.get_function(&instance)
        else {
            continue;
        };
        for block in &view.function.blocks {
            for instruction in &block.instructions {
                match &instruction.kind {
                    InstructionKind::SumNew { nominal, .. }
                    | InstructionKind::SumPayloadGet { nominal, .. }
                    | InstructionKind::SumVariantIs { nominal, .. } => {
                        nominals.insert(*nominal);
                    }
                    _ => {}
                }
            }
        }
    }

    let mut types = Vec::with_capacity(nominals.len());
    for nominal in nominals {
        let Some(sum) = plan.sum_representations().get(&nominal)
        else {
            return Err(format!("CLR 最小 lowering：sum nominal `{nominal:?}` 没有冻结的 `SumRepresentation`"));
        };
        if sum.tag_width == 0 || sum.tag_width > 4 {
            return Err(format!(
                "CLR 最小 lowering：sum nominal `{nominal:?}` 的 `tag_width`={} 超出 1..=4 合同",
                sum.tag_width
            ));
        }
        let type_name = clr_sum_type_name(nominal);
        types.push(MsilTypeDef {
            methods: vec![clr_instance_constructor(&type_name)],
            full_name: type_name,
            namespace: String::new(),
            fields: vec![
                MsilField { name: CLR_SUM_TAG_FIELD.to_string(), ty: MsilType::Int32 { signed: true }, is_static: false },
                MsilField { name: CLR_SUM_PAYLOAD_FIELD.to_string(), ty: MsilType::Object, is_static: false },
            ],
            is_value_type: false,
        });
    }
    Ok(types)
}

/// 生成类型全名必须模块内唯一，否则字段/构造 token 会静默漂移。
fn reject_duplicate_type_names(types: &[MsilTypeDef]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for type_def in types {
        if !seen.insert(type_def.qualified_name()) {
            return Err(format!("CLR 最小 lowering：生成类型全名 `{}` 重复，拒绝输出歧义模块", type_def.qualified_name()));
        }
    }
    Ok(())
}

/// 数组类型 `Array<T>` / `FixedArray { element, .. }` 的元素类型。
fn clr_array_element_type(array_type: &NyarType) -> Result<&NyarType> {
    match array_type {
        NyarType::Array(element) => Ok(element),
        NyarType::FixedArray { element, .. } => Ok(element),
        other => Err(format!("CLR 最小 lowering：`{other}` 不是数组形态，缺少元素类型合同")),
    }
}

/// `Int32` 元素走专用指令，引用载体走 `ldelem.ref`，其余统一 `ldelem.any + Type`。
fn clr_array_load_opcode(element: &MsilType) -> MsilOpcode {
    match element {
        MsilType::Int32 { .. } => MsilOpcode::LdelemI4,
        MsilType::String | MsilType::Object | MsilType::Named(_) | MsilType::SzArray(_) => MsilOpcode::LdelemRef,
        _ => MsilOpcode::LdelemAny,
    }
}

/// 与 [`clr_array_load_opcode`] 对称的元素存储 opcode。
fn clr_array_store_opcode(element: &MsilType) -> MsilOpcode {
    match element {
        MsilType::Int32 { .. } => MsilOpcode::StelemI4,
        MsilType::String | MsilType::Object | MsilType::Named(_) | MsilType::SzArray(_) => MsilOpcode::StelemRef,
        _ => MsilOpcode::StelemAny,
    }
}

/// 数组元素位置的 CLR 载体。
///
/// acorn-pe 没有 `ldelem.i8`/`stelem.i8`，i64 元素由块 4 用
/// `ldelem.any`/`stelem.any` + `Type("int64")` 存取；载体本身仍是 `Int64`。
fn msil_array_element_type(layouts: &AggregateLayoutPlan, element: &NyarType) -> Result<MsilType> {
    Ok(match element {
        NyarType::Boolean => MsilType::Bool,
        NyarType::Integer8 { signed } => MsilType::Int8 { signed: *signed },
        NyarType::Integer16 { signed } => MsilType::Int16 { signed: *signed },
        NyarType::Integer32 { signed } => MsilType::Int32 { signed: *signed },
        // V 的 `char` 是 u32 码位载体；CLR `Char` 只有 16 位，物理载体固定 32 位 int32。
        NyarType::Character => MsilType::Int32 { signed: false },
        NyarType::Integer64 { signed } => MsilType::Int64 { signed: *signed },
        NyarType::Float32 => MsilType::Float32,
        NyarType::Float64 => MsilType::Float64,
        NyarType::Utf8 | NyarType::Utf16 => MsilType::String,
        NyarType::Apply(..) => MsilType::Object,
        // 名义布局按冻结身份取类型定义名；值存储聚合没有单槽 CLR 载体。
        NyarType::Nominal { layout, spelling } => {
            let Some(layout) = layouts.layouts.iter().find(|candidate| candidate.id == *layout)
            else {
                return Err(format!("CLR 最小 lowering：`{spelling}` 的布局身份 `{layout}` 未在计划中注册：缺少数组元素 CLR 载体合同"));
            };
            if layout.storage.is_reference() {
                MsilType::Named(clr_layout_type_name(layout))
            }
            else {
                return Err(format!("CLR 最小 lowering 不支持值存储聚合 `{spelling}` 作为数组元素：缺少单槽 CLR 载体合同"));
            }
        }
        NyarType::Array(inner) | NyarType::FixedArray { element: inner, .. } => MsilType::sz_array(msil_array_element_type(layouts, inner)?),
        // 可空元素只能复用引用载体（`null` 表示空）；标量需要 sum 表示。
        NyarType::Nullable(inner) => {
            let ty = msil_array_element_type(layouts, inner)?;
            if matches!(ty, MsilType::String | MsilType::Object | MsilType::Named(_) | MsilType::SzArray(_)) {
                ty
            }
            else {
                return Err(format!("CLR 最小 lowering：可空标量 `{element}` 需要 sum 表示，不能擦成引用"));
            }
        }
        other => {
            return Err(format!("CLR 最小 lowering 不支持数组元素类型 `{other}`：缺少该形状的 CLR 载体合同"));
        }
    })
}

/// 类型的 CLR 载体：标量取原生类型，数组取 `SzArray`，引用布局取类型定义名。
pub(crate) fn msil_type(layouts: &AggregateLayoutPlan, ty: &NyarType) -> Result<MsilType> {
    Ok(match ty {
        NyarType::Bottom | NyarType::Unit => MsilType::Void,
        NyarType::Boolean => MsilType::Bool,
        // V 的 `char` 是 u32 码位，载体固定 32 位；不得用宿主 16 位 `Char`。
        NyarType::Character => MsilType::Int32 { signed: false },
        NyarType::Integer8 { signed } => MsilType::Int8 { signed: *signed },
        NyarType::Integer16 { signed } => MsilType::Int16 { signed: *signed },
        NyarType::Integer32 { signed } => MsilType::Int32 { signed: *signed },
        NyarType::Integer64 { signed } => MsilType::Int64 { signed: *signed },
        NyarType::Float32 => MsilType::Float32,
        NyarType::Float64 => MsilType::Float64,
        NyarType::Utf8 | NyarType::Utf16 => MsilType::String,
        // 栈/堆数组的载体是元素载体数组；`[T; N]` 的长度由 `ArrayFromElements` 表达。
        NyarType::Array(element) => MsilType::sz_array(msil_array_element_type(layouts, element)?),
        NyarType::FixedArray { element, .. } => MsilType::sz_array(msil_array_element_type(layouts, element)?),
        // 布局身份只经 `TupleNew`/`AggregateCopy` 的 `layout_id` 进入后端，类型描述符
        // 没有该事实，禁止按形状猜布局。
        NyarType::Tuple(..) => {
            return Err(format!(
                "CLR 最小 lowering：裸 tuple `{ty}` 没有布局身份：布局身份只经 `TupleNew`/`AggregateCopy` 的 `layout_id` 进入后端，类型描述符没有该事实"
            ));
        }
        // 可空引用直接复用引用载体（`null` 表示空）；标量需要 sum 表示，不得静默装箱。
        NyarType::Nullable(inner) => {
            let ty = msil_type(layouts, inner)?;
            if matches!(ty, MsilType::String | MsilType::Object | MsilType::Named(_) | MsilType::SzArray(_)) {
                ty
            }
            else if ty == MsilType::Void {
                return Err(format!("CLR 最小 lowering：可空 `{ty}` 的内层是 void，没有可复用的引用载体"));
            }
            else {
                return Err(format!("CLR 最小 lowering：可空标量 `{ty}` 需要 sum 表示，不得静默装箱"));
            }
        }
        // 泛型实例：存储类由形状决定为引用，载体擦除为 `object`。
        NyarType::Apply(..) => MsilType::Object,
        // 携带身份的名义类型：存储类按 `layout` 身份解析，引用/值存储都指向
        // 布局类型定义。CLR 没有内联结构体，逐字段拷贝由 `AggregateCopy` 承担，
        // 它是唯一允许的复制路径；零字段值布局与 wasm 标量语义一致，维持结构化失败。
        NyarType::Nominal { layout, spelling } => {
            let Some(layout) = layouts.layouts.iter().find(|candidate| candidate.id == *layout)
            else {
                return Err(format!("CLR 最小 lowering：`{spelling}` 的布局身份 `{layout}` 未在计划中注册：缺少该形状的 CLR 载体合同"));
            };
            if layout.storage.is_reference() || !layout.fields.is_empty() {
                MsilType::Named(clr_layout_type_name(layout))
            }
            else {
                return Err(format!("CLR 最小 lowering 不支持零字段值存储聚合 `{spelling}`：缺少单槽 CLR 载体合同"));
            }
        }
        other => {
            return Err(format!("CLR 最小 lowering 不支持类型 `{other}`：缺少该形状的 CLR 载体合同"));
        }
    })
}

struct ClrMethodEmitter<'a> {
    submission: &'a FragmentSubmission,
    function: &'a MirFunction,
    return_type: MsilType,
    slots: ExecutableSlotPlan,
    method_plans: &'a BTreeMap<ItemInstanceId, ClrMethodPlan>,
    operator_by_instance: &'a BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)>,
    instructions: Vec<MsilInstruction>,
    /// 值聚合递归复制使用的临时引用局部：每层复制占 2 槽（源/目标）。
    copy_scratch: Vec<u16>,
    /// 编译器内部标签序号（递归复制空值分支使用）。
    label_ordinal: usize,
}

fn lower_function(
    submission: &FragmentSubmission,
    function: &MirFunction,
    method: &ClrMethodPlan,
    method_plans: &BTreeMap<ItemInstanceId, ClrMethodPlan>,
    operator_by_instance: &BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)>,
    is_entry_point: bool,
) -> Result<MsilMethodBody> {
    let slots = ExecutableSlotPlan::plan_nyar(function);
    let mut locals =
        slots.local_types.iter().map(|ty| msil_parameter_type(submission.backend_plan.aggregate_layouts(), ty)).collect::<Result<Vec<_>>>()?;

    // 值聚合递归复制：按冻结布局的嵌套深度预留 2 槽/层的临时引用局部。
    let layout_plan = submission.backend_plan.aggregate_layouts();
    let mut copy_depth = 0u32;
    for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
        if let InstructionKind::AggregateCopy { layout_id, .. } = &instruction.kind
            && let Some(layout) = layout_plan.layouts.iter().find(|candidate| candidate.id == *layout_id)
        {
            copy_depth = copy_depth.max(value_aggregate_copy_depth(layout_plan, layout));
        }
    }
    if copy_depth > MAX_VALUE_AGGREGATE_COPY_DEPTH {
        return Err(format!(
            "CLR 最小 lowering：值聚合复制嵌套 {copy_depth} 层超过上限 {MAX_VALUE_AGGREGATE_COPY_DEPTH}: function={}",
            function.symbol
        ));
    }
    let mut copy_scratch = Vec::with_capacity((copy_depth as usize).saturating_mul(2));
    for _ in 0..(copy_depth as usize).saturating_mul(2) {
        copy_scratch.push(u16::try_from(locals.len()).map_err(|_| format!("CLR 局部槽数量溢出: function={}", function.symbol))?);
        locals.push(MsilType::Object);
    }

    let mut emitter = ClrMethodEmitter {
        submission,
        function,
        return_type: method.signature.return_type.clone(),
        slots,
        method_plans,
        operator_by_instance,
        instructions: Vec::new(),
        copy_scratch,
        label_ordinal: 0,
    };
    emitter.emit_entry_parameter_prologue()?;
    for block_id in collect_reachable_blocks(function) {
        let Some(block) = function.blocks.get(block_id.0 as usize)
        else {
            continue;
        };
        emitter.emit_block(block)?;
    }

    let max_stack = u16::try_from(emitter.instructions.len().saturating_mul(2).saturating_add(2)).unwrap_or(u16::MAX);
    Ok(MsilMethodBody {
        method: MsilMethodRef { owner: None, name: method.name.clone(), signature: method.signature.clone() },
        locals,
        instructions: emitter.instructions,
        max_stack,
        is_entry_point,
        is_async: false,
    })
}

impl ClrMethodEmitter<'_> {
    /// 方法入口把 `ldarg` 写入 entry 块参数局部，保证后续 `ldloc` 有定义。
    fn emit_entry_parameter_prologue(&mut self) -> Result<()> {
        let entry = self
            .function
            .blocks
            .get(self.function.entry.0 as usize)
            .ok_or_else(|| format!("CLR 最小 lowering：函数 `{}` 缺少入口块", self.function.symbol))?;
        for index in 0..entry.parameters.len() {
            let local = *self
                .slots
                .block_param_locals
                .get(&(self.function.entry, index))
                .ok_or_else(|| format!("CLR 最小 lowering：入口形参 {index} 缺少局部槽: function={}", self.function.symbol))?;
            self.local_kind(local)?;
            self.instructions.push(MsilInstruction {
                label: None,
                opcode: MsilOpcode::Ldarg,
                operand: Some(MsilInstructionOperand::Integer(index as i64)),
            });
            self.instructions.push(MsilInstruction {
                label: None,
                opcode: MsilOpcode::Stloc,
                operand: Some(MsilInstructionOperand::Integer(i64::from(local))),
            });
        }
        Ok(())
    }

    fn emit_block(&mut self, block: &MirBlock) -> Result<()> {
        self.instructions.push(MsilInstruction { label: Some(block_label(block.id)), opcode: MsilOpcode::Nop, operand: None });
        for instruction in &block.instructions {
            self.emit_instruction(instruction)?;
        }
        self.emit_terminator(block)
    }

    fn emit_instruction(&mut self, instruction: &MirInstruction) -> Result<()> {
        if instruction.results.len() > 1 {
            return Err(format!(
                "CLR 最小 lowering 不支持多结果指令（results={}）: function={}",
                instruction.results.len(),
                self.function.symbol
            ));
        }
        match &instruction.kind {
            InstructionKind::LoadConstant { constant, .. } => {
                let (slot, kind) = self.require_output(instruction)?;
                self.emit_constant(constant, Some(kind))?;
                self.store_local(slot);
            }
            InstructionKind::Copy { source } => {
                let (slot, kind) = self.require_output(instruction)?;
                self.emit_operand(source, Some(kind))?;
                self.store_local(slot);
            }
            InstructionKind::StoreVar { value, .. } => {
                let output = self.output_of(instruction)?;
                let expected = match (&output, value) {
                    (Some((_, kind)), _) => Some(*kind),
                    (None, Operand::Value(value)) => Some(self.value_slot(*value)?.1),
                    _ => None,
                };
                self.emit_operand(value, expected)?;
                match output {
                    Some((slot, _)) => self.store_local(slot),
                    None => self.emit_pop(),
                }
            }
            InstructionKind::Call { callee, arguments } => {
                let output = self.output_of(instruction)?;
                self.emit_call(callee, arguments, output)?;
            }
            InstructionKind::StructNew { nominal, fields } => {
                let layout = self.layout_by_nominal(*nominal)?;
                let output = self.require_output(instruction)?;
                self.emit_layout_new(&layout, output)?;
                for (field, value) in fields {
                    let Some((layout_id, slot)) = self.submission.backend_plan.aggregate_layout_by_field().get(field).copied()
                    else {
                        return Err(format!(
                            "CLR 最小 lowering：字段身份 `{field:?}` 未登记布局索引: function={}",
                            self.function.symbol
                        ));
                    };
                    if layout_id != layout.id {
                        return Err(format!(
                            "CLR 最小 lowering：字段 `{field:?}` 属于布局身份 `{layout_id}`，与构造的 nominal `{nominal:?}` 布局 `{}` 不符: function={}",
                            layout.id, self.function.symbol
                        ));
                    }
                    self.emit_layout_field_write(&layout, output.0, slot as usize, value)?;
                }
            }
            InstructionKind::TupleGet { object, layout_id, index } => {
                let layout = self.layout_by_id(*layout_id)?;
                if (*index as usize) >= layout.fields.len() {
                    return Err(format!(
                        "CLR 最小 lowering：TupleGet 位置索引 {index} 越界（layout `{}` 长度 {}）: function={}",
                        layout.name,
                        layout.fields.len(),
                        self.function.symbol
                    ));
                }
                // 标签只在类型身份层；CLR 载体按冻结 layout 的字段槽读取。
                let (_, field_kind) = self.layout_field_type(&layout, *index)?;
                self.emit_operand(object, Some(ClrValueKind::Reference))?;
                self.push_field_opcode(MsilOpcode::Ldfld, &clr_layout_type_name(&layout), &clr_layout_field_name(*index as usize));
                match self.output_of(instruction)? {
                    Some((output, output_kind)) => {
                        if output_kind != field_kind {
                            return Err(format!(
                                "CLR 最小 lowering：TupleGet 位置 {index} 载体 {field_kind:?} 与结果槽 {output_kind:?} 不符: function={}",
                                self.function.symbol
                            ));
                        }
                        self.store_local(output);
                    }
                    None => self.emit_pop(),
                }
            }
            InstructionKind::TupleNew { layout_id, fields } => {
                let layout = self.layout_by_id(*layout_id)?;
                if fields.len() != layout.fields.len() {
                    return Err(format!(
                        "CLR 最小 lowering：tuple 布局 `{}` 字段数量与操作数不符: function={} expected={} actual={}",
                        layout.id,
                        self.function.symbol,
                        layout.fields.len(),
                        fields.len()
                    ));
                }
                let output = self.require_output(instruction)?;
                self.emit_layout_new(&layout, output)?;
                for (slot, value) in fields.iter().enumerate() {
                    self.emit_layout_field_write(&layout, output.0, slot, value)?;
                }
            }
            InstructionKind::AggregateCopy { layout_id, source, dest } => {
                let layout = self.layout_by_id(*layout_id)?;
                if layout.fields.is_empty() {
                    self.emit_operand(source, Some(ClrValueKind::Reference))?;
                    self.store_operand(dest, ClrValueKind::Reference)?;
                }
                else {
                    let dest_slot = self.reference_operand_slot(dest)?;
                    let source_slot = self.reference_operand_slot(source)?;
                    self.emit_value_aggregate_copy(&layout, source_slot, dest_slot, 0)?;
                }
            }
            InstructionKind::FieldGet { object, field } => {
                let (layout, slot) = self.field_location(*field)?;
                let (_, field_kind) = self.layout_field_type(&layout, slot)?;
                self.emit_operand(object, Some(ClrValueKind::Reference))?;
                self.push_field_opcode(MsilOpcode::Ldfld, &clr_layout_type_name(&layout), &clr_layout_field_name(slot as usize));
                match self.output_of(instruction)? {
                    Some((output, output_kind)) => {
                        if output_kind != field_kind {
                            return Err(format!(
                                "CLR 最小 lowering：字段 `{field:?}` 载体 {field_kind:?} 与结果槽 {output_kind:?} 不符: function={}",
                                self.function.symbol
                            ));
                        }
                        self.store_local(output);
                    }
                    None => self.emit_pop(),
                }
            }
            InstructionKind::FieldSet { object, field, value } => {
                let (layout, slot) = self.field_location(*field)?;
                let (_, field_kind) = self.layout_field_type(&layout, slot)?;
                self.emit_operand(object, Some(ClrValueKind::Reference))?;
                self.emit_operand(value, Some(field_kind))?;
                self.push_field_opcode(MsilOpcode::Stfld, &clr_layout_type_name(&layout), &clr_layout_field_name(slot as usize));
            }
            InstructionKind::SumNew { nominal, variant, payload } => {
                self.emit_sum_new(instruction, *nominal, *variant, payload)?;
            }
            InstructionKind::SumPayloadGet { nominal, variant, object } => {
                self.emit_sum_payload_get(instruction, *nominal, *variant, object)?;
            }
            InstructionKind::SumVariantIs { nominal, variant, object } => {
                self.emit_sum_variant_is(instruction, *nominal, *variant, object)?;
            }
            InstructionKind::ArrayFromElements { layout_id: _, array_type, elements } => {
                self.emit_array_from_elements(instruction, array_type, elements)?;
            }
            InstructionKind::ArrayNew { array_type, length, initialization } => match initialization {
                ArrayInitialization::Default => self.emit_array_new(instruction, array_type, length)?,
                ArrayInitialization::Fill(_) => {
                    return Err(format!("CLR 最小 lowering 不支持数组 Fill 初始化: function={}", self.function.symbol));
                }
            },
            InstructionKind::ArrayGet { array, index } => {
                self.emit_array_get(instruction, array, index)?;
            }
            InstructionKind::ArraySet { array, index, value } => {
                self.emit_array_set(array, index, value)?;
            }
            InstructionKind::ArrayLength { array } => {
                self.emit_operand(array, Some(ClrValueKind::Reference))?;
                self.push_opcode(MsilOpcode::Ldlen);
                match self.output_of(instruction)? {
                    Some((slot, kind)) => {
                        if kind != ClrValueKind::Int32 {
                            return Err(format!(
                                "CLR 最小 lowering：`ArrayLength` 结果必须是 `int32` 槽: function={} actual={kind:?}",
                                self.function.symbol
                            ));
                        }
                        self.store_local(slot);
                    }
                    None => self.emit_pop(),
                }
            }
            InstructionKind::NullIsNull { value } => {
                self.emit_operand(value, Some(ClrValueKind::Reference))?;
                self.push_opcode(MsilOpcode::Ldnull);
                self.push_opcode(MsilOpcode::Ceq);
                self.store_or_pop_boolean(instruction)?;
            }
            InstructionKind::NullUnwrap { value } => {
                let output = self.output_of(instruction)?;
                self.emit_operand(value, Some(ClrValueKind::Reference))?;
                match output {
                    Some((slot, kind)) => {
                        if kind != ClrValueKind::Reference {
                            return Err(format!(
                                "CLR 最小 lowering：可空标量需要 sum 表示，不能按引用直接解包: function={} actual={kind:?}",
                                self.function.symbol
                            ));
                        }
                        self.store_local(slot);
                    }
                    None => self.emit_pop(),
                }
            }
            // 闭包捕获环境在 CLR 没有载体：必须点名 lambda 实例与捕获名结构化失败，
            // 不得丢弃环境、把捕获提升为模块全局或用直调替代。
            InstructionKind::LoadFunctionRef { instance, environment: Some(_), captured } => {
                return Err(format!(
                    "CLR 目标没有闭包载体：lambda instance `{instance:?}` 捕获 [{}]；function={}",
                    captured.join(", "),
                    self.function.symbol
                ));
            }
            other => {
                return Err(format!("CLR 最小 lowering 尚不支持指令 `{}`：function={}", instruction_kind_name(other), self.function.symbol));
            }
        }
        Ok(())
    }

    fn emit_terminator(&mut self, block: &MirBlock) -> Result<()> {
        match &block.terminator {
            Terminator::Return { value } => {
                let is_void = self.return_type == MsilType::Void;
                match (value, is_void) {
                    (Some(value), false) => {
                        let kind = ClrValueKind::of_msil_type(&self.return_type)?;
                        self.emit_operand(value, Some(kind))?;
                        self.push_opcode(MsilOpcode::Ret);
                    }
                    (None, true) => self.push_opcode(MsilOpcode::Ret),
                    (Some(_), true) => {
                        return Err(format!("CLR 最小 lowering：函数 `{}` 返回了值但签名是 void", self.function.symbol));
                    }
                    (None, false) => {
                        return Err(format!("CLR 最小 lowering：非 void 函数 `{}` 缺少返回值", self.function.symbol));
                    }
                }
            }
            Terminator::Jump { target, arguments } => {
                let target_label = block_label(*target);
                let (parameter_count, kinds) = {
                    let target_block = self
                        .function
                        .blocks
                        .get(target.0 as usize)
                        .ok_or_else(|| format!("CLR 最小 lowering：函数 `{}` 跳转目标块 {:?} 不存在", self.function.symbol, target))?;
                    let kinds = target_block
                        .parameters
                        .iter()
                        .map(|parameter| {
                            let ty = self
                                .function
                                .value_types
                                .get(parameter)
                                .ok_or_else(|| format!("CLR 最小 lowering：块参数缺少冻结类型: function={}", self.function.symbol))?;
                            let kind = ClrValueKind::of_msil_type(&msil_parameter_type(self.submission.backend_plan.aggregate_layouts(), ty)?)?;
                            Ok(kind)
                        })
                        .collect::<Result<Vec<_>>>()?;
                    (target_block.parameters.len(), kinds)
                };
                if arguments.len() != parameter_count {
                    return Err(format!(
                        "CLR 最小 lowering：跳转目标块 {:?} 形参数量与实参不符: function={} expected={} actual={}",
                        target,
                        self.function.symbol,
                        parameter_count,
                        arguments.len()
                    ));
                }
                for (argument, kind) in arguments.iter().zip(&kinds) {
                    self.emit_operand(argument, Some(*kind))?;
                }
                for index in (0..arguments.len()).rev() {
                    let local = *self
                        .slots
                        .block_param_locals
                        .get(&(*target, index))
                        .ok_or_else(|| format!("CLR 最小 lowering：跳转目标块 {:?} 形参 {index} 缺少局部槽", target))?;
                    self.store_local(local);
                }
                self.push_branch(MsilOpcode::Br, target_label);
            }
            Terminator::Branch { condition, then_target, else_target } => {
                let targets_have_parameters = self
                    .function
                    .blocks
                    .iter()
                    .filter(|block| block.id == *then_target || block.id == *else_target)
                    .any(|block| !block.parameters.is_empty());
                if targets_have_parameters {
                    return Err(format!("CLR 最小 lowering 只支持无块参数的条件分支：function={}", self.function.symbol));
                }
                self.emit_operand(condition, Some(ClrValueKind::Int32))?;
                self.push_branch(MsilOpcode::Brfalse, block_label(*else_target));
                self.push_branch(MsilOpcode::Br, block_label(*then_target));
            }
            Terminator::Unreachable => {
                if self.return_type == MsilType::Void {
                    self.push_opcode(MsilOpcode::Ret);
                }
                else {
                    return Err(format!("CLR 最小 lowering：非 void 函数 `{}` 的 `Unreachable` 终结缺少返回指令", self.function.symbol));
                }
            }
            other => {
                return Err(format!("CLR 最小 lowering 尚不支持终结符 `{other:?}`: function={}", self.function.symbol));
            }
        }
        Ok(())
    }

    fn emit_call(&mut self, callee: &Operand, arguments: &[Operand], output: Option<(u16, ClrValueKind)>) -> Result<()> {
        let Operand::Item(instance) = callee
        else {
            return Err(format!("CLR 最小 lowering 只支持 `Item` 身份的静态调用: function={} callee={callee:?}", self.function.symbol));
        };
        if let Some((mnemonic, binding)) = self.operator_by_instance.get(instance) {
            return self.emit_operator_call(mnemonic, binding, arguments, output);
        }
        if let Some(import) = self.submission.backend_plan.imports().get(instance) {
            return Err(format!(
                "CLR 最小 lowering 尚不支持宿主导入 `{instance:?}`（链接 `{}`）：缺少 CLR host 编码，function={}",
                import.link, self.function.symbol
            ));
        }
        let Some(method) = self.method_plans.get(instance).cloned()
        else {
            return Err(format!("CLR 最小 lowering：同片段调用目标 `{instance:?}` 不在可达方法集中: function={}", self.function.symbol));
        };
        let parameter_kinds = method.signature.parameter_types.iter().map(ClrValueKind::of_msil_type).collect::<Result<Vec<_>>>()?;
        if arguments.len() != parameter_kinds.len() {
            return Err(format!(
                "CLR 最小 lowering：调用 `{}` 实参数量与冻结签名不符: function={} expected={} actual={}",
                method.name,
                self.function.symbol,
                parameter_kinds.len(),
                arguments.len()
            ));
        }
        for (argument, kind) in arguments.iter().zip(&parameter_kinds) {
            self.emit_operand(argument, Some(*kind))?;
        }
        self.instructions.push(MsilInstruction {
            label: None,
            opcode: MsilOpcode::Call,
            operand: Some(MsilInstructionOperand::Method(MsilMethodRef {
                owner: None,
                name: method.name.clone(),
                signature: method.signature.clone(),
            })),
        });
        self.store_call_result(&method, output)
    }

    /// `[clr_operator("...")]` 绑定：只机械展开计划冻结的配方。
    fn emit_operator_call(
        &mut self,
        mnemonic: &str,
        binding: &TargetOperatorBinding,
        arguments: &[Operand],
        output: Option<(u16, ClrValueKind)>,
    ) -> Result<()> {
        let TargetOperatorBinding::Frozen(plan) = binding
        else {
            return Err(format!("CLR 目标 `clr_operator` 绑定缺少可编码助记符 `{mnemonic}`: function={}", self.function.symbol));
        };
        let TargetOperatorEncoding::Clr(opcode) = plan.encoding
        else {
            return Err(format!("CLR 目标 `clr_operator` 绑定 `{mnemonic}` 冻结了非 CLR 编码: function={}", self.function.symbol));
        };
        if arguments.len() != plan.arity {
            return Err(format!(
                "CLR 目标 operator `{mnemonic}` 实参数量与冻结 arity 不符: function={} expected={} actual={}",
                self.function.symbol,
                plan.arity,
                arguments.len()
            ));
        }
        let expected_kind = ClrValueKind::of_operand_type(plan.operand_type);
        self.emit_operator_operands(mnemonic, expected_kind, plan, arguments)?;
        self.push_opcode(opcode);
        match output {
            Some((slot, result_kind)) => {
                if result_kind != ClrValueKind::of_operand_type(plan.result_type) {
                    return Err(format!(
                        "CLR 目标 operator `{mnemonic}` 结果槽载体与冻结 result_type 不符: function={} expected={:?} actual={result_kind:?}",
                        self.function.symbol, plan.result_type
                    ));
                }
                self.store_local(slot);
            }
            // CLR `pop` 与栈值宽度无关，允许按需丢弃 operator 结果。
            None => self.emit_pop(),
        }
        Ok(())
    }

    fn emit_operator_operands(
        &mut self,
        mnemonic: &str,
        expected_kind: ClrValueKind,
        plan: &TargetOperatorPlan,
        arguments: &[Operand],
    ) -> Result<()> {
        match plan.recipe {
            TargetOperatorRecipe::Operands => self.emit_operator_arguments(mnemonic, expected_kind, arguments),
            TargetOperatorRecipe::Steps(steps) => {
                for step in steps {
                    match step {
                        TargetOperandStep::Arguments => self.emit_operator_arguments(mnemonic, expected_kind, arguments)?,
                        TargetOperandStep::NarrowTopI64ToI32 => self.push_opcode(MsilOpcode::ConvI4),
                        TargetOperandStep::IntegerConstant { operand_type, value } => {
                            let kind = ClrValueKind::of_operand_type(*operand_type);
                            if kind != expected_kind {
                                return Err(format!(
                                    "CLR 目标 operator `{mnemonic}` 的常量配方载体与冻结操作数载体不符: function={} expected={expected_kind:?} actual={kind:?}",
                                    self.function.symbol
                                ));
                            }
                            match kind {
                                ClrValueKind::Int32 => {
                                    self.push_integer_constant(i64::from(*value), MsilOpcode::LdcI4);
                                }
                                ClrValueKind::Int64 => {
                                    self.push_integer_constant(i64::from(*value), MsilOpcode::LdcI8);
                                }
                                ClrValueKind::Float32 | ClrValueKind::Float64 | ClrValueKind::Reference => {
                                    return Err(format!(
                                        "CLR 目标 operator `{mnemonic}` 冻结了非整数常量配方 {operand_type:?}: function={}",
                                        self.function.symbol
                                    ));
                                }
                            }
                        }
                    }
                }
                Ok(())
            }
        }
    }

    fn emit_operator_arguments(&mut self, mnemonic: &str, expected_kind: ClrValueKind, arguments: &[Operand]) -> Result<()> {
        for argument in arguments {
            self.emit_operand(argument, Some(expected_kind)).map_err(|error| {
                format!("CLR 目标 operator `{mnemonic}` 实参载体与冻结 operand_type 不符: function={} detail={error}", self.function.symbol)
            })?;
        }
        Ok(())
    }

    fn store_call_result(&mut self, method: &ClrMethodPlan, output: Option<(u16, ClrValueKind)>) -> Result<()> {
        if method.signature.return_type == MsilType::Void {
            if output.is_some() {
                return Err(format!("CLR 最小 lowering：void 调用 `{}` 不得绑定结果槽: function={}", method.name, self.function.symbol));
            }
            return Ok(());
        }
        let return_kind = ClrValueKind::of_msil_type(&method.signature.return_type)?;
        match output {
            Some((slot, kind)) => {
                if kind != return_kind {
                    return Err(format!(
                        "CLR 最小 lowering：调用 `{}` 结果载体与冻结返回签名不符: function={} expected={return_kind:?} actual={kind:?}",
                        method.name, self.function.symbol
                    ));
                }
                self.store_local(slot);
                Ok(())
            }
            None => {
                self.emit_pop();
                Ok(())
            }
        }
    }

    fn layout_by_id(&self, layout_id: LayoutId) -> Result<AggregateLayout> {
        self.submission
            .backend_plan
            .aggregate_layouts()
            .layouts
            .iter()
            .find(|layout| layout.id == layout_id)
            .cloned()
            .ok_or_else(|| format!("CLR 最小 lowering：布局身份 `{layout_id}` 未在计划中注册: function={}", self.function.symbol))
    }

    fn layout_by_nominal(&self, nominal: NominalInstanceId) -> Result<AggregateLayout> {
        let Some(layout_id) = self.submission.backend_plan.aggregate_layout_by_nominal().get(&nominal).copied()
        else {
            return Err(format!("CLR 最小 lowering：nominal `{nominal:?}` 没有冻结的布局身份: function={}", self.function.symbol));
        };
        self.layout_by_id(layout_id)
    }

    fn layout_field_type(&self, layout: &AggregateLayout, slot: u32) -> Result<(MsilType, ClrValueKind)> {
        let Some(field) = layout.fields.get(slot as usize)
        else {
            return Err(format!("CLR 最小 lowering：布局 `{}` 缺少字段槽 {slot}: function={}", layout.name, self.function.symbol));
        };
        let carrier = msil_type(self.submission.backend_plan.aggregate_layouts(), &field.ty)?;
        let kind = ClrValueKind::of_msil_type(&carrier)?;
        Ok((carrier, kind))
    }

    fn field_location(&self, field: FieldId) -> Result<(AggregateLayout, u32)> {
        let Some((layout_id, slot)) = self.submission.backend_plan.aggregate_layout_by_field().get(&field).copied()
        else {
            return Err(format!("CLR 最小 lowering：字段身份 `{field:?}` 未登记布局索引: function={}", self.function.symbol));
        };
        Ok((self.layout_by_id(layout_id)?, slot))
    }

    fn operand_type(&self, operand: &Operand) -> Result<NyarType> {
        match operand {
            Operand::Value(value) => self
                .function
                .value_types
                .get(value)
                .cloned()
                .ok_or_else(|| format!("CLR 最小 lowering：SSA 值 {:?} 缺少冻结类型: function={}", value, self.function.symbol)),
            Operand::Constant(constant) => Ok(match constant {
                Constant::Int(_) => NyarType::Integer32 { signed: true },
                Constant::Float64(_) => NyarType::Float64,
                Constant::Bool(_) => NyarType::Boolean,
                Constant::Utf8(_) | Constant::Utf16(_) => NyarType::Utf8,
                other => {
                    return Err(format!("CLR 最小 lowering：常量 `{other:?}` 没有可装箱的冻结类型: function={}", self.function.symbol));
                }
            }),
            other => Err(format!("CLR 最小 lowering：操作数 `{other:?}` 没有冻结类型: function={}", self.function.symbol)),
        }
    }

    fn output_type(&self, instruction: &MirInstruction) -> Result<Option<NyarType>> {
        let Some(value) = self.slots.instruction_output(instruction)
        else {
            return Ok(None);
        };
        self.function
            .value_types
            .get(&value)
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("CLR 最小 lowering：SSA 值 {:?} 缺少冻结类型: function={}", value, self.function.symbol))
    }

    fn reference_operand_slot(&self, operand: &Operand) -> Result<u16> {
        let Operand::Value(value) = operand
        else {
            return Err(format!("CLR 最小 lowering：只有 SSA 值可作为引用局部目标: function={} actual={operand:?}", self.function.symbol));
        };
        let (slot, kind) = self.value_slot(*value)?;
        if kind != ClrValueKind::Reference {
            return Err(format!("CLR 最小 lowering：聚合拷贝目标必须是引用载体: function={} actual={kind:?}", self.function.symbol));
        }
        Ok(slot)
    }

    fn store_operand(&mut self, operand: &Operand, kind: ClrValueKind) -> Result<()> {
        let Operand::Value(value) = operand
        else {
            return Err(format!("CLR 最小 lowering：只有 SSA 值可作为局部目标: function={} dest={operand:?}", self.function.symbol));
        };
        let (slot, actual) = self.value_slot(*value)?;
        if actual != kind {
            return Err(format!(
                "CLR 最小 lowering：目标 SSA 值 {:?} 的槽载体 {actual:?} 与期望 {kind:?} 不符: function={}",
                value, self.function.symbol
            ));
        }
        self.store_local(slot);
        Ok(())
    }

    fn push_newobj(&mut self, type_name: &str) {
        self.instructions.push(MsilInstruction {
            label: None,
            opcode: MsilOpcode::Newobj,
            operand: Some(MsilInstructionOperand::Method(MsilMethodRef {
                owner: Some(type_name.to_string()),
                name: ".ctor".to_string(),
                signature: MsilMethodSignature::new_instance(MsilType::Void, Vec::new()),
            })),
        });
    }

    fn push_type_opcode(&mut self, opcode: MsilOpcode, ty: &MsilType) {
        self.instructions.push(MsilInstruction { label: None, opcode, operand: Some(MsilInstructionOperand::Type(ty.to_string())) });
    }

    /// 数组元素加载：`ldelem.any` 必须携带元素类型 token，其它专用指令不带。
    fn push_array_load(&mut self, element: &MsilType) {
        let opcode = clr_array_load_opcode(element);
        if opcode == MsilOpcode::LdelemAny {
            self.push_type_opcode(opcode, element);
        }
        else {
            self.push_opcode(opcode);
        }
    }

    /// 与 [`Self::push_array_load`] 对称的元素写入。
    fn push_array_store(&mut self, element: &MsilType) {
        let opcode = clr_array_store_opcode(element);
        if opcode == MsilOpcode::StelemAny {
            self.push_type_opcode(opcode, element);
        }
        else {
            self.push_opcode(opcode);
        }
    }

    fn push_field_opcode(&mut self, opcode: MsilOpcode, owner: &str, name: &str) {
        self.instructions.push(MsilInstruction {
            label: None,
            opcode,
            operand: Some(MsilInstructionOperand::Field(owner.to_string(), name.to_string())),
        });
    }

    fn emit_layout_new(&mut self, layout: &AggregateLayout, output: (u16, ClrValueKind)) -> Result<()> {
        if output.1 != ClrValueKind::Reference {
            return Err(format!(
                "CLR 最小 lowering：布局 `{}` 的构造结果必须是引用槽: function={} actual={:?}",
                layout.name, self.function.symbol, output.1
            ));
        }
        self.push_newobj(&clr_layout_type_name(layout));
        self.store_local(output.0);
        Ok(())
    }

    /// 递归复制值语义聚合：目标槽得到独立对象，嵌套值聚合逐层新建。
    ///
    /// 引用语义字段按共享分类直接搬运；空嵌套字段保持 `null`。分类事实来自
    /// 共享 `value_copy` 模块，不在本后端按类型名或字段名特判。
    fn emit_value_aggregate_copy(&mut self, layout: &AggregateLayout, source_slot: u16, dest_slot: u16, depth: u32) -> Result<()> {
        if depth > MAX_VALUE_AGGREGATE_COPY_DEPTH {
            return Err(format!(
                "CLR 最小 lowering：值聚合复制嵌套 {depth} 层超过上限 {MAX_VALUE_AGGREGATE_COPY_DEPTH}: function={}",
                self.function.symbol
            ));
        }
        self.emit_layout_new(layout, (dest_slot, ClrValueKind::Reference))?;
        let type_name = clr_layout_type_name(layout);
        for slot in 0..layout.fields.len() {
            let field_name = clr_layout_field_name(slot);
            let Some(nested_id) = nested_value_aggregate_layout(self.submission.backend_plan.aggregate_layouts(), &layout.fields[slot].ty)
            else {
                self.load_local(dest_slot);
                self.load_local(source_slot);
                self.push_field_opcode(MsilOpcode::Ldfld, &type_name, &field_name);
                self.push_field_opcode(MsilOpcode::Stfld, &type_name, &field_name);
                continue;
            };
            let nested_layout = self.layout_by_id(nested_id)?;
            let nested_source = self.copy_scratch_slot(depth, 0)?;
            let nested_dest = self.copy_scratch_slot(depth, 1)?;
            self.load_local(source_slot);
            self.push_field_opcode(MsilOpcode::Ldfld, &type_name, &field_name);
            self.store_local(nested_source);
            let null_label = self.fresh_copy_label("null");
            let end_label = self.fresh_copy_label("end");
            self.load_local(nested_source);
            self.push_branch(MsilOpcode::Brfalse, null_label.clone());
            self.emit_value_aggregate_copy(&nested_layout, nested_source, nested_dest, depth + 1)?;
            self.load_local(dest_slot);
            self.load_local(nested_dest);
            self.push_field_opcode(MsilOpcode::Stfld, &type_name, &field_name);
            self.push_branch(MsilOpcode::Br, end_label.clone());
            self.push_label(null_label);
            self.load_local(dest_slot);
            self.push_opcode(MsilOpcode::Ldnull);
            self.push_field_opcode(MsilOpcode::Stfld, &type_name, &field_name);
            self.push_label(end_label);
        }
        Ok(())
    }

    fn copy_scratch_slot(&self, depth: u32, offset: usize) -> Result<u16> {
        let index = (depth as usize).saturating_mul(2).saturating_add(offset);
        self.copy_scratch
            .get(index)
            .copied()
            .ok_or_else(|| format!("CLR 最小 lowering：值聚合复制临时槽 {index} 未预留: function={}", self.function.symbol))
    }

    fn fresh_copy_label(&mut self, suffix: &str) -> String {
        self.label_ordinal = self.label_ordinal.saturating_add(1);
        format!("copy_{suffix}_{}", self.label_ordinal)
    }

    fn push_label(&mut self, label: String) {
        self.instructions.push(MsilInstruction { label: Some(label), opcode: MsilOpcode::Nop, operand: None });
    }

    fn emit_layout_field_write(&mut self, layout: &AggregateLayout, output_slot: u16, slot: usize, value: &Operand) -> Result<()> {
        let (_, kind) = self.layout_field_type(layout, u32::try_from(slot).unwrap_or(u32::MAX))?;
        self.load_local(output_slot);
        self.emit_operand(value, Some(kind))?;
        self.push_field_opcode(MsilOpcode::Stfld, &clr_layout_type_name(layout), &clr_layout_field_name(slot));
        Ok(())
    }

    /// 发射一个可能已处于标量槽的 payload/元素，并按目标类型按需装箱。
    fn emit_boxed_operand(&mut self, operand: &Operand, ty: &NyarType) -> Result<()> {
        let carrier = msil_type(self.submission.backend_plan.aggregate_layouts(), ty)
            .map_err(|error| format!("{error}: function={}", self.function.symbol))?;
        match &carrier {
            MsilType::String | MsilType::Object | MsilType::Named(_) | MsilType::SzArray(_) => {
                self.emit_operand(operand, Some(ClrValueKind::Reference))
            }
            MsilType::Void => {
                Err(format!("CLR 最小 lowering：`{ty}` 是 void 形态，不能作为需要引用的值: function={}", self.function.symbol))
            }
            scalar => {
                let kind = ClrValueKind::of_msil_type(scalar)?;
                self.emit_operand(operand, Some(kind))?;
                self.push_type_opcode(MsilOpcode::Box, scalar);
                Ok(())
            }
        }
    }

    /// 发射数组元素值：标量进入引用元素槽时按槽的精确保鲜载体装箱。
    fn emit_array_element_value(&mut self, operand: &Operand, element: &NyarType) -> Result<()> {
        let element_carrier = msil_array_element_type(self.submission.backend_plan.aggregate_layouts(), element)
            .map_err(|error| format!("{error}: function={}", self.function.symbol))?;
        let element_kind = ClrValueKind::of_msil_type(&element_carrier)?;
        if element_kind == ClrValueKind::Reference {
            let operand_ty = self.operand_type(operand)?;
            let operand_carrier = msil_type(self.submission.backend_plan.aggregate_layouts(), &operand_ty)
                .map_err(|error| format!("{error}: function={}", self.function.symbol))?;
            let operand_kind = ClrValueKind::of_msil_type(&operand_carrier)?;
            if operand_kind != ClrValueKind::Reference {
                self.emit_operand(operand, Some(operand_kind))?;
                self.push_type_opcode(MsilOpcode::Box, &operand_carrier);
                return Ok(());
            }
        }
        self.emit_operand(operand, Some(element_kind))
    }

    fn emit_sum_new(
        &mut self,
        instruction: &MirInstruction,
        nominal: NominalInstanceId,
        variant: VariantId,
        payload: &Option<Operand>,
    ) -> Result<()> {
        let Some(sum) = self.submission.backend_plan.sum_representations().get(&nominal).cloned()
        else {
            return Err(format!(
                "CLR 最小 lowering：sum nominal `{nominal:?}` 没有冻结的 `SumRepresentation`: function={}",
                self.function.symbol
            ));
        };
        let Some(variant_rep) = sum.variants.get(&variant).copied()
        else {
            return Err(format!(
                "CLR 最小 lowering：variant `{variant:?}` 不属于 sum nominal `{nominal:?}`: function={}",
                self.function.symbol
            ));
        };
        if payload.is_some() != variant_rep.payload_type.is_some() {
            return Err(format!(
                "CLR 最小 lowering：variant `{variant:?}` 的 payload 操作数与冻结 payload 类型不一致: function={} nominal={nominal:?} payload={} expected={}",
                self.function.symbol,
                payload.is_some(),
                variant_rep.payload_type.is_some()
            ));
        }
        let tag = i32::try_from(variant_rep.tag)
            .map_err(|_| format!("CLR 最小 lowering：variant `{variant:?}` 的 tag 超出 `int32` 载体: function={}", self.function.symbol))?;
        let output = self.require_output(instruction)?;
        if output.1 != ClrValueKind::Reference {
            return Err(format!("CLR 最小 lowering：sum 构造结果必须是引用槽: function={} actual={:?}", self.function.symbol, output.1));
        }
        let type_name = clr_sum_type_name(nominal);
        self.push_newobj(&type_name);
        self.store_local(output.0);
        self.load_local(output.0);
        self.push_integer_constant(i64::from(tag), MsilOpcode::LdcI4);
        self.push_field_opcode(MsilOpcode::Stfld, &type_name, CLR_SUM_TAG_FIELD);
        if let Some(payload) = payload {
            let payload_type = self.operand_type(payload)?;
            self.load_local(output.0);
            self.emit_boxed_operand(payload, &payload_type)?;
            self.push_field_opcode(MsilOpcode::Stfld, &type_name, CLR_SUM_PAYLOAD_FIELD);
        }
        Ok(())
    }

    fn emit_sum_variant_is(
        &mut self,
        instruction: &MirInstruction,
        nominal: NominalInstanceId,
        variant: VariantId,
        object: &Operand,
    ) -> Result<()> {
        let Some(sum) = self.submission.backend_plan.sum_representations().get(&nominal).cloned()
        else {
            return Err(format!(
                "CLR 最小 lowering：sum nominal `{nominal:?}` 没有冻结的 `SumRepresentation`: function={}",
                self.function.symbol
            ));
        };
        let Some(variant_rep) = sum.variants.get(&variant).copied()
        else {
            return Err(format!(
                "CLR 最小 lowering：variant `{variant:?}` 不属于 sum nominal `{nominal:?}`: function={}",
                self.function.symbol
            ));
        };
        let tag = i32::try_from(variant_rep.tag)
            .map_err(|_| format!("CLR 最小 lowering：variant `{variant:?}` 的 tag 超出 `int32` 载体: function={}", self.function.symbol))?;
        let type_name = clr_sum_type_name(nominal);
        self.emit_operand(object, Some(ClrValueKind::Reference))?;
        self.push_field_opcode(MsilOpcode::Ldfld, &type_name, CLR_SUM_TAG_FIELD);
        self.push_integer_constant(i64::from(tag), MsilOpcode::LdcI4);
        self.push_opcode(MsilOpcode::Ceq);
        self.store_or_pop_boolean(instruction)
    }

    fn emit_sum_payload_get(
        &mut self,
        instruction: &MirInstruction,
        nominal: NominalInstanceId,
        variant: VariantId,
        object: &Operand,
    ) -> Result<()> {
        let Some(sum) = self.submission.backend_plan.sum_representations().get(&nominal).cloned()
        else {
            return Err(format!(
                "CLR 最小 lowering：sum nominal `{nominal:?}` 没有冻结的 `SumRepresentation`: function={}",
                self.function.symbol
            ));
        };
        let Some(variant_rep) = sum.variants.get(&variant).copied()
        else {
            return Err(format!(
                "CLR 最小 lowering：variant `{variant:?}` 不属于 sum nominal `{nominal:?}`: function={}",
                self.function.symbol
            ));
        };
        if variant_rep.payload_type.is_none() {
            return Err(format!(
                "CLR 最小 lowering：variant `{variant:?}` 没有 payload，不能取载荷: function={} nominal={nominal:?}",
                self.function.symbol
            ));
        }
        let type_name = clr_sum_type_name(nominal);
        self.emit_operand(object, Some(ClrValueKind::Reference))?;
        self.push_field_opcode(MsilOpcode::Ldfld, &type_name, CLR_SUM_PAYLOAD_FIELD);

        let Some((slot, kind)) = self.output_of(instruction)?
        else {
            self.emit_pop();
            return Ok(());
        };
        let Some(target_type) = self.output_type(instruction)?
        else {
            return Err(format!("CLR 最小 lowering：sum payload 结果缺少冻结类型: function={}", self.function.symbol));
        };
        let target_carrier = msil_type(self.submission.backend_plan.aggregate_layouts(), &target_type)?;
        let target_kind = ClrValueKind::of_msil_type(&target_carrier)?;
        if target_kind != kind {
            return Err(format!(
                "CLR 最小 lowering：sum payload 目标载体 {target_kind:?} 与结果槽 {kind:?} 不符: function={}",
                self.function.symbol
            ));
        }
        if target_kind != ClrValueKind::Reference {
            self.push_type_opcode(MsilOpcode::UnboxAny, &target_carrier);
        }
        else if matches!(target_carrier, MsilType::Named(_) | MsilType::SzArray(_)) {
            self.push_type_opcode(MsilOpcode::Castclass, &target_carrier);
        }
        self.store_local(slot);
        Ok(())
    }

    fn store_or_pop_boolean(&mut self, instruction: &MirInstruction) -> Result<()> {
        match self.output_of(instruction)? {
            Some((slot, kind)) => {
                if kind != ClrValueKind::Int32 {
                    return Err(format!(
                        "CLR 最小 lowering：布尔结果槽载体必须是 `int32`: function={} actual={kind:?}",
                        self.function.symbol
                    ));
                }
                self.store_local(slot);
                Ok(())
            }
            None => {
                self.emit_pop();
                Ok(())
            }
        }
    }

    fn emit_array_from_elements(&mut self, instruction: &MirInstruction, array_type: &NyarType, elements: &[Operand]) -> Result<()> {
        let element = clr_array_element_type(array_type)
            .map_err(|error| format!("{error}: function={}", self.function.symbol))?
            .clone();
        let carrier = msil_array_element_type(self.submission.backend_plan.aggregate_layouts(), &element)
            .map_err(|error| format!("{error}: function={}", self.function.symbol))?;
        let length = i32::try_from(elements.len())
            .map_err(|_| format!("CLR 最小 lowering：数组元素数量超出 `int32` 载体: function={}", self.function.symbol))?;
        self.push_integer_constant(i64::from(length), MsilOpcode::LdcI4);
        self.push_type_opcode(MsilOpcode::Newarr, &carrier);
        for (index, value) in elements.iter().enumerate() {
            let index = i32::try_from(index)
                .map_err(|_| format!("CLR 最小 lowering：数组元素下标超出 `int32` 载体: function={}", self.function.symbol))?;
            self.push_opcode(MsilOpcode::Dup);
            self.push_integer_constant(i64::from(index), MsilOpcode::LdcI4);
            self.emit_array_element_value(value, &element)?;
            self.push_array_store(&carrier);
        }
        match self.output_of(instruction)? {
            Some((slot, kind)) => {
                if kind != ClrValueKind::Reference {
                    return Err(format!("CLR 最小 lowering：数组结果必须是引用槽: function={} actual={kind:?}", self.function.symbol));
                }
                self.store_local(slot);
                Ok(())
            }
            None => {
                self.emit_pop();
                Ok(())
            }
        }
    }

    fn emit_array_new(&mut self, instruction: &MirInstruction, array_type: &NyarType, length: &Operand) -> Result<()> {
        let element = clr_array_element_type(array_type)
            .map_err(|error| format!("{error}: function={}", self.function.symbol))?
            .clone();
        let carrier = msil_array_element_type(self.submission.backend_plan.aggregate_layouts(), &element)
            .map_err(|error| format!("{error}: function={}", self.function.symbol))?;
        self.emit_operand(length, Some(ClrValueKind::Int32))?;
        self.push_type_opcode(MsilOpcode::Newarr, &carrier);
        match self.output_of(instruction)? {
            Some((slot, kind)) => {
                if kind != ClrValueKind::Reference {
                    return Err(format!("CLR 最小 lowering：数组结果必须是引用槽: function={} actual={kind:?}", self.function.symbol));
                }
                self.store_local(slot);
                Ok(())
            }
            None => {
                self.emit_pop();
                Ok(())
            }
        }
    }

    fn emit_array_get(&mut self, instruction: &MirInstruction, array: &Operand, index: &Operand) -> Result<()> {
        let array_type = self.operand_type(array)?;
        let element = clr_array_element_type(&array_type)
            .map_err(|error| format!("{error}: function={}", self.function.symbol))?
            .clone();
        let carrier = msil_array_element_type(self.submission.backend_plan.aggregate_layouts(), &element)
            .map_err(|error| format!("{error}: function={}", self.function.symbol))?;
        let carrier_kind = ClrValueKind::of_msil_type(&carrier)?;
        self.emit_operand(array, Some(ClrValueKind::Reference))?;
        self.emit_operand(index, Some(ClrValueKind::Int32))?;
        self.push_array_load(&carrier);
        let Some((slot, kind)) = self.output_of(instruction)?
        else {
            self.emit_pop();
            return Ok(());
        };
        let Some(target_type) = self.output_type(instruction)?
        else {
            return Err(format!("CLR 最小 lowering：数组元素结果缺少冻结类型: function={}", self.function.symbol));
        };
        let target_carrier = msil_type(self.submission.backend_plan.aggregate_layouts(), &target_type)?;
        let target_kind = ClrValueKind::of_msil_type(&target_carrier)?;
        if target_kind != kind {
            return Err(format!(
                "CLR 最小 lowering：数组元素目标载体 {target_kind:?} 与结果槽 {kind:?} 不符: function={}",
                self.function.symbol
            ));
        }
        if carrier_kind == ClrValueKind::Reference && target_kind != ClrValueKind::Reference {
            self.push_type_opcode(MsilOpcode::UnboxAny, &target_carrier);
        }
        self.store_local(slot);
        Ok(())
    }

    fn emit_array_set(&mut self, array: &Operand, index: &Operand, value: &Operand) -> Result<()> {
        let array_type = self.operand_type(array)?;
        let element = clr_array_element_type(&array_type)
            .map_err(|error| format!("{error}: function={}", self.function.symbol))?
            .clone();
        let carrier = msil_array_element_type(self.submission.backend_plan.aggregate_layouts(), &element)
            .map_err(|error| format!("{error}: function={}", self.function.symbol))?;
        self.emit_operand(array, Some(ClrValueKind::Reference))?;
        self.emit_operand(index, Some(ClrValueKind::Int32))?;
        self.emit_array_element_value(value, &element)?;
        self.push_array_store(&carrier);
        Ok(())
    }

    fn emit_operand(&mut self, operand: &Operand, expected: Option<ClrValueKind>) -> Result<()> {
        match operand {
            Operand::Value(value) => {
                let (slot, kind) = self.value_slot(*value)?;
                if let Some(expected) = expected
                    && expected != kind
                {
                    return Err(format!(
                        "CLR 最小 lowering：SSA 值 {:?} 的槽载体 {kind:?} 与期望 {expected:?} 不符: function={}",
                        value, self.function.symbol
                    ));
                }
                self.load_local(slot);
                Ok(())
            }
            Operand::Constant(constant) => self.emit_constant(constant, expected),
            Operand::Symbol(path) => Err(format!("CLR 最小 lowering 不支持符号操作数 `{path:?}`: function={}", self.function.symbol)),
            Operand::Item(instance) => {
                Err(format!("CLR 最小 lowering 不支持把可调用身份 `{instance:?}` 当值使用: function={}", self.function.symbol))
            }
        }
    }

    fn emit_constant(&mut self, constant: &Constant, expected: Option<ClrValueKind>) -> Result<()> {
        match constant {
            Constant::Int(value) => match expected {
                Some(ClrValueKind::Int64) => {
                    self.push_integer_constant(*value, MsilOpcode::LdcI8);
                    Ok(())
                }
                Some(ClrValueKind::Int32) | None => {
                    let narrowed = i32::try_from(*value).map_err(|_| {
                        format!(
                            "CLR 最小 lowering：整数常量 {value} 超出 `int32` 载体，且目标槽位未声明 `int64`: function={}",
                            self.function.symbol
                        )
                    })?;
                    self.push_integer_constant(i64::from(narrowed), MsilOpcode::LdcI4);
                    Ok(())
                }
                Some(other) => Err(format!("CLR 最小 lowering：整数常量不能进入 {other:?} 槽: function={}", self.function.symbol)),
            },
            Constant::Float64(value) => match expected {
                Some(ClrValueKind::Float64) => {
                    let value = value.into_inner();
                    if !value.is_finite() {
                        return Err(format!(
                            "CLR 最小 lowering：非有限 f64 常量 `{value}` 缺少文本载体，拒绝猜测编码: function={}",
                            self.function.symbol
                        ));
                    }
                    self.instructions.push(MsilInstruction {
                        label: None,
                        opcode: MsilOpcode::LdcR8,
                        operand: Some(MsilInstructionOperand::Float(format!("{value:?}"))),
                    });
                    Ok(())
                }
                Some(ClrValueKind::Float32) => Err(format!(
                    "CLR 最小 lowering 不支持把 f64 常量压入 `float32` 槽：当前 MSIL 模型没有 `ldc.r4`，function={}",
                    self.function.symbol
                )),
                Some(other) => Err(format!("CLR 最小 lowering：f64 常量不能进入 {other:?} 槽: function={}", self.function.symbol)),
                None => Err(format!("CLR 最小 lowering：f64 常量缺少明确的 `float64` 结果槽，拒绝猜测载体: function={}", self.function.symbol)),
            },
            Constant::Bool(value) => {
                if !matches!(expected, Some(ClrValueKind::Int32) | None) {
                    return Err(format!("CLR 最小 lowering：布尔常量不能进入 {expected:?} 槽: function={}", self.function.symbol));
                }
                self.push_integer_constant(i64::from(*value), MsilOpcode::LdcI4);
                Ok(())
            }
            Constant::Utf8(value) | Constant::Utf16(value) => match expected {
                Some(ClrValueKind::Reference) | None => {
                    self.instructions.push(MsilInstruction {
                        label: None,
                        opcode: MsilOpcode::Ldstr,
                        operand: Some(MsilInstructionOperand::StringLiteral(value.clone())),
                    });
                    Ok(())
                }
                Some(other) => Err(format!("CLR 最小 lowering：字符串常量不能进入 {other:?} 槽: function={}", self.function.symbol)),
            },
            other => Err(format!("CLR 最小 lowering 尚不支持常量 `{other:?}`：缺少 CLR 常量载体合同，function={}", self.function.symbol)),
        }
    }

    fn push_integer_constant(&mut self, value: i64, opcode: MsilOpcode) {
        self.instructions.push(MsilInstruction { label: None, opcode, operand: Some(MsilInstructionOperand::Integer(value)) });
    }

    fn load_local(&mut self, slot: u16) {
        self.instructions.push(MsilInstruction {
            label: None,
            opcode: MsilOpcode::Ldloc,
            operand: Some(MsilInstructionOperand::Integer(i64::from(slot))),
        });
    }

    fn store_local(&mut self, slot: u16) {
        self.instructions.push(MsilInstruction {
            label: None,
            opcode: MsilOpcode::Stloc,
            operand: Some(MsilInstructionOperand::Integer(i64::from(slot))),
        });
    }

    fn push_opcode(&mut self, opcode: MsilOpcode) {
        self.instructions.push(MsilInstruction { label: None, opcode, operand: None });
    }

    fn push_branch(&mut self, opcode: MsilOpcode, target: String) {
        self.instructions.push(MsilInstruction { label: None, opcode, operand: Some(MsilInstructionOperand::BranchTarget(target)) });
    }

    fn emit_pop(&mut self) {
        self.push_opcode(MsilOpcode::Pop);
    }

    fn output_of(&self, instruction: &MirInstruction) -> Result<Option<(u16, ClrValueKind)>> {
        match self.slots.instruction_output(instruction) {
            None => Ok(None),
            Some(value) => self.value_slot(value).map(Some),
        }
    }

    fn require_output(&self, instruction: &MirInstruction) -> Result<(u16, ClrValueKind)> {
        self.output_of(instruction)?
            .ok_or_else(|| format!("CLR 最小 lowering：指令缺少 SSA 结果槽，无法确定局部载体: function={}", self.function.symbol))
    }

    fn value_slot(&self, value: MirValueRef) -> Result<(u16, ClrValueKind)> {
        let local = *self
            .slots
            .value_locals
            .get(&value)
            .ok_or_else(|| format!("CLR 最小 lowering：SSA 值 {:?} 缺少规划的局部槽: function={}", value, self.function.symbol))?;
        Ok((local, self.local_kind(local)?))
    }

    fn local_kind(&self, local: u16) -> Result<ClrValueKind> {
        let ty = self
            .slots
            .local_types
            .get(local as usize)
            .ok_or_else(|| format!("CLR 最小 lowering：局部槽 {local} 超出规划表: function={}", self.function.symbol))?;
        ClrValueKind::of_msil_type(&msil_parameter_type(self.submission.backend_plan.aggregate_layouts(), ty)?)
    }
}

/// 指令种类的稳定诊断名（不依赖 Debug 输出细节）。
fn instruction_kind_name(kind: &InstructionKind) -> &'static str {
    match kind {
        InstructionKind::LoadConstant { .. } => "LoadConstant",
        InstructionKind::LoadSymbol { .. } => "LoadSymbol",
        InstructionKind::Copy { .. } => "Copy",
        InstructionKind::Cast { .. } => "Cast",
        InstructionKind::StoreVar { .. } => "StoreVar",
        InstructionKind::Call { .. } => "Call",
        InstructionKind::LoadFunctionRef { .. } => "LoadFunctionRef",
        InstructionKind::StructNew { .. } => "StructNew",
        InstructionKind::TupleNew { .. } => "TupleNew",
        InstructionKind::TupleGet { .. } => "TupleGet",
        InstructionKind::AggregateCopy { .. } => "AggregateCopy",
        InstructionKind::FieldGet { .. } => "FieldGet",
        InstructionKind::FieldSet { .. } => "FieldSet",
        InstructionKind::SumNew { .. } => "SumNew",
        InstructionKind::SumPayloadGet { .. } => "SumPayloadGet",
        InstructionKind::SumVariantIs { .. } => "SumVariantIs",
        InstructionKind::PatternMatch { .. } => "PatternMatch",
        InstructionKind::ArrayNew { .. } => "ArrayNew",
        InstructionKind::ArrayFromElements { .. } => "ArrayFromElements",
        InstructionKind::ArrayGet { .. } => "ArrayGet",
        InstructionKind::ArraySet { .. } => "ArraySet",
        InstructionKind::ArrayLength { .. } => "ArrayLength",
        InstructionKind::NullIsNull { .. } => "NullIsNull",
        InstructionKind::NullUnwrap { .. } => "NullUnwrap",
    }
}

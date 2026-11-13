//! JVM 最小 MIR → `class` 降低。
//!
//! 只消费计划已经冻结的事实：整型/浮点常量、SSA local、冻结 operator、
//! 块标签与条件分支、同片段 `invokestatic` 调用以及 `Return`；容器形状按冻结的
//! 布局身份、sum 表示与元素类型发射布局/sum companion 类及构造、字段、数组、
//! sum、可空指令。未冻结身份或缺少 JVM 载体合同的形状仍返回结构化错误，
//! 不补造指令、默认值或空体方法。
//!
//! 类内部名与方法命名都是稳定规则：类内部名取模块名与片段 id 的
//! [`crate::lowering::sanitize_symbol`] 结果；导出函数取计划冻结的
//! `wasm_export_names`，其余可达函数取 ABI 名的
//! [`crate::lowering::sanitize_jvm_method_symbol`] 结果，冲突时追加稳定序号。

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    FragmentSubmission, Result,
    backend_plan_views::{
        ExecutableBlock as MirBlock, ExecutableBlockRef as MirBlockRef, ExecutableFunction as MirFunction,
        ExecutableInstruction as MirInstruction, ExecutableValueRef as MirValueRef, NyarType,
    },
    contracts::{Constant, InstructionKind, Operand, Terminator},
    lowering::shared::executable::{block_label, collect_reachable_blocks, slots::ExecutableSlotPlan},
    managed_abi::{ManagedAbiContract, ManagedAbiEntry},
    target_operator::{
        TargetOperandStep, TargetOperandType, TargetOperatorBinding, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe,
    },
    transitional::binary::class::{
        JvmClassFile, JvmCodeBody, JvmFieldRef, JvmFieldSignature, JvmInstruction, JvmMethodDescriptor, JvmMethodRef, JvmMethodSignature,
        JvmTypeDescriptor,
    },
};
use nyar_types::{
    AggregateLayout, AggregateLayoutPlan, FieldId, ItemInstanceId, LayoutId, NominalInstanceId, VariantId,
    layout_choice::SumRepresentation,
};

use super::JvmOperatorCode;

const JVM_ACC_PUBLIC: u16 = 0x0001;
const JVM_ACC_STATIC: u16 = 0x0008;

/// sum companion 类的物理字段名；字段顺序由后端合同固定，禁止按 variant 名推断槽位。
const JVM_SUM_TAG_FIELD: &str = "tag";
const JVM_SUM_PAYLOAD_FIELD: &str = "payload";

/// 引用存储形状在 JVM 上的擦除载体：最小 lowering 只搬运引用（`aload`/`astore`/
/// `areturn`/调用描述符），值的物化与字段访问由计划冻结的实例级布局
/// （discriminator + payload 槽）与绑定 operator 负责，backend 不展开字段。
const JVM_REFERENCE_CARRIER: &str = "java/lang/Object";

/// JVM 槽位种类：决定 `iload`/`lload`/`dload` 等指令与宽度。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JvmSlotKind {
    /// `int` / `boolean` / `byte` / `char` / `short`，占 1 槽。
    Int,
    /// `long`，占 2 槽。
    Long,
    /// `float`，占 1 槽。
    Float,
    /// `double`，占 2 槽。
    Double,
    /// 引用，占 1 槽。
    Reference,
}

impl JvmSlotKind {
    fn of(descriptor: &JvmTypeDescriptor) -> Result<Self> {
        Ok(match descriptor {
            JvmTypeDescriptor::Void => return Err("`void` 不能作为局部变量或操作数载体".to_string()),
            JvmTypeDescriptor::Long => Self::Long,
            JvmTypeDescriptor::Float => Self::Float,
            JvmTypeDescriptor::Double => Self::Double,
            JvmTypeDescriptor::Object(_) | JvmTypeDescriptor::Array(_) => Self::Reference,
            JvmTypeDescriptor::Byte
            | JvmTypeDescriptor::Char
            | JvmTypeDescriptor::Int
            | JvmTypeDescriptor::Short
            | JvmTypeDescriptor::Boolean => Self::Int,
        })
    }

    fn of_operand_type(operand_type: TargetOperandType) -> Self {
        match operand_type {
            TargetOperandType::I32 => Self::Int,
            TargetOperandType::I64 => Self::Long,
            TargetOperandType::F32 => Self::Float,
            TargetOperandType::F64 => Self::Double,
        }
    }

    fn slot_width(self) -> u16 {
        match self {
            Self::Long | Self::Double => 2,
            Self::Int | Self::Float | Self::Reference => 1,
        }
    }

    fn load(self, slot: u16) -> JvmInstruction {
        match self {
            Self::Int => JvmInstruction::ILoad(slot),
            Self::Long => JvmInstruction::LLoad(slot),
            Self::Float => JvmInstruction::FLoad(slot),
            Self::Double => JvmInstruction::DLoad(slot),
            Self::Reference => JvmInstruction::ALoad(slot),
        }
    }

    fn store(self, slot: u16) -> JvmInstruction {
        match self {
            Self::Int => JvmInstruction::IStore(slot),
            Self::Long => JvmInstruction::LStore(slot),
            Self::Float => JvmInstruction::FStore(slot),
            Self::Double => JvmInstruction::DStore(slot),
            Self::Reference => JvmInstruction::AStore(slot),
        }
    }
}

/// 函数级 JVM 方法计划：稳定名字与冻结签名。
#[derive(Debug, Clone)]
struct JvmMethodPlan {
    name: String,
    descriptor: JvmMethodDescriptor,
}

/// JVM lowering 结果：主类、launcher companion 类与 managed ABI 契约。
#[derive(Debug, Clone)]
pub(crate) struct JvmLoweredFragment {
    /// 片段主类。
    pub class_file: JvmClassFile,
    /// 每个导出入口独立的 argv launcher 类。
    pub companion_classes: Vec<JvmClassFile>,
    /// 导出入口 ABI 契约；没有可导出入口时为 `None`。
    pub managed_abi: Option<ManagedAbiContract>,
}

/// 将片段降低为单个 JVM `class`，并为每个导出入口生成 argv launcher。
///
/// 只降低导出面（`wasm_export_names` / `exported_operations` / 入口操作）以及
/// 从它们出发、经同片段静态调用可达的函数；其余函数不进入 class，避免为不可达
/// 形状发明载体。
pub(crate) fn lower_fragment_to_jvm_class(submission: &FragmentSubmission) -> Result<JvmLoweredFragment> {
    let plan = &submission.backend_plan;
    let operations = collect_jvm_operations(submission);
    if operations.is_empty() {
        return Err("JVM 最小 lowering 没有可导出的可执行函数：片段既无 `wasm_export_names` 也无入口操作，拒绝凭空生成 class".to_string());
    }

    let mut used_names = BTreeMap::<String, ()>::new();
    let mut method_plans = BTreeMap::<ItemInstanceId, JvmMethodPlan>::new();
    let mut logical_signatures = BTreeMap::<ItemInstanceId, (Vec<NyarType>, NyarType)>::new();
    for (ordinal, instance) in operations.iter().enumerate() {
        let view = plan.get_function(instance).ok_or_else(|| format!("JVM 最小 lowering：可达操作 `{instance:?}` 缺少函数体"))?;
        logical_signatures.insert(*instance, (view.function.param_types.clone(), view.function.return_type.clone()));
        let base = plan
            .wasm_export_names()
            .get(instance)
            .cloned()
            .or_else(|| plan.abi_name_for_instance(*instance).map(|name| crate::lowering::sanitize_jvm_method_symbol(&name)))
            .unwrap_or_else(|| format!("function_{ordinal}"));
        let base = crate::lowering::sanitize_symbol(&base);
        let mut name = base.clone();
        let mut suffix = 0usize;
        while used_names.contains_key(&name) {
            suffix += 1;
            name = format!("{base}__{suffix}");
        }
        used_names.insert(name.clone(), ());
        method_plans.insert(*instance, JvmMethodPlan { name, descriptor: function_descriptor(plan.aggregate_layouts(), &view.function)? });
    }

    let internal_name = format!(
        "nyar/{}_{}",
        crate::lowering::sanitize_symbol(plan.module_name()),
        crate::lowering::sanitize_symbol(plan.fragment_id().as_str())
    );
    let operator_by_instance = external_operator_by_instance(submission);
    let mut class_file = JvmClassFile::new(internal_name.clone());
    for instance in &operations {
        let Some(view) = plan.get_function(instance)
        else {
            continue;
        };
        let method = method_plans.get(instance).cloned().ok_or_else(|| format!("JVM 最小 lowering：可达操作 `{instance:?}` 缺少方法计划"))?;
        class_file.methods.push(lower_function(submission, &internal_name, &view.function, &method, &method_plans, &operator_by_instance)?);
    }

    let (launcher_classes, managed_abi) = build_managed_abi_surface(&class_file, plan.wasm_export_names(), &method_plans, &logical_signatures)?;
    let mut companion_classes = build_layout_classes(plan.aggregate_layouts())?;
    companion_classes.extend(build_sum_classes(submission)?);
    companion_classes.extend(launcher_classes);
    let mut companion_names = BTreeSet::new();
    for companion in &companion_classes {
        if !companion_names.insert(companion.internal_name.clone()) {
            return Err(format!(
                "JVM 最小 lowering：companion 类内部名 `{}` 冲突：布局类、sum 类与 launcher 类必须各有唯一载体",
                companion.internal_name
            ));
        }
    }
    Ok(JvmLoweredFragment { class_file, companion_classes, managed_abi })
}

/// 为每个导出入口生成 argv launcher，并汇总 managed ABI 契约。
///
/// 入口必须已经以 `public static` 方法落在主类中；缺失方法或不可表达的标量类型
/// 都返回结构化错误，不静默跳过入口。
fn build_managed_abi_surface(
    class_file: &JvmClassFile,
    export_names: &BTreeMap<ItemInstanceId, String>,
    method_plans: &BTreeMap<ItemInstanceId, JvmMethodPlan>,
    logical_signatures: &BTreeMap<ItemInstanceId, (Vec<NyarType>, NyarType)>,
) -> Result<(Vec<JvmClassFile>, Option<ManagedAbiContract>)> {
    let mut entries = Vec::with_capacity(export_names.len());
    let mut companion_classes = Vec::with_capacity(export_names.len());
    let mut used_launcher_names = BTreeSet::new();
    for (instance, logical_entry) in export_names {
        let method = method_plans
            .get(instance)
            .ok_or_else(|| format!("JVM managed ABI：导出入口 `{logical_entry}` 的实例 `{instance:?}` 没有已降低方法"))?;
        let lowered = class_file
            .methods
            .iter()
            .find(|candidate| candidate.name == method.name && candidate.descriptor == method.descriptor)
            .ok_or_else(|| format!("JVM managed ABI：导出入口 `{logical_entry}` 的方法 `{}` 不在主类中", method.name))?;
        if lowered.access_flags & (JVM_ACC_PUBLIC | JVM_ACC_STATIC) != (JVM_ACC_PUBLIC | JVM_ACC_STATIC) {
            return Err(format!(
                "JVM managed ABI：导出入口 `{logical_entry}` 的方法 `{}` 不是 `public static`，拒绝生成 launcher",
                method.name
            ));
        }
        // 逻辑 ABI 类型来自冻结的 `NyarType` 签名，物理 descriptor 另存；不得从
        // 已擦除 signedness 的 descriptor 反推作者声明的逻辑类型。
        let (logical_params, logical_return) = logical_signatures
            .get(instance)
            .ok_or_else(|| format!("JVM managed ABI：导出入口 `{logical_entry}` 的实例 `{instance:?}` 缺少冻结逻辑签名"))?;
        let parameter_types = logical_params.iter().map(managed_logical_type_name).collect::<Result<Vec<_>>>()?;
        let return_type = managed_logical_type_name(logical_return)?;
        let launcher_class = managed_launcher_internal_name(logical_entry, &mut used_launcher_names)?;
        let launcher = build_managed_launcher_class(&launcher_class, &class_file.internal_name, method)?;
        entries.push(ManagedAbiEntry {
            logical_entry: logical_entry.clone(),
            owner: class_file.internal_name.clone(),
            method: method.name.clone(),
            signature: method.descriptor.to_string(),
            parameter_types,
            return_type,
            launcher_class,
        });
        companion_classes.push(launcher);
    }
    let managed_abi = if entries.is_empty() {
        None
    }
    else {
        // 产物文件名由驱动层最终确定；这里先留空，桥接层负责补全并刷新摘要。
        Some(ManagedAbiContract::new("jvm", String::new(), entries))
    };
    Ok((companion_classes, managed_abi))
}

/// managed ABI 的逻辑标量类型名：只来自冻结的 `NyarType` 签名，保留 signedness。
///
/// 没有 unsigned 编组能力时明确拒绝，禁止用有符号解析器冒充 unsigned；
/// 物理 descriptor 单独保存在 `signature` 字段，不参与逻辑类型推断。
fn managed_logical_type_name(ty: &NyarType) -> Result<String> {
    Ok(match ty {
        NyarType::Bottom | NyarType::Unit => "void",
        NyarType::Boolean => "bool",
        NyarType::Integer32 { signed: true } => "i32",
        NyarType::Integer64 { signed: true } => "i64",
        NyarType::Float64 => "f64",
        NyarType::Integer32 { signed: false } | NyarType::Integer64 { signed: false } => {
            return Err(format!("JVM managed ABI 没有 unsigned 编组能力（`{ty}`）：拒绝生成 argv launcher，不得用有符号解析器冒充"));
        }
        NyarType::Integer8 { .. } | NyarType::Integer16 { .. } | NyarType::Integer128 { .. } => {
            return Err(format!("JVM managed ABI 入口不支持 `{ty}`：只接受 i32 / i64 / f64 / bool / void"));
        }
        other => {
            return Err(format!("JVM managed ABI 不支持 `{other}` 作为入口标量：缺少 managed 标量契约"));
        }
    }
    .to_string())
}

/// launcher 内部名：`nyar/invoke/{sanitize(logical_entry)}`，非字母数字转 `_`。
fn managed_launcher_internal_name(logical_entry: &str, used: &mut BTreeSet<String>) -> Result<String> {
    let component = logical_entry.chars().map(|character| if character.is_ascii_alphanumeric() { character } else { '_' }).collect::<String>();
    if component.is_empty() {
        return Err(format!("JVM managed ABI：导出入口 `{logical_entry}` 无法生成合法 launcher 类名"));
    }
    let internal_name = format!("nyar/invoke/{component}");
    if !used.insert(internal_name.clone()) {
        return Err(format!("JVM managed ABI：多个导出入口映射到同一 launcher 类 `{internal_name}`"));
    }
    Ok(internal_name)
}

/// 构造 `public static void main(String[])` argv launcher。
///
/// 栈顺序：先压 `System.out`，再按声明类型解析每个 argv，调用主类静态方法，
/// 最后用 `println` 输出返回值；`void` 入口只调用不输出。
fn build_managed_launcher_class(launcher_internal_name: &str, owner: &str, method: &JvmMethodPlan) -> Result<JvmClassFile> {
    let mut launcher = JvmClassFile::new(launcher_internal_name.to_string());
    let mut instructions = Vec::new();
    let print_result = method.descriptor.return_type != JvmTypeDescriptor::Void;
    if print_result {
        instructions.push(JvmInstruction::GetStatic(JvmFieldRef {
            owner: "java/lang/System".to_string(),
            name: "out".to_string(),
            descriptor: JvmTypeDescriptor::Object("java/io/PrintStream".to_string()),
        }));
    }
    let mut argument_slots = 0u16;
    for (index, parameter) in method.descriptor.parameter_types.iter().enumerate() {
        let index = i32::try_from(index).map_err(|_| format!("JVM managed ABI launcher 形参序号超出 `int` 载体: {index}"))?;
        instructions.push(JvmInstruction::ALoad0);
        instructions.push(JvmInstruction::IConst(index));
        instructions.push(JvmInstruction::AALoad);
        instructions.push(JvmInstruction::InvokeStatic(argument_parse_method_ref(parameter)?));
        argument_slots = argument_slots.saturating_add(parameter.slot_count());
    }
    instructions.push(JvmInstruction::InvokeStatic(JvmMethodRef {
        owner: owner.to_string(),
        name: method.name.clone(),
        descriptor: method.descriptor.clone(),
    }));
    if print_result {
        instructions.push(JvmInstruction::InvokeVirtual(JvmMethodRef {
            owner: "java/io/PrintStream".to_string(),
            name: "println".to_string(),
            descriptor: JvmMethodDescriptor::new(vec![method.descriptor.return_type.clone()], JvmTypeDescriptor::Void),
        }));
    }
    instructions.push(JvmInstruction::Return);

    let max_stack = u16::from(print_result).saturating_add(argument_slots).saturating_add(2);
    launcher.methods.push(JvmMethodSignature {
        name: "main".to_string(),
        descriptor: JvmMethodDescriptor::new(
            vec![JvmTypeDescriptor::array(JvmTypeDescriptor::Object("java/lang/String".to_string()))],
            JvmTypeDescriptor::Void,
        ),
        access_flags: JVM_ACC_PUBLIC | JVM_ACC_STATIC,
        code: Some(JvmCodeBody { max_stack, max_locals: 1, instructions }),
    });
    Ok(launcher)
}

/// argv 解析调用：按声明标量类型选择 `Long.parseLong` / `Integer.parseInt` /
/// `Double.parseDouble` / `Boolean.parseBoolean`。
fn argument_parse_method_ref(descriptor: &JvmTypeDescriptor) -> Result<JvmMethodRef> {
    let string = JvmTypeDescriptor::Object("java/lang/String".to_string());
    let (owner, name, return_type) = match descriptor {
        JvmTypeDescriptor::Long => ("java/lang/Long", "parseLong", JvmTypeDescriptor::Long),
        JvmTypeDescriptor::Int | JvmTypeDescriptor::Byte | JvmTypeDescriptor::Char | JvmTypeDescriptor::Short => {
            ("java/lang/Integer", "parseInt", JvmTypeDescriptor::Int)
        }
        JvmTypeDescriptor::Double => ("java/lang/Double", "parseDouble", JvmTypeDescriptor::Double),
        JvmTypeDescriptor::Boolean => ("java/lang/Boolean", "parseBoolean", JvmTypeDescriptor::Boolean),
        JvmTypeDescriptor::Float => {
            return Err("JVM managed ABI launcher 不支持 `float` (f32) 形参".to_string());
        }
        JvmTypeDescriptor::Object(name) => {
            return Err(format!("JVM managed ABI launcher 不支持引用类型形参 `{name}`"));
        }
        JvmTypeDescriptor::Array(item) => {
            return Err(format!("JVM managed ABI launcher 不支持数组类型形参 `[{item}`"));
        }
        JvmTypeDescriptor::Void => {
            return Err("JVM managed ABI launcher 形参不能是 `void`".to_string());
        }
    };
    Ok(JvmMethodRef { owner: owner.to_string(), name: name.to_string(), descriptor: JvmMethodDescriptor::new(vec![string], return_type) })
}

/// 从导出面出发收集可达操作（按实例身份升序，保证稳定输出顺序）。
fn collect_jvm_operations(submission: &FragmentSubmission) -> Vec<ItemInstanceId> {
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

/// 冻结的 `[jvm_operator("...")]` 绑定：助记符只用于诊断，配方来自计划。
fn external_operator_by_instance(submission: &FragmentSubmission) -> BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)> {
    submission
        .backend_plan
        .imports()
        .iter()
        .filter_map(|(instance, import)| {
            let mnemonic = crate::lowering::shared::interop::jvm_operator_target(&import.link)?;
            Some((*instance, (mnemonic.to_string(), import.target_operator.unwrap_or(TargetOperatorBinding::Unfrozen))))
        })
        .collect()
}

fn function_descriptor(layouts: &AggregateLayoutPlan, function: &MirFunction) -> Result<JvmMethodDescriptor> {
    let entry =
        function.blocks.get(function.entry.0 as usize).ok_or_else(|| format!("JVM 最小 lowering：函数 `{}` 缺少入口块", function.symbol))?;
    let mut parameter_types = Vec::with_capacity(entry.parameters.len());
    for parameter in &entry.parameters {
        let ty =
            function.value_types.get(parameter).ok_or_else(|| format!("JVM 最小 lowering：函数 `{}` 入口形参缺少冻结类型", function.symbol))?;
        parameter_types.push(jvm_parameter_descriptor(layouts, ty)?);
    }
    Ok(JvmMethodDescriptor::new(parameter_types, jvm_return_descriptor(layouts, &function.return_type)?))
}

/// 局部 / 形参位置的 JVM 载体：`Unit`/`Bottom` 没有值槽。
fn jvm_parameter_descriptor(layouts: &AggregateLayoutPlan, ty: &NyarType) -> Result<JvmTypeDescriptor> {
    let descriptor = jvm_type_descriptor(layouts, ty)?;
    if descriptor == JvmTypeDescriptor::Void {
        return Err(format!("JVM 最小 lowering：`{ty}` 是 void 形态，不能作为方法形参或局部值"));
    }
    Ok(descriptor)
}

fn jvm_return_descriptor(layouts: &AggregateLayoutPlan, ty: &NyarType) -> Result<JvmTypeDescriptor> {
    jvm_type_descriptor(layouts, ty)
}

/// 布局 companion 类的稳定内部名：`nyar/layout/<sanitized(name)>$<layout id>`。
///
/// 同一 layout id 稳定、不同布局不撞名；名字只用于 companion 类，不是语义身份。
pub(crate) fn jvm_layout_class_name(layout: &AggregateLayout) -> String {
    format!("nyar/layout/{}${}", crate::lowering::sanitize_symbol(&layout.name), layout.id)
}

/// 布局字段在 companion 类中的唯一名字；构造与字段访问必须共用这一条规则。
fn jvm_layout_field_name(slot: usize) -> String {
    format!("field{slot}")
}

/// sum companion 类的稳定内部名：只由 nominal 身份派生，禁止按 variant 名/短名推断。
fn jvm_sum_class_name(nominal: NominalInstanceId, _sum: &SumRepresentation) -> String {
    format!("nyar/sum/${}", nominal.index())
}

/// 布局/sum companion 类的默认构造器签名。
///
/// `JvmClassFile::to_bytes` 对 `code: None` 的 `"<init>"` 方法自动合成
/// `aload_0; invokespecial java/lang/Object.<init>; return`，因此这里不手写空体。
fn jvm_default_constructor() -> JvmMethodSignature {
    JvmMethodSignature {
        name: "<init>".to_string(),
        descriptor: JvmMethodDescriptor::new(Vec::new(), JvmTypeDescriptor::Void),
        access_flags: JVM_ACC_PUBLIC,
        code: None,
    }
}

fn jvm_constructor_ref(owner: &str) -> JvmMethodRef {
    JvmMethodRef {
        owner: owner.to_string(),
        name: "<init>".to_string(),
        descriptor: JvmMethodDescriptor::new(Vec::new(), JvmTypeDescriptor::Void),
    }
}

fn jvm_layout_field_ref(layout: &AggregateLayout, slot: usize, descriptor: JvmTypeDescriptor) -> JvmFieldRef {
    JvmFieldRef { owner: jvm_layout_class_name(layout), name: jvm_layout_field_name(slot), descriptor }
}

/// 为所有需要堆载体的布局生成 companion 类。
///
/// 类名只由 layout 身份派生；JVM 没有内联结构体，逐字段复制只能走
/// `AggregateCopy`。零字段值存储布局与块 2 的载体拒绝保持同一合同。
fn build_layout_classes(layouts: &AggregateLayoutPlan) -> Result<Vec<JvmClassFile>> {
    let mut classes = Vec::new();
    for layout in &layouts.layouts {
        if !(layout.storage.is_reference() || !layout.fields.is_empty()) {
            continue;
        }
        let mut class_file = JvmClassFile::new(jvm_layout_class_name(layout));
        for (slot, field) in layout.fields.iter().enumerate() {
            class_file.fields.push(JvmFieldSignature {
                name: jvm_layout_field_name(slot),
                descriptor: jvm_type_descriptor(layouts, &field.ty)?,
                access_flags: JVM_ACC_PUBLIC,
            });
        }
        class_file.methods.push(jvm_default_constructor());
        classes.push(class_file);
    }
    Ok(classes)
}

/// 只为 MIR 中实际出现 sum 指令的 nominal 生成 tag/payload companion 类。
fn build_sum_classes(submission: &FragmentSubmission) -> Result<Vec<JvmClassFile>> {
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

    let mut classes = Vec::with_capacity(nominals.len());
    for nominal in nominals {
        let Some(sum) = plan.sum_representations().get(&nominal)
        else {
            return Err(format!("JVM 最小 lowering：sum nominal `{nominal:?}` 没有冻结的 `SumRepresentation`"));
        };
        if sum.tag_width == 0 || sum.tag_width > 4 {
            return Err(format!(
                "JVM 最小 lowering：sum nominal `{nominal:?}` 的 `tag_width`={} 超出 1..=4 合同",
                sum.tag_width
            ));
        }
        let mut class_file = JvmClassFile::new(jvm_sum_class_name(nominal, sum));
        class_file.fields.push(JvmFieldSignature {
            name: JVM_SUM_TAG_FIELD.to_string(),
            descriptor: JvmTypeDescriptor::Int,
            access_flags: JVM_ACC_PUBLIC,
        });
        class_file.fields.push(JvmFieldSignature {
            name: JVM_SUM_PAYLOAD_FIELD.to_string(),
            descriptor: JvmTypeDescriptor::Object(JVM_REFERENCE_CARRIER.to_string()),
            access_flags: JVM_ACC_PUBLIC,
        });
        class_file.methods.push(jvm_default_constructor());
        classes.push(class_file);
    }
    Ok(classes)
}

/// 数组创建指令：标量元素走 `newarray`，引用/装箱元素走 `anewarray`。
fn jvm_array_creation_instruction(element: &JvmTypeDescriptor) -> Result<JvmInstruction> {
    Ok(match element {
        JvmTypeDescriptor::Boolean => JvmInstruction::NewArray(4),
        JvmTypeDescriptor::Char => JvmInstruction::NewArray(5),
        JvmTypeDescriptor::Byte => JvmInstruction::NewArray(8),
        JvmTypeDescriptor::Short => JvmInstruction::NewArray(9),
        JvmTypeDescriptor::Int => JvmInstruction::NewArray(10),
        JvmTypeDescriptor::Object(name) => JvmInstruction::ANewArray(name.clone()),
        JvmTypeDescriptor::Array(_) => JvmInstruction::ANewArray(element.to_string()),
        JvmTypeDescriptor::Long | JvmTypeDescriptor::Float | JvmTypeDescriptor::Double => {
            return Err(format!(
                "JVM 最小 lowering：`{element}` 数组元素必须使用装箱引用数组，禁止生成无元素存取指令的原生数组"
            ));
        }
        JvmTypeDescriptor::Void => return Err("JVM 最小 lowering：`void` 不能作为数组元素载体".to_string()),
    })
}

/// 数组元素读写指令；宽值/引用元素统一走 `aaload`/`aastore`。
fn jvm_array_access_instructions(element: &JvmTypeDescriptor) -> Result<(JvmInstruction, JvmInstruction)> {
    Ok(match element {
        JvmTypeDescriptor::Boolean | JvmTypeDescriptor::Byte => (JvmInstruction::BALoad, JvmInstruction::BAStore),
        JvmTypeDescriptor::Char => (JvmInstruction::CALoad, JvmInstruction::CAStore),
        JvmTypeDescriptor::Short => (JvmInstruction::SALoad, JvmInstruction::SAStore),
        JvmTypeDescriptor::Int => (JvmInstruction::IALoad, JvmInstruction::IAStore),
        JvmTypeDescriptor::Object(_) | JvmTypeDescriptor::Array(_) => (JvmInstruction::AALoad, JvmInstruction::AAStore),
        JvmTypeDescriptor::Long | JvmTypeDescriptor::Float | JvmTypeDescriptor::Double => {
            return Err(format!("JVM 最小 lowering：`{element}` 数组元素缺少 `laload`/`faload`/`daload` 指令，必须先装箱"));
        }
        JvmTypeDescriptor::Void => return Err("JVM 最小 lowering：`void` 不能作为数组元素载体".to_string()),
    })
}

/// 标量载体的装箱类：i8/i16/i32 统一 `Integer`，其余按精确标量盒。
fn jvm_box_class_name(descriptor: &JvmTypeDescriptor) -> Result<String> {
    Ok(match descriptor {
        JvmTypeDescriptor::Long => "java/lang/Long".to_string(),
        JvmTypeDescriptor::Float => "java/lang/Float".to_string(),
        JvmTypeDescriptor::Double => "java/lang/Double".to_string(),
        JvmTypeDescriptor::Boolean => "java/lang/Boolean".to_string(),
        JvmTypeDescriptor::Char => "java/lang/Character".to_string(),
        JvmTypeDescriptor::Int | JvmTypeDescriptor::Byte | JvmTypeDescriptor::Short => "java/lang/Integer".to_string(),
        JvmTypeDescriptor::Object(name) => name.clone(),
        JvmTypeDescriptor::Array(_) => descriptor.to_string(),
        JvmTypeDescriptor::Void => return Err("JVM 最小 lowering：`void` 没有装箱载体".to_string()),
    })
}

fn jvm_box_value_of(descriptor: &JvmTypeDescriptor) -> Result<JvmMethodRef> {
    let (owner, parameter, return_type) = match descriptor {
        JvmTypeDescriptor::Long => ("java/lang/Long", JvmTypeDescriptor::Long, JvmTypeDescriptor::Object("java/lang/Long".to_string())),
        JvmTypeDescriptor::Float => {
            ("java/lang/Float", JvmTypeDescriptor::Float, JvmTypeDescriptor::Object("java/lang/Float".to_string()))
        }
        JvmTypeDescriptor::Double => {
            ("java/lang/Double", JvmTypeDescriptor::Double, JvmTypeDescriptor::Object("java/lang/Double".to_string()))
        }
        JvmTypeDescriptor::Boolean => {
            ("java/lang/Boolean", JvmTypeDescriptor::Boolean, JvmTypeDescriptor::Object("java/lang/Boolean".to_string()))
        }
        JvmTypeDescriptor::Char => {
            ("java/lang/Character", JvmTypeDescriptor::Char, JvmTypeDescriptor::Object("java/lang/Character".to_string()))
        }
        JvmTypeDescriptor::Int | JvmTypeDescriptor::Byte | JvmTypeDescriptor::Short => {
            ("java/lang/Integer", JvmTypeDescriptor::Int, JvmTypeDescriptor::Object("java/lang/Integer".to_string()))
        }
        JvmTypeDescriptor::Object(_) | JvmTypeDescriptor::Array(_) | JvmTypeDescriptor::Void => {
            return Err(format!("JVM 最小 lowering：`{descriptor}` 不是可装箱标量载体"));
        }
    };
    Ok(JvmMethodRef {
        owner: owner.to_string(),
        name: "valueOf".to_string(),
        descriptor: JvmMethodDescriptor::new(vec![parameter], return_type),
    })
}

fn jvm_unbox_method(descriptor: &JvmTypeDescriptor) -> Result<JvmMethodRef> {
    let (owner, name, return_type) = match descriptor {
        JvmTypeDescriptor::Long => ("java/lang/Long", "longValue", JvmTypeDescriptor::Long),
        JvmTypeDescriptor::Float => ("java/lang/Float", "floatValue", JvmTypeDescriptor::Float),
        JvmTypeDescriptor::Double => ("java/lang/Double", "doubleValue", JvmTypeDescriptor::Double),
        JvmTypeDescriptor::Boolean => ("java/lang/Boolean", "booleanValue", JvmTypeDescriptor::Boolean),
        JvmTypeDescriptor::Char => ("java/lang/Character", "charValue", JvmTypeDescriptor::Char),
        JvmTypeDescriptor::Int | JvmTypeDescriptor::Byte | JvmTypeDescriptor::Short => {
            ("java/lang/Integer", "intValue", JvmTypeDescriptor::Int)
        }
        JvmTypeDescriptor::Object(_) | JvmTypeDescriptor::Array(_) | JvmTypeDescriptor::Void => {
            return Err(format!("JVM 最小 lowering：`{descriptor}` 不是可解箱的标量载体"));
        }
    };
    Ok(JvmMethodRef {
        owner: owner.to_string(),
        name: name.to_string(),
        descriptor: JvmMethodDescriptor::new(Vec::new(), return_type),
    })
}

/// 数组类型 `Array<T>` / `FixedArray { element, .. }` 的元素类型。
fn jvm_array_element_type(array_type: &NyarType) -> Result<&NyarType> {
    match array_type {
        NyarType::Array(element) => Ok(element),
        NyarType::FixedArray { element, .. } => Ok(element),
        other => Err(format!("JVM 最小 lowering：`{other}` 不是数组类型，缺少元素载体")),
    }
}

/// 数组元素位置的 JVM 载体。
///
/// acorn-jvm 没有 `laload`/`lastore`/`faload`/`daload`，因此 i64/f32/f64 元素
/// 必须用装箱引用数组（`java/lang/Long` / `Double` / `Float`），这是块 3 存取
/// 指令的前提；标量元素选 `NewArray` atype 可直接承载的基本类型。
fn jvm_array_element_descriptor(layouts: &AggregateLayoutPlan, element: &NyarType) -> Result<JvmTypeDescriptor> {
    Ok(match element {
        NyarType::Boolean => JvmTypeDescriptor::Boolean,
        NyarType::Integer8 { .. } => JvmTypeDescriptor::Byte,
        NyarType::Integer16 { .. } => JvmTypeDescriptor::Short,
        NyarType::Integer32 { .. } => JvmTypeDescriptor::Int,
        // V 的 `char` 是 u32 码位载体；JVM `char` 只有 16 位，会把 U+10000 以上截断。
        // 物理载体固定为 32 位 int，UTF-16 code unit 转换只在显式编码边界发生。
        NyarType::Character => JvmTypeDescriptor::Int,
        NyarType::Integer64 { .. } => JvmTypeDescriptor::Object("java/lang/Long".to_string()),
        NyarType::Float32 => JvmTypeDescriptor::Object("java/lang/Float".to_string()),
        NyarType::Float64 => JvmTypeDescriptor::Object("java/lang/Double".to_string()),
        NyarType::Utf8 | NyarType::Utf16 => JvmTypeDescriptor::Object("java/lang/String".to_string()),
        // 名义布局按冻结身份取 companion 类名；值存储聚合没有单槽 JVM 载体。
        NyarType::Nominal { layout, spelling } => {
            let Some(layout) = layouts.layouts.iter().find(|candidate| candidate.id == *layout)
            else {
                return Err(format!("JVM 最小 lowering：`{spelling}` 的布局身份 `{layout}` 未在计划中注册：缺少数组元素 JVM 载体合同"));
            };
            if layout.storage.is_reference() {
                JvmTypeDescriptor::Object(jvm_layout_class_name(layout))
            }
            else {
                return Err(format!("JVM 最小 lowering 不支持值存储聚合 `{spelling}` 作为数组元素：缺少单槽 JVM 载体合同"));
            }
        }
        // 泛型实例（`Option<char>` 等）：存储类由形状决定为引用，按擦除载体。
        NyarType::Apply(..) => JvmTypeDescriptor::Object(JVM_REFERENCE_CARRIER.to_string()),
        NyarType::Array(inner) | NyarType::FixedArray { element: inner, .. } => {
            JvmTypeDescriptor::array(jvm_array_element_descriptor(layouts, inner)?)
        }
        // 可空元素只能复用引用载体（`null` 表示空）；标量需要 sum 表示。
        NyarType::Nullable(inner) => {
            let descriptor = jvm_array_element_descriptor(layouts, inner)?;
            if matches!(descriptor, JvmTypeDescriptor::Object(_) | JvmTypeDescriptor::Array(_)) {
                descriptor
            }
            else {
                return Err(format!("JVM 最小 lowering：可空标量 `{element}` 需要 sum 表示，不能擦成引用"));
            }
        }
        other => {
            return Err(format!("JVM 最小 lowering 不支持数组元素类型 `{other}`：缺少该形状的 JVM 载体合同"));
        }
    })
}

pub(crate) fn jvm_type_descriptor(layouts: &AggregateLayoutPlan, ty: &NyarType) -> Result<JvmTypeDescriptor> {
    Ok(match ty {
        NyarType::Bottom | NyarType::Unit => JvmTypeDescriptor::Void,
        NyarType::Boolean => JvmTypeDescriptor::Boolean,
        // V 的 `char` 是 u32 码位，载体固定 32 位；不得用宿主 16 位 `Char`。
        NyarType::Character => JvmTypeDescriptor::Int,
        NyarType::Integer8 { .. } | NyarType::Integer16 { .. } | NyarType::Integer32 { .. } => JvmTypeDescriptor::Int,
        NyarType::Integer64 { .. } => JvmTypeDescriptor::Long,
        NyarType::Float32 => JvmTypeDescriptor::Float,
        NyarType::Float64 => JvmTypeDescriptor::Double,
        NyarType::Utf8 | NyarType::Utf16 => JvmTypeDescriptor::Object("java/lang/String".to_string()),
        // 栈/堆数组的载体是元素载体数组；`[T; N]` 的长度由 `ArrayFromElements` 表达。
        NyarType::Array(element) => JvmTypeDescriptor::array(jvm_array_element_descriptor(layouts, element)?),
        NyarType::FixedArray { element, .. } => JvmTypeDescriptor::array(jvm_array_element_descriptor(layouts, element)?),
        // 布局身份只经 `TupleNew`/`AggregateCopy` 的 `layout_id` 进入后端，类型描述符
        // 没有该事实，禁止按形状猜布局。
        NyarType::Tuple(..) => {
            return Err(format!(
                "JVM 最小 lowering：裸 tuple `{ty}` 没有布局身份：布局身份只经 `TupleNew`/`AggregateCopy` 的 `layout_id` 进入后端，类型描述符没有该事实"
            ));
        }
        // 可空引用直接复用引用载体（`null` 表示空）；标量需要 sum 表示，不得静默装箱。
        NyarType::Nullable(inner) => {
            let descriptor = jvm_type_descriptor(layouts, inner)?;
            if matches!(descriptor, JvmTypeDescriptor::Object(_) | JvmTypeDescriptor::Array(_)) {
                descriptor
            }
            else if descriptor == JvmTypeDescriptor::Void {
                return Err(format!("JVM 最小 lowering：可空 `{ty}` 的内层是 void，没有可复用的引用载体"));
            }
            else {
                return Err(format!("JVM 最小 lowering：可空标量 `{ty}` 需要 sum 表示，不得静默装箱"));
            }
        }
        // 泛型实例（`Option<char>` 等）：存储类由形状决定为引用，载体按
        // [`JVM_REFERENCE_CARRIER`] 擦除；实例级布局仍在计划中按身份冻结。
        NyarType::Apply(..) => JvmTypeDescriptor::Object(JVM_REFERENCE_CARRIER.to_string()),
        // 携带身份的名义类型：存储类按 `layout` 身份解析，引用/值存储都指向
        // companion 布局类。JVM 没有内联结构体，逐字段拷贝由 `AggregateCopy` 承担，
        // 它是唯一允许的复制路径；零字段值布局与 wasm 标量语义一致，维持结构化失败。
        NyarType::Nominal { layout, spelling } => {
            let Some(layout) = layouts.layouts.iter().find(|candidate| candidate.id == *layout)
            else {
                return Err(format!("JVM 最小 lowering：`{spelling}` 的布局身份 `{layout}` 未在计划中注册：缺少该形状的 JVM 载体合同"));
            };
            if layout.storage.is_reference() || !layout.fields.is_empty() {
                JvmTypeDescriptor::Object(jvm_layout_class_name(layout))
            }
            else {
                return Err(format!("JVM 最小 lowering 不支持零字段值存储聚合 `{spelling}`：缺少单槽 JVM 载体合同"));
            }
        }
        other => {
            return Err(format!("JVM 最小 lowering 不支持类型 `{other}`：缺少该形状的 JVM 载体合同"));
        }
    })
}

fn jvm_local_kind(layouts: &AggregateLayoutPlan, ty: &NyarType) -> Result<JvmSlotKind> {
    JvmSlotKind::of(&jvm_parameter_descriptor(layouts, ty)?)
}

struct JvmMethodEmitter<'a> {
    submission: &'a FragmentSubmission,
    internal_name: &'a str,
    function: &'a MirFunction,
    return_descriptor: JvmTypeDescriptor,
    slots: ExecutableSlotPlan,
    slot_offsets: Vec<u16>,
    method_plans: &'a BTreeMap<ItemInstanceId, JvmMethodPlan>,
    operator_by_instance: &'a BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)>,
    instructions: Vec<JvmInstruction>,
    current_block: MirBlockRef,
    comparison_ordinal: usize,
    /// 无符号 `u64` 比较展开使用的 scratch `long` 槽（2 槽宽），仅当函数实际引用
    /// `LongCompareLessThanUnsigned` 时在 `max_locals` 尾部预留；未预留即未引用。
    scratch_long_slot: Option<u16>,
}

fn lower_function(
    submission: &FragmentSubmission,
    internal_name: &str,
    function: &MirFunction,
    method: &JvmMethodPlan,
    method_plans: &BTreeMap<ItemInstanceId, JvmMethodPlan>,
    operator_by_instance: &BTreeMap<ItemInstanceId, (String, TargetOperatorBinding)>,
) -> Result<JvmMethodSignature> {
    let slots = ExecutableSlotPlan::plan_nyar(function);
    let mut slot_offsets = Vec::with_capacity(slots.local_types.len());
    let mut local_count = 0u16;
    for ty in &slots.local_types {
        slot_offsets.push(local_count);
        local_count = local_count.saturating_add(jvm_local_kind(submission.backend_plan.aggregate_layouts(), ty)?.slot_width());
    }

    // 无符号 `u64` 比较的冻结展开需要 2 槽 scratch long 周转（JVM 没有 `swap2`）；
    // 只在函数实际引用该 operator 时预留，避免无关方法的 `max_locals` 虚增。
    let needs_scratch_long = function.blocks.iter().flat_map(|block| &block.instructions).any(|instruction| {
        let InstructionKind::Call { callee: Operand::Item(instance), .. } = &instruction.kind
        else {
            return false;
        };
        operator_by_instance
            .get(instance)
            .is_some_and(|(_, binding)| matches!(binding, TargetOperatorBinding::Frozen(plan) if plan.encoding == TargetOperatorEncoding::Jvm(JvmOperatorCode::LongCompareLessThanUnsigned)))
    });
    let scratch_long_slot = if needs_scratch_long {
        let slot = local_count;
        local_count = local_count.saturating_add(2);
        Some(slot)
    }
    else {
        None
    };

    let mut emitter = JvmMethodEmitter {
        submission,
        internal_name,
        function,
        return_descriptor: method.descriptor.return_type.clone(),
        slots,
        slot_offsets,
        method_plans,
        operator_by_instance,
        instructions: Vec::new(),
        current_block: function.entry,
        comparison_ordinal: 0,
        scratch_long_slot,
    };

    for block_id in collect_reachable_blocks(function) {
        let Some(block) = function.blocks.get(block_id.0 as usize)
        else {
            continue;
        };
        emitter.emit_block(block)?;
    }

    let max_stack = u16::try_from(emitter.instructions.len().saturating_mul(2).saturating_add(2)).unwrap_or(u16::MAX);
    Ok(JvmMethodSignature {
        name: method.name.clone(),
        descriptor: method.descriptor.clone(),
        access_flags: JVM_ACC_PUBLIC | JVM_ACC_STATIC,
        code: Some(JvmCodeBody { max_stack, max_locals: local_count, instructions: emitter.instructions }),
    })
}

impl JvmMethodEmitter<'_> {
    fn emit_block(&mut self, block: &MirBlock) -> Result<()> {
        self.current_block = block.id;
        self.instructions.push(JvmInstruction::Label(block_label(block.id)));
        for instruction in &block.instructions {
            self.emit_instruction(instruction)?;
        }
        self.emit_terminator(block)
    }

    fn emit_instruction(&mut self, instruction: &MirInstruction) -> Result<()> {
        if instruction.results.len() > 1 {
            return Err(format!(
                "JVM 最小 lowering 不支持多结果指令（results={}）: function={}",
                instruction.results.len(),
                self.function.symbol
            ));
        }
        match &instruction.kind {
            InstructionKind::LoadConstant { constant, .. } => {
                let (slot, kind) = self.require_output(instruction)?;
                self.emit_constant(constant, Some(kind))?;
                self.instructions.push(kind.store(slot));
            }
            InstructionKind::Copy { source } => {
                let (slot, kind) = self.require_output(instruction)?;
                self.emit_operand(source, Some(kind))?;
                self.instructions.push(kind.store(slot));
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
                    Some((slot, kind)) => self.instructions.push(kind.store(slot)),
                    None => self.emit_pop(expected)?,
                }
            }
            InstructionKind::Call { callee, arguments } => {
                let output = self.output_of(instruction)?;
                self.emit_call(callee, arguments, &output)?;
            }
            InstructionKind::StructNew { nominal, fields } => {
                let layout = self.layout_by_nominal(*nominal)?;
                let output = self.require_output(instruction)?;
                self.emit_layout_new(&layout, output)?;
                for (field, value) in fields {
                    let Some((layout_id, slot)) = self.submission.backend_plan.aggregate_layout_by_field().get(field).copied()
                    else {
                        return Err(format!(
                            "JVM 最小 lowering：字段身份 `{field:?}` 未登记布局索引: function={}",
                            self.function.symbol
                        ));
                    };
                    if layout_id != layout.id {
                        return Err(format!(
                            "JVM 最小 lowering：字段 `{field:?}` 属于布局身份 `{layout_id}`，与构造的 nominal `{nominal:?}` 布局 `{}` 不符: function={}",
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
                        "JVM 最小 lowering：TupleGet 位置索引 {index} 越界（layout `{}` 长度 {}）: function={}",
                        layout.name,
                        layout.fields.len(),
                        self.function.symbol
                    ));
                }
                // 标签只在类型身份层；JVM 载体按冻结 layout 的字段槽读取。
                let (descriptor, kind) = self.layout_field_descriptor(&layout, *index)?;
                let field_ref = jvm_layout_field_ref(&layout, *index as usize, descriptor);
                self.emit_operand(object, Some(JvmSlotKind::Reference))?;
                self.instructions.push(JvmInstruction::GetField(field_ref));
                match self.output_of(instruction)? {
                    Some((output, output_kind)) => {
                        if output_kind != kind {
                            return Err(format!(
                                "JVM 最小 lowering：TupleGet 位置 {index} 载体 {kind:?} 与结果槽 {output_kind:?} 不符: function={}",
                                self.function.symbol
                            ));
                        }
                        self.instructions.push(kind.store(output));
                    }
                    None => match kind {
                        JvmSlotKind::Long | JvmSlotKind::Double => {
                            return Err(format!(
                                "JVM 最小 lowering：丢弃 2 槽 tuple 元素需要 `pop2` 编码，当前 JVM 指令模型不提供: function={}",
                                self.function.symbol
                            ));
                        }
                        _ => self.instructions.push(JvmInstruction::Pop),
                    },
                }
            }
            InstructionKind::TupleNew { layout_id, fields } => {
                let layout = self.layout_by_id(*layout_id)?;
                if fields.len() != layout.fields.len() {
                    return Err(format!(
                        "JVM 最小 lowering：tuple 布局 `{}` 字段数量与操作数不符: function={} expected={} actual={}",
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
                    self.emit_operand(source, Some(JvmSlotKind::Reference))?;
                    self.store_operand(dest, JvmSlotKind::Reference)?;
                }
                else {
                    let dest_slot = self.reference_operand_slot(dest)?;
                    let source_slot = self.reference_operand_slot(source)?;
                    let class_name = jvm_layout_class_name(&layout);
                    self.instructions.push(JvmInstruction::New(class_name.clone()));
                    self.instructions.push(JvmInstruction::Dup);
                    self.instructions.push(JvmInstruction::InvokeSpecial(jvm_constructor_ref(&class_name)));
                    self.instructions.push(JvmInstruction::AStore(dest_slot));
                    for slot in 0..layout.fields.len() {
                        let (descriptor, _) = self.layout_field_descriptor(&layout, u32::try_from(slot).unwrap_or(u32::MAX))?;
                        let field_ref = jvm_layout_field_ref(&layout, slot, descriptor);
                        // `putfield` 栈序是 `object, value`：先压目标对象，再从源对象读字段。
                        self.instructions.push(JvmInstruction::ALoad(dest_slot));
                        self.instructions.push(JvmInstruction::ALoad(source_slot));
                        self.instructions.push(JvmInstruction::GetField(field_ref.clone()));
                        self.instructions.push(JvmInstruction::PutField(field_ref));
                    }
                }
            }
            InstructionKind::FieldGet { object, field } => {
                let (layout, slot) = self.field_location(*field)?;
                let (descriptor, kind) = self.layout_field_descriptor(&layout, slot)?;
                let field_ref = jvm_layout_field_ref(&layout, slot as usize, descriptor);
                self.emit_operand(object, Some(JvmSlotKind::Reference))?;
                self.instructions.push(JvmInstruction::GetField(field_ref));
                match self.output_of(instruction)? {
                    Some((output, output_kind)) => {
                        if output_kind != kind {
                            return Err(format!(
                                "JVM 最小 lowering：字段 `{field:?}` 载体 {kind:?} 与结果槽 {output_kind:?} 不符: function={}",
                                self.function.symbol
                            ));
                        }
                        self.instructions.push(kind.store(output));
                    }
                    None => match kind {
                        JvmSlotKind::Long | JvmSlotKind::Double => {
                            return Err(format!(
                                "JVM 最小 lowering：丢弃 2 槽字段需要 `pop2` 编码，当前 JVM 指令模型不提供: function={}",
                                self.function.symbol
                            ));
                        }
                        _ => self.instructions.push(JvmInstruction::Pop),
                    },
                }
            }
            InstructionKind::FieldSet { object, field, value } => {
                let (layout, slot) = self.field_location(*field)?;
                let (descriptor, kind) = self.layout_field_descriptor(&layout, slot)?;
                let field_ref = jvm_layout_field_ref(&layout, slot as usize, descriptor);
                self.emit_operand(object, Some(JvmSlotKind::Reference))?;
                self.emit_operand(value, Some(kind))?;
                self.instructions.push(JvmInstruction::PutField(field_ref));
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
                crate::contracts::ArrayInitialization::Default => self.emit_array_new(instruction, array_type, length)?,
                crate::contracts::ArrayInitialization::Fill(_) => {
                    return Err("JVM 最小 lowering 不支持数组 Fill 初始化".to_string());
                }
            },
            InstructionKind::ArrayGet { array, index } => {
                self.emit_array_get(instruction, array, index)?;
            }
            InstructionKind::ArraySet { array, index, value } => {
                self.emit_array_set(array, index, value)?;
            }
            InstructionKind::ArrayLength { array } => {
                self.emit_operand(array, Some(JvmSlotKind::Reference))?;
                self.instructions.push(JvmInstruction::ArrayLength);
                match self.output_of(instruction)? {
                    Some((slot, kind)) => {
                        if kind != JvmSlotKind::Int {
                            return Err(format!(
                                "JVM 最小 lowering：`ArrayLength` 结果必须是 `int` 槽: function={} actual={kind:?}",
                                self.function.symbol
                            ));
                        }
                        self.instructions.push(JvmInstruction::IStore(slot));
                    }
                    None => self.instructions.push(JvmInstruction::Pop),
                }
            }
            InstructionKind::NullIsNull { value } => {
                self.emit_operand(value, Some(JvmSlotKind::Reference))?;
                let true_label = self.fresh_comparison_label("null_true");
                let end_label = self.fresh_comparison_label("null_end");
                self.instructions.push(JvmInstruction::IfNull(true_label.clone()));
                self.instructions.push(JvmInstruction::IConst(0));
                self.instructions.push(JvmInstruction::Goto(end_label.clone()));
                self.instructions.push(JvmInstruction::Label(true_label));
                self.instructions.push(JvmInstruction::IConst(1));
                self.instructions.push(JvmInstruction::Label(end_label));
                self.store_or_pop_boolean(instruction)?;
            }
            InstructionKind::NullUnwrap { value } => {
                let output = self.output_of(instruction)?;
                self.emit_operand(value, Some(JvmSlotKind::Reference))?;
                match output {
                    Some((slot, kind)) => {
                        if kind != JvmSlotKind::Reference {
                            return Err("JVM 最小 lowering：可空标量需要 sum 表示，不能按引用直接解包".to_string());
                        }
                        self.instructions.push(JvmInstruction::AStore(slot));
                    }
                    None => self.instructions.push(JvmInstruction::Pop),
                }
            }
            // 闭包捕获环境在 JVM 没有载体：必须点名 lambda 实例与捕获名结构化失败，
            // 不得丢弃环境、把捕获提升为模块全局或用直调替代。
            InstructionKind::LoadFunctionRef { instance, environment: Some(_), captured } => {
                return Err(format!(
                    "JVM 目标没有闭包载体：lambda instance `{instance:?}` 捕获 [{}]；function={}",
                    captured.join(", "),
                    self.function.symbol
                ));
            }
            other => {
                return Err(format!("JVM 最小 lowering 尚不支持指令 `{}`：function={}", instruction_kind_name(other), self.function.symbol));
            }
        }
        Ok(())
    }

    fn emit_terminator(&mut self, block: &MirBlock) -> Result<()> {
        match &block.terminator {
            Terminator::Return { value } => {
                let return_kind =
                    if self.return_descriptor == JvmTypeDescriptor::Void { None } else { Some(JvmSlotKind::of(&self.return_descriptor)?) };
                match (value, return_kind) {
                    (Some(value), Some(kind)) => {
                        self.emit_operand(value, Some(kind))?;
                        self.instructions.push(JvmMethodDescriptor::new(Vec::new(), self.return_descriptor.clone()).return_instruction());
                    }
                    (None, None) => self.instructions.push(JvmInstruction::Return),
                    (Some(_), None) => {
                        return Err(format!("JVM 最小 lowering：函数 `{}` 返回了值但签名是 void", self.function.symbol));
                    }
                    (None, Some(_)) => {
                        return Err(format!("JVM 最小 lowering：非 void 函数 `{}` 缺少返回值", self.function.symbol));
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
                        .ok_or_else(|| format!("JVM 最小 lowering：函数 `{}` 跳转目标块 {:?} 不存在", self.function.symbol, target))?;
                    let kinds = target_block
                        .parameters
                        .iter()
                        .map(|parameter| {
                            self.function
                                .value_types
                                .get(parameter)
                                .ok_or_else(|| format!("JVM 最小 lowering：块参数缺少冻结类型: function={}", self.function.symbol))
                                .and_then(|ty| jvm_local_kind(self.submission.backend_plan.aggregate_layouts(), ty))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    (target_block.parameters.len(), kinds)
                };
                if arguments.len() != parameter_count {
                    return Err(format!(
                        "JVM 最小 lowering：跳转目标块 {:?} 形参数量与实参不符: function={} expected={} actual={}",
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
                        .ok_or_else(|| format!("JVM 最小 lowering：跳转目标块 {:?} 形参 {index} 缺少局部槽", target))?;
                    self.instructions.push(kinds[index].store(self.slot_offsets[local as usize]));
                }
                self.instructions.push(JvmInstruction::Goto(target_label));
            }
            Terminator::Branch { condition, then_target, else_target } => {
                let targets_have_parameters = self
                    .function
                    .blocks
                    .iter()
                    .filter(|block| block.id == *then_target || block.id == *else_target)
                    .any(|block| !block.parameters.is_empty());
                if targets_have_parameters {
                    return Err(format!("JVM 最小 lowering 只支持无块参数的条件分支：function={}", self.function.symbol));
                }
                self.emit_operand(condition, Some(JvmSlotKind::Int))?;
                self.instructions.push(JvmInstruction::IfEq(block_label(*else_target)));
                self.instructions.push(JvmInstruction::Goto(block_label(*then_target)));
            }
            Terminator::Unreachable => {
                if self.return_descriptor == JvmTypeDescriptor::Void {
                    self.instructions.push(JvmInstruction::Return);
                }
                else {
                    return Err(format!("JVM 最小 lowering：非 void 函数 `{}` 的 `Unreachable` 终结缺少返回指令", self.function.symbol));
                }
            }
            other => {
                return Err(format!("JVM 最小 lowering 尚不支持终结符 `{other:?}`: function={}", self.function.symbol));
            }
        }
        Ok(())
    }

    fn emit_call(&mut self, callee: &Operand, arguments: &[Operand], output: &Option<(u16, JvmSlotKind)>) -> Result<()> {
        let Operand::Item(instance) = callee
        else {
            return Err(format!("JVM 最小 lowering 只支持 `Item` 身份的静态调用: function={} callee={callee:?}", self.function.symbol));
        };
        if let Some((mnemonic, binding)) = self.operator_by_instance.get(instance) {
            return self.emit_operator_call(mnemonic, binding, arguments, *output);
        }
        if let Some(import) = self.submission.backend_plan.imports().get(instance) {
            return Err(format!(
                "JVM 最小 lowering 尚不支持宿主导入 `{instance:?}`（链接 `{}`）：缺少 JVM host 编码，function={}",
                import.link, self.function.symbol
            ));
        }
        let Some(method) = self.method_plans.get(instance).cloned()
        else {
            return Err(format!("JVM 最小 lowering：同片段调用目标 `{instance:?}` 不在可达方法集中: function={}", self.function.symbol));
        };
        let parameter_kinds = method.descriptor.parameter_types.iter().map(JvmSlotKind::of).collect::<Result<Vec<_>>>()?;
        if arguments.len() != parameter_kinds.len() {
            return Err(format!(
                "JVM 最小 lowering：调用 `{}` 实参数量与冻结签名不符: function={} expected={} actual={}",
                method.name,
                self.function.symbol,
                parameter_kinds.len(),
                arguments.len()
            ));
        }
        for (argument, kind) in arguments.iter().zip(&parameter_kinds) {
            self.emit_operand(argument, Some(*kind))?;
        }
        self.instructions.push(JvmInstruction::InvokeStatic(JvmMethodRef {
            owner: self.internal_name.to_string(),
            name: method.name.clone(),
            descriptor: method.descriptor.clone(),
        }));
        self.store_call_result(*instance, &method, *output)
    }

    /// `[jvm_operator("...")]` 绑定：只机械展开计划冻结的配方。
    fn emit_operator_call(
        &mut self,
        mnemonic: &str,
        binding: &TargetOperatorBinding,
        arguments: &[Operand],
        output: Option<(u16, JvmSlotKind)>,
    ) -> Result<()> {
        let TargetOperatorBinding::Frozen(plan) = binding
        else {
            return Err(format!("JVM 目标 `jvm_operator` 绑定缺少可编码助记符 `{mnemonic}`: function={}", self.function.symbol));
        };
        let TargetOperatorEncoding::Jvm(code) = plan.encoding
        else {
            return Err(format!("JVM 目标 `jvm_operator` 绑定 `{mnemonic}` 冻结了非 JVM 编码: function={}", self.function.symbol));
        };
        if arguments.len() != plan.arity {
            return Err(format!(
                "JVM 目标 operator `{mnemonic}` 实参数量与冻结 arity 不符: function={} expected={} actual={}",
                self.function.symbol,
                plan.arity,
                arguments.len()
            ));
        }
        let Some((slot, result_kind)) = output
        else {
            return Err(format!(
                "JVM 目标 operator `{mnemonic}` 调用必须绑定结果槽：缺少 `pop2` 编码，拒绝静默丢弃结果: function={}",
                self.function.symbol
            ));
        };
        let expected_kind = JvmSlotKind::of_operand_type(plan.operand_type);
        if result_kind != JvmSlotKind::of_operand_type(plan.result_type) {
            return Err(format!(
                "JVM 目标 operator `{mnemonic}` 结果槽载体与冻结 result_type 不符: function={} expected={:?} actual={result_kind:?}",
                self.function.symbol, plan.result_type
            ));
        }
        self.emit_operator_operands(mnemonic, expected_kind, plan, arguments)?;

        // 无符号 `u64` 比较的展开要消费方法级 scratch `long` 槽；槽在 `lower_function`
        // 按同一绑定表预留，未预留说明计划与发射扫描不一致，必须失败闭合。
        let scratch_long_slot = if code == JvmOperatorCode::LongCompareLessThanUnsigned {
            self.scratch_long_slot.ok_or_else(|| {
                format!("JVM 目标 operator `{mnemonic}` 的展开需要 scratch long 槽，但方法级预留缺失: function={}", self.function.symbol)
            })?
        }
        else {
            0
        };
        let expansion = code.instructions(scratch_long_slot);
        if expansion.is_empty() {
            return Err(format!("JVM 目标 operator `{mnemonic}` 的冻结编码 `{code:?}` 没有指令展开: function={}", self.function.symbol));
        }
        let label_prefix = self.operator_label_prefix();
        self.comparison_ordinal += 1;
        for instruction in expansion {
            self.instructions.push(relabel_operator_branch(instruction, &label_prefix));
        }
        self.instructions.push(result_kind.store(slot));
        Ok(())
    }

    fn emit_operator_operands(
        &mut self,
        mnemonic: &str,
        expected_kind: JvmSlotKind,
        plan: &TargetOperatorPlan,
        arguments: &[Operand],
    ) -> Result<()> {
        match plan.recipe {
            TargetOperatorRecipe::Operands => self.emit_operator_arguments(mnemonic, expected_kind, arguments),
            TargetOperatorRecipe::Steps(steps) => {
                for step in steps {
                    match step {
                        TargetOperandStep::Arguments => self.emit_operator_arguments(mnemonic, expected_kind, arguments)?,
                        TargetOperandStep::NarrowTopI64ToI32 => self.instructions.push(JvmInstruction::L2I),
                        TargetOperandStep::IntegerConstant { operand_type, value } => {
                            let kind = JvmSlotKind::of_operand_type(*operand_type);
                            if kind != expected_kind {
                                return Err(format!(
                                    "JVM 目标 operator `{mnemonic}` 的常量配方载体与冻结操作数载体不符: function={} expected={expected_kind:?} actual={kind:?}",
                                    self.function.symbol
                                ));
                            }
                            match kind {
                                JvmSlotKind::Int => self.instructions.push(JvmInstruction::IConst(*value)),
                                JvmSlotKind::Long => self.instructions.push(JvmInstruction::LdcLong(i64::from(*value))),
                                JvmSlotKind::Float | JvmSlotKind::Double | JvmSlotKind::Reference => {
                                    return Err(format!(
                                        "JVM 目标 operator `{mnemonic}` 冻结了非整数常量配方 {operand_type:?}: function={}",
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

    fn emit_operator_arguments(&mut self, mnemonic: &str, expected_kind: JvmSlotKind, arguments: &[Operand]) -> Result<()> {
        for argument in arguments {
            self.emit_operand(argument, Some(expected_kind)).map_err(|error| {
                format!("JVM 目标 operator `{mnemonic}` 实参载体与冻结 operand_type 不符: function={} detail={error}", self.function.symbol)
            })?;
        }
        Ok(())
    }

    fn store_call_result(&mut self, instance: ItemInstanceId, method: &JvmMethodPlan, output: Option<(u16, JvmSlotKind)>) -> Result<()> {
        if method.descriptor.return_type == JvmTypeDescriptor::Void {
            if output.is_some() {
                return Err(format!(
                    "JVM 最小 lowering：void 调用 `{}` 不得绑定结果槽: function={} callee={instance:?}",
                    method.name, self.function.symbol
                ));
            }
            return Ok(());
        }
        let return_kind = JvmSlotKind::of(&method.descriptor.return_type)?;
        match output {
            Some((slot, kind)) => {
                if kind != return_kind {
                    return Err(format!(
                        "JVM 最小 lowering：调用 `{}` 结果载体与冻结返回签名不符: function={} expected={return_kind:?} actual={kind:?}",
                        method.name, self.function.symbol
                    ));
                }
                self.instructions.push(kind.store(slot));
                Ok(())
            }
            None => match return_kind {
                JvmSlotKind::Int | JvmSlotKind::Reference | JvmSlotKind::Float => {
                    self.instructions.push(JvmInstruction::Pop);
                    Ok(())
                }
                JvmSlotKind::Long | JvmSlotKind::Double => Err(format!(
                    "JVM 最小 lowering：调用 `{}` 返回 2 槽值但缺少 `pop2` 编码，拒绝静默丢弃: function={}",
                    method.name, self.function.symbol
                )),
            },
        }
    }

    fn emit_operand(&mut self, operand: &Operand, expected: Option<JvmSlotKind>) -> Result<()> {
        match operand {
            Operand::Value(value) => {
                let (slot, kind) = self.value_slot(*value)?;
                if let Some(expected) = expected
                    && expected != kind
                {
                    return Err(format!(
                        "JVM 最小 lowering：SSA 值 {:?} 的槽载体 {kind:?} 与期望 {expected:?} 不符: function={}",
                        value, self.function.symbol
                    ));
                }
                self.instructions.push(kind.load(slot));
                Ok(())
            }
            Operand::Constant(constant) => self.emit_constant(constant, expected),
            Operand::Symbol(path) => Err(format!("JVM 最小 lowering 不支持符号操作数 `{path:?}`: function={}", self.function.symbol)),
            Operand::Item(instance) => {
                Err(format!("JVM 最小 lowering 不支持把可调用身份 `{instance:?}` 当值使用: function={}", self.function.symbol))
            }
        }
    }

    fn emit_constant(&mut self, constant: &Constant, expected: Option<JvmSlotKind>) -> Result<()> {
        match constant {
            Constant::Int(value) => match expected {
                Some(JvmSlotKind::Long) => {
                    self.instructions.push(JvmInstruction::LdcLong(*value));
                    Ok(())
                }
                Some(JvmSlotKind::Int) | None => {
                    let value = i32::try_from(*value).map_err(|_| {
                        format!(
                            "JVM 最小 lowering：整数常量 {value} 超出 `int` 载体，且目标槽位未声明 `long`: function={}",
                            self.function.symbol
                        )
                    })?;
                    self.instructions.push(JvmInstruction::IConst(value));
                    Ok(())
                }
                Some(other) => Err(format!("JVM 最小 lowering：整数常量不能进入 {other:?} 槽: function={}", self.function.symbol)),
            },
            Constant::Float64(value) => match expected {
                Some(JvmSlotKind::Float) => {
                    Err(format!("JVM 最小 lowering 不支持把 f64 常量压入 `float` 槽：function={}", self.function.symbol))
                }
                Some(JvmSlotKind::Int | JvmSlotKind::Long | JvmSlotKind::Reference) => {
                    Err(format!("JVM 最小 lowering：f64 常量不能进入 {expected:?} 槽: function={}", self.function.symbol))
                }
                Some(JvmSlotKind::Double) => {
                    self.instructions.push(JvmInstruction::LdcDouble(value.into_inner().to_bits()));
                    Ok(())
                }
                None => Err(format!("JVM 最小 lowering：f64 常量缺少明确的 `double` 结果槽，拒绝猜测载体: function={}", self.function.symbol)),
            },
            Constant::Bool(value) => {
                if matches!(expected, Some(JvmSlotKind::Long | JvmSlotKind::Float | JvmSlotKind::Double | JvmSlotKind::Reference)) {
                    return Err(format!("JVM 最小 lowering：布尔常量不能进入 {expected:?} 槽: function={}", self.function.symbol));
                }
                self.instructions.push(JvmInstruction::IConst(i32::from(*value)));
                Ok(())
            }
            other => Err(format!("JVM 最小 lowering 尚不支持常量 `{other:?}`：缺少 JVM 常量池载体合同，function={}", self.function.symbol)),
        }
    }

    fn emit_pop(&mut self, kind: Option<JvmSlotKind>) -> Result<()> {
        match kind {
            None | Some(JvmSlotKind::Int | JvmSlotKind::Reference | JvmSlotKind::Float) => {
                self.instructions.push(JvmInstruction::Pop);
                Ok(())
            }
            Some(JvmSlotKind::Long | JvmSlotKind::Double) => {
                Err(format!("JVM 最小 lowering：丢弃 2 槽值需要 `pop2` 编码，当前 JVM 指令模型不提供: function={}", self.function.symbol))
            }
        }
    }

    fn output_of(&self, instruction: &MirInstruction) -> Result<Option<(u16, JvmSlotKind)>> {
        match self.slots.instruction_output(instruction) {
            None => Ok(None),
            Some(value) => self.value_slot(value).map(Some),
        }
    }

    fn require_output(&self, instruction: &MirInstruction) -> Result<(u16, JvmSlotKind)> {
        self.output_of(instruction)?
            .ok_or_else(|| format!("JVM 最小 lowering：指令缺少 SSA 结果槽，无法确定局部载体: function={}", self.function.symbol))
    }

    fn value_slot(&self, value: MirValueRef) -> Result<(u16, JvmSlotKind)> {
        let local = *self
            .slots
            .value_locals
            .get(&value)
            .ok_or_else(|| format!("JVM 最小 lowering：SSA 值 {:?} 缺少规划的局部槽: function={}", value, self.function.symbol))?;
        let ty = self
            .slots
            .local_types
            .get(local as usize)
            .ok_or_else(|| format!("JVM 最小 lowering：局部槽 {local} 超出规划表: function={}", self.function.symbol))?;
        let kind = jvm_local_kind(self.submission.backend_plan.aggregate_layouts(), ty)?;
        Ok((self.slot_offsets[local as usize], kind))
    }

    /// 比较展开标签前缀：当前块标签 + 方法内比较序号，保证方法内唯一。
    fn operator_label_prefix(&self) -> String {
        format!("{}_{}", block_label(self.current_block), self.comparison_ordinal)
    }

    /// 为布尔化展开分配方法内唯一标签。
    fn fresh_comparison_label(&mut self, suffix: &str) -> String {
        let label = format!("{}_{}_{}", block_label(self.current_block), self.comparison_ordinal, suffix);
        self.comparison_ordinal += 1;
        label
    }

    fn layout_by_id(&self, layout_id: LayoutId) -> Result<AggregateLayout> {
        self.submission
            .backend_plan
            .aggregate_layouts()
            .layouts
            .iter()
            .find(|layout| layout.id == layout_id)
            .cloned()
            .ok_or_else(|| format!("JVM 最小 lowering：布局身份 `{layout_id}` 未在计划中注册: function={}", self.function.symbol))
    }

    fn layout_by_nominal(&self, nominal: NominalInstanceId) -> Result<AggregateLayout> {
        let Some(layout_id) = self.submission.backend_plan.aggregate_layout_by_nominal().get(&nominal).copied()
        else {
            return Err(format!(
                "JVM 最小 lowering：nominal `{nominal:?}` 没有冻结的布局身份: function={}",
                self.function.symbol
            ));
        };
        self.layout_by_id(layout_id)
    }

    fn layout_field_descriptor(&self, layout: &AggregateLayout, slot: u32) -> Result<(JvmTypeDescriptor, JvmSlotKind)> {
        let Some(field) = layout.fields.get(slot as usize)
        else {
            return Err(format!(
                "JVM 最小 lowering：布局 `{}` 缺少字段槽 {slot}: function={}",
                layout.name, self.function.symbol
            ));
        };
        let layouts = self.submission.backend_plan.aggregate_layouts();
        let descriptor = jvm_type_descriptor(layouts, &field.ty)?;
        Ok((descriptor.clone(), JvmSlotKind::of(&descriptor)?))
    }

    fn field_location(&self, field: FieldId) -> Result<(AggregateLayout, u32)> {
        let Some((layout_id, slot)) = self.submission.backend_plan.aggregate_layout_by_field().get(&field).copied()
        else {
            return Err(format!(
                "JVM 最小 lowering：字段身份 `{field:?}` 未登记布局索引: function={}",
                self.function.symbol
            ));
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
                .ok_or_else(|| format!("JVM 最小 lowering：SSA 值 {:?} 缺少冻结类型: function={}", value, self.function.symbol)),
            Operand::Constant(constant) => Ok(match constant {
                Constant::Int(_) => NyarType::Integer32 { signed: true },
                Constant::Float64(_) => NyarType::Float64,
                Constant::Bool(_) => NyarType::Boolean,
                Constant::Utf8(_) | Constant::Utf16(_) => NyarType::Utf8,
                other => {
                    return Err(format!(
                        "JVM 最小 lowering：常量 `{other:?}` 没有可装箱的冻结类型: function={}",
                        self.function.symbol
                    ));
                }
            }),
            other => Err(format!("JVM 最小 lowering：操作数 `{other:?}` 没有冻结类型: function={}", self.function.symbol)),
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
            .ok_or_else(|| format!("JVM 最小 lowering：SSA 值 {:?} 缺少冻结类型: function={}", value, self.function.symbol))
    }

    fn reference_operand_slot(&self, operand: &Operand) -> Result<u16> {
        let Operand::Value(value) = operand
        else {
            return Err(format!(
                "JVM 最小 lowering：只有 SSA 值可作为引用局部目标: function={} dest={operand:?}",
                self.function.symbol
            ));
        };
        let (slot, kind) = self.value_slot(*value)?;
        if kind != JvmSlotKind::Reference {
            return Err(format!(
                "JVM 最小 lowering：聚合拷贝目标必须是引用载体: function={} actual={kind:?}",
                self.function.symbol
            ));
        }
        Ok(slot)
    }

    fn store_operand(&mut self, operand: &Operand, kind: JvmSlotKind) -> Result<()> {
        let Operand::Value(value) = operand
        else {
            return Err(format!(
                "JVM 最小 lowering：只有 SSA 值可作为局部目标: function={} dest={operand:?}",
                self.function.symbol
            ));
        };
        let (slot, actual) = self.value_slot(*value)?;
        if actual != kind {
            return Err(format!(
                "JVM 最小 lowering：目标 SSA 值 {:?} 的槽载体 {actual:?} 与期望 {kind:?} 不符: function={}",
                value, self.function.symbol
            ));
        }
        self.instructions.push(kind.store(slot));
        Ok(())
    }

    /// 发射一个可能已处于标量槽的 payload/元素，并按目标类型按需装箱。
    fn emit_boxed_operand(&mut self, operand: &Operand, ty: &NyarType) -> Result<()> {
        let descriptor = {
            let layouts = self.submission.backend_plan.aggregate_layouts();
            jvm_type_descriptor(layouts, ty)?
        };
        match &descriptor {
            JvmTypeDescriptor::Object(_) | JvmTypeDescriptor::Array(_) => self.emit_operand(operand, Some(JvmSlotKind::Reference)),
            JvmTypeDescriptor::Void => Err(format!(
                "JVM 最小 lowering：`{ty}` 是 void 形态，不能作为需要引用的值: function={}",
                self.function.symbol
            )),
            scalar => {
                let kind = JvmSlotKind::of(scalar)?;
                self.emit_operand(operand, Some(kind))?;
                let boxing = jvm_box_value_of(scalar)?;
                self.instructions.push(JvmInstruction::InvokeStatic(boxing));
                Ok(())
            }
        }
    }

    fn emit_array_element_value(&mut self, operand: &Operand, element: &NyarType) -> Result<()> {
        let (target, element_descriptor) = {
            let layouts = self.submission.backend_plan.aggregate_layouts();
            (jvm_type_descriptor(layouts, element)?, jvm_array_element_descriptor(layouts, element)?)
        };
        let target_kind = JvmSlotKind::of(&target)?;
        if matches!(element_descriptor, JvmTypeDescriptor::Object(_) | JvmTypeDescriptor::Array(_))
            && target_kind != JvmSlotKind::Reference
        {
            return self.emit_boxed_operand(operand, element);
        }
        self.emit_operand(operand, Some(target_kind))
    }

    fn emit_layout_new(&mut self, layout: &AggregateLayout, output: (u16, JvmSlotKind)) -> Result<()> {
        if output.1 != JvmSlotKind::Reference {
            return Err(format!(
                "JVM 最小 lowering：布局 `{}` 的构造结果必须是引用槽: function={} actual={:?}",
                layout.name, self.function.symbol, output.1
            ));
        }
        let class_name = jvm_layout_class_name(layout);
        self.instructions.push(JvmInstruction::New(class_name.clone()));
        self.instructions.push(JvmInstruction::Dup);
        self.instructions.push(JvmInstruction::InvokeSpecial(jvm_constructor_ref(&class_name)));
        self.instructions.push(JvmSlotKind::Reference.store(output.0));
        Ok(())
    }

    fn emit_layout_field_write(&mut self, layout: &AggregateLayout, output_slot: u16, slot: usize, value: &Operand) -> Result<()> {
        let (descriptor, kind) = self.layout_field_descriptor(layout, u32::try_from(slot).unwrap_or(u32::MAX))?;
        let field_ref = jvm_layout_field_ref(layout, slot, descriptor);
        self.instructions.push(JvmInstruction::ALoad(output_slot));
        self.emit_operand(value, Some(kind))?;
        self.instructions.push(JvmInstruction::PutField(field_ref));
        Ok(())
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
                "JVM 最小 lowering：sum nominal `{nominal:?}` 没有冻结的 `SumRepresentation`: function={}",
                self.function.symbol
            ));
        };
        let Some(variant_rep) = sum.variants.get(&variant).copied()
        else {
            return Err(format!(
                "JVM 最小 lowering：variant `{variant:?}` 不属于 sum nominal `{nominal:?}`: function={}",
                self.function.symbol
            ));
        };
        if payload.is_some() != variant_rep.payload_type.is_some() {
            return Err(format!(
                "JVM 最小 lowering：variant `{variant:?}` 的 payload 操作数与冻结 payload 类型不一致: function={} nominal={nominal:?} payload={} expected={}",
                self.function.symbol,
                payload.is_some(),
                variant_rep.payload_type.is_some()
            ));
        }
        let tag = i32::try_from(variant_rep.tag)
            .map_err(|_| format!("JVM 最小 lowering：variant `{variant:?}` 的 tag 超出 `int` 载体: function={}", self.function.symbol))?;
        let output = self.require_output(instruction)?;
        if output.1 != JvmSlotKind::Reference {
            return Err(format!(
                "JVM 最小 lowering：sum 构造结果必须是引用槽: function={} actual={:?}",
                self.function.symbol, output.1
            ));
        }
        let class_name = jvm_sum_class_name(nominal, &sum);
        self.instructions.push(JvmInstruction::New(class_name.clone()));
        self.instructions.push(JvmInstruction::Dup);
        self.instructions.push(JvmInstruction::InvokeSpecial(jvm_constructor_ref(&class_name)));
        self.instructions.push(JvmSlotKind::Reference.store(output.0));

        self.instructions.push(JvmInstruction::ALoad(output.0));
        self.instructions.push(JvmInstruction::IConst(tag));
        self.instructions.push(JvmInstruction::PutField(JvmFieldRef {
            owner: class_name.clone(),
            name: JVM_SUM_TAG_FIELD.to_string(),
            descriptor: JvmTypeDescriptor::Int,
        }));

        if let Some(payload) = payload {
            let payload_type = self.operand_type(payload)?;
            self.instructions.push(JvmInstruction::ALoad(output.0));
            self.emit_boxed_operand(payload, &payload_type)?;
            self.instructions.push(JvmInstruction::PutField(JvmFieldRef {
                owner: class_name,
                name: JVM_SUM_PAYLOAD_FIELD.to_string(),
                descriptor: JvmTypeDescriptor::Object(JVM_REFERENCE_CARRIER.to_string()),
            }));
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
                "JVM 最小 lowering：sum nominal `{nominal:?}` 没有冻结的 `SumRepresentation`: function={}",
                self.function.symbol
            ));
        };
        let Some(variant_rep) = sum.variants.get(&variant).copied()
        else {
            return Err(format!(
                "JVM 最小 lowering：variant `{variant:?}` 不属于 sum nominal `{nominal:?}`: function={}",
                self.function.symbol
            ));
        };
        let tag = i32::try_from(variant_rep.tag)
            .map_err(|_| format!("JVM 最小 lowering：variant `{variant:?}` 的 tag 超出 `int` 载体: function={}", self.function.symbol))?;
        let class_name = jvm_sum_class_name(nominal, &sum);
        self.emit_operand(object, Some(JvmSlotKind::Reference))?;
        self.instructions.push(JvmInstruction::GetField(JvmFieldRef {
            owner: class_name,
            name: JVM_SUM_TAG_FIELD.to_string(),
            descriptor: JvmTypeDescriptor::Int,
        }));
        self.instructions.push(JvmInstruction::IConst(tag));
        let true_label = self.fresh_comparison_label("sum_true");
        let end_label = self.fresh_comparison_label("sum_end");
        self.instructions.push(JvmInstruction::IfICmpEq(true_label.clone()));
        self.instructions.push(JvmInstruction::IConst(0));
        self.instructions.push(JvmInstruction::Goto(end_label.clone()));
        self.instructions.push(JvmInstruction::Label(true_label));
        self.instructions.push(JvmInstruction::IConst(1));
        self.instructions.push(JvmInstruction::Label(end_label));
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
                "JVM 最小 lowering：sum nominal `{nominal:?}` 没有冻结的 `SumRepresentation`: function={}",
                self.function.symbol
            ));
        };
        let Some(variant_rep) = sum.variants.get(&variant).copied()
        else {
            return Err(format!(
                "JVM 最小 lowering：variant `{variant:?}` 不属于 sum nominal `{nominal:?}`: function={}",
                self.function.symbol
            ));
        };
        if variant_rep.payload_type.is_none() {
            return Err(format!(
                "JVM 最小 lowering：variant `{variant:?}` 没有 payload，不能取载荷: function={} nominal={nominal:?}",
                self.function.symbol
            ));
        }
        let class_name = jvm_sum_class_name(nominal, &sum);
        self.emit_operand(object, Some(JvmSlotKind::Reference))?;
        self.instructions.push(JvmInstruction::GetField(JvmFieldRef {
            owner: class_name,
            name: JVM_SUM_PAYLOAD_FIELD.to_string(),
            descriptor: JvmTypeDescriptor::Object(JVM_REFERENCE_CARRIER.to_string()),
        }));

        let output = self.output_of(instruction)?;
        let Some((slot, kind)) = output
        else {
            self.instructions.push(JvmInstruction::Pop);
            return Ok(());
        };
        let Some(target_type) = self.output_type(instruction)?
        else {
            return Err(format!("JVM 最小 lowering：sum payload 结果缺少冻结类型: function={}", self.function.symbol));
        };
        let target = {
            let layouts = self.submission.backend_plan.aggregate_layouts();
            jvm_type_descriptor(layouts, &target_type)?
        };
        let target_kind = JvmSlotKind::of(&target)?;
        if target_kind != kind {
            return Err(format!(
                "JVM 最小 lowering：sum payload 目标载体 {target_kind:?} 与结果槽 {kind:?} 不符: function={}",
                self.function.symbol
            ));
        }
        self.instructions.push(JvmInstruction::CheckCast(jvm_box_class_name(&target)?));
        if target_kind != JvmSlotKind::Reference {
            self.instructions.push(JvmInstruction::InvokeVirtual(jvm_unbox_method(&target)?));
        }
        self.instructions.push(kind.store(slot));
        Ok(())
    }

    fn store_or_pop_boolean(&mut self, instruction: &MirInstruction) -> Result<()> {
        match self.output_of(instruction)? {
            Some((slot, kind)) => {
                if kind != JvmSlotKind::Int {
                    return Err(format!(
                        "JVM 最小 lowering：布尔结果槽载体必须是 `int`: function={} actual={kind:?}",
                        self.function.symbol
                    ));
                }
                self.instructions.push(JvmInstruction::IStore(slot));
                Ok(())
            }
            None => {
                self.instructions.push(JvmInstruction::Pop);
                Ok(())
            }
        }
    }

    fn emit_array_from_elements(&mut self, instruction: &MirInstruction, array_type: &NyarType, elements: &[Operand]) -> Result<()> {
        let element = jvm_array_element_type(array_type)?.clone();
        let (creation, store) = {
            let layouts = self.submission.backend_plan.aggregate_layouts();
            let element_descriptor = jvm_array_element_descriptor(layouts, &element)?;
            let creation = jvm_array_creation_instruction(&element_descriptor)?;
            let (_, store) = jvm_array_access_instructions(&element_descriptor)?;
            (creation, store)
        };
        let length = i32::try_from(elements.len())
            .map_err(|_| format!("JVM 最小 lowering：数组元素数量超出 `int` 载体: function={}", self.function.symbol))?;
        self.instructions.push(JvmInstruction::IConst(length));
        self.instructions.push(creation);
        for (index, value) in elements.iter().enumerate() {
            let index = i32::try_from(index)
                .map_err(|_| format!("JVM 最小 lowering：数组元素下标超出 `int` 载体: function={}", self.function.symbol))?;
            self.instructions.push(JvmInstruction::Dup);
            self.instructions.push(JvmInstruction::IConst(index));
            self.emit_array_element_value(value, &element)?;
            self.instructions.push(store.clone());
        }
        match self.output_of(instruction)? {
            Some((slot, kind)) => {
                if kind != JvmSlotKind::Reference {
                    return Err(format!(
                        "JVM 最小 lowering：数组结果必须是引用槽: function={} actual={kind:?}",
                        self.function.symbol
                    ));
                }
                self.instructions.push(JvmInstruction::AStore(slot));
                Ok(())
            }
            None => {
                self.instructions.push(JvmInstruction::Pop);
                Ok(())
            }
        }
    }

    fn emit_array_new(&mut self, instruction: &MirInstruction, array_type: &NyarType, length: &Operand) -> Result<()> {
        let element = jvm_array_element_type(array_type)?.clone();
        let creation = {
            let layouts = self.submission.backend_plan.aggregate_layouts();
            jvm_array_creation_instruction(&jvm_array_element_descriptor(layouts, &element)?)?
        };
        self.emit_operand(length, Some(JvmSlotKind::Int))?;
        self.instructions.push(creation);
        match self.output_of(instruction)? {
            Some((slot, kind)) => {
                if kind != JvmSlotKind::Reference {
                    return Err(format!(
                        "JVM 最小 lowering：数组结果必须是引用槽: function={} actual={kind:?}",
                        self.function.symbol
                    ));
                }
                self.instructions.push(JvmInstruction::AStore(slot));
                Ok(())
            }
            None => {
                self.instructions.push(JvmInstruction::Pop);
                Ok(())
            }
        }
    }

    fn emit_array_get(&mut self, instruction: &MirInstruction, array: &Operand, index: &Operand) -> Result<()> {
        let array_type = self.operand_type(array)?;
        let element = jvm_array_element_type(&array_type)?.clone();
        let (element_descriptor, target, load) = {
            let layouts = self.submission.backend_plan.aggregate_layouts();
            let element_descriptor = jvm_array_element_descriptor(layouts, &element)?;
            let target = jvm_type_descriptor(layouts, &element)?;
            let (load, _) = jvm_array_access_instructions(&element_descriptor)?;
            (element_descriptor, target, load)
        };
        let target_kind = JvmSlotKind::of(&target)?;
        self.emit_operand(array, Some(JvmSlotKind::Reference))?;
        self.emit_operand(index, Some(JvmSlotKind::Int))?;
        self.instructions.push(load);
        if matches!(element_descriptor, JvmTypeDescriptor::Object(_) | JvmTypeDescriptor::Array(_))
            && target_kind != JvmSlotKind::Reference
        {
            self.instructions.push(JvmInstruction::CheckCast(jvm_box_class_name(&target)?));
            self.instructions.push(JvmInstruction::InvokeVirtual(jvm_unbox_method(&target)?));
        }
        match self.output_of(instruction)? {
            Some((slot, kind)) => {
                if kind != target_kind {
                    return Err(format!(
                        "JVM 最小 lowering：数组元素目标载体 {target_kind:?} 与结果槽 {kind:?} 不符: function={}",
                        self.function.symbol
                    ));
                }
                self.instructions.push(kind.store(slot));
                Ok(())
            }
            None => match target_kind {
                JvmSlotKind::Long | JvmSlotKind::Double => Err(format!(
                    "JVM 最小 lowering：丢弃 2 槽数组元素需要 `pop2` 编码，当前 JVM 指令模型不提供: function={}",
                    self.function.symbol
                )),
                _ => {
                    self.instructions.push(JvmInstruction::Pop);
                    Ok(())
                }
            },
        }
    }

    fn emit_array_set(&mut self, array: &Operand, index: &Operand, value: &Operand) -> Result<()> {
        let array_type = self.operand_type(array)?;
        let element = jvm_array_element_type(&array_type)?.clone();
        let store = {
            let layouts = self.submission.backend_plan.aggregate_layouts();
            jvm_array_access_instructions(&jvm_array_element_descriptor(layouts, &element)?)?.1
        };
        self.emit_operand(array, Some(JvmSlotKind::Reference))?;
        self.emit_operand(index, Some(JvmSlotKind::Int))?;
        self.emit_array_element_value(value, &element)?;
        self.instructions.push(store);
        Ok(())
    }
}

/// 把冻结比较序列的占位标签替换为方法内唯一标签。
fn relabel_operator_branch(instruction: JvmInstruction, prefix: &str) -> JvmInstruction {
    let relabel = |label: &str| format!("{prefix}_{label}");
    match instruction {
        JvmInstruction::Label(label) => JvmInstruction::Label(relabel(&label)),
        JvmInstruction::Goto(label) => JvmInstruction::Goto(relabel(&label)),
        JvmInstruction::IfEq(label) => JvmInstruction::IfEq(relabel(&label)),
        JvmInstruction::IfNe(label) => JvmInstruction::IfNe(relabel(&label)),
        JvmInstruction::IfLt(label) => JvmInstruction::IfLt(relabel(&label)),
        JvmInstruction::IfLe(label) => JvmInstruction::IfLe(relabel(&label)),
        JvmInstruction::IfGt(label) => JvmInstruction::IfGt(relabel(&label)),
        JvmInstruction::IfGe(label) => JvmInstruction::IfGe(relabel(&label)),
        JvmInstruction::IfICmpEq(label) => JvmInstruction::IfICmpEq(relabel(&label)),
        JvmInstruction::IfICmpNe(label) => JvmInstruction::IfICmpNe(relabel(&label)),
        JvmInstruction::IfICmpLt(label) => JvmInstruction::IfICmpLt(relabel(&label)),
        JvmInstruction::IfICmpLe(label) => JvmInstruction::IfICmpLe(relabel(&label)),
        JvmInstruction::IfICmpGt(label) => JvmInstruction::IfICmpGt(relabel(&label)),
        JvmInstruction::IfICmpGe(label) => JvmInstruction::IfICmpGe(relabel(&label)),
        other => other,
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

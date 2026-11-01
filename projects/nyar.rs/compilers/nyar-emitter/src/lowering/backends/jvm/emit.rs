//! JVM 最小 MIR → `class` 降低。
//!
//! 只消费计划已经冻结的标量事实：整型/浮点常量、SSA local、冻结 operator、
//! 块标签与条件分支、同片段 `invokestatic` 调用以及 `Return`。聚合、数组、sum、
//! 宿主导入等形状在这里返回结构化错误，不补造指令、默认值或空体方法。
//!
//! 类内部名与方法命名都是稳定规则：类内部名取模块名与片段 id 的
//! [`crate::lowering::sanitize_symbol`] 结果；导出函数取计划冻结的
//! `wasm_export_names`，其余可达函数取 ABI 名的
//! [`crate::lowering::sanitize_jvm_method_symbol`] 结果，冲突时追加稳定序号。

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    FragmentSubmission, Result,
    backend_plan_views::{
        ExecutableBlock as MirBlock, ExecutableBlockRef as MirBlockRef, ExecutableFunction as MirFunction, ExecutableInstruction as MirInstruction,
        ExecutableValueRef as MirValueRef, NyarType,
    },
    contracts::{Constant, InstructionKind, Operand, Terminator},
    lowering::shared::executable::{block_label, collect_reachable_blocks, slots::ExecutableSlotPlan},
    managed_abi::{ManagedAbiContract, ManagedAbiEntry},
    target_operator::{TargetOperandStep, TargetOperandType, TargetOperatorBinding, TargetOperatorEncoding, TargetOperatorPlan, TargetOperatorRecipe},
    transitional::binary::class::{
        JvmClassFile, JvmCodeBody, JvmFieldRef, JvmInstruction, JvmMethodDescriptor, JvmMethodRef, JvmMethodSignature, JvmTypeDescriptor,
    },
};
use nyar_types::ItemInstanceId;

const JVM_ACC_PUBLIC: u16 = 0x0001;
const JVM_ACC_STATIC: u16 = 0x0008;

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
    for (ordinal, instance) in operations.iter().enumerate() {
        let view = plan
            .get_function(instance)
            .ok_or_else(|| format!("JVM 最小 lowering：可达操作 `{instance:?}` 缺少函数体"))?;
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
        method_plans.insert(*instance, JvmMethodPlan { name, descriptor: function_descriptor(&view.function)? });
    }

    let internal_name =
        format!("nyar/{}_{}", crate::lowering::sanitize_symbol(plan.module_name()), crate::lowering::sanitize_symbol(plan.fragment_id().as_str()));
    let operator_by_instance = external_operator_by_instance(submission);
    let mut class_file = JvmClassFile::new(internal_name.clone());
    for instance in &operations {
        let Some(view) = plan.get_function(instance)
        else {
            continue;
        };
        let method = method_plans
            .get(instance)
            .cloned()
            .ok_or_else(|| format!("JVM 最小 lowering：可达操作 `{instance:?}` 缺少方法计划"))?;
        class_file.methods.push(lower_function(submission, &internal_name, &view.function, &method, &method_plans, &operator_by_instance)?);
    }

    let (companion_classes, managed_abi) = build_managed_abi_surface(&class_file, plan.wasm_export_names(), &method_plans)?;
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
        let parameter_types = method.descriptor.parameter_types.iter().map(managed_scalar_type_name).collect::<Result<Vec<_>>>()?;
        let return_type = managed_scalar_type_name(&method.descriptor.return_type)?;
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

/// managed ABI 只接受冻结的标量类型：`i64` / `i32` / `f64` / `bool` / `void`。
fn managed_scalar_type_name(descriptor: &JvmTypeDescriptor) -> Result<String> {
    Ok(match descriptor {
        JvmTypeDescriptor::Long => "i64",
        JvmTypeDescriptor::Int | JvmTypeDescriptor::Byte | JvmTypeDescriptor::Char | JvmTypeDescriptor::Short => "i32",
        JvmTypeDescriptor::Double => "f64",
        JvmTypeDescriptor::Boolean => "bool",
        JvmTypeDescriptor::Void => "void",
        JvmTypeDescriptor::Float => {
            return Err("JVM managed ABI 不支持 `float` (f32) 载体：缺少对应 managed 标量类型名".to_string());
        }
        JvmTypeDescriptor::Object(name) => {
            return Err(format!("JVM managed ABI 不支持引用类型 `{name}`：缺少 managed 标量契约"));
        }
        JvmTypeDescriptor::Array(item) => {
            return Err(format!("JVM managed ABI 不支持数组类型 `[{item}`：缺少 managed 标量契约"));
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

fn function_descriptor(function: &MirFunction) -> Result<JvmMethodDescriptor> {
    let entry = function
        .blocks
        .get(function.entry.0 as usize)
        .ok_or_else(|| format!("JVM 最小 lowering：函数 `{}` 缺少入口块", function.symbol))?;
    let mut parameter_types = Vec::with_capacity(entry.parameters.len());
    for parameter in &entry.parameters {
        let ty = function
            .value_types
            .get(parameter)
            .ok_or_else(|| format!("JVM 最小 lowering：函数 `{}` 入口形参缺少冻结类型", function.symbol))?;
        parameter_types.push(jvm_parameter_descriptor(ty)?);
    }
    Ok(JvmMethodDescriptor::new(parameter_types, jvm_return_descriptor(&function.return_type)?))
}

/// 局部 / 形参位置的 JVM 载体：`Unit`/`Bottom` 没有值槽。
fn jvm_parameter_descriptor(ty: &NyarType) -> Result<JvmTypeDescriptor> {
    let descriptor = jvm_type_descriptor(ty)?;
    if descriptor == JvmTypeDescriptor::Void {
        return Err(format!("JVM 最小 lowering：`{ty}` 是 void 形态，不能作为方法形参或局部值"));
    }
    Ok(descriptor)
}

fn jvm_return_descriptor(ty: &NyarType) -> Result<JvmTypeDescriptor> {
    jvm_type_descriptor(ty)
}

fn jvm_type_descriptor(ty: &NyarType) -> Result<JvmTypeDescriptor> {
    Ok(match ty {
        NyarType::Bottom | NyarType::Unit => JvmTypeDescriptor::Void,
        NyarType::Boolean => JvmTypeDescriptor::Boolean,
        NyarType::Character => JvmTypeDescriptor::Char,
        NyarType::Integer8 { .. } | NyarType::Integer16 { .. } | NyarType::Integer32 { .. } => JvmTypeDescriptor::Int,
        NyarType::Integer64 { .. } => JvmTypeDescriptor::Long,
        NyarType::Float32 => JvmTypeDescriptor::Float,
        NyarType::Float64 => JvmTypeDescriptor::Double,
        NyarType::Utf8 | NyarType::Utf16 => JvmTypeDescriptor::Object("java/lang/String".to_string()),
        other => {
            return Err(format!("JVM 最小 lowering 不支持类型 `{other}`：缺少该形状的 JVM 载体合同"));
        }
    })
}

fn jvm_local_kind(ty: &NyarType) -> Result<JvmSlotKind> {
    JvmSlotKind::of(&jvm_parameter_descriptor(ty)?)
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
        local_count = local_count.saturating_add(jvm_local_kind(ty)?.slot_width());
    }

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
            other => {
                return Err(format!(
                    "JVM 最小 lowering 尚不支持指令 `{}`：function={}",
                    instruction_kind_name(other),
                    self.function.symbol
                ));
            }
        }
        Ok(())
    }

    fn emit_terminator(&mut self, block: &MirBlock) -> Result<()> {
        match &block.terminator {
            Terminator::Return { value } => {
                let return_kind = if self.return_descriptor == JvmTypeDescriptor::Void {
                    None
                }
                else {
                    Some(JvmSlotKind::of(&self.return_descriptor)?)
                };
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
                    let target_block = self.function.blocks.get(target.0 as usize).ok_or_else(|| {
                        format!("JVM 最小 lowering：函数 `{}` 跳转目标块 {:?} 不存在", self.function.symbol, target)
                    })?;
                    let kinds = target_block
                        .parameters
                        .iter()
                        .map(|parameter| {
                            self.function
                                .value_types
                                .get(parameter)
                                .ok_or_else(|| format!("JVM 最小 lowering：块参数缺少冻结类型: function={}", self.function.symbol))
                                .and_then(jvm_local_kind)
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
                    let local = *self.slots.block_param_locals.get(&(*target, index)).ok_or_else(|| {
                        format!("JVM 最小 lowering：跳转目标块 {:?} 形参 {index} 缺少局部槽", target)
                    })?;
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
                    return Err(format!(
                        "JVM 最小 lowering 只支持无块参数的条件分支：function={}",
                        self.function.symbol
                    ));
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
                    return Err(format!(
                        "JVM 最小 lowering：非 void 函数 `{}` 的 `Unreachable` 终结缺少返回指令",
                        self.function.symbol
                    ));
                }
            }
            other => {
                return Err(format!(
                    "JVM 最小 lowering 尚不支持终结符 `{other:?}`: function={}",
                    self.function.symbol
                ));
            }
        }
        Ok(())
    }

    fn emit_call(&mut self, callee: &Operand, arguments: &[Operand], output: &Option<(u16, JvmSlotKind)>) -> Result<()> {
        let Operand::Item(instance) = callee
        else {
            return Err(format!(
                "JVM 最小 lowering 只支持 `Item` 身份的静态调用: function={} callee={callee:?}",
                self.function.symbol
            ));
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
            return Err(format!(
                "JVM 最小 lowering：同片段调用目标 `{instance:?}` 不在可达方法集中: function={}",
                self.function.symbol
            ));
        };
        let parameter_kinds = method
            .descriptor
            .parameter_types
            .iter()
            .map(JvmSlotKind::of)
            .collect::<Result<Vec<_>>>()?;
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
            return Err(format!(
                "JVM 目标 `jvm_operator` 绑定缺少可编码助记符 `{mnemonic}`: function={}",
                self.function.symbol
            ));
        };
        let TargetOperatorEncoding::Jvm(code) = plan.encoding
        else {
            return Err(format!(
                "JVM 目标 `jvm_operator` 绑定 `{mnemonic}` 冻结了非 JVM 编码: function={}",
                self.function.symbol
            ));
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

        let expansion = code.instructions();
        if expansion.is_empty() {
            return Err(format!(
                "JVM 目标 operator `{mnemonic}` 的冻结编码 `{code:?}` 没有指令展开: function={}",
                self.function.symbol
            ));
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
            Operand::Item(instance) => Err(format!(
                "JVM 最小 lowering 不支持把可调用身份 `{instance:?}` 当值使用: function={}",
                self.function.symbol
            )),
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
                Some(other) => Err(format!(
                    "JVM 最小 lowering：整数常量不能进入 {other:?} 槽: function={}",
                    self.function.symbol
                )),
            },
            Constant::Float64(value) => match expected {
                Some(JvmSlotKind::Float) => Err(format!(
                    "JVM 最小 lowering 不支持把 f64 常量压入 `float` 槽：function={}",
                    self.function.symbol
                )),
                Some(JvmSlotKind::Int | JvmSlotKind::Long | JvmSlotKind::Reference) => Err(format!(
                    "JVM 最小 lowering：f64 常量不能进入 {expected:?} 槽: function={}",
                    self.function.symbol
                )),
                Some(JvmSlotKind::Double) => {
                    self.instructions.push(JvmInstruction::LdcDouble(value.into_inner().to_bits()));
                    Ok(())
                }
                None => Err(format!(
                    "JVM 最小 lowering：f64 常量缺少明确的 `double` 结果槽，拒绝猜测载体: function={}",
                    self.function.symbol
                )),
            },
            Constant::Bool(value) => {
                if matches!(expected, Some(JvmSlotKind::Long | JvmSlotKind::Float | JvmSlotKind::Double | JvmSlotKind::Reference)) {
                    return Err(format!(
                        "JVM 最小 lowering：布尔常量不能进入 {expected:?} 槽: function={}",
                        self.function.symbol
                    ));
                }
                self.instructions.push(JvmInstruction::IConst(i32::from(*value)));
                Ok(())
            }
            other => Err(format!(
                "JVM 最小 lowering 尚不支持常量 `{other:?}`：缺少 JVM 常量池载体合同，function={}",
                self.function.symbol
            )),
        }
    }

    fn emit_pop(&mut self, kind: Option<JvmSlotKind>) -> Result<()> {
        match kind {
            None | Some(JvmSlotKind::Int | JvmSlotKind::Reference | JvmSlotKind::Float) => {
                self.instructions.push(JvmInstruction::Pop);
                Ok(())
            }
            Some(JvmSlotKind::Long | JvmSlotKind::Double) => Err(format!(
                "JVM 最小 lowering：丢弃 2 槽值需要 `pop2` 编码，当前 JVM 指令模型不提供: function={}",
                self.function.symbol
            )),
        }
    }

    fn output_of(&self, instruction: &MirInstruction) -> Result<Option<(u16, JvmSlotKind)>> {
        match self.slots.instruction_output(instruction) {
            None => Ok(None),
            Some(value) => self.value_slot(value).map(Some),
        }
    }

    fn require_output(&self, instruction: &MirInstruction) -> Result<(u16, JvmSlotKind)> {
        self.output_of(instruction)?.ok_or_else(|| {
            format!(
                "JVM 最小 lowering：指令缺少 SSA 结果槽，无法确定局部载体: function={}",
                self.function.symbol
            )
        })
    }

    fn value_slot(&self, value: MirValueRef) -> Result<(u16, JvmSlotKind)> {
        let local = *self.slots.value_locals.get(&value).ok_or_else(|| {
            format!("JVM 最小 lowering：SSA 值 {:?} 缺少规划的局部槽: function={}", value, self.function.symbol)
        })?;
        let ty = self
            .slots
            .local_types
            .get(local as usize)
            .ok_or_else(|| format!("JVM 最小 lowering：局部槽 {local} 超出规划表: function={}", self.function.symbol))?;
        let kind = jvm_local_kind(ty)?;
        Ok((self.slot_offsets[local as usize], kind))
    }

    /// 比较展开标签前缀：当前块标签 + 方法内比较序号，保证方法内唯一。
    fn operator_label_prefix(&self) -> String {
        format!("{}_{}", block_label(self.current_block), self.comparison_ordinal)
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

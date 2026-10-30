//! CanonicalProgram 与编译处理阶段的唯一成功载荷。
//!
//! `CompiledProgram` 只允许把已验证的 Canonical Semantic MIR 与同一程序
//! 生成的表示计划一起交给后续阶段。

use super::{
    canonical_mir::{CanonicalMirError, CanonicalOperation, CanonicalSemanticMir},
    linked_program::LinkedSemanticProgram,
};
use crate::semantic_ids::{InstructionId, ItemInstanceId, MirValueId, NominalInstanceId, ValueIdentity, layout_choice::RepresentationPlan};

/// Top-level canonical success bundle after link + MIR validation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanonicalProgram {
    /// Linked semantic closure.
    pub linked: LinkedSemanticProgram,
    /// Validated MIR (semantic only).
    pub mir: CanonicalSemanticMir,
}

impl CanonicalProgram {
    /// 验证链接闭包与 Semantic MIR 后，才允许进入 processing half。
    pub fn validate(&self) -> Result<(), CanonicalMirError> {
        self.mir.validate(&self.linked)
    }
}

/// 编译处理阶段的唯一成功载荷。
///
/// 该类型只允许把已验证的 Canonical Semantic MIR 与对应的稀疏表示计划
/// 一起交给后续阶段。它不包含 HIR、字符串 callable、旧 executable 或
/// `FragmentSubmission` 字段；后端若需要这些信息，说明上游合同仍未闭合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledProgram {
    canonical: CanonicalProgram,
    representation: RepresentationPlan,
}

/// `CompiledProgram` 构造时发现的跨阶段合同错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompiledProgramError {
    /// 已链接的名义实例缺少表示选择。
    MissingAdtRepresentation {
        /// 缺少 ADT 表示的名义实例。
        nominal: NominalInstanceId,
    },
    /// 表示计划引用了程序之外的名义实例。
    ExtraAdtRepresentation {
        /// 多余的 ADT 表示名义实例。
        nominal: NominalInstanceId,
    },
    /// 已链接的 sum nominal 缺少完整表示合同。
    MissingSumRepresentation {
        /// 缺少 sum 表示的名义实例。
        nominal: NominalInstanceId,
    },
    /// 表示计划包含不属于该程序的 sum nominal。
    ExtraSumRepresentation {
        /// 多余的 sum 表示名义实例。
        nominal: NominalInstanceId,
    },
    /// sum 表示合同缺少或多出 variant。
    SumVariantRepresentationMismatch {
        /// variant 表示不一致的名义实例。
        nominal: NominalInstanceId,
    },
    /// Canonical Semantic MIR 尚未通过 M2 验证。
    Canonical(CanonicalMirError),
    /// 某个 Semantic MIR 值没有对应表示选择。
    MissingValueRepresentation {
        /// 所在函数实例。
        function: ItemInstanceId,
        /// 缺少表示选择的 SSA 值。
        value: MirValueId,
    },
    /// 某条调用指令没有对应调用表示选择。
    MissingInvokeRepresentation {
        /// 缺少调用表示的指令身份。
        instruction: InstructionId,
    },
    /// 表示计划包含不属于该程序的值键。
    ExtraValueRepresentation {
        /// 所在函数实例。
        function: ItemInstanceId,
        /// 多余的值表示键。
        value: MirValueId,
    },
    /// 表示计划包含不属于该程序的调用键。
    ExtraInvokeRepresentation {
        /// 多余的调用表示指令身份。
        instruction: InstructionId,
    },
}

/// 错误的渲染归错误自身所有：消费方只能拿到结构化字段的稳定文本，不得自己
/// 枚举变体或转储 Debug 结构。`Canonical` 变体在表示规划先行验证下不可达，
/// 只保留变体名，避免把另一类型的 Debug 结构漏进诊断。
impl std::fmt::Display for CompiledProgramError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingAdtRepresentation { nominal } => write!(formatter, "MissingAdtRepresentation(nominal={nominal})"),
            Self::ExtraAdtRepresentation { nominal } => write!(formatter, "ExtraAdtRepresentation(nominal={nominal})"),
            Self::MissingSumRepresentation { nominal } => write!(formatter, "MissingSumRepresentation(nominal={nominal})"),
            Self::ExtraSumRepresentation { nominal } => write!(formatter, "ExtraSumRepresentation(nominal={nominal})"),
            Self::SumVariantRepresentationMismatch { nominal } => write!(formatter, "SumVariantRepresentationMismatch(nominal={nominal})"),
            Self::Canonical(_) => write!(formatter, "Canonical(unvalidated canonical MIR)"),
            Self::MissingValueRepresentation { function, value } => {
                write!(formatter, "MissingValueRepresentation(function={function}, value={value})")
            }
            Self::MissingInvokeRepresentation { instruction } => write!(formatter, "MissingInvokeRepresentation(instruction={instruction})"),
            Self::ExtraValueRepresentation { function, value } => {
                write!(formatter, "ExtraValueRepresentation(function={function}, value={value})")
            }
            Self::ExtraInvokeRepresentation { instruction } => write!(formatter, "ExtraInvokeRepresentation(instruction={instruction})"),
        }
    }
}

impl CompiledProgram {
    /// 只从完整 CanonicalProgram 与同一程序生成的 RepresentationPlan 构造。
    pub fn new(canonical: CanonicalProgram, representation: RepresentationPlan) -> Result<Self, CompiledProgramError> {
        canonical.validate().map_err(CompiledProgramError::Canonical)?;
        for nominal in canonical.linked.nominal_instances.keys() {
            if !representation.adt_reps.contains_key(nominal) {
                return Err(CompiledProgramError::MissingAdtRepresentation { nominal: *nominal });
            }
        }
        if let Some(nominal) = representation.adt_reps.keys().find(|nominal| !canonical.linked.nominal_instances.contains_key(nominal)) {
            return Err(CompiledProgramError::ExtraAdtRepresentation { nominal: *nominal });
        }
        let expected_sum_nominals = canonical.linked.sum_types.iter().map(|sum| sum.nominal).collect::<std::collections::BTreeSet<_>>();
        for nominal in &expected_sum_nominals {
            let Some(sum) = representation.sum_reps.get(nominal)
            else {
                return Err(CompiledProgramError::MissingSumRepresentation { nominal: *nominal });
            };
            let expected_variants = canonical
                .linked
                .variants
                .keys()
                .filter_map(|(owner, variant)| (*owner == *nominal).then_some(*variant))
                .collect::<std::collections::BTreeSet<_>>();
            if sum.variants.keys().copied().collect::<std::collections::BTreeSet<_>>() != expected_variants {
                return Err(CompiledProgramError::SumVariantRepresentationMismatch { nominal: *nominal });
            }
        }
        if let Some(nominal) = representation.sum_reps.keys().find(|nominal| !expected_sum_nominals.contains(nominal)) {
            return Err(CompiledProgramError::ExtraSumRepresentation { nominal: *nominal });
        }
        let mut expected_values = std::collections::BTreeSet::new();
        let mut expected_invokes = std::collections::BTreeSet::new();
        for function in canonical.mir.functions.values() {
            for value in function.value_types.keys() {
                let identity = ValueIdentity::new(function.instance, *value);
                expected_values.insert(identity);
                if !representation.value_reps.contains_key(&identity) {
                    return Err(CompiledProgramError::MissingValueRepresentation { function: function.instance, value: *value });
                }
            }
            for block in function.blocks.values() {
                for instruction in &block.instructions {
                    if matches!(&instruction.operation, CanonicalOperation::Invoke { .. }) {
                        expected_invokes.insert(instruction.id);
                        if !representation.invoke_lowerings.contains_key(&instruction.id) {
                            return Err(CompiledProgramError::MissingInvokeRepresentation { instruction: instruction.id });
                        }
                    }
                }
            }
        }
        if let Some(identity) = representation.value_reps.keys().find(|identity| !expected_values.contains(identity)) {
            return Err(CompiledProgramError::ExtraValueRepresentation { function: identity.function, value: identity.value });
        }
        if let Some(instruction) = representation.invoke_lowerings.keys().find(|instruction| !expected_invokes.contains(instruction)) {
            return Err(CompiledProgramError::ExtraInvokeRepresentation { instruction: *instruction });
        }
        Ok(Self { canonical, representation })
    }

    /// 返回 Compiler 产生的 Canonical Semantic MIR。
    pub fn canonical(&self) -> &CanonicalProgram {
        &self.canonical
    }

    /// 返回与 CanonicalProgram 同步生成的表示计划。
    pub fn representation(&self) -> &RepresentationPlan {
        &self.representation
    }

    /// 已链接导出、入口与函数实例的稳定表面摘要（不含字节码）。
    pub fn surface_digest(&self) -> String {
        let canonical = self.canonical();
        let mut parts = vec![canonical.mir.module_name.clone(), "surface-v1".to_string()];
        for (instance, export) in canonical.linked.exports.iter() {
            parts.push(format!("export:{}:{}", export.exported_name, instance.index()));
        }
        for instance in canonical.linked.entries.keys() {
            parts.push(format!("entry:{}", instance.index()));
        }
        for instance in canonical.mir.functions.keys() {
            parts.push(format!("fn:{}", instance.index()));
        }
        let borrowed = parts.iter().map(String::as_str).collect::<Vec<_>>();
        crate::digest::combined_content_hash(&borrowed)
    }
}

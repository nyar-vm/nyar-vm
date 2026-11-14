//! `ValkyrieMirProgram` 与编译处理阶段的唯一成功载荷。
//!
//! 该成功类型归 Valkyrie 所有；`CompiledValkyrieProgram` 只允许把已验证的
//! Valkyrie MIR 与同一程序生成的表示计划一起交给后续阶段（语言 × 目标适配器）。

use super::operation::{ValkyrieMir, ValkyrieMirCallee, ValkyrieMirError, ValkyrieMirOperation, ValkyrieMirWitness};
use nyar_types::LinkedSemanticProgram;
use nyar_types::semantic_ids::{InstructionId, ItemInstanceId, MirValueId, NominalInstanceId, ValueIdentity};

use super::representation::{RepresentationPlan, TextRepresentation};

/// 链接与 MIR 验证后的 Valkyrie 语义成功载荷。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValkyrieMirProgram {
    /// Linked semantic closure.
    pub linked: LinkedSemanticProgram,
    /// Validated MIR (semantic only).
    pub mir: ValkyrieMir,
}

impl ValkyrieMirProgram {
    /// 在实例化固定点把具体 witness 分派消去为冻结的 `ItemInstanceId`。
    ///
    /// 消去只认已冻结事实：`(method_declaration, self_type)` 必须对应唯一个
    /// `linked.item_instances` 记录。开放 self、缺实现或多候选都返回结构化错误；
    /// 目标层不得再扫描语义实例库决定调用谁。消去后由现有 `Item` 校验路径执行
    /// 等价的完整调用签名验证。
    pub fn resolve_witness_dispatch(&mut self) -> Result<(), ValkyrieMirError> {
        let mut dispatched: Vec<(ValkyrieMirWitness, ItemInstanceId)> = Vec::new();
        for function in self.mir.functions.values() {
            for block in function.blocks.values() {
                for instruction in &block.instructions {
                    if let ValkyrieMirOperation::Invoke { callee: ValkyrieMirCallee::Witness(witness), .. } = &instruction.operation
                        && !dispatched.iter().any(|(known, _)| known == witness)
                    {
                        let instance = self.resolve_witness_instance(witness)?;
                        dispatched.push((*witness, instance));
                    }
                }
            }
        }
        if dispatched.is_empty() {
            return Ok(());
        }
        for function in self.mir.functions.values_mut() {
            for block in function.blocks.values_mut() {
                for instruction in &mut block.instructions {
                    if let ValkyrieMirOperation::Invoke { callee, .. } = &mut instruction.operation
                        && let ValkyrieMirCallee::Witness(witness) = callee
                        && let Some((_, instance)) = dispatched.iter().find(|(known, _)| known == witness)
                    {
                        *callee = ValkyrieMirCallee::Item(*instance);
                    }
                }
            }
        }
        Ok(())
    }

    fn resolve_witness_instance(&self, witness: &ValkyrieMirWitness) -> Result<ItemInstanceId, ValkyrieMirError> {
        let unresolved = |candidates: usize| ValkyrieMirError::UnresolvedWitnessDispatch {
            trait_declaration: witness.trait_declaration,
            method_declaration: witness.method_declaration,
            self_type: witness.self_type,
            candidates,
        };
        let Some(self_type) = witness.self_type
        else {
            return Err(unresolved(0));
        };
        let mut matching = self
            .linked
            .item_instances
            .iter()
            .filter(|(_, record)| record.declaration == witness.method_declaration && record.parameter_types.first() == Some(&self_type));
        let Some((instance, _)) = matching.next()
        else {
            return Err(unresolved(0));
        };
        let remaining = matching.count();
        if remaining != 0 {
            return Err(unresolved(1 + remaining));
        }
        Ok(*instance)
    }

    /// 验证链接闭包与 Semantic MIR 后，才允许进入 processing half。
    pub fn validate(&self) -> Result<(), ValkyrieMirError> {
        super::validation::validate_named_residues(&self.linked)?;
        self.mir.validate(&self.linked)
    }
}

/// 编译处理阶段的唯一成功载荷。
///
/// 该类型只允许把已验证的 Valkyrie MIR 与对应的稀疏表示计划
/// 一起交给后续阶段。它不包含 HIR、字符串 callable、旧 executable 或
/// `FragmentSubmission` 字段；后端若需要这些信息，说明上游合同仍未闭合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledValkyrieProgram {
    program: ValkyrieMirProgram,
    representation: RepresentationPlan,
}

/// `CompiledValkyrieProgram` 构造时发现的跨阶段合同错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompiledValkyrieProgramError {
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
    /// Valkyrie MIR 尚未通过 M2 验证。
    Canonical(ValkyrieMirError),
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
    /// 程序级文本表示与 canonical 声明 / 值类型事实不一致。
    ///
    /// 缺失（该有 `Some` 却是 `None`，或该是 `None` 却有值）与矛盾（声明了文本载体
    /// 却计划为 `Raw`，或没有载体却计划为 `Managed`）都在构造期失败。
    TextRepresentationMismatch {
        /// 由 canonical 事实判定的期望表示。
        expected: Option<TextRepresentation>,
        /// 表示计划声明的实际表示。
        actual: Option<TextRepresentation>,
    },
}

/// 错误的渲染归错误自身所有：消费方只能拿到结构化字段的稳定文本，不得自己
/// 枚举变体或转储 Debug 结构。`Canonical` 变体在表示规划先行验证下不可达，
/// 只保留变体名，避免把另一类型的 Debug 结构漏进诊断。
impl std::fmt::Display for CompiledValkyrieProgramError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingAdtRepresentation { nominal } => write!(formatter, "MissingAdtRepresentation(nominal={nominal})"),
            Self::ExtraAdtRepresentation { nominal } => write!(formatter, "ExtraAdtRepresentation(nominal={nominal})"),
            Self::Canonical(_) => write!(formatter, "ValkyrieMir(unvalidated Valkyrie MIR)"),
            Self::MissingValueRepresentation { function, value } => {
                write!(formatter, "MissingValueRepresentation(function={function}, value={value})")
            }
            Self::MissingInvokeRepresentation { instruction } => write!(formatter, "MissingInvokeRepresentation(instruction={instruction})"),
            Self::ExtraValueRepresentation { function, value } => {
                write!(formatter, "ExtraValueRepresentation(function={function}, value={value})")
            }
            Self::ExtraInvokeRepresentation { instruction } => write!(formatter, "ExtraInvokeRepresentation(instruction={instruction})"),
            Self::TextRepresentationMismatch { expected, actual } => write!(
                formatter,
                "TextRepresentationMismatch(expected={}, actual={})",
                text_representation_label(*expected),
                text_representation_label(*actual)
            ),
        }
    }
}

fn text_representation_label(value: Option<TextRepresentation>) -> &'static str {
    match value {
        None => "none",
        Some(TextRepresentation::Raw) => "raw",
        Some(TextRepresentation::Managed) => "managed",
    }
}

impl CompiledValkyrieProgram {
    /// 只从完整 ValkyrieMirProgram 与同一程序生成的 RepresentationPlan 构造。
    pub fn new(program: ValkyrieMirProgram, representation: RepresentationPlan) -> Result<Self, CompiledValkyrieProgramError> {
        program.validate().map_err(CompiledValkyrieProgramError::Canonical)?;
        let declared_text = super::text::declared_text_representation(&program);
        if representation.text_representation != declared_text {
            return Err(CompiledValkyrieProgramError::TextRepresentationMismatch {
                expected: declared_text,
                actual: representation.text_representation,
            });
        }
        for nominal in program.linked.nominal_instances.keys() {
            if !representation.adt_reps.contains_key(nominal) {
                return Err(CompiledValkyrieProgramError::MissingAdtRepresentation { nominal: *nominal });
            }
        }
        if let Some(nominal) = representation.adt_reps.keys().find(|nominal| !program.linked.nominal_instances.contains_key(nominal)) {
            return Err(CompiledValkyrieProgramError::ExtraAdtRepresentation { nominal: *nominal });
        }
        // sum 的 variant/payload 事实由 `LinkedSemanticProgram` 唯一拥有；表示计划
        // 不再复制它们，也不提前决定平台 tag 宽度。
        let mut expected_values = std::collections::BTreeSet::new();
        let mut expected_invokes = std::collections::BTreeSet::new();
        for function in program.mir.functions.values() {
            for value in function.value_types.keys() {
                let identity = ValueIdentity::new(function.instance, *value);
                expected_values.insert(identity);
                if !representation.value_reps.contains_key(&identity) {
                    return Err(CompiledValkyrieProgramError::MissingValueRepresentation { function: function.instance, value: *value });
                }
            }
            for block in function.blocks.values() {
                for instruction in &block.instructions {
                    if matches!(&instruction.operation, ValkyrieMirOperation::Invoke { .. }) {
                        expected_invokes.insert(instruction.id);
                        if !representation.invoke_lowerings.contains_key(&instruction.id) {
                            return Err(CompiledValkyrieProgramError::MissingInvokeRepresentation { instruction: instruction.id });
                        }
                    }
                }
            }
        }
        if let Some(identity) = representation.value_reps.keys().find(|identity| !expected_values.contains(identity)) {
            return Err(CompiledValkyrieProgramError::ExtraValueRepresentation { function: identity.function, value: identity.value });
        }
        if let Some(instruction) = representation.invoke_lowerings.keys().find(|instruction| !expected_invokes.contains(instruction)) {
            return Err(CompiledValkyrieProgramError::ExtraInvokeRepresentation { instruction: *instruction });
        }
        Ok(Self { program, representation })
    }

    /// 返回 Compiler 产生的已验证 Valkyrie MIR。
    pub fn mir_program(&self) -> &ValkyrieMirProgram {
        &self.program
    }

    /// 返回与 ValkyrieMirProgram 同步生成的表示计划。
    pub fn representation(&self) -> &RepresentationPlan {
        &self.representation
    }

    /// 已链接导出、入口与函数实例的稳定表面摘要（不含字节码）。
    pub fn surface_digest(&self) -> String {
        let canonical = self.mir_program();
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
        nyar_types::combined_content_hash(&borrowed)
    }
}

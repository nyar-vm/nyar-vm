use nyar_types::{DiagnosticSource, DiagnosticSpan};

use super::model::*;

impl HirError {
    /// 源码引用了已移除的旧内建类型别名。
    pub fn legacy_builtin_type_alias(alias: impl Into<String>, canonical: impl Into<String>) -> Self {
        Self::LegacyBuiltinTypeAlias {
            alias: alias.into(),
            canonical: canonical.into(),
            source: nyar_types::DiagnosticSource { id: "<type_lowering>".into(), revision: None },
            span: nyar_types::DiagnosticSpan { start: 0, end: 0 },
        }
    }

    /// 源码使用了不指定编码的歧义文本类型名。
    pub fn ambiguous_text_type_name(name: impl Into<String>) -> Self {
        Self::AmbiguousTextTypeName {
            name: name.into(),
            source: nyar_types::DiagnosticSource { id: "<type_lowering>".into(), revision: None },
            span: nyar_types::DiagnosticSpan { start: 0, end: 0 },
        }
    }

    /// TGrammar 模板展开失败；span 指向触发失败的模板 token 流。
    pub(crate) fn template_expansion_failed(construct: TemplateConstruct, failure: TemplateFailure, span: DiagnosticSpan) -> Self {
        Self::TemplateExpansionFailed { construct, failure, source: DiagnosticSource { id: "<template>".into(), revision: None }, span }
    }

    /// `match` 源码恢复失败；span 指向被恢复的 `match` 块。
    pub(crate) fn match_recovery_failed(stage: MatchRecoveryStage, pattern: Option<String>, span: DiagnosticSpan) -> Self {
        Self::MatchRecoveryFailed { stage, pattern, source: DiagnosticSource { id: "<match>".into(), revision: None }, span }
    }

    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::UnresolvedCall { .. } => "valkyrie.hir.unresolved_call",
            Self::UnresolvedConstructor { .. } => "valkyrie.hir.unresolved_constructor",
            Self::NominalAmbiguous { .. } => "valkyrie.hir.nominal_ambiguous",
            Self::SumUniteAmbiguous { .. } => "valkyrie.hir.sum_unite_ambiguous",
            Self::StructureMissingConstructor { .. } => "valkyrie.hir.structure_missing_constructor",
            Self::SumUniteMissingDeclaration { .. } => "valkyrie.hir.sum_unite_missing_declaration",
            Self::InvalidDiscriminator { .. } => "valkyrie.hir.invalid_discriminator",
            Self::NoBodySourceInGroup { .. } => "valkyrie.hir.no_body_source",
            Self::MirValidationRejected(error) => error.code(),
            Self::FrontendRejected(_) => "valkyrie.frontend.rejected",
            Self::TemplateExpansionFailed { .. } => "valkyrie.hir.template_expansion_failed",
            Self::MatchRecoveryFailed { .. } => "valkyrie.hir.match_recovery_failed",
            Self::ControlFlowContextViolation { .. } => "valkyrie.hir.control_flow_context_violation",
            Self::FallthroughWithoutNextArm { .. } => "valkyrie.hir.fallthrough_without_next_arm",
            Self::FallthroughOutsideCaseChain { .. } => "valkyrie.hir.fallthrough_outside_case_chain",
            Self::FallthroughBindingLeak { .. } => "valkyrie.hir.fallthrough_binding_leak",
            Self::LoopTargetMissing { .. } => "valkyrie.hir.loop_target_missing",
            Self::BreakValueRejected { .. } => "valkyrie.hir.break_value_rejected",
            Self::BreakValueTypeMismatch { .. } => "valkyrie.hir.break_value_type_mismatch",
            Self::TryPropagateOperandUnsupported { .. } => "valkyrie.hir.try_propagate_operand_unsupported",
            Self::TryPropagateReturnMismatch { .. } => "valkyrie.hir.try_propagate_return_mismatch",
            Self::TryPropagateErrorTypeMismatch { .. } => "valkyrie.hir.try_propagate_error_type_mismatch",
            Self::FutureControlOperandMismatch { .. } => "valkyrie.hir.future_control_operand_mismatch",
            Self::ControlFlowValueRequired { .. } => "valkyrie.hir.control_flow_value_required",
            Self::ReturnValueTypeMismatch { .. } => "valkyrie.hir.return_value_type_mismatch",
            Self::YieldValueTypeMismatch { .. } => "valkyrie.hir.yield_value_type_mismatch",
            Self::YieldFromOperandNotGenerator { .. } => "valkyrie.hir.yield_from_operand_not_generator",
            Self::YieldFromItemTypeMismatch { .. } => "valkyrie.hir.yield_from_item_type_mismatch",
            Self::AmbiguousBareNamePattern { .. } => "valkyrie.hir.ambiguous_bare_name_pattern",
            Self::RefutableLetPattern { .. } => "valkyrie.hir.refutable_let_pattern",
            Self::LetInitializerTypeMismatch { .. } => "valkyrie.hir.let_initializer_type_mismatch",
            Self::CollectionLiteralContractMissing { .. } => "valkyrie.hir.collection_literal_contract_missing",
            Self::AssignToImmutable { .. } => "valkyrie.hir.assign_to_immutable",
            Self::SemanticCheckViolations { .. } => "valkyrie.hir.semantic_check_violations",
            Self::CopyDisciplineViolations { .. } => "valkyrie.hir.copy_discipline_violations",
            Self::DuplicateTypeDefinition { .. } => "valkyrie.hir.duplicate_type_definition",
            Self::SourceGroupProducerMissing { .. } => "valkyrie.hir.source_group_producer_missing",
            Self::SourceGroupNotEntry { .. } => "valkyrie.hir.source_group_not_entry",
            Self::SourceGroupIdentityInvalid { .. } => "valkyrie.hir.source_group_identity_invalid",
            Self::SourceGroupDependencyUnavailable { .. } => "valkyrie.hir.source_group_dependency_unavailable",
            Self::SourceFileIdentityInvalid { .. } => "valkyrie.hir.source_file_identity_invalid",
            Self::EntryGroupWithoutFiles { .. } => "valkyrie.hir.entry_group_without_files",
            Self::SourceInputEntryGroupMissing { .. } => "valkyrie.hir.source_input_entry_group_missing",
            Self::SourceReadFailed { .. } => "valkyrie.hir.source_read_failed",
            Self::EmptySemanticSourceGroupPlan { .. } => "valkyrie.hir.empty_semantic_source_group_plan",
            Self::SourceFileRejected { .. } => "valkyrie.hir.source_file_rejected",
            Self::DependencyExportUnavailable { .. } => "valkyrie.hir.dependency_export_unavailable",
            Self::DependencyExportIdentityCollision { .. } => "valkyrie.hir.dependency_export_identity_collision",
            Self::DeclarationIndexExhausted { .. } => "valkyrie.hir.declaration_index_exhausted",
            Self::DeclarationIdentityDuplicated { .. } => "valkyrie.hir.declaration_identity_duplicated",
            Self::DependencyDeclarationsRequireSourceClosure { .. } => "valkyrie.hir.dependency_declarations_require_source_closure",
            Self::UnionNotLowered { .. } => "valkyrie.hir.union_not_lowered",
            Self::UnexpandedTemplateItem { .. } => "valkyrie.hir.unexpanded_template_item",
            Self::UnsupportedRootItem { .. } => "valkyrie.hir.unsupported_root_item",
            Self::PipelineRejected { .. } => "valkyrie.hir.pipeline_rejected",
            Self::LegacyBuiltinTypeAlias { .. } => "valkyrie.hir.legacy_builtin_type_alias",
            Self::AmbiguousTextTypeName { .. } => "valkyrie.hir.ambiguous_text_type_name",
            Self::TypeNotFound { .. } => "valkyrie.hir.type_not_found",
            Self::VxUnsupported => "valkyrie.hir.vx_unsupported",
            Self::SourceGroupClosureFailed(_) => "valkyrie.hir.source_group_closure_failed",
            Self::DeriveResolutionFailed { .. } => "valkyrie.hir.derive_resolution_failed",
            Self::BackendContractRejected(_) => "backend.contract.rejected",
        }
    }
}

use nyar_types::{DiagnosticArg, DiagnosticOwner, DiagnosticPhase, DiagnosticRow, DiagnosticSeverity, DiagnosticSource, DiagnosticSpan};

use super::model::*;

impl From<HirError> for DiagnosticRow {
    fn from(error: HirError) -> Self {
        // MIR 校验阶段自己生产 code / source / span / args，这里整体转发，不重新解释。
        if let HirError::MirValidationRejected(error) = error {
            return DiagnosticRow::from(error);
        }
        let source = match &error {
            HirError::VxUnsupported => DiagnosticSource { id: ".vx".into(), revision: None },
            other => other_source(other),
        };
        let span = match &error {
            HirError::VxUnsupported => DiagnosticSpan { start: 0, end: 0 },
            other => other_span(other),
        };

        let (phase, owner) = match &error {
            HirError::BackendContractRejected(_) => (DiagnosticPhase::Backend, DiagnosticOwner::backend("backend")),
            _ => (DiagnosticPhase::Hir, DiagnosticOwner::frontend("rusty-valkyrie")),
        };

        let mut row = DiagnosticRow::new(error.code(), DiagnosticSeverity::Error, phase, owner, source, span);

        match &error {
            HirError::UnresolvedCall { selector, context, .. } => {
                row = row
                    .with_arg("selector", DiagnosticArg::String(selector.clone()))
                    .with_arg("context", DiagnosticArg::String(context.clone()));
            }
            HirError::UnresolvedConstructor { name, context, .. } => {
                row = row.with_arg("name", DiagnosticArg::Aggregate(name.clone())).with_arg("context", DiagnosticArg::String(context.clone()));
            }
            HirError::NominalAmbiguous { name, candidate_count, .. } => {
                row = row
                    .with_arg("name", DiagnosticArg::TypeName(name.clone()))
                    .with_arg("candidate_count", DiagnosticArg::Int(*candidate_count as i64));
            }
            HirError::SumUniteAmbiguous { name, candidate_count, .. } => {
                row = row
                    .with_arg("name", DiagnosticArg::Aggregate(name.clone()))
                    .with_arg("candidate_count", DiagnosticArg::Int(*candidate_count as i64));
            }
            HirError::StructureMissingConstructor { qualified_name, .. } => {
                row = row.with_arg("aggregate", DiagnosticArg::Aggregate(qualified_name.clone()));
            }
            HirError::SumUniteMissingDeclaration { name, .. } => {
                row = row.with_arg("aggregate", DiagnosticArg::Aggregate(name.clone()));
            }
            HirError::InvalidDiscriminator { aggregate, failure, variant, value, .. } => {
                row = row
                    .with_arg("aggregate", DiagnosticArg::Aggregate(aggregate.clone()))
                    .with_arg("scenario", DiagnosticArg::String(failure.as_str().to_string()));
                if let Some(v) = variant {
                    row = row.with_arg("variant", DiagnosticArg::CallableShort(v.clone()));
                }
                if let Some(v) = value {
                    row = row.with_arg("value", DiagnosticArg::String(v.clone()));
                }
            }
            HirError::NoBodySourceInGroup { dependency_key, .. } => {
                row = row.with_arg("dependency_key", DiagnosticArg::DependencyKey(dependency_key.clone()));
            }
            HirError::MirValidationRejected(_) => {}
            HirError::FrontendRejected(error) => {
                row = row.with_cause(error.to_string());
            }
            HirError::TemplateExpansionFailed { construct, failure, .. } => {
                row = row
                    .with_arg("construct", DiagnosticArg::String(template_construct_name(*construct).to_string()))
                    .with_arg("failure", DiagnosticArg::String(template_failure_name(failure).to_string()));
                if let TemplateFailure::LoopRangeInvalid { start, end } = failure {
                    row = row.with_arg("range_start", DiagnosticArg::Int(*start)).with_arg("range_end", DiagnosticArg::Int(*end));
                }
                if let TemplateFailure::BlockFragmentUnsupportedNode { kind } = failure {
                    row = row.with_arg("node_kind", DiagnosticArg::String(kind.clone()));
                }
                if let TemplateFailure::FragmentNotMicroDeclaration { produced_items } = failure {
                    row = row.with_arg("produced_items", DiagnosticArg::Int(*produced_items as i64));
                }
            }
            HirError::MatchRecoveryFailed { stage, pattern, .. } => {
                row = row.with_arg("stage", DiagnosticArg::String(match_recovery_stage_name(*stage).to_string()));
                if let Some(pattern) = pattern {
                    row = row.with_arg("pattern", DiagnosticArg::String(pattern.clone()));
                }
            }
            HirError::ControlFlowContextViolation { operation, required, .. } => {
                row = row
                    .with_arg("operation", DiagnosticArg::String(control_flow_op_name(*operation).to_string()))
                    .with_arg("required", DiagnosticArg::String(control_flow_context_name(*required).to_string()));
            }
            HirError::FallthroughBindingLeak { name, .. } => {
                row = row.with_arg("name", DiagnosticArg::String(name.clone()));
            }
            HirError::LoopTargetMissing { operation, carries_value, .. } => {
                row = row
                    .with_arg("operation", DiagnosticArg::String(loop_jump_operation_name(*operation).to_string()))
                    .with_arg("carries_value", DiagnosticArg::String(carries_value.to_string()));
            }
            HirError::BreakValueTypeMismatch { actual, expected, .. } => {
                row =
                    row.with_arg("actual", DiagnosticArg::String(actual.clone())).with_arg("expected", DiagnosticArg::String(expected.clone()));
            }
            HirError::TryPropagateOperandUnsupported { operand_type, .. } => {
                row = row.with_arg("operand_type", DiagnosticArg::String(operand_type.clone()));
            }
            HirError::TryPropagateReturnMismatch { protocol, return_type, .. } => {
                row = row
                    .with_arg("protocol", DiagnosticArg::String(propagate_protocol_name(*protocol).to_string()))
                    .with_arg("return_type", DiagnosticArg::String(return_type.clone()));
            }
            HirError::TryPropagateErrorTypeMismatch { error_type, return_error, .. } => {
                row = row
                    .with_arg("error_type", DiagnosticArg::TypeName(error_type.clone()))
                    .with_arg("return_error", DiagnosticArg::TypeName(return_error.clone()));
            }
            HirError::FutureControlOperandMismatch { operation, operand_type, .. } => {
                row = row
                    .with_arg("operation", DiagnosticArg::String(control_flow_op_name(*operation).to_string()))
                    .with_arg("operand_type", DiagnosticArg::String(operand_type.clone()));
            }
            HirError::ControlFlowValueRequired { operation, expected_type, .. } => {
                row = row
                    .with_arg("operation", DiagnosticArg::String(control_flow_op_name(*operation).to_string()))
                    .with_arg("expected_type", DiagnosticArg::String(expected_type.clone()));
            }
            HirError::ReturnValueTypeMismatch { actual, expected, function, .. } => {
                row =
                    row.with_arg("actual", DiagnosticArg::String(actual.clone())).with_arg("expected", DiagnosticArg::String(expected.clone()));
                if let Some(function) = function {
                    row = row.with_arg("function", DiagnosticArg::CallableShort(function.clone()));
                }
            }
            HirError::YieldValueTypeMismatch { actual, expected_return, item_type, function, .. } => {
                row = row
                    .with_arg("actual", DiagnosticArg::String(actual.clone()))
                    .with_arg("expected_return", DiagnosticArg::String(expected_return.clone()))
                    .with_arg("item_type", DiagnosticArg::String(item_type.clone()));
                if let Some(function) = function {
                    row = row.with_arg("function", DiagnosticArg::CallableShort(function.clone()));
                }
            }
            HirError::YieldFromOperandNotGenerator { operand_type, .. } => {
                row = row.with_arg("operand_type", DiagnosticArg::String(operand_type.clone()));
            }
            HirError::YieldFromItemTypeMismatch { source_item, expected_return, expected_item, .. } => {
                row = row
                    .with_arg("source_item", DiagnosticArg::String(source_item.clone()))
                    .with_arg("expected_return", DiagnosticArg::String(expected_return.clone()))
                    .with_arg("expected_item", DiagnosticArg::String(expected_item.clone()));
            }
            HirError::AmbiguousBareNamePattern { name, .. } => {
                row = row.with_arg("name", DiagnosticArg::String(name.clone()));
            }
            HirError::RefutableLetPattern { pattern, .. } => {
                row = row.with_arg("pattern", DiagnosticArg::String(pattern.clone()));
            }
            HirError::SemanticCheckViolations { checks, .. } => {
                row = row.with_arg("checker_count", DiagnosticArg::Int(checks.len() as i64));
                for (index, check) in checks.iter().enumerate() {
                    row = row.with_arg(format!("checker_{index}"), DiagnosticArg::String(semantic_checker_name(check.checker).to_string()));
                }
            }
            HirError::CopyDisciplineViolations { violations, .. } => {
                row = row.with_arg("violation_count", DiagnosticArg::Int(violations.len() as i64));
            }
            HirError::DuplicateTypeDefinition { name, namespace, prior_kind, current_kind, .. } => {
                row = row
                    .with_arg("name", DiagnosticArg::String(name.clone()))
                    .with_arg("namespace", DiagnosticArg::String(namespace.clone()))
                    .with_arg("prior_kind", DiagnosticArg::String(prior_kind.clone()))
                    .with_arg("current_kind", DiagnosticArg::String(current_kind.clone()));
            }
            HirError::SourceGroupProducerMissing { dependency_key, .. } => {
                if let Some(dependency_key) = dependency_key {
                    row = row.with_arg("dependency_key", DiagnosticArg::DependencyKey(dependency_key.clone()));
                }
            }
            HirError::SourceGroupNotEntry { dependency_key, .. } => {
                row = row.with_arg("dependency_key", DiagnosticArg::DependencyKey(dependency_key.clone()));
            }
            HirError::SourceGroupIdentityInvalid { dependency_key, empty, .. } => {
                row = row
                    .with_arg("dependency_key", DiagnosticArg::DependencyKey(dependency_key.clone()))
                    .with_arg("empty", DiagnosticArg::String(empty.to_string()));
            }
            HirError::SourceGroupDependencyUnavailable { group, dependency, .. } => {
                row = row
                    .with_arg("group", DiagnosticArg::DependencyKey(group.clone()))
                    .with_arg("dependency", DiagnosticArg::DependencyKey(dependency.clone()));
            }
            HirError::SourceFileIdentityInvalid { group, path, empty, .. } => {
                row = row
                    .with_arg("group", DiagnosticArg::DependencyKey(group.clone()))
                    .with_arg("path", DiagnosticArg::String(path.clone()))
                    .with_arg("empty", DiagnosticArg::String(empty.to_string()));
            }
            HirError::EntryGroupWithoutFiles { group, .. } => {
                row = row.with_arg("group", DiagnosticArg::DependencyKey(group.clone()));
            }
            HirError::SourceInputEntryGroupMissing { explicit, .. } => {
                row = row.with_arg("explicit", DiagnosticArg::String(explicit.to_string()));
            }
            HirError::SourceReadFailed { path, kind, .. } => {
                row = row
                    .with_arg("path", DiagnosticArg::String(path.clone()))
                    .with_arg("kind", DiagnosticArg::String(io_error_kind_name(*kind).to_string()));
            }
            HirError::EmptySemanticSourceGroupPlan { .. } | HirError::UnexpandedTemplateItem { .. } => {}
            HirError::SourceFileRejected { dependency_key, path, error } => {
                row = row
                    .with_arg("dependency_key", DiagnosticArg::DependencyKey(dependency_key.clone()))
                    .with_arg("path", DiagnosticArg::String(path.clone()))
                    .with_cause(error.to_string());
            }
            HirError::DependencyExportUnavailable { group, dependency, .. } => {
                row = row
                    .with_arg("group", DiagnosticArg::DependencyKey(group.clone()))
                    .with_arg("dependency", DiagnosticArg::DependencyKey(dependency.clone()));
            }
            HirError::DependencyExportIdentityCollision { dependency_key, .. } => {
                row = row.with_arg("dependency_key", DiagnosticArg::DependencyKey(dependency_key.clone()));
            }
            HirError::DeclarationIndexExhausted { space, owner, .. } => {
                row = row
                    .with_arg("space", DiagnosticArg::String(index_space_name(*space).to_string()))
                    .with_arg("owner", DiagnosticArg::CallableShort(owner.clone()));
            }
            HirError::DeclarationIdentityDuplicated { space, owner, .. } => {
                row = row
                    .with_arg("space", DiagnosticArg::String(index_space_name(*space).to_string()))
                    .with_arg("owner", DiagnosticArg::CallableShort(owner.clone()));
            }
            HirError::DependencyDeclarationsRequireSourceClosure { imported_export_count, .. } => {
                row = row.with_arg("imported_export_count", DiagnosticArg::Int(*imported_export_count as i64));
            }
            HirError::UnionNotLowered { union_name, .. } => {
                row = row.with_arg("union", DiagnosticArg::Aggregate(union_name.clone()));
            }
            HirError::UnsupportedRootItem { item_kind, .. } => {
                row = row.with_arg("item_kind", DiagnosticArg::String(item_kind.clone()));
            }
            HirError::PipelineRejected { codes, module } => {
                row =
                    row.with_arg("module", DiagnosticArg::String(module.clone())).with_arg("codes", DiagnosticArg::Aggregate(codes.join(",")));
            }
            HirError::FallthroughWithoutNextArm { .. } | HirError::FallthroughOutsideCaseChain { .. } | HirError::BreakValueRejected { .. } => {
            }
            HirError::LegacyBuiltinTypeAlias { alias, canonical, .. } => {
                row = row
                    .with_arg("alias", DiagnosticArg::TypeName(alias.clone()))
                    .with_arg("canonical", DiagnosticArg::TypeName(canonical.clone()));
            }
            HirError::AmbiguousTextTypeName { name, .. } => {
                row = row.with_arg("name", DiagnosticArg::TypeName(name.clone()));
            }
            HirError::TypeNotFound { name, .. } => {
                row = row.with_arg("name", DiagnosticArg::TypeName(name.clone()));
            }
            HirError::VxUnsupported => {}
            HirError::SourceGroupClosureFailed(e) => {
                row = row.with_arg("details", DiagnosticArg::String(e.to_string()));
            }
            HirError::DeriveResolutionFailed { errors } => {
                row = row.with_arg("count", DiagnosticArg::Int(errors.len() as i64));
                for (i, err) in errors.iter().enumerate() {
                    row = row.with_arg(&format!("derive_{i}"), DiagnosticArg::String(err.to_string()));
                }
            }
            HirError::BackendContractRejected(error) => {
                row = row.with_arg("details", DiagnosticArg::String(error.to_string()));
            }
        }

        row.with_internal_message(error.code().to_string())
    }
}

/// frontend 失败事实的缺省 source 身份。
pub(super) fn frontend_source(_error: &crate::compiler::syntax::oak::ParseError) -> DiagnosticSource {
    DiagnosticSource { id: "<frontend>".into(), revision: None }
}

/// 从 frontend `ParseError` 提取可定位的源码 span。
pub(super) fn frontend_span(error: &crate::compiler::syntax::oak::ParseError) -> DiagnosticSpan {
    match error {
        crate::compiler::syntax::oak::ParseError::Invalid { span: Some(range), .. } => DiagnosticSpan { start: range.start, end: range.end },
        _ => empty_span(),
    }
}

/// 模板构造的稳定短名（用于诊断参数）。
pub(super) fn template_construct_name(construct: TemplateConstruct) -> &'static str {
    match construct {
        TemplateConstruct::MetaExpression => "meta_expression",
        TemplateConstruct::Loop => "loop",
        TemplateConstruct::Match => "match",
        TemplateConstruct::BlockFragment => "block_fragment",
    }
}

/// 模板失败原因的稳定短名（用于诊断参数）。
pub(super) fn template_failure_name(failure: &TemplateFailure) -> &'static str {
    match failure {
        TemplateFailure::EmptyExpression => "empty_expression",
        TemplateFailure::TrailingTokens => "trailing_tokens",
        TemplateFailure::IncompleteExpression => "incomplete_expression",
        TemplateFailure::UnclosedParenthesis => "unclosed_parenthesis",
        TemplateFailure::UnevaluableToken => "unevaluable_token",
        TemplateFailure::IntegerLiteralUnparsable => "integer_literal_unparsable",
        TemplateFailure::IncompatibleComparisonOperands => "incompatible_comparison_operands",
        TemplateFailure::LoopHeaderTooShort => "loop_header_too_short",
        TemplateFailure::LoopBindingNotDiscard => "loop_binding_not_discard",
        TemplateFailure::LoopMissingInKeyword => "loop_missing_in_keyword",
        TemplateFailure::LoopHeaderTrailingTokens => "loop_header_trailing_tokens",
        TemplateFailure::LoopRangeMissing => "loop_range_missing",
        TemplateFailure::RangeShapeUnsupported => "range_shape_unsupported",
        TemplateFailure::RangeEndpointNotInteger => "range_endpoint_not_integer",
        TemplateFailure::LoopRangeInvalid { .. } => "loop_range_invalid",
        TemplateFailure::NonArchScrutinee => "non_arch_scrutinee",
        TemplateFailure::BlockFragmentUnsupportedNode { .. } => "block_fragment_unsupported_node",
        TemplateFailure::FragmentNotMicroDeclaration { .. } => "fragment_not_micro_declaration",
    }
}

/// 模板失败原因的人类可读描述（仅用于 `Display`）。
pub(super) fn template_failure_message(failure: &TemplateFailure) -> String {
    match failure {
        TemplateFailure::EmptyExpression => "meta expression is empty",
        TemplateFailure::TrailingTokens => "meta expression has unevaluable trailing tokens",
        TemplateFailure::IncompleteExpression => "meta expression ended before it is complete",
        TemplateFailure::UnclosedParenthesis => "meta expression parenthesis is not closed",
        TemplateFailure::UnevaluableToken => "meta expression contains an unevaluable token",
        TemplateFailure::IntegerLiteralUnparsable => "integer literal cannot be parsed as `i64`",
        TemplateFailure::IncompatibleComparisonOperands => "comparison operands have incompatible types",
        TemplateFailure::LoopHeaderTooShort => "`loop` header must be `_ in start..end`",
        TemplateFailure::LoopBindingNotDiscard => "`loop` only supports the discard binding `_`",
        TemplateFailure::LoopMissingInKeyword => "`loop` header must contain `in`",
        TemplateFailure::LoopHeaderTrailingTokens => "`loop` header has unevaluable syntax after the range",
        TemplateFailure::LoopRangeMissing => "`loop` is missing a range expression",
        TemplateFailure::RangeShapeUnsupported => "range endpoints must be integer literals",
        TemplateFailure::RangeEndpointNotInteger => "range endpoints must be integer literals",
        TemplateFailure::LoopRangeInvalid { start, end } => return format!("`loop` range `{start}..{end}` is invalid"),
        TemplateFailure::NonArchScrutinee => "`match` only supports the `arch` scrutinee",
        TemplateFailure::BlockFragmentUnsupportedNode { kind } => return format!("expanded `{kind}` cannot be kept inside a block"),
        TemplateFailure::FragmentNotMicroDeclaration { produced_items } => {
            return format!("expanded meta expression produced {produced_items} top-level items instead of one `micro` declaration");
        }
    }
    .to_string()
}

/// match 恢复阶段的稳定短名（用于诊断参数）。
pub(super) fn match_recovery_stage_name(stage: MatchRecoveryStage) -> &'static str {
    match stage {
        MatchRecoveryStage::BlockInterior => "block_interior",
        MatchRecoveryStage::Arms => "arms",
        MatchRecoveryStage::Pattern => "pattern",
        MatchRecoveryStage::ArmBody => "arm_body",
        MatchRecoveryStage::ArmBodyExpression => "arm_body_expression",
    }
}

/// 控制流操作的稳定短名（用于诊断参数）。
pub(super) fn control_flow_op_name(operation: ControlFlowOp) -> &'static str {
    match operation {
        ControlFlowOp::Await => "await",
        ControlFlowOp::Awake => "awake",
        ControlFlowOp::Block => "block",
        ControlFlowOp::Yield => "yield",
        ControlFlowOp::YieldFrom => "yield_from",
        ControlFlowOp::Resume => "resume",
        ControlFlowOp::TryPropagate => "try_propagate",
        ControlFlowOp::Fallthrough => "fallthrough",
        ControlFlowOp::Break => "break",
        ControlFlowOp::Continue => "continue",
        ControlFlowOp::Return => "return",
    }
}

/// 控制流上下文的稳定短名（用于诊断参数）。
pub(super) fn control_flow_context_name(context: RequiredContext) -> &'static str {
    match context {
        RequiredContext::CatchArmBody => "catch_arm_body",
        RequiredContext::OutsideGuard => "outside_guard",
        RequiredContext::AsyncScope => "async_scope",
        RequiredContext::BlockingScope => "blocking_scope",
        RequiredContext::GeneratorScope => "generator_scope",
    }
}

/// loop 跳转操作的稳定短名（用于诊断参数）。
pub(super) fn loop_jump_operation_name(operation: LoopJumpOperation) -> &'static str {
    match operation {
        LoopJumpOperation::Break => "break",
        LoopJumpOperation::Continue => "continue",
    }
}

/// `?` 传播协议的稳定短名（用于诊断参数）。
pub(super) fn propagate_protocol_name(protocol: PropagateProtocol) -> &'static str {
    match protocol {
        PropagateProtocol::Nullable => "nullable",
        PropagateProtocol::Option => "option",
        PropagateProtocol::Result => "result",
    }
}

/// IO 失败种类的稳定短名（用于诊断参数）。
pub(super) fn io_error_kind_name(kind: std::io::ErrorKind) -> &'static str {
    match kind {
        std::io::ErrorKind::NotFound => "not_found",
        std::io::ErrorKind::PermissionDenied => "permission_denied",
        std::io::ErrorKind::InvalidData => "invalid_data",
        std::io::ErrorKind::InvalidInput => "invalid_input",
        _ => "other",
    }
}

/// 声明索引空间的稳定短名（用于诊断参数）。
pub(super) fn index_space_name(space: IndexSpace) -> &'static str {
    match space {
        IndexSpace::Item => "item",
        IndexSpace::ItemInstance => "item_instance",
        IndexSpace::Field => "field",
    }
}

pub(super) fn semantic_checker_name(checker: SemanticChecker) -> &'static str {
    match checker {
        SemanticChecker::ValueType => "value_type",
        SemanticChecker::SealedMatch => "sealed_match",
        SemanticChecker::LiteralExhaustiveness => "literal_exhaustiveness",
        SemanticChecker::UnreachableArm => "unreachable_arm",
        SemanticChecker::CompositePatternExhaustiveness => "composite_pattern_exhaustiveness",
        SemanticChecker::Singleton => "singleton",
    }
}

pub(super) fn copy_discipline_reason_name(reason: CopyDisciplineReason) -> &'static str {
    match reason {
        CopyDisciplineReason::ReferenceTypedField => "value type has a reference-typed field",
        CopyDisciplineReason::ParameterReferenceSemantics => "value-type parameter uses reference semantics",
        CopyDisciplineReason::ParameterNonCopyValueType => "parameter uses a value type with reference fields",
        CopyDisciplineReason::ReturnReferenceSemantics => "value-type return uses reference semantics",
        CopyDisciplineReason::ReturnNonCopyValueType => "return uses a value type with reference fields",
    }
}

/// 无源码定位时的空 span。
pub(super) fn empty_span() -> DiagnosticSpan {
    DiagnosticSpan { start: 0, end: 0 }
}

pub(super) fn other_source(error: &HirError) -> DiagnosticSource {
    match error {
        HirError::UnresolvedCall { source, .. } => source.clone(),
        HirError::UnresolvedConstructor { source, .. } => source.clone(),
        HirError::NominalAmbiguous { source, .. } => source.clone(),
        HirError::SumUniteAmbiguous { source, .. } => source.clone(),
        HirError::StructureMissingConstructor { source, .. } => source.clone(),
        HirError::SumUniteMissingDeclaration { source, .. } => source.clone(),
        HirError::InvalidDiscriminator { source, .. } => source.clone(),
        HirError::NoBodySourceInGroup { source, .. } => source.clone(),
        HirError::MirValidationRejected(_) => unreachable!(),
        HirError::FrontendRejected(error) => frontend_source(error),
        HirError::TemplateExpansionFailed { source, .. } => source.clone(),
        HirError::MatchRecoveryFailed { source, .. } => source.clone(),
        HirError::ControlFlowContextViolation { source, .. } => source.clone(),
        HirError::FallthroughWithoutNextArm { source, .. } => source.clone(),
        HirError::FallthroughOutsideCaseChain { source, .. } => source.clone(),
        HirError::FallthroughBindingLeak { source, .. } => source.clone(),
        HirError::LoopTargetMissing { source, .. } => source.clone(),
        HirError::BreakValueRejected { source, .. } => source.clone(),
        HirError::BreakValueTypeMismatch { source, .. } => source.clone(),
        HirError::TryPropagateOperandUnsupported { source, .. } => source.clone(),
        HirError::TryPropagateReturnMismatch { source, .. } => source.clone(),
        HirError::FutureControlOperandMismatch { source, .. } => source.clone(),
        HirError::ControlFlowValueRequired { source, .. } => source.clone(),
        HirError::ReturnValueTypeMismatch { source, .. } => source.clone(),
        HirError::YieldValueTypeMismatch { source, .. } => source.clone(),
        HirError::YieldFromOperandNotGenerator { source, .. } => source.clone(),
        HirError::YieldFromItemTypeMismatch { source, .. } => source.clone(),
        HirError::AmbiguousBareNamePattern { source, .. } => source.clone(),
        HirError::RefutableLetPattern { source, .. } => source.clone(),
        HirError::SemanticCheckViolations { source, .. } => source.clone(),
        HirError::CopyDisciplineViolations { source, .. } => source.clone(),
        HirError::DuplicateTypeDefinition { source, .. } => source.clone(),
        HirError::TryPropagateErrorTypeMismatch { source, .. } => source.clone(),
        HirError::BackendContractRejected(_) => DiagnosticSource { id: "<backend_contract>".into(), revision: None },
        HirError::SourceGroupProducerMissing { source, .. } => source.clone(),
        HirError::SourceGroupNotEntry { source, .. } => source.clone(),
        HirError::SourceGroupIdentityInvalid { source, .. } => source.clone(),
        HirError::SourceGroupDependencyUnavailable { source, .. } => source.clone(),
        HirError::SourceFileIdentityInvalid { source, .. } => source.clone(),
        HirError::EntryGroupWithoutFiles { source, .. } => source.clone(),
        HirError::SourceInputEntryGroupMissing { source, .. } => source.clone(),
        HirError::SourceReadFailed { source, .. } => source.clone(),
        HirError::EmptySemanticSourceGroupPlan { source, .. } => source.clone(),
        HirError::SourceFileRejected { error, .. } => frontend_source(error),
        HirError::DependencyExportUnavailable { source, .. } => source.clone(),
        HirError::DependencyExportIdentityCollision { source, .. } => source.clone(),
        HirError::DeclarationIndexExhausted { source, .. } => source.clone(),
        HirError::DeclarationIdentityDuplicated { source, .. } => source.clone(),
        HirError::DependencyDeclarationsRequireSourceClosure { source, .. } => source.clone(),
        HirError::UnionNotLowered { source, .. } => source.clone(),
        HirError::UnexpandedTemplateItem { source, .. } => source.clone(),
        HirError::UnsupportedRootItem { source, .. } => source.clone(),
        HirError::PipelineRejected { .. } => DiagnosticSource { id: "<pipeline>".into(), revision: None },
        HirError::LegacyBuiltinTypeAlias { source, .. } | HirError::AmbiguousTextTypeName { source, .. } => source.clone(),
        HirError::TypeNotFound { source, .. } => source.clone(),
        HirError::VxUnsupported => unreachable!(),
        HirError::SourceGroupClosureFailed(_) => unreachable!(),
        HirError::DeriveResolutionFailed { .. } => unreachable!(),
    }
}

pub(super) fn other_span(error: &HirError) -> DiagnosticSpan {
    match error {
        HirError::UnresolvedCall { span, .. } => span.clone(),
        HirError::UnresolvedConstructor { span, .. } => span.clone(),
        HirError::NominalAmbiguous { span, .. } => span.clone(),
        HirError::SumUniteAmbiguous { span, .. } => span.clone(),
        HirError::StructureMissingConstructor { span, .. } => span.clone(),
        HirError::SumUniteMissingDeclaration { span, .. } => span.clone(),
        HirError::InvalidDiscriminator { span, .. } => span.clone(),
        HirError::NoBodySourceInGroup { span, .. } => span.clone(),
        HirError::MirValidationRejected(_) => unreachable!(),
        HirError::FrontendRejected(error) => frontend_span(error),
        HirError::TemplateExpansionFailed { span, .. } => span.clone(),
        HirError::MatchRecoveryFailed { span, .. } => span.clone(),
        HirError::ControlFlowContextViolation { span, .. } => span.clone(),
        HirError::FallthroughWithoutNextArm { span, .. } => span.clone(),
        HirError::FallthroughOutsideCaseChain { span, .. } => span.clone(),
        HirError::FallthroughBindingLeak { span, .. } => span.clone(),
        HirError::LoopTargetMissing { span, .. } => span.clone(),
        HirError::BreakValueRejected { span, .. } => span.clone(),
        HirError::BreakValueTypeMismatch { span, .. } => span.clone(),
        HirError::TryPropagateOperandUnsupported { span, .. } => span.clone(),
        HirError::TryPropagateReturnMismatch { span, .. } => span.clone(),
        HirError::FutureControlOperandMismatch { span, .. } => span.clone(),
        HirError::ControlFlowValueRequired { span, .. } => span.clone(),
        HirError::ReturnValueTypeMismatch { span, .. } => span.clone(),
        HirError::YieldValueTypeMismatch { span, .. } => span.clone(),
        HirError::YieldFromOperandNotGenerator { span, .. } => span.clone(),
        HirError::YieldFromItemTypeMismatch { span, .. } => span.clone(),
        HirError::AmbiguousBareNamePattern { span, .. } => span.clone(),
        HirError::RefutableLetPattern { span, .. } => span.clone(),
        HirError::SemanticCheckViolations { span, .. } => span.clone(),
        HirError::CopyDisciplineViolations { span, .. } => span.clone(),
        HirError::DuplicateTypeDefinition { span, .. } => span.clone(),
        HirError::TryPropagateErrorTypeMismatch { span, .. } => span.clone(),
        HirError::BackendContractRejected(error) => error.span(),
        HirError::SourceGroupProducerMissing { span, .. } => span.clone(),
        HirError::SourceGroupNotEntry { span, .. } => span.clone(),
        HirError::SourceGroupIdentityInvalid { span, .. } => span.clone(),
        HirError::SourceGroupDependencyUnavailable { span, .. } => span.clone(),
        HirError::SourceFileIdentityInvalid { span, .. } => span.clone(),
        HirError::EntryGroupWithoutFiles { span, .. } => span.clone(),
        HirError::SourceInputEntryGroupMissing { span, .. } => span.clone(),
        HirError::SourceReadFailed { span, .. } => span.clone(),
        HirError::EmptySemanticSourceGroupPlan { span, .. } => span.clone(),
        HirError::SourceFileRejected { error, .. } => frontend_span(error),
        HirError::DependencyExportUnavailable { span, .. } => span.clone(),
        HirError::DependencyExportIdentityCollision { span, .. } => span.clone(),
        HirError::DeclarationIndexExhausted { span, .. } => span.clone(),
        HirError::DeclarationIdentityDuplicated { span, .. } => span.clone(),
        HirError::DependencyDeclarationsRequireSourceClosure { span, .. } => span.clone(),
        HirError::UnionNotLowered { span, .. } => span.clone(),
        HirError::UnexpandedTemplateItem { span, .. } => span.clone(),
        HirError::UnsupportedRootItem { span, .. } => span.clone(),
        HirError::PipelineRejected { .. } => empty_span(),
        HirError::LegacyBuiltinTypeAlias { span, .. } | HirError::AmbiguousTextTypeName { span, .. } => span.clone(),
        HirError::TypeNotFound { span, .. } => span.clone(),
        HirError::VxUnsupported => unreachable!(),
        HirError::SourceGroupClosureFailed(_) => unreachable!(),
        HirError::DeriveResolutionFailed { .. } => unreachable!(),
    }
}

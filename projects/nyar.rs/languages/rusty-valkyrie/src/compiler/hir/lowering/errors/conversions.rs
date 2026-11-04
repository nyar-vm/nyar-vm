use super::{
    diagnostic_row::{
        control_flow_context_name, control_flow_op_name, copy_discipline_reason_name, index_space_name, io_error_kind_name,
        loop_jump_operation_name, match_recovery_stage_name, propagate_protocol_name, semantic_checker_name, template_construct_name,
        template_failure_message,
    },
    model::{DiscriminatorFailure, HirError},
};
impl From<crate::compiler::syntax::oak::ParseError> for HirError {
    fn from(error: crate::compiler::syntax::oak::ParseError) -> Self {
        Self::FrontendRejected(error)
    }
}

impl From<crate::compiler::semantic::closure::SemanticClosureError> for HirError {
    fn from(error: crate::compiler::semantic::closure::SemanticClosureError) -> Self {
        Self::SourceGroupClosureFailed(error)
    }
}

impl From<crate::compiler::mir::validation::MirValidationError> for HirError {
    fn from(error: crate::compiler::mir::validation::MirValidationError) -> Self {
        Self::MirValidationRejected(error)
    }
}

impl std::error::Error for HirError {}

impl std::fmt::Display for HirError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnresolvedCall { selector, context, reason, source, span } => {
                write!(f, "unresolved call `{selector}` in {context} at {}:{}..{}: {}", source.id, span.start, span.end, reason.describe())
            }
            Self::UnresolvedConstructor { name, context, source, span } => {
                write!(f, "unresolved constructor `{name}` in {context} at {}:{}..{}", source.id, span.start, span.end)
            }
            Self::NominalAmbiguous { name, candidate_count, .. } => {
                write!(f, "nominal `{name}` is ambiguous ({candidate_count} candidates)")
            }
            Self::SumUniteAmbiguous { name, candidate_count, .. } => {
                write!(f, "sum/unite `{name}` is ambiguous ({candidate_count} candidates)")
            }
            Self::ArrayOwnerAmbiguous { candidate_count, .. } => {
                write!(f, "builtin array owner `[T]` is ambiguous ({candidate_count} candidates)")
            }
            Self::StructureMissingConstructor { qualified_name, .. } => {
                write!(f, "structure `{qualified_name}` missing constructor_declaration")
            }
            Self::SumUniteMissingDeclaration { name, .. } => {
                write!(f, "sum/unite `{name}` missing declaration")
            }
            Self::InvalidDiscriminator { aggregate, failure, variant, value, .. } => {
                match failure {
                    DiscriminatorFailure::NotNonNegativeInteger => {
                        write!(f, "discriminator in `{aggregate}` must be a non-negative integer literal")?;
                    }
                    DiscriminatorFailure::DuplicateTag => write!(f, "duplicate discriminator in `{aggregate}`")?,
                }
                if let Some(v) = variant {
                    write!(f, " variant={v}")?;
                }
                if let Some(v) = value {
                    write!(f, " value={v}")?;
                }
                Ok(())
            }
            Self::NoBodySourceInGroup { dependency_key, .. } => {
                write!(f, "no body source in group `{dependency_key}`")
            }
            Self::MirValidationRejected(error) => write!(f, "{error}"),
            Self::FrontendRejected(error) => write!(f, "{error}"),
            Self::TemplateExpansionFailed { construct, failure, .. } => {
                write!(f, "TGrammar {} expansion failed: {}", template_construct_name(*construct), template_failure_message(failure))
            }
            Self::MatchRecoveryFailed { stage, pattern, .. } => {
                write!(f, "match recovery failed at stage `{}`", match_recovery_stage_name(*stage))?;
                if let Some(pattern) = pattern {
                    write!(f, " for pattern `{pattern}`")?;
                }
                Ok(())
            }
            Self::ControlFlowContextViolation { operation, required, .. } => {
                write!(f, "`{}` is only allowed in `{}`", control_flow_op_name(*operation), control_flow_context_name(*required))
            }
            Self::FallthroughWithoutNextArm { .. } => write!(f, "the last `case` arm cannot use `fallthrough`"),
            Self::FallthroughOutsideCaseChain { .. } => write!(f, "`fallthrough` is only allowed inside a `case` chain"),
            Self::FallthroughBindingLeak { name, .. } => {
                write!(f, "`fallthrough` does not inherit the previous arm pattern bindings, yet `{name}` is referenced")
            }
            Self::LoopTargetMissing { operation, carries_value, .. } => {
                write!(f, "`{}`", loop_jump_operation_name(*operation))?;
                if *carries_value {
                    write!(f, " with a value")?;
                }
                write!(f, " has no target loop")
            }
            Self::BreakValueRejected { .. } => write!(f, "the target loop does not accept a `break` value"),
            Self::BreakValueTypeMismatch { actual, expected, .. } => {
                write!(f, "`break` value type `{actual}` is incompatible with the target loop value type `{expected}`")
            }
            Self::TryPropagateOperandUnsupported { operand_type, .. } => {
                write!(f, "`?` cannot be applied to `{operand_type}`")
            }
            Self::TryPropagateReturnMismatch { protocol, return_type, .. } => {
                write!(
                    f,
                    "`?` on a `{}` operand requires a matching enclosing return type, got `{return_type}`",
                    propagate_protocol_name(*protocol)
                )
            }
            Self::TryPropagateErrorTypeMismatch { error_type, return_error, .. } => {
                write!(f, "`?` error type `{error_type}` is incompatible with enclosing return error `{return_error}`")
            }
            Self::FutureControlOperandMismatch { operation, operand_type, .. } => {
                write!(f, "`{}` requires a `Future<T>` or `Promise<T>` operand, got `{operand_type}`", control_flow_op_name(*operation))
            }
            Self::ControlFlowValueRequired { operation, expected_type, .. } => {
                write!(f, "`{}` requires a carried value of type `{expected_type}`", control_flow_op_name(*operation))
            }
            Self::ReturnValueTypeMismatch { actual, expected, function, .. } => {
                write!(f, "`return` value type `{actual}` is incompatible with the function return type `{expected}`")?;
                match function {
                    Some(function) => write!(f, " in function `{function}`"),
                    None => Ok(()),
                }
            }
            Self::YieldValueTypeMismatch { actual, expected_return, item_type, function, .. } => {
                write!(f, "`yield` value type `{actual}` is incompatible with the `{item_type}` item of return type `{expected_return}`")?;
                match function {
                    Some(function) => write!(f, " in function `{function}`"),
                    None => Ok(()),
                }
            }
            Self::YieldFromOperandNotGenerator { operand_type, .. } => {
                write!(f, "`yield from` operand type `{operand_type}` is not a generator protocol")
            }
            Self::YieldFromItemTypeMismatch { source_item, expected_return, expected_item, .. } => write!(
                f,
                "`yield from` item type `{source_item}` is incompatible with the `{expected_item}` item of return type `{expected_return}`"
            ),
            Self::AmbiguousBareNamePattern { name, .. } => {
                write!(f, "bare lowercase single-segment pattern `{name}` is ambiguous with a variable binding, use a variant form instead")
            }
            Self::RefutableLetPattern { pattern, .. } => {
                write!(f, "refutable pattern `{pattern}` is not allowed in a `let` binding, use `if let` or `case` instead")
            }
            Self::LetInitializerTypeMismatch { actual, expected, .. } => {
                write!(f, "`let` initializer type `{actual}` is incompatible with the declared type `{expected}`")
            }
            Self::CollectionLiteralContractMissing { type_label, .. } => {
                write!(f, "collection literal requires a `LiteralSequence` contract impl for the declared type `{type_label}`")
            }
            Self::AssignToImmutable { name, .. } => {
                write!(f, "cannot assign to immutable binding `{name}`, declare it with `let mut` to allow reassignment")
            }
            Self::SemanticCheckViolations { checks, .. } => {
                write!(f, "semantic checks rejected the module")?;
                for check in checks {
                    for message in &check.messages {
                        write!(f, "; {}: {}", semantic_checker_name(check.checker), message)?;
                    }
                }
                Ok(())
            }
            Self::CopyDisciplineViolations { violations, .. } => {
                write!(f, "copy discipline violation")?;
                for violation in violations {
                    write!(f, "; {} in `{}`", copy_discipline_reason_name(violation.reason), violation.owner)?;
                }
                Ok(())
            }
            Self::AutoConstructorAttributeOnNonUnityVariant { aggregate, variant, declared_kind, .. } => {
                write!(f, "`[constructor]` on `{aggregate}` variant `{variant}` requires a `unite`, but the aggregate is `{declared_kind}`")
            }
            Self::AutoConstructorAttributeOnNonVariant { target_kind, name, .. } => {
                write!(f, "`[constructor]` is only valid on `unite` variants, but found on {target_kind} `{name}`")
            }
            Self::DuplicateAutoConstructorAttribute { aggregate, variant, .. } => {
                write!(f, "duplicate `[constructor]` attribute on `{aggregate}` variant `{variant}`")
            }
            Self::DuplicateTypeDefinition { name, namespace, prior_kind, current_kind, .. } => {
                write!(f, "duplicate definition of `{name}` in namespace `{namespace}`: {prior_kind} and {current_kind}")
            }
            Self::ClassBodyMethodDeclared { aggregate, method, .. } => {
                write!(
                    f,
                    "泛型声明体 `{aggregate}` 的成员方法 `{method}` 暂不支持；声明体方法目前只覆盖单态 owner，泛型 owner 的实例闭合尚未接入"
                )
            }
            Self::SourceGroupProducerMissing { dependency_key, .. } => {
                write!(f, "generated source group has no producer identity")?;
                match dependency_key {
                    Some(dependency_key) => write!(f, " (group `{dependency_key}`)"),
                    None => Ok(()),
                }
            }
            Self::SourceGroupNotEntry { dependency_key, .. } => {
                write!(f, "generated source group `{dependency_key}` must be explicitly marked as entry")
            }
            Self::SourceGroupIdentityInvalid { dependency_key, empty, .. } => {
                if *empty {
                    write!(f, "Compiler 源码组身份为空")
                }
                else {
                    write!(f, "Compiler 源码组身份为空或重复: `{dependency_key}`")
                }
            }
            Self::SourceGroupDependencyUnavailable { group, dependency, .. } => {
                write!(f, "依赖 `{dependency}` 未在 `{group}` 之前提供")
            }
            Self::SourceFileIdentityInvalid { group, path, empty, .. } => {
                if *empty {
                    write!(f, "source group `{group}` contains a source file with an empty path")
                }
                else {
                    write!(f, "source group `{group}` contains duplicated source file `{path}`")
                }
            }
            Self::EntryGroupWithoutFiles { group, .. } => write!(f, "entry source group `{group}` declares no source file"),
            Self::SourceInputEntryGroupMissing { explicit, .. } => {
                if *explicit {
                    write!(f, "compiler source snapshot has no explicitly marked entry group")
                }
                else {
                    write!(f, "compiler source snapshot has no entry group")
                }
            }
            Self::SourceReadFailed { path, kind, .. } => {
                write!(f, "failed to read source file `{path}` ({})", io_error_kind_name(*kind))
            }
            Self::EmptySemanticSourceGroupPlan { .. } => write!(f, "semantic source group plan is empty"),
            Self::SourceFileRejected { dependency_key, path, error } => {
                write!(f, "source group `{dependency_key}` file `{path}` was rejected: {error}")
            }
            Self::PendingErrorsRejected { count, source, span } => {
                write!(
                    f,
                    "compile unit closed with {count} recoverable error diagnostic(s), first at {}:{}..{}",
                    source.id, span.start, span.end
                )
            }
            Self::DependencyExportUnavailable { group, dependency, .. } => {
                write!(f, "source group `{group}` requires the semantic export of `{dependency}`, which is unavailable")
            }
            Self::DependencyExportIdentityCollision { dependency_key, .. } => {
                write!(f, "dependency `{dependency_key}` registered its semantic export twice")
            }
            Self::DeclarationIndexExhausted { space, owner, .. } => {
                write!(f, "`{}` space is exhausted while registering `{owner}`", index_space_name(*space))
            }
            Self::DeclarationIdentityDuplicated { space, owner, .. } => {
                write!(f, "`{}` identity of `{owner}` was already registered", index_space_name(*space))
            }
            Self::DependencyDeclarationsRequireSourceClosure { imported_export_count, .. } => write!(
                f,
                "dependency declaration registration requires the full compiler source closure, got {imported_export_count} imported exports"
            ),
            Self::UnionNotLowered { union_name, .. } => write!(f, "named union `{union_name}` is not yet lowered into HIR"),
            Self::UnexpandedTemplateItem { .. } => write!(f, "TGrammar template item reached HIR lowering without expansion"),
            Self::UnsupportedRootItem { item_kind, .. } => {
                write!(f, "root item kind `{item_kind}` is not supported by HIR lowering")
            }
            Self::UnrecognizedImport { source, span } => {
                write!(f, "unrecognized import syntax at {}:{}..{}: the import declaration has no path", source.id, span.start, span.end)
            }
            Self::PrimaryImportOutsidePrimaryNamespace { source, span } => {
                write!(
                    f,
                    "`using!` at {}:{}..{} is outside `namespace!`: primary imports may only be declared in the project's primary namespace",
                    source.id, span.start, span.end
                )
            }
            Self::DuplicatePrimaryNamespace { namespaces, source, span } => {
                write!(
                    f,
                    "duplicate `namespace!` declaration at {}:{}..{}: a compilation project may declare exactly one primary namespace, found [{}]",
                    source.id,
                    span.start,
                    span.end,
                    namespaces.join(", ")
                )
            }
            Self::HirWireVersionRejected { found, expected, source, span } => {
                write!(
                    f,
                    "HIR wire version mismatch at {}:{}..{}: found `{found}`, expected `{expected}`; rebuild the artifact instead of reading a stale payload",
                    source.id, span.start, span.end
                )
            }
            Self::PipelineRejected { codes, module, messages } => {
                write!(f, "compile pipeline rejected module `{module}`")?;
                if !codes.is_empty() {
                    write!(f, " ({})", codes.join(","))?;
                }
                if let Some(message) = messages.first() {
                    write!(f, ": {message}")?;
                }
                Ok(())
            }
            Self::LegacyBuiltinTypeAlias { alias, canonical, .. } => {
                write!(f, "legacy builtin type alias `{alias}` has been removed; use `{canonical}` explicitly")
            }
            Self::AmbiguousTextTypeName { name, .. } => {
                write!(f, "ambiguous text type `{name}` is forbidden; use an explicit encoding such as `utf8`, `utf16`, or `c_str`")
            }
            Self::TypeNotFound { name, .. } => write!(f, "unknown type name `{name}`: not a module type alias or language builtin"),
            Self::ExpressionNestingExceeded { depth, limit, .. } => {
                write!(f, "expression nesting exceeds lowering guard: depth={depth} limit={limit}")
            }
            Self::GenericInstantiationLimitExceeded { kind, limit, actual, top_growth, .. } => {
                use super::model::GenericInstantiationLimitKind;
                let kind_label = match kind {
                    GenericInstantiationLimitKind::Instances => "instances",
                    GenericInstantiationLimitKind::Rounds => "fixpoint rounds",
                };
                write!(f, "generic instantiation limit exceeded: {kind_label} actual={actual} limit={limit}")?;
                if !top_growth.is_empty() {
                    write!(f, "; fastest growing declarations:")?;
                    for (name, count) in top_growth {
                        write!(f, " {name}×{count}")?;
                    }
                }
                Ok(())
            }
            Self::VxUnsupported => write!(f, ".vx source is not supported"),
            Self::SourceGroupClosureFailed(e) => write!(f, "source group closure failed: {e}"),
            Self::DeriveResolutionFailed { errors } => {
                write!(f, "derive injection failed: {} errors", errors.len())?;
                for err in errors {
                    write!(f, "; {err}")?;
                }
                Ok(())
            }
            Self::InvalidTupleLabeling { failure, .. } => {
                use super::model::TupleLabelingFailure;
                match failure {
                    TupleLabelingFailure::Mixed => {
                        write!(f, "tuple type mixes labeled and unlabeled elements; label all elements or none")
                    }
                    TupleLabelingFailure::DuplicateLabel { name } => {
                        write!(f, "tuple type repeats the label `{name}`")
                    }
                }
            }
            Self::InvalidArrayExtent { failure, .. } => {
                use super::model::ArrayExtentFailure;
                match failure {
                    ArrayExtentFailure::NotLiteral => write!(
                        f,
                        "fixed array extent must be an integer literal or an integer tuple literal"
                    ),
                    ArrayExtentFailure::LengthNotRepresentable => {
                        write!(f, "fixed array length exceeds the representable `usize` range")
                    }
                    ArrayExtentFailure::NegativeLength => write!(f, "fixed array extent must not be negative"),
                    ArrayExtentFailure::ZeroDimension => write!(f, "array shape dimensions must be greater than zero"),
                    ArrayExtentFailure::UnsupportedDimensionCount { count } => write!(
                        f,
                        "array shape has {count} dimensions; only 1, 2 or 3 dimensions are supported"
                    ),
                }
            }
            Self::BackendContractRejected(error) => write!(f, "backend contract rejected the declaration: {error}"),
            Self::IntegerLiteralOutOfRange { literal, magnitude, range, .. } => {
                write!(f, "integer literal `{literal}` (magnitude {magnitude}) is out of range `{range}`")
            }
            Self::InvalidSingleQuoteLiteral { failure, span, .. } => {
                use super::model::SingleQuoteLiteralFailure;
                match failure {
                    SingleQuoteLiteralFailure::UnknownPrefix { prefix } => {
                        write!(
                            f,
                            "unknown single-quote literal prefix `{prefix}` at {}..{} (only `r` and `b` are defined)",
                            span.start, span.end
                        )
                    }
                    SingleQuoteLiteralFailure::EmptyByteLiteral => {
                        write!(f, "empty byte literal `b''` at {}..{}", span.start, span.end)
                    }
                    SingleQuoteLiteralFailure::ByteLiteralNotSingleByte { count } => {
                        write!(f, "byte literal must decode to exactly one byte, got {count} at {}..{}", span.start, span.end)
                    }
                    SingleQuoteLiteralFailure::ByteLiteralNotAscii { character } => {
                        write!(
                            f,
                            "byte literal character `{character}` is not ASCII; write a `\\xNN` escape instead (at {}..{})",
                            span.start, span.end
                        )
                    }
                    SingleQuoteLiteralFailure::UnicodeEscapeInByteLiteral => {
                        write!(f, "byte literal does not accept `\\u{{...}}`; write `\\xNN` instead (at {}..{})", span.start, span.end)
                    }
                    SingleQuoteLiteralFailure::InvalidEscapeSequence { escape } => {
                        write!(f, "invalid escape sequence `{escape}` at {}..{}", span.start, span.end)
                    }
                    SingleQuoteLiteralFailure::InvalidUnicodeEscape { escape } => {
                        write!(f, "invalid unicode escape `{escape}` at {}..{}", span.start, span.end)
                    }
                    SingleQuoteLiteralFailure::DanglingEscape => {
                        write!(f, "dangling backslash at the end of the captured literal at {}..{}", span.start, span.end)
                    }
                    SingleQuoteLiteralFailure::CharacterBeyondBmp { value } => {
                        write!(
                            f,
                            "character literal U+{value:X} is beyond the BMP: `char` is a u16 code unit (at {}..{})",
                            span.start, span.end
                        )
                    }
                }
            }
        }
    }
}

use std::cell::RefCell;

use nyar_types::{DiagnosticSource, DiagnosticSpan};

use crate::compiler::{
    hir::lowering::errors::{DiscriminatorFailure, HirError, IndexSpace},
    types::{SourceSpan, hir::HirCompileWarning},
};

/// 源码组 / 索引注册等无源码定位失败使用的占位 source 身份。
pub(crate) fn plan_source() -> DiagnosticSource {
    DiagnosticSource { id: "<source_group_plan>".into(), revision: None }
}

/// 无源码定位失败使用的空 span。
pub(crate) fn plan_span() -> DiagnosticSpan {
    DiagnosticSpan { start: 0, end: 0 }
}

/// 声明身份索引空间耗尽。
pub(crate) fn index_exhausted(space: IndexSpace, owner: &str) -> HirError {
    HirError::DeclarationIndexExhausted { space, owner: owner.to_string(), source: plan_source(), span: plan_span() }
}

/// 声明身份被重复注册。
pub(crate) fn identity_duplicated(space: IndexSpace, owner: &str) -> HirError {
    HirError::DeclarationIdentityDuplicated { space, owner: owner.to_string(), source: plan_source(), span: plan_span() }
}

thread_local! {
    static COMPILE_WARNINGS: RefCell<Vec<HirCompileWarning>> = const { RefCell::new(Vec::new()) };
}

pub(crate) struct CompileWarningScope;

impl CompileWarningScope {
    pub(crate) fn enter() -> Self {
        COMPILE_WARNINGS.with(|warnings| warnings.borrow_mut().clear());
        Self
    }
}

pub(crate) fn push_compile_warning(code: &'static str, message: impl Into<String>, span: SourceSpan) {
    COMPILE_WARNINGS.with(|warnings| warnings.borrow_mut().push(HirCompileWarning { code: code.to_string(), message: message.into(), span }));
}

pub(crate) fn take_compile_warnings() -> Vec<HirCompileWarning> {
    COMPILE_WARNINGS.with(|warnings| std::mem::take(&mut *warnings.borrow_mut()))
}

/// 把下游 pipeline 的结构化诊断转发为 HIR facade 失败，不重新拼装失败文本。
pub(crate) fn pipeline_rejected(diagnostics: nyar_types::StructuredDiagnosticSet) -> HirError {
    HirError::PipelineRejected {
        codes: diagnostics.records.iter().map(|record| record.code.clone()).collect(),
        module: diagnostics.records.first().map(|record| record.module.clone()).unwrap_or_default(),
    }
}

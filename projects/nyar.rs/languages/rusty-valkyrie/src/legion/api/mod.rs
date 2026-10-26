//! Legion 程序化 API：plan / compile / session；CLI 与 N-API 应调用此模块。

mod compile;
mod diagnostic;
mod plan;
mod request;
mod session;

pub use compile::{LegionCompileReport, LegionStageStatus, compile, compile_project};
pub use diagnostic::{LegionDiagnostic, LegionSeverity, LegionStage};
pub use plan::{LegionBuildPlan, LegionHostProviderSummary, LegionSemanticGroupSummary, plan};
pub use request::{CompileOptions, PlanRequest};
pub use session::CompilerSession;

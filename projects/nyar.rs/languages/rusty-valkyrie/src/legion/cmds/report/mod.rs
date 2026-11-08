//! Legion 报告生成（AWSL SSG + asgard hydrate 图表岛）。

mod assets;
#[doc(hidden)]
pub mod codegen;
mod islands;
#[doc(hidden)]
pub mod models;
mod project;
#[doc(hidden)]
pub mod render;
#[doc(hidden)]
pub mod standalone;
#[doc(hidden)]
pub mod write;

pub use models::{BenchReport, BenchResultRow, CoverageFeatureEntry, CoverageReport, TestResultEntry};
pub use project::{legion_report_project_dir, report_roots};
pub use render::{render_bench_report, render_coverage_report, render_test_report};
pub use standalone::{build_standalone_html, finish_standalone_report};
pub use write::atomic_write_all_text;

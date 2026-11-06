#![doc = include_str!("readme.md")]

mod island;
mod literal;
mod spec;

pub use island::{IslandKind, awsl_island_kind};
pub use literal::{ReportLiteralError, series_to_init_literal};
pub use spec::{ColSeriesItem, HydratedChartSpec, StaticPageSpec};

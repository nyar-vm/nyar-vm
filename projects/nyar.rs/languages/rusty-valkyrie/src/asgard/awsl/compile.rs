//! AWSL single-file component unified compile entry.

use oak_awsl::{AwslParser, validate_component_contract};

use super::{LoweredComponent, LoweringOptions, lower_component};
use crate::asgard::error::{AsgardError, AsgardResult};

/// Parse, validate, and lower a single `.awsl` file into a `LoweredComponent`.
///
/// Contract: one file corresponds to one widget; `file_stem` is the sole source for route name and widget name.
/// The style contract (`<style>` contract DSL) is parsed here and validated fail-closed against the dom/html capability matrix.
pub fn compile_awsl_source(source: &str, file_stem: &str, source_path: &str, options: &LoweringOptions) -> AsgardResult<LoweredComponent> {
    let root = AwslParser::parse_root_with_options(source, options.strict_mode)
        .map_err(|error| AsgardError::msg(report!("AWSL parse failed {}: {}", source_path, error.message)))?;
    validate_component_contract(&root, file_stem).map_err(|error| AsgardError::msg(report!("AWSL contract validation failed {}: {}", source_path, error.message)))?;
    let component = lower_component(&root, file_stem, source_path, options).map_err(|error| error.report())?;
    if let Some(contract) = &component.style_contract {
        crate::asgard::style::web::validate_for_dom(contract)?;
    }
    // 度量拒绝在编译入口 fail-closed：任何后端降级都不得静默跳过。
    for binding in &component.layout {
        if let crate::asgard::layout::LayoutBinding::MeasureRejected { class, reason } = binding {
            return Err(AsgardError::msg(report!("布局合同度量被拒绝（{class}）：{reason}")));
        }
    }
    Ok(component)
}

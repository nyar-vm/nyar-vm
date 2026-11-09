//! Semantic MIR 与 MIR 的合同校验。
//!
//! 两个事实域各自独立：
//!
//! - `semantic_contract` / `symbols` / `module_validation` / `function_validation` 负责
//!   Semantic MIR 的 stable-ID、聚合、名义结构与静态调用合同。
//! - `control_flow` / `effect_validation` / `type_inference` / `diagnostics` 负责 MIR 的
//!   可达块、effect 恢复与静态类型校验。
//!
//! 本模块只拒绝不合法 IR，不创建、不修正 callable、layout 或类型等语义事实；
//! 根文件只声明子模块并重导出公共项。

mod control_flow;
mod diagnostics;
mod effect_validation;
mod errors;
mod function_validation;
mod module_validation;
mod semantic_contract;
mod type_inference;

pub use errors::MirValidationError;
pub use module_validation::validate_module;
pub use semantic_contract::{SemanticMirContractError, semantic_observation, validate_semantic_module};
// SSA lowering 与 MIR 校验共用同一文本 carrier 归一口径（`type utf8 = Utf8Text` 名义声明
// 与内建编码 carrier 是同一值空间），故对 `mir` 内部再导出，避免 lowering 另立判定。
pub(in crate::compiler::mir) use type_inference::text_carriers_equivalent;

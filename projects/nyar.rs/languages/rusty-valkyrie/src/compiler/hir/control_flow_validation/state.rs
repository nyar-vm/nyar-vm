use std::collections::{BTreeMap, BTreeSet};

use crate::compiler::{
    hir::{
        is_nullable_type, is_option_apply_type, is_result_apply_type,
        lowering::{
            HirError,
            errors::{ControlFlowOp, LoopJumpOperation, PropagateProtocol, RequiredContext},
        },
        nullable_payload_type,
    },
    semantic::control_flow::{ControlFlowContext, TryScopeData},
    types::{
        Identifier, NamePath,
        hir::{
            HirBlock, HirExpr, HirExprKind, HirFunction, HirLiteral, HirMatchArm, HirModule, HirPattern, HirStatement, HirStatementKind,
            ValkyrieType,
        },
    },
};

use nyar_types::{DiagnosticSource, DiagnosticSpan};

use super::{control_flow_source, expr_diagnostic_span};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HirValidationState {
    pub(crate) current_function: Option<String>,
    pub(crate) in_guard: bool,
    pub(crate) self_type_stack: Vec<ValkyrieType>,
    pub(crate) return_type_stack: Vec<ValkyrieType>,
    pub(crate) local_scopes: Vec<BTreeMap<String, ValkyrieType>>,
    /// Unified loop / try / case chain / catch / generator / async scope tracking.
    pub(crate) control_flow: ControlFlowContext,
    /// 本模块内已冻结为声明身份的数值 primitive（`core/primitive` 的 `structure`）。
    /// 只有这份声明事实能判定「期望类型是数值」；不得改成对调用点写的类型名做特判。
    pub(crate) numeric_declarations: BTreeSet<nyar_types::ItemId>,
    /// `core::primitive::bool` 的声明身份；字面量与注解 `bool` 的 `Boolean` 形态与之等价。
    pub(crate) boolean_declarations: BTreeSet<nyar_types::ItemId>,
}

pub type LoopValidationContext = crate::compiler::semantic::control_flow::LoopScopeData;

/// 模块内数值 primitive 的声明身份集合（含子模块与导入语义导出的 `structure`）。
///
/// 判定口径复用 `type_relation` 既有的数值名集合，喂进去的是**声明名**；
/// 调用点写的类型名不参与判定。
pub fn numeric_declarations_in_module(module: &HirModule) -> BTreeSet<nyar_types::ItemId> {
    fn collect(module: &HirModule, out: &mut BTreeSet<nyar_types::ItemId>) {
        let structs = module.structs.iter().chain(module.imported_semantic_exports.iter().flat_map(|export| export.structs.iter()));
        for item in structs {
            if crate::compiler::hir::type_relation::is_numeric_type_name_of(&ValkyrieType::Named(item.name.clone())) {
                if let Some(declaration) = item.constructor_declaration {
                    out.insert(declaration);
                }
            }
        }
        for submodule in &module.submodules {
            collect(submodule, out);
        }
    }
    let mut declarations = BTreeSet::new();
    collect(module, &mut declarations);
    declarations
}

/// 模块内 `bool` primitive 的声明身份集合。
pub fn boolean_declarations_in_module(module: &HirModule) -> BTreeSet<nyar_types::ItemId> {
    fn collect(module: &HirModule, out: &mut BTreeSet<nyar_types::ItemId>) {
        let structs = module.structs.iter().chain(module.imported_semantic_exports.iter().flat_map(|export| export.structs.iter()));
        for item in structs {
            if item.name.as_str() == "bool" {
                if let Some(declaration) = item.constructor_declaration {
                    out.insert(declaration);
                }
            }
        }
        for submodule in &module.submodules {
            collect(submodule, out);
        }
    }
    let mut declarations = BTreeSet::new();
    collect(module, &mut declarations);
    declarations
}

pub fn validation_state(
    numeric_declarations: &BTreeSet<nyar_types::ItemId>,
    boolean_declarations: &BTreeSet<nyar_types::ItemId>,
) -> HirValidationState {
    HirValidationState {
        numeric_declarations: numeric_declarations.clone(),
        boolean_declarations: boolean_declarations.clone(),
        ..Default::default()
    }
}

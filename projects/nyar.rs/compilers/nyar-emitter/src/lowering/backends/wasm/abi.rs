//! Wasm 私有库导出 ABI 载体合同。
//!
//! 载体是 **Wasm 物理投影**的冻结事实：它必须与 `nyar.library_invoke` 消费的
//! `WasmAbiType` 槽位一一对应，不是跨目标中立的“标量/句柄”分类。其它 lane
//! 拥有自己的载体投影与本 lane 合同，不得复用本表。
//!
//! 公共 [`crate::TargetExecutionPlan`] 只携带逻辑导出身份与签名；Wasm lane 在
//! 需要导出 ABI 时按函数签名现场冻结，不把本 lane 的载体缓存写回共享计划。

use nyar_types::{ExecutableFunction, ItemInstanceId, NyarType};

use crate::Result;

/// Wasm 库导出参数或返回值的物理载体。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum WasmAbiCarrier {
    /// `i32` 槽位。
    I32,
    /// `i64` 槽位。
    I64,
    /// `f32` 槽位。
    F32,
    /// `f64` 槽位。
    F64,
    /// 宿主句柄（数组 / Option / Tuple / 具名聚合）。
    Handle,
    /// 无返回值。
    Void,
}

impl WasmAbiCarrier {
    /// 返回载体名称（lowercase）。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::Handle => "handle",
            Self::Void => "void",
        }
    }
}

/// 一个库导出实例的冻结 Wasm ABI 载体合同。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WasmLibraryExportAbi {
    /// 按声明顺序排列的参数载体。
    pub params: Vec<WasmAbiCarrier>,
    /// 返回值载体；无返回值时为 [`WasmAbiCarrier::Void`]。
    pub returns: WasmAbiCarrier,
}

/// 按 Wasm 物理投影冻结一个导出函数。
///
/// 参数按 `param_types` 声明顺序逐个映射；返回值按 `return_type` 映射。没有稳定
/// Wasm 载体的类型（`Integer128` / `Utf16` / `Bottom` / `Function` / `TraitObject` /
/// `Union`）结构化失败，不得回落为默认载体。
pub(crate) fn freeze_wasm_export_abi(function: &ExecutableFunction) -> Result<WasmLibraryExportAbi> {
    let mut params = Vec::with_capacity(function.param_types.len());
    for (index, ty) in function.param_types.iter().enumerate() {
        let carrier = freeze_wasm_carrier(ty)
            .map_err(|reason| format!("函数 `{}` 参数 {index} 类型 `{ty}` 没有库导出 ABI 载体: {reason}", function.symbol))?;
        if carrier == WasmAbiCarrier::Void {
            return Err(format!("函数 `{}` 参数 {index} 类型 `{ty}` 映射为 Void 载体，Void 只允许出现在返回位", function.symbol));
        }
        params.push(carrier);
    }
    let returns = freeze_wasm_carrier(&function.return_type)
        .map_err(|reason| format!("函数 `{}` 返回类型 `{}` 没有库导出 ABI 载体: {reason}", function.symbol, function.return_type))?;
    Ok(WasmLibraryExportAbi { params, returns })
}

fn freeze_wasm_carrier(ty: &NyarType) -> Result<WasmAbiCarrier> {
    match ty {
        NyarType::Unit => Ok(WasmAbiCarrier::Void),
        NyarType::Boolean | NyarType::Character => Ok(WasmAbiCarrier::I32),
        NyarType::Integer8 { .. } | NyarType::Integer16 { .. } | NyarType::Integer32 { .. } => Ok(WasmAbiCarrier::I32),
        NyarType::Integer64 { .. } => Ok(WasmAbiCarrier::I64),
        NyarType::Float32 => Ok(WasmAbiCarrier::F32),
        NyarType::Float64 => Ok(WasmAbiCarrier::F64),
        NyarType::Utf8 => Ok(WasmAbiCarrier::I32),
        NyarType::Array(_) | NyarType::FixedArray { .. } | NyarType::Nullable(_) | NyarType::Tuple(_) => Ok(WasmAbiCarrier::Handle),
        NyarType::Named(_) | NyarType::Nominal { .. } | NyarType::Apply(_, _) => Ok(WasmAbiCarrier::Handle),
        NyarType::Integer128 { .. }
        | NyarType::Utf16
        | NyarType::Bottom
        | NyarType::Function(_)
        | NyarType::TraitObject(_)
        | NyarType::Union(_) => Err(format!("Wasm 导出面没有该类型的稳定 ABI 载体: {ty}")),
    }
}

/// Wasm 导出面把一个函数实例冻结为 carrier 合同。
pub(crate) fn freeze_wasm_export_abi_for(plan: &crate::TargetExecutionPlan, instance: &ItemInstanceId) -> Result<WasmLibraryExportAbi> {
    let view = plan.get_function(instance).ok_or_else(|| format!("库导出 `{instance}` 缺少 Compiler 函数体，无法冻结 Wasm ABI 载体合同"))?;
    freeze_wasm_export_abi(&view.function)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn function_with(return_type: NyarType, param_types: Vec<NyarType>) -> ExecutableFunction {
        ExecutableFunction {
            symbol: "probe".to_string(),
            return_type,
            param_types,
            value_types: std::collections::BTreeMap::new(),
            value_layouts: std::collections::BTreeMap::new(),
            return_layout: None,
            entry: crate::contracts::BlockRef(0),
            values: Vec::new(),
            suspend_points: Vec::new(),
            frame_layouts: Vec::new(),
            continuations: Vec::new(),
            case_chains: Vec::new(),
            #[allow(deprecated)]
            state_machine: None,
            suspend_plan: None,
            blocks: vec![crate::contracts::Block {
                id: crate::contracts::BlockRef(0),
                label: "entry".to_string(),
                parameters: Vec::new(),
                instructions: Vec::new(),
                terminator: crate::contracts::Terminator::Return { value: None },
            }],
            diagnostics: Vec::new(),
        }
    }

    /// f32 有独立载体，不得并入 64 位。
    #[test]
    fn float32_has_independent_carrier() {
        let abi = freeze_wasm_export_abi(&function_with(NyarType::Float32, Vec::new())).expect("f32 有独立 ABI 载体");
        assert_eq!(abi.returns, WasmAbiCarrier::F32);
    }

    /// `utf16` 在 Wasm 导出面没有稳定载体，必须失败闭合。
    #[test]
    fn utf16_fails_closed() {
        let error = freeze_wasm_export_abi(&function_with(NyarType::Utf16, Vec::new())).expect_err("utf16 不得有默认载体");
        assert!(error.contains("Utf16") || error.contains("utf16"), "{error}");
    }
}

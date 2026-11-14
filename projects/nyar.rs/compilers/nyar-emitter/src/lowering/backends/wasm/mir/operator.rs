//! Wasm 平台私有的 `[wasm_operator("...")]` 冻结入口。
//!
//! 助记符表与数组操作身份归 Wasm backend 所有：共享 `TargetExecutionPlan` 只保存
//! 逻辑导入链接，不再携带任何平台的冻结结果。计划构造阶段由本模块一次性冻结，
//! 发射期只消费 [`WasmOperatorBinding`]，不得再从助记符字符串推断 arity 或配方。

use acorn_wasm::WasmOpcode;
use nyar_types::{ItemInstanceId, NyarType};

use crate::target_operator::{
    TargetOperatorPlan, WasmGcArrayOperation, WasmGcArrayOperatorPlan, operand_carrier, validate_frozen_operator,
};

use super::{array::freeze_wasm_array_binding, numeric::freeze_wasm_numeric_binding};

/// 已冻结的 Wasm operator 绑定。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WasmOperatorBinding {
    /// 单指令数值/比较 operator。
    Numeric(TargetOperatorPlan<WasmOpcode>),
    /// 多步 GC 数组 operator（`ref.cast` + `array.*`）。
    GcArray(WasmGcArrayOperatorPlan),
    /// 声明了目标 operator 绑定，但没有可编码配方；emit 必须结构化失败。
    Unfrozen,
}

/// 按声明的导入签名冻结 Wasm operator 绑定。
///
/// `None` 表示该导入不是 `[wasm_operator]` 绑定；`Some(Unfrozen)` 表示绑定了
/// Wasm operator 但助记符不在 Wasm 私有表内。两种状态都必须由调用方保留，不允许
/// 回落为宿主导入或默认成功。
pub(crate) fn freeze_wasm_operator_binding(
    instance: ItemInstanceId,
    symbol: &str,
    mnemonic: &str,
    parameter_types: &[NyarType],
    return_type: &NyarType,
) -> crate::Result<Option<WasmOperatorBinding>> {
    if let Some(plan) = freeze_wasm_numeric_binding(mnemonic) {
        validate_frozen_operator(mnemonic, instance, symbol, parameter_types, return_type, &plan)?;
        return Ok(Some(WasmOperatorBinding::Numeric(plan)));
    }
    if let Some(plan) = freeze_wasm_array_binding(mnemonic) {
        validate_frozen_array_operator(instance, symbol, mnemonic, parameter_types, return_type, &plan)?;
        return Ok(Some(WasmOperatorBinding::GcArray(plan)));
    }
    Ok(Some(WasmOperatorBinding::Unfrozen))
}

/// 校验冻结的 wasm GC 数组计划与导入声明的实例化签名一致。
///
/// 数组 operator 的物理编码是多步的（`ref.cast` + `array.*`），arraytype 索引属于
/// 模块装配期 RepresentationPlan 注册表事实，不在计划层冻结；此处校验的是声明签名
/// 与冻结 arity/操作身份的语义形状：聚合参数必须是堆数组 `[T]`，长度/索引参数必须
/// 携带 i32 载体，`get`/`set` 的元素槽必须与数组元素类型逐点一致。
fn validate_frozen_array_operator(
    instance: ItemInstanceId,
    function: &str,
    mnemonic: &str,
    parameter_types: &[NyarType],
    return_type: &NyarType,
    plan: &WasmGcArrayOperatorPlan,
) -> crate::Result<()> {
    use WasmGcArrayOperation::{Get, Length, New, Set};
    let fail = |reason: String| -> crate::Result<()> {
        Err(format!(
            "目标数组 operator 绑定 `{mnemonic}` 的声明签名与冻结计划不符: function={function} {reason} instance={instance:?}"
        ))
    };
    if parameter_types.len() != plan.arity {
        return fail(format!("expected arity={} actual={}", plan.arity, parameter_types.len()));
    }
    fn array_element(ty: &NyarType) -> Option<&NyarType> {
        match ty {
            NyarType::Array(element) => Some(element.as_ref()),
            _ => None,
        }
    }
    let index_carrier = |ty: &NyarType| operand_carrier(ty) == Some(crate::target_operator::TargetOperandType::I32);
    match plan.operation {
        New => {
            let Some(length) = parameter_types.first()
            else {
                return fail("arity 事实缺失".to_owned());
            };
            if !index_carrier(length) {
                return fail(format!("`array.new` 的长度参数必须携带 i32 载体: actual={length:?}"));
            }
            if array_element(return_type).is_none() {
                return fail(format!("`array.new` 的返回类型必须是堆数组 `[T]`: actual={return_type:?}"));
            }
        }
        Length => {
            let Some(array) = parameter_types.first()
            else {
                return fail("arity 事实缺失".to_owned());
            };
            if array_element(array).is_none() {
                return fail(format!("`array.length` 的聚合参数必须是堆数组 `[T]`: actual={array:?}"));
            }
            if !index_carrier(return_type) {
                return fail(format!("`array.length` 的返回类型必须携带 i32 载体: actual={return_type:?}"));
            }
        }
        Get => {
            let (Some(array), Some(index)) = (parameter_types.first(), parameter_types.get(1))
            else {
                return fail("arity 事实缺失".to_owned());
            };
            let Some(element) = array_element(array)
            else {
                return fail(format!("`array.get` 的聚合参数必须是堆数组 `[T]`: actual={array:?}"));
            };
            if !index_carrier(index) {
                return fail(format!("`array.get` 的索引参数必须携带 i32 载体: actual={index:?}"));
            }
            if return_type != element {
                return fail(format!("`array.get` 的返回类型必须与数组元素类型一致: element={element:?} actual={return_type:?}"));
            }
        }
        Set => {
            let (Some(array), Some(index), Some(value)) = (parameter_types.first(), parameter_types.get(1), parameter_types.get(2))
            else {
                return fail("arity 事实缺失".to_owned());
            };
            let Some(element) = array_element(array)
            else {
                return fail(format!("`array.set` 的聚合参数必须是堆数组 `[T]`: actual={array:?}"));
            };
            if !index_carrier(index) {
                return fail(format!("`array.set` 的索引参数必须携带 i32 载体: actual={index:?}"));
            }
            if value != element {
                return fail(format!("`array.set` 的值参数必须与数组元素类型一致: element={element:?} actual={value:?}"));
            }
            if *return_type != NyarType::Unit {
                return fail(format!("`array.set` 的返回类型必须是 unit: actual={return_type:?}"));
            }
        }
    }
    Ok(())
}

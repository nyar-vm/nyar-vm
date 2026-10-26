//! Split from `lowerer.rs`.
use super::{lowerer::array_element_type, *};

impl<'a> WasmMirLowerer<'a> {
    /// 从操作数结构类型取 heap array 元素类型；仅认 `Array` / `FixedArray`，禁止类型名白名单。
    pub(super) fn infer_array_element_type(&self, operand: &MirOperand) -> Option<NyarType> {
        match operand {
            MirOperand::Value(value) => self.mir_fn.value_types.get(value).and_then(|ty| {
                if let Some(element) = array_element_type(ty) {
                    return Some(element.clone());
                }
                // 未实例化的元素类型没有可编码的 wasm GC carrier，必须在上游实例化后再进入 backend。
                None
            }),
            _ => None,
        }
    }

    /// 只查询已经预规划的 SSA 引用槽。
    pub(super) fn operand_reference_local(&self, operand: &MirOperand) -> Option<u32> {
        match operand {
            MirOperand::Value(value) => {
                if let Some(local) = self.reference_locals.get(value).copied() {
                    return Some(local);
                }
                if let Some(local) = self.value_locals.get(value).copied() {
                    let ty = self.wasm_local_value_type(local);
                    if ty == WASM_GC_ANYREF || ty == WASM_GC_EXTERNREF {
                        return Some(local);
                    }
                }
                None
            }
            _ => None,
        }
    }
}

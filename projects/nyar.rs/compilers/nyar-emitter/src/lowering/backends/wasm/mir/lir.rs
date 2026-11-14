//! Wasm 目标私有低层 IR（LIR）与函数体编码器。
//!
//! 所有权边界（01 §5.5）：`wasm/mir` 的降低器把计划事实转成 [`WasmBodyInstruction`]；
//! 本模块只按 Wasm 二进制编码形把 LIR 写成函数体字节，不读取 `ExecutableFunction`、
//! 计划、类型注册表或布局。控制指令的块类型、立即数与 br_table 深度都是显式 LIR
//! 事实，编码器不做任何 CFG 或语义推导。

use crate::lowering::backends::wasm::sections::encode_uleb128;
use acorn_wasm::{
    WasmOpcode, encode_array_get, encode_array_len, encode_array_new_default, encode_array_new_fixed, encode_array_set, encode_f32_const,
    encode_f64_const, encode_i32_const, encode_i64_const, encode_memarg, encode_memory_copy, encode_memory_fill, encode_memory_grow,
    encode_memory_size, encode_ref_cast_array, encode_ref_cast_type_index, encode_ref_null_anyref, encode_ref_null_externref,
    encode_struct_get, encode_struct_new_default, encode_struct_set,
};

/// 单条 Wasm 函数体 LIR 指令。
///
/// LIR 以“二进制指令形”而不是字节片段为单位：每条变体都对应一个完整的 Wasm
/// 指令及其立即数，编码器只做机械序列化。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum WasmBodyInstruction {
    /// 无立即数叶子操作码。
    Opcode(WasmOpcode),
    /// 叶子操作码 + 单个 `u32` LEB 立即数（`local.*` / `global.*` / `br*` / `call`）。
    U32 { opcode: WasmOpcode, operand: u32 },
    /// 叶子操作码 + 两个 `u32` LEB 立即数（`call_indirect`）。
    U32Pair { opcode: WasmOpcode, first: u32, second: u32 },
    /// `block` / `loop` / `if` 的块类型立即数。
    BlockType { opcode: WasmOpcode, block_type: u8 },
    /// `br_table`：分支深度表与默认深度。
    BrTable { depths: Vec<u32>, default: u32 },
    /// `i32.const`。
    I32Const(i32),
    /// `i64.const`。
    I64Const(i64),
    /// `f32.const`。
    F32Const(f32),
    /// `f64.const`。
    F64Const(f64),
    /// 带 memarg 的 load/store（含 f32 本地编码）。
    MemArg { opcode: WasmOpcode, align: u32, offset: u32 },
    /// `memory.size`。
    MemorySize,
    /// `memory.grow`。
    MemoryGrow,
    /// `memory.copy`。
    MemoryCopy,
    /// `memory.fill`。
    MemoryFill,
    /// `ref.null anyref`。
    RefNullAnyref,
    /// `ref.null externref`。
    RefNullExternref,
    /// `ref.cast <type_index>`。
    RefCastTypeIndex(u32),
    /// `ref.cast array`。
    RefCastArray,
    /// `struct.new_default <type_index>`。
    StructNewDefault(u32),
    /// `struct.get <type_index> <field_index>`。
    StructGet { type_index: u32, field_index: u32 },
    /// `struct.set <type_index> <field_index>`。
    StructSet { type_index: u32, field_index: u32 },
    /// `array.new_default <type_index>`。
    ArrayNewDefault(u32),
    /// `array.new_fixed <type_index> <count>`。
    ArrayNewFixed { type_index: u32, count: u32 },
    /// `array.get <type_index>`。
    ArrayGet(u32),
    /// `array.set <type_index>`。
    ArraySet(u32),
    /// `array.len`。
    ArrayLen,
    /// GC 前缀操作码 + 单个类型/字段立即数（`struct.new <type_index>` 等）。
    GcTypeIndex { opcode: acorn_wasm::WasmGcOpcode, operand: u32 },
}

/// 把函数体 LIR 编码为 Wasm 字节码。
pub(crate) fn encode_wasm_body(instructions: &[WasmBodyInstruction]) -> Vec<u8> {
    let mut out = Vec::new();
    for instruction in instructions {
        match instruction {
            WasmBodyInstruction::Opcode(opcode) => opcode.encode(&mut out),
            WasmBodyInstruction::U32 { opcode, operand } => {
                opcode.encode(&mut out);
                encode_uleb128(*operand, &mut out);
            }
            WasmBodyInstruction::U32Pair { opcode, first, second } => {
                opcode.encode(&mut out);
                encode_uleb128(*first, &mut out);
                encode_uleb128(*second, &mut out);
            }
            WasmBodyInstruction::BlockType { opcode, block_type } => {
                opcode.encode(&mut out);
                out.push(*block_type);
            }
            WasmBodyInstruction::BrTable { depths, default } => {
                WasmOpcode::BrTable.encode(&mut out);
                encode_uleb128(u32::try_from(depths.len()).unwrap_or(u32::MAX), &mut out);
                for depth in depths {
                    encode_uleb128(*depth, &mut out);
                }
                encode_uleb128(*default, &mut out);
            }
            WasmBodyInstruction::I32Const(value) => encode_i32_const(*value, &mut out),
            WasmBodyInstruction::I64Const(value) => encode_i64_const(*value, &mut out),
            WasmBodyInstruction::F32Const(value) => encode_f32_const(*value, &mut out),
            WasmBodyInstruction::F64Const(value) => encode_f64_const(*value, &mut out),
            WasmBodyInstruction::MemArg { opcode, align, offset } => {
                encode_memarg(*opcode, *align, *offset, &mut out);
            }
            WasmBodyInstruction::MemorySize => encode_memory_size(&mut out),
            WasmBodyInstruction::MemoryGrow => encode_memory_grow(&mut out),
            WasmBodyInstruction::MemoryCopy => encode_memory_copy(&mut out),
            WasmBodyInstruction::MemoryFill => encode_memory_fill(&mut out),
            WasmBodyInstruction::RefNullAnyref => encode_ref_null_anyref(&mut out),
            WasmBodyInstruction::RefNullExternref => encode_ref_null_externref(&mut out),
            WasmBodyInstruction::RefCastTypeIndex(type_index) => encode_ref_cast_type_index(*type_index, &mut out),
            WasmBodyInstruction::RefCastArray => encode_ref_cast_array(&mut out),
            WasmBodyInstruction::StructNewDefault(type_index) => encode_struct_new_default(*type_index, &mut out),
            WasmBodyInstruction::StructGet { type_index, field_index } => encode_struct_get(*type_index, *field_index, &mut out),
            WasmBodyInstruction::StructSet { type_index, field_index } => encode_struct_set(*type_index, *field_index, &mut out),
            WasmBodyInstruction::ArrayNewDefault(type_index) => encode_array_new_default(*type_index, &mut out),
            WasmBodyInstruction::ArrayNewFixed { type_index, count } => encode_array_new_fixed(*type_index, *count, &mut out),
            WasmBodyInstruction::ArrayGet(type_index) => encode_array_get(*type_index, &mut out),
            WasmBodyInstruction::ArraySet(type_index) => encode_array_set(*type_index, &mut out),
            WasmBodyInstruction::ArrayLen => encode_array_len(&mut out),
            WasmBodyInstruction::GcTypeIndex { opcode, operand } => {
                opcode.encode(&mut out);
                encode_uleb128(*operand, &mut out);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 编码器只消费 LIR：块类型、立即数与 br_table 都按显式 LIR 事实序列化。
    #[test]
    fn encodes_explicit_immediates_without_reading_plan() {
        let body = vec![
            WasmBodyInstruction::BlockType { opcode: WasmOpcode::Loop, block_type: 0x40 },
            WasmBodyInstruction::U32 { opcode: WasmOpcode::LocalGet, operand: 3 },
            WasmBodyInstruction::BrTable { depths: vec![0, 1], default: 2 },
            WasmBodyInstruction::I32Const(42),
            WasmBodyInstruction::MemArg { opcode: WasmOpcode::F32Load, align: 2, offset: 0 },
            WasmBodyInstruction::Opcode(WasmOpcode::RefIsNull),
            WasmBodyInstruction::Opcode(WasmOpcode::End),
        ];
        assert_eq!(
            encode_wasm_body(&body),
            vec![
                WasmOpcode::Loop as u8,
                0x40,
                WasmOpcode::LocalGet as u8,
                0x03,
                WasmOpcode::BrTable as u8,
                0x02,
                0x00,
                0x01,
                0x02,
                WasmOpcode::I32Const as u8,
                42,
                WasmOpcode::F32Load as u8,
                0x02,
                0x00,
                WasmOpcode::RefIsNull as u8,
                WasmOpcode::End as u8,
            ]
        );
    }
}

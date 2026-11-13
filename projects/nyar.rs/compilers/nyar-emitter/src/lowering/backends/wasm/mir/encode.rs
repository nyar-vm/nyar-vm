//! 自旧单体 wasm mir lowerer 拆出。
//!
//! 本文件只把计划事实转换成 [`WasmBodyInstruction`]；不写字节。字节编码归
//! `lir::encode_wasm_body`（01 §5.5）。

#![allow(deprecated)]

#[allow(deprecated)]
use super::super::cabi::CABI_HEAP_GLOBAL_INDEX;
#[allow(deprecated)]
use super::*;

#[allow(deprecated)]
impl<'a> WasmMirLowerer<'a> {
    pub(super) fn bump_allocate(&mut self, size: u32, align: u32) {
        let align = align.max(1);
        let align_mask = (align - 1) as i32;
        // aligned = (heap + align - 1) & ~(align - 1)
        self.code.push(WasmBodyInstruction::U32 { opcode: WasmOpcode::GlobalGet, operand: CABI_HEAP_GLOBAL_INDEX });
        if align_mask != 0 {
            self.emit_i32_const(align_mask);
            self.emit_i32_add();
            self.emit_i32_const(!align_mask);
            self.emit_i32_and();
        }
        self.emit_local_tee(self.stack_ptr_local);

        // new_end = aligned + size; trap on unsigned wrap
        self.emit_local_get(self.stack_ptr_local);
        self.emit_i32_const(size as i32);
        self.emit_i32_add();
        self.emit_local_tee(self.bump_end_local);
        self.emit_local_get(self.stack_ptr_local);
        self.code.push(WasmBodyInstruction::Opcode(WasmOpcode::I32LtU)); // i32.lt_u
        self.emit_block_type(WasmOpcode::If);
        self.code.push(WasmBodyInstruction::Opcode(WasmOpcode::Unreachable));
        self.code.push(WasmBodyInstruction::Opcode(WasmOpcode::End));

        // grow if needed
        self.emit_local_get(self.bump_end_local);
        self.emit_i32_const(65535);
        self.emit_i32_add();
        self.emit_i32_const(16);
        self.code.push(WasmBodyInstruction::Opcode(WasmOpcode::I32ShrU)); // i32.shr_u
        self.code.push(WasmBodyInstruction::MemorySize);
        self.code.push(WasmBodyInstruction::Opcode(WasmOpcode::I32Sub)); // i32.sub
        self.emit_local_tee(self.bump_pages_local);
        self.emit_i32_const(0);
        self.code.push(WasmBodyInstruction::Opcode(WasmOpcode::I32GtS)); // i32.gt_s
        self.emit_block_type(WasmOpcode::If);
        self.emit_local_get(self.bump_pages_local);
        self.code.push(WasmBodyInstruction::MemoryGrow);
        self.emit_i32_const(-1);
        self.code.push(WasmBodyInstruction::Opcode(WasmOpcode::I32Eq)); // i32.eq
        self.emit_block_type(WasmOpcode::If);
        self.code.push(WasmBodyInstruction::Opcode(WasmOpcode::Unreachable));
        self.code.push(WasmBodyInstruction::Opcode(WasmOpcode::End));
        self.code.push(WasmBodyInstruction::Opcode(WasmOpcode::End));

        // zero-fill: memory.fill(aligned, 0, size)
        self.emit_local_get(self.stack_ptr_local);
        self.emit_i32_const(0);
        self.emit_i32_const(size as i32);
        self.code.push(WasmBodyInstruction::MemoryFill);

        // commit heap cursor
        self.emit_local_get(self.bump_end_local);
        self.code.push(WasmBodyInstruction::U32 { opcode: WasmOpcode::GlobalSet, operand: CABI_HEAP_GLOBAL_INDEX });

        // leave aligned pointer on stack
        self.emit_local_get(self.stack_ptr_local);
    }

    pub(super) fn emit_i32_const(&mut self, value: i32) {
        self.code.push(WasmBodyInstruction::I32Const(value));
    }

    /// 发射 `ref.null anyref`：产?null anyref 引用值?
    pub(super) fn emit_ref_null_anyref(&mut self) {
        self.code.push(WasmBodyInstruction::RefNullAnyref);
    }

    pub(super) fn emit_ref_null_extern(&mut self) {
        self.code.push(WasmBodyInstruction::RefNullExternref);
    }

    pub(super) fn emit_i64_const(&mut self, value: i64) {
        self.code.push(WasmBodyInstruction::I64Const(value));
    }

    pub(super) fn emit_f64_const(&mut self, value: f64) {
        self.code.push(WasmBodyInstruction::F64Const(value));
    }

    pub(super) fn emit_local_get(&mut self, local: u32) {
        self.code.push(WasmBodyInstruction::U32 { opcode: WasmOpcode::LocalGet, operand: local });
    }

    pub(super) fn emit_local_set(&mut self, local: u32) {
        self.code.push(WasmBodyInstruction::U32 { opcode: WasmOpcode::LocalSet, operand: local });
    }

    pub(super) fn emit_local_tee(&mut self, local: u32) {
        self.code.push(WasmBodyInstruction::U32 { opcode: WasmOpcode::LocalTee, operand: local });
    }

    pub(super) fn emit_i32_add(&mut self) {
        self.code.push(WasmBodyInstruction::Opcode(WasmOpcode::I32Add));
    }

    pub(super) fn emit_i32_and(&mut self) {
        self.code.push(WasmBodyInstruction::Opcode(WasmOpcode::I32And));
    }

    pub(super) fn emit_memory_copy(&mut self) {
        self.code.push(WasmBodyInstruction::MemoryCopy);
    }

    /// 发射 `if <blocktype>`：块类型是显式 LIR 事实。
    pub(super) fn emit_block_type(&mut self, opcode: WasmOpcode) {
        self.code.push(WasmBodyInstruction::BlockType { opcode, block_type: BLOCKTYPE_EMPTY });
    }

    // ── wasm-gc 辅助方法 ──────────────────────────────────────────

    /// 查找引用类型 layout 对应的 wasm-gc structtype 的 `type_index`。
    ///
    /// 未注册即计划不一致：调用方只有在 `layout_uses_gc_struct` 为真时才会走到这里，
    /// 找不到就说明注册条件与使用点分叉，必须失败闭合而不是发 `unreachable` 掩盖。
    pub(super) fn resolve_gc_struct_type_index(&self, layout_id: LayoutId, type_name: &str) -> u32 {
        self.gc_struct_type_indices
            .get(&layout_id)
            .copied()
            .unwrap_or_else(|| panic!("WASM gc structtype 未注册: layout_id={layout_id:?} type `{type_name}` in `{}`", self.mir_fn.symbol))
    }

    /// 查找 sum nominal identity 对应的 wasm-gc structtype `[i32 tag, anyref payload]`。
    ///
    /// 未注册就说明「计划里有这个 sum，降低却没为它登记类型」，必须失败闭合：
    /// 回落到别的 struct 会把编译期不一致变成运行期 `illegal cast`。
    pub(super) fn resolve_gc_sum_type_index(&self, nominal: &NominalInstanceId) -> u32 {
        self.gc_sum_type_indices
            .get(nominal)
            .copied()
            .unwrap_or_else(|| panic!("WASM sum lowering missing gc structtype for nominal {nominal:?} in `{}`", self.mir_fn.symbol))
    }

    /// 读取计划冻结的 variant tag 与 payload 类型；缺项即计划不完整，失败闭合。
    pub(super) fn sum_variant_representation(&self, nominal: &NominalInstanceId, variant: &VariantId) -> (u32, Option<nyar_types::TypeId>) {
        let sums = self.ctx.submission.backend_plan.sum_representations();
        let sum = sums
            .get(nominal)
            .unwrap_or_else(|| panic!("WASM sum lowering missing SumRepresentation for nominal {nominal:?} in `{}`", self.mir_fn.symbol));
        let entry = sum
            .variants
            .get(variant)
            .unwrap_or_else(|| panic!("WASM sum lowering missing variant {variant:?} of nominal {nominal:?} in `{}`", self.mir_fn.symbol));
        (entry.tag, entry.payload_type)
    }

    /// 查找 heap `[T]` 的 element_type 对应 wasm-gc arraytype 的 type_index。
    pub(super) fn resolve_gc_array_type_index(&self, element_type: &NyarType) -> Option<u32> {
        let key = wasm_array_element_type_key(element_type);
        self.gc_array_type_indices.get(&key).copied()
    }

    /// 将栈上 anyref 转为 typed struct ref，供 `struct.get` / `struct.set` 使用。
    pub(super) fn emit_ref_cast_struct(&mut self, type_index: u32) {
        self.code.push(WasmBodyInstruction::RefCastTypeIndex(type_index));
    }

    /// 将栈?anyref 收窄为抽?arrayref，供 `array.len` 等不带类型索引的数组指令使用?
    ///
    /// heap array local ?`reference_locals` 中声明为 anyref（`VALTYPE_ANYREF`），
    /// ?`array.len` 期望 arrayref。从 anyref local 读取后必?`ref.cast array`
    /// 收窄类型，否则触?`array.len expected type arrayref, found anyref` 验证错误?
    pub(super) fn emit_ref_cast_array(&mut self) {
        self.code.push(WasmBodyInstruction::RefCastArray);
    }

    pub(super) fn emit_ref_cast_array_type(&mut self, type_index: u32) {
        self.code.push(WasmBodyInstruction::RefCastTypeIndex(type_index));
    }

    /// 发射 `struct.new_default` <type_index>：分配并初始化所有字段为默认值?
    pub(super) fn emit_struct_new_default(&mut self, type_index: u32) {
        self.code.push(WasmBodyInstruction::StructNewDefault(type_index));
    }

    /// 发射 `struct.get` <type_index> <field_index>：从结构体引用读取字段值?
    pub(super) fn emit_struct_get(&mut self, type_index: u32, field_index: u32) {
        self.code.push(WasmBodyInstruction::StructGet { type_index, field_index });
    }

    /// 发射 `struct.set` <type_index> <field_index>；栈顺序 `[struct_ref, field_value]`（value 在栈顶）。
    pub(super) fn emit_struct_set(&mut self, type_index: u32, field_index: u32) {
        self.code.push(WasmBodyInstruction::StructSet { type_index, field_index });
    }

    /// 发射 `array.new_default` <type_index>：分配指定长度的默认值数组?
    /// 栈：[length: i32] →?[arrayref]?
    pub(super) fn emit_array_new_default(&mut self, type_index: u32) {
        assert!(
            !self.gc_array_type_indices.is_empty(),
            "heap array.new_default requires wasm-gc arraytype (mandatory GC; arraytype map empty)"
        );
        self.code.push(WasmBodyInstruction::ArrayNewDefault(type_index));
    }

    /// 发射 `array.new_fixed` <type_index> <count>：从栈上 count 个值构造定长数组?
    pub(super) fn emit_array_new_fixed(&mut self, type_index: u32, count: u32) {
        assert!(!self.gc_array_type_indices.is_empty(), "heap array.new_fixed requires wasm-gc arraytype (mandatory GC; arraytype map empty)");
        self.code.push(WasmBodyInstruction::ArrayNewFixed { type_index, count });
    }

    /// 发射 `array.get` <type_index>：读取数组指定索引的元素?
    /// 栈：[arrayref, i32_index] →?[value]?
    pub(super) fn emit_array_get(&mut self, type_index: u32) {
        assert!(!self.gc_array_type_indices.is_empty(), "heap array.get requires wasm-gc arraytype (mandatory GC; arraytype map empty)");
        self.code.push(WasmBodyInstruction::ArrayGet(type_index));
    }

    /// 发射 `array.set` <type_index>：写入数组指定索引的元素?
    /// 栈：[arrayref, i32_index, value] →?[]?
    pub(super) fn emit_array_set(&mut self, type_index: u32) {
        assert!(!self.gc_array_type_indices.is_empty(), "heap array.set requires wasm-gc arraytype (mandatory GC; arraytype map empty)");
        self.code.push(WasmBodyInstruction::ArraySet(type_index));
    }

    /// 发射 `array.len`：获取数组长度?
    /// 栈：[arrayref] →?[i32]?
    pub(super) fn emit_array_len(&mut self) {
        assert!(!self.gc_array_type_indices.is_empty(), "heap array.len requires wasm-gc arraytype (mandatory GC; arraytype map empty)");
        self.code.push(WasmBodyInstruction::ArrayLen);
    }
}

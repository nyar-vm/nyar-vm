//! Wasm CFG 调度只消费已验证的块、SSA 槽和返回合同。

use super::{MirBlock, MirBlockRef, MirConstant, MirOperand, MirTerminator, MirValueRef, WasmMirLowerer, is_js_glue_host_string_type};
use crate::lowering::backends::wasm::sections::encode_uleb128;
use acorn_wasm::{BLOCKTYPE_EMPTY, VALTYPE_I32, WasmOpcode, encode_return, encode_unreachable};

impl<'a> WasmMirLowerer<'a> {
    pub(super) fn emit_function_body(&mut self) -> crate::Result<()> {
        assert!(!self.block_order.is_empty(), "WASM 函数缺少可执行入口块");
        let entry_index = self.block_index[&self.mir_fn.entry];
        self.emit_i32_const(i32::try_from(entry_index).expect("WASM 入口块索引溢出"));
        self.emit_local_set(self.pc_local);
        let case_count = self.block_order.len();
        WasmOpcode::Loop.encode(&mut self.code);
        self.code.push(BLOCKTYPE_EMPTY);
        for _ in 0..case_count {
            WasmOpcode::Block.encode(&mut self.code);
            self.code.push(BLOCKTYPE_EMPTY);
        }
        self.emit_local_get(self.pc_local);
        WasmOpcode::BrTable.encode(&mut self.code);
        encode_uleb128(u32::try_from(case_count).expect("WASM 块数量溢出"), &mut self.code);
        for index in 0..case_count {
            encode_uleb128(u32::try_from(index).expect("WASM 块索引溢出"), &mut self.code);
        }
        encode_uleb128(u32::try_from(case_count).expect("WASM 默认分支深度溢出"), &mut self.code);
        WasmOpcode::End.encode(&mut self.code);
        self.emit_cfg_case(0, case_count)?;
        for case_index in 1..case_count {
            WasmOpcode::End.encode(&mut self.code);
            self.emit_cfg_case(case_index, case_count)?;
        }
        WasmOpcode::End.encode(&mut self.code);
        encode_unreachable(&mut self.code);
        Ok(())
    }

    fn emit_cfg_case(&mut self, case_index: usize, case_count: usize) -> crate::Result<()> {
        let block_id = self.block_order[case_index];
        let block = self.mir_fn.blocks.iter().find(|block| block.id == block_id).expect("WASM 调度块必须存在");
        for instruction in &block.instructions {
            self.current_instruction_id = Some(instruction.id);
            self.emit_instruction(instruction)?;
        }
        self.current_instruction_id = None;
        self.emit_terminator(case_index, case_count, block)?;
        Ok(())
    }

    fn cfg_continue_depth(&self, case_index: usize, case_count: usize) -> u32 {
        u32::try_from(case_count.checked_sub(case_index + 1).expect("WASM case 不在调度表内")).expect("WASM 分支深度溢出")
    }

    fn emit_terminator(&mut self, case_index: usize, case_count: usize, block: &MirBlock) -> crate::Result<()> {
        let continue_depth = self.cfg_continue_depth(case_index, case_count);
        match &block.terminator {
            MirTerminator::Return { value } => {
                match (self.return_value_type, value) {
                    (Some(expected), Some(value)) => self.emit_operand_into_text_slot(value, expected, self.mir_fn.return_layout)?,
                    (None, None) => {}
                    _ => panic!("WASM 返回值与已确定签名不一致: {}", self.mir_fn.symbol),
                }
                encode_return(&mut self.code);
            }
            MirTerminator::Jump { target, arguments } => {
                self.emit_jump_arguments(*target, arguments)?;
                let target_index = self.block_index[target];
                self.emit_i32_const(i32::try_from(target_index).expect("WASM 跳转目标溢出"));
                self.emit_local_set(self.pc_local);
                self.emit_br_depth(continue_depth);
            }
            MirTerminator::Branch { condition, then_target, else_target } => {
                for target in [then_target, else_target] {
                    let target_block = self.mir_fn.blocks.iter().find(|block| block.id == *target).expect("WASM 分支目标不存在");
                    assert!(target_block.parameters.is_empty(), "WASM Branch 没有块实参，不能跳到有参数的块");
                }
                self.emit_i32_const(i32::try_from(self.block_index[then_target]).expect("WASM 分支目标溢出"));
                self.emit_i32_const(i32::try_from(self.block_index[else_target]).expect("WASM 分支目标溢出"));
                self.emit_contract_operand(condition, VALTYPE_I32)?;
                WasmOpcode::Select.encode(&mut self.code);
                self.emit_local_set(self.pc_local);
                self.emit_br_depth(continue_depth);
            }
            MirTerminator::Unreachable => encode_unreachable(&mut self.code),
            // `Raise` 的 wasm 目标合同：payload 已由前置指令完成求值。Node 宿主声明了
            // `env::host_raise` 且 payload 是文本时，先把文本句柄交给宿主抛出结构化异常；
            // `unreachable` 只是永不返回的兜底。非文本 payload（结构化 effect 值）没有
            // 文本 ABI，仍按 trap 终止。resume 块在 wasm 上永不可达（trap 不会返回），
            // 仍保留在调度表中以保持块身份与验证器一致。
            MirTerminator::PerformEffect { effect: crate::contracts::EffectKind::Raise, payload, .. } => {
                self.emit_host_raise(payload)?;
                encode_unreachable(&mut self.code);
            }
            other => panic!("WASM 不支持此终结符 {:?}: {}", std::mem::discriminant(other), self.mir_fn.symbol),
        }
        Ok(())
    }

    /// 未捕获 `raise` 的宿主边界：文本 payload 以结构化宿主异常呈现。
    ///
    /// 只消费 Node 宿主声明的合成导入 `env::host_raise`；WASI 轨与结构化 effect
    /// 值（没有文本 ABI）保持既有 trap 合同。必须发射真实 payload 句柄，禁止用
    /// 空字符串或 `0` 占位。
    fn emit_host_raise(&mut self, payload: &Option<MirOperand>) -> crate::Result<()> {
        if self.wasi_mode {
            return Ok(());
        }
        let Some(payload) = payload.as_ref()
        else {
            return Ok(());
        };
        let is_text = match payload {
            MirOperand::Constant(MirConstant::Utf8(_)) => true,
            _ => self.operand_semantic_type(payload).is_some_and(is_js_glue_host_string_type),
        };
        if !is_text {
            return Ok(());
        }
        let Some(index) = self.find_host_import_index("env", "host_raise")
        else {
            return Ok(());
        };
        self.emit_operand_coerced(payload, VALTYPE_I32)?;
        self.emit_call(index);
        Ok(())
    }

    pub(super) fn planned_value_local(&self, value: MirValueRef) -> u32 {
        self.reference_locals
            .get(&value)
            .or_else(|| self.value_locals.get(&value))
            .copied()
            .unwrap_or_else(|| panic!("WASM SSA 值缺少预规划槽: %{} in {}", value.0, self.mir_fn.symbol))
    }

    pub(super) fn emit_contract_operand(&mut self, operand: &MirOperand, expected: u8) -> crate::Result<()> {
        match operand {
            MirOperand::Value(value) => {
                let local = self.planned_value_local(*value);
                let actual = self.wasm_local_value_type(local);
                if actual != expected {
                    return Err(format!(
                        "WASM 目标合同槽位自检失败 in `{}`: operand=%{} local={} expected={} actual={}",
                        self.mir_fn.symbol, value.0, local, expected, actual
                    ));
                }
                self.emit_local_get(local);
                Ok(())
            }
            MirOperand::Constant(constant) => self.emit_load_constant_for_slot(constant, expected),
            MirOperand::Symbol(_) => panic!("WASM CFG 操作数必须是 SSA 值或已类型化常量"),
            MirOperand::Item(_) => panic!("WASM CFG callable identity cannot be a block operand"),
        }
    }

    fn emit_jump_arguments(&mut self, target: MirBlockRef, arguments: &[MirOperand]) -> crate::Result<()> {
        let target_block = self.mir_fn.blocks.iter().find(|block| block.id == target).expect("WASM 跳转目标不存在");
        assert_eq!(target_block.parameters.len(), arguments.len(), "WASM 块参数数量不一致");
        let destinations = target_block.parameters.iter().map(|parameter| self.planned_value_local(*parameter)).collect::<Vec<_>>();
        for ((argument, parameter), destination) in arguments.iter().zip(&target_block.parameters).zip(&destinations) {
            let slot_layout = self.mir_fn.value_layouts.get(parameter).copied();
            self.emit_operand_into_text_slot(argument, self.wasm_local_value_type(*destination), slot_layout)?;
        }
        for destination in destinations.into_iter().rev() {
            self.emit_local_set(destination);
        }
        Ok(())
    }

    fn emit_br_depth(&mut self, depth: u32) {
        WasmOpcode::Br.encode(&mut self.code);
        encode_uleb128(depth, &mut self.code);
    }
}

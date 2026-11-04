use crate::compiler::types::hir::{HirBlock, ValkyrieType};

use crate::compiler::semantic::control_flow::TryScopeData;

use super::{MirBuilder, MirDiagnostic, MirOperand, MirTerminator, control_flow_context::MirTryScopeContext, infer_builder_operand_type};

impl MirBuilder {
    pub(super) fn lower_try_scope_expr(
        &mut self,
        is_optional: bool,
        is_forced: bool,
        result_type: &Option<ValkyrieType>,
        body: &HirBlock,
    ) -> MirOperand {
        let _ = is_forced;
        let entry_block = self.current_block;
        let entry_label = self.current_label.clone();

        let try_exit = self.new_block("try_exit");
        let mut exit_value = None;
        if is_optional || result_type.is_some() {
            let exit_type = result_type.clone().unwrap_or_else(|| ValkyrieType::Nullable(Box::new(ValkyrieType::Unit)));
            self.ensure_branch_exit_parameter(try_exit, &mut exit_value, Some(exit_type));
        }

        self.current_block = entry_block;
        self.current_label = entry_label.clone();
        self.instructions.clear();
        self.terminator = None;

        let pre_bindings = self.bindings.clone();
        self.control_flow.push_try(
            TryScopeData { is_optional, is_forced, result_type: result_type.clone() },
            MirTryScopeContext { exit: try_exit, exit_value, early_exit: false },
        );

        let body_result = self.lower_block_expr(body, None);
        let early_exit = self.control_flow.current_try_scope().is_some_and(|scope| scope.early_exit);

        self.control_flow.pop_try();

        // 始终 flush 当前块：若 body 内的 `break`/`continue`/`return` 已经设置了 terminator，
        // 必须保留该 terminator 写入 entry 块，否则跳转会被静默丢弃（参见 lower_if_expr 的同款模式）。
        if self.terminator.is_none() {
            let arguments = if exit_value.is_some() { vec![body_result.clone()] } else { Vec::new() };
            self.terminate(MirTerminator::Jump { target: try_exit, arguments });
        }
        self.flush_block(&entry_label);

        self.current_block = try_exit;
        self.current_label = "try_exit".to_string();
        self.instructions.clear();
        self.terminator = None;

        if early_exit {
            // 有提前出口时，块外写回没有被线程化到 try_exit 的块参数：错误边与
            // fallthrough 边的绑定不可合并，不能静默退回入口槽造成算错，只能拒绝。
            let visible_bindings = std::mem::replace(&mut self.bindings, pre_bindings.clone());
            for (name, operand) in visible_bindings {
                if pre_bindings.get(&name).is_some_and(|previous| previous != &operand) {
                    self.diagnostics.push(MirDiagnostic::UnsupportedExpression {
                        span: body.span.clone(),
                        kind: format!("try scope writes outer binding `{name}` while `?` can exit before the write; value is not threaded to `try_exit`"),
                    });
                }
            }
        }
        // 没有提前出口时 try 作用域退化为普通块，`lower_block_expr` 已经按声明集
        // 裁剪并保留了块外绑定的最新操作数，直接用当前绑定即可。

        if let Some(value) = exit_value {
            if let Some(ty) = self.value_types.get(&value).cloned().or_else(|| infer_builder_operand_type(&body_result, &self.value_types)) {
                self.value_types.insert(value, ty);
            }
            MirOperand::Value(value)
        }
        else {
            body_result
        }
    }
}

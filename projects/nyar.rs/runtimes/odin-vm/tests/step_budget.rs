//! Lang-kyrie OVM 执行期步数预算：挂死 → 可归因失败，且不误杀长循环。
//!
//! 预算按"单次调用"计（覆盖该次 `run` 的整个调用树），超限时报
//! `StepBudgetExceeded { function, ip, limit }`。

use odin_bytecode::{
    Odin_VERSION, OdinConstant, OdinExport, OdinExportKind, OdinFunction, OdinHeadCode, OdinModuleData, emit_imm1, emit_plain, encode_module,
};
use odin_vm::{error::OdinRuntimeError, module::LoadedModule, Value};

fn module_with(code: Vec<u8>, constants: Vec<OdinConstant>, local_count: i32) -> LoadedModule {
    let data = OdinModuleData {
        version: Odin_VERSION,
        name: "step_budget".to_string(),
        constants,
        functions: vec![OdinFunction {
            name: "main".to_string(),
            arity: 0,
            local_count,
            code_offset: 0,
            code_length: code.len() as i32,
        }],
        imports: Vec::new(),
        exports: vec![OdinExport { kind: OdinExportKind::Function, symbol_name: "main".to_string(), function_index: 0 }],
        witness_entries: Vec::new(),
        code_bytes: code,
        globals: Vec::new(),
        init_function_indices: Vec::new(),
        layouts: Vec::new(),
    };
    LoadedModule::from_bytes(&encode_module(&data)).expect("测试模块必须能加载并通过 verifier")
}

/// 倒计时循环：`local0 = total; while local0 > 0 { local0 -= 1 }`，每轮 8 条指令。
fn countdown_module(total: i32) -> LoadedModule {
    let mut code = Vec::new();
    emit_imm1(&mut code, OdinHeadCode::Constant, 0); // 常量 0 = total
    emit_imm1(&mut code, OdinHeadCode::StoreLocal, 0);
    let loop_head = code.len();
    emit_imm1(&mut code, OdinHeadCode::LoadLocal, 0);
    emit_imm1(&mut code, OdinHeadCode::Constant, 1); // 常量 1 = 1
    emit_plain(&mut code, OdinHeadCode::I32Sub);
    emit_imm1(&mut code, OdinHeadCode::StoreLocal, 0);
    emit_imm1(&mut code, OdinHeadCode::LoadLocal, 0);
    emit_imm1(&mut code, OdinHeadCode::Constant, 2); // 常量 2 = 0
    emit_plain(&mut code, OdinHeadCode::I32GtS);
    let jump_at = code.len();
    emit_imm1(&mut code, OdinHeadCode::JumpIfTrue, 0);
    emit_imm1(&mut code, OdinHeadCode::Constant, 2);
    emit_plain(&mut code, OdinHeadCode::Return);
    let back_edge = loop_head as i32 - jump_at as i32;
    code[jump_at + 1..jump_at + 5].copy_from_slice(&back_edge.to_le_bytes());
    module_with(code, vec![OdinConstant::Integer32(total), OdinConstant::Integer32(1), OdinConstant::Integer32(0)], 1)
}

/// 正例：有显式退出条件的循环在预算内正常跑完。
#[test]
fn loop_with_explicit_exit_completes_within_budget() {
    let module = countdown_module(10);
    let mut vm = odin_vm::OdinVm::new();
    vm.set_step_budget(Some(1_000));
    let value = vm.run(&module, "main", Vec::new()).expect("10 轮循环必须在预算内完成");
    assert_eq!(value, Value::I32(0));
    assert!(vm.last_step_count() < 1_000);
}

/// 反例（挂死）：合法 `loop {}` 的字节码形态是块尾 `Jump` 回块首（不是 `Trap`）。
/// 它永远自旋，只能由预算截断；断言失败形态是 `StepBudgetExceeded` 而非 `TrapExecuted`。
#[test]
fn legit_empty_loop_is_budgeted_not_trapped() {
    let mut code = Vec::new();
    emit_imm1(&mut code, OdinHeadCode::Jump, 0); // 自旋：目标就是块首自身
    let module = module_with(code, Vec::new(), 0);
    let mut vm = odin_vm::OdinVm::new();
    vm.set_step_budget(Some(64));
    let error = vm.run(&module, "main", Vec::new()).expect_err("自旋必须被预算截断");
    match error {
        OdinRuntimeError::StepBudgetExceeded { function, ip, limit } => {
            assert_eq!(function, "main");
            assert_eq!(ip, 0, "自旋停在块首 pc");
            assert_eq!(limit, 64);
        }
        OdinRuntimeError::TrapExecuted { function, ip } => {
            panic!("合法 `loop {{}}` 的回边不得被当成 `Trap`：function={function}, ip={ip}")
        }
        other => panic!("期望 StepBudgetExceeded，实际 {other:?}"),
    }
    assert_eq!(vm.last_step_count(), 65, "超限判定发生在第 limit+1 条指令");
}

/// 长循环不被误杀：10^6 轮 ≈ 8×10^6 条指令，默认预算必须有量级余量。
/// 同时打印 10^3 轮与 10^6 轮两端的指令数 / 耗时，作为默认预算的取值依据。
#[test]
fn long_loop_is_not_false_killed_by_default_budget() {
    let mut vm = odin_vm::OdinVm::new();
    let small = countdown_module(1_000);
    vm.run(&small, "main", Vec::new()).expect("10^3 轮循环");
    let small_steps = vm.last_step_count();

    let module = countdown_module(1_000_000);
    let started = std::time::Instant::now();
    let value = vm.run(&module, "main", Vec::new()).expect("10^6 轮循环不得被默认预算误杀");
    let elapsed = started.elapsed();
    assert_eq!(value, Value::I32(0));
    let steps = vm.last_step_count();
    eprintln!(
        "step_budget: 10^3 轮 = {small_steps} 条指令; 10^6 轮 = {steps} 条指令, 耗时 {elapsed:?}, 默认预算 {:?}",
        vm.step_budget()
    );
    assert!(small_steps > 8_000, "每轮 8 条指令，10^3 轮实测 {small_steps}");
    assert!(steps > 8_000_000, "每轮 8 条指令，10^6 轮实测 {steps}");
    assert!(
        steps.saturating_mul(10) <= odin_vm::executor::DEFAULT_STEP_BUDGET,
        "默认预算对 10^6 轮长循环至少要有 10 倍余量：steps={steps}"
    );
}

/// `Trap` 指令：前端把可达块误标成 `Unreachable` 时，运行期立即得到带 function/pc 的结构化失败
/// （不必等到预算用尽）。
#[test]
fn trap_instruction_reports_function_and_ip() {
    let mut code = Vec::new();
    emit_imm1(&mut code, OdinHeadCode::Constant, 0);
    emit_plain(&mut code, OdinHeadCode::Trap);
    let module = module_with(code, vec![OdinConstant::Integer32(7)], 0);
    let mut vm = odin_vm::OdinVm::new();
    let error = vm.run(&module, "main", Vec::new()).expect_err("Trap 必须失败关闭");
    match error {
        OdinRuntimeError::TrapExecuted { function, ip } => {
            assert_eq!(function, "main");
            assert_eq!(ip, 1, "pc 指向 Trap 指令本身");
        }
        other => panic!("期望 TrapExecuted，实际 {other:?}"),
    }
}

/// 预算入口可关：`None` 表示不做上限比较（计数仍进行，诊断值保留）。
#[test]
fn budget_can_be_disabled_via_config() {
    let module = countdown_module(1);
    let mut vm = odin_vm::OdinVm::new();
    vm.set_step_budget(None);
    assert_eq!(vm.step_budget(), None, "关闭预算后 step_budget() 必须返回 None");
    assert!(vm.run(&module, "main", Vec::new()).is_ok(), "关闭预算不影响正常执行");
    assert!(vm.last_step_count() > 0, "关闭预算不关计数：诊断值仍须可用");
}

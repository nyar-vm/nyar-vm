//! CLR 宿主执行验证：把冻结配方降低出的托管程序集真跑起来，读回数值与异常。
//!
//! 发射产物自带 `managed_entry(string[])` 入口点：`args[0]` 是逻辑入口名，其余参数
//! 按声明类型解析；测试按 `dotnet exec <dll> <entry> <args...>` 启动它。
//! harness 只存在于测试侧，不进入产品 API。

use std::{
    collections::BTreeMap,
    path::Path,
    process::{Command, Output},
};

use nyar::{ExternalImportLink, Identifier, QualifiedName};
use nyar_emitter::{
    BackendImport, FragmentSubmission, TargetExecutionPlan,
    backend::clr::{
        host_process::{configure_quiet_host_command, suppress_host_error_dialogs},
        write_dotnet_runtime_config,
    },
    nyar_backend_clr::{MsilModule, MsilOpcode},
    target_operator::{TargetOperandStep, TargetOperandType, TargetOperatorPlan, TargetOperatorRecipe},
    testing::lower_fragment_to_clr_managed_surface,
    transitional::binary::pe::{ClrImageKind, PeWriter, PeWriterOptions},
};
use nyar_types::{
    Block, BlockRef, ExecutableFunction, ImportIndex, Instruction, InstructionKind, ItemInstanceId, NyarType, Operand, Terminator, ValueRef,
};

const LONG: NyarType = NyarType::Integer64 { signed: true };
const INTEGER: NyarType = NyarType::Integer32 { signed: true };

fn function(symbol: &str, return_type: NyarType, parameters: Vec<NyarType>) -> ExecutableFunction {
    let value_types = parameters.iter().enumerate().map(|(index, ty)| (ValueRef(index as u32), ty.clone())).collect();
    let block_parameters = (0..parameters.len() as u32).map(ValueRef).collect();
    ExecutableFunction {
        return_layout: None,
        value_layouts: BTreeMap::new(),
        symbol: symbol.to_string(),
        return_type,
        param_types: parameters,
        value_types,
        entry: BlockRef(0),
        values: Vec::new(),
        suspend_points: Vec::new(),
        frame_layouts: Vec::new(),
        continuations: Vec::new(),
        case_chains: Vec::new(),
        #[allow(deprecated)]
        state_machine: None,
        suspend_plan: None,
        blocks: vec![Block {
            id: BlockRef(0),
            label: "entry".to_string(),
            parameters: block_parameters,
            instructions: Vec::new(),
            terminator: Terminator::Return { value: None },
        }],
        diagnostics: Vec::new(),
    }
}

fn instr(kind: InstructionKind, results: Vec<ValueRef>) -> Instruction {
    let mut instruction = Instruction::from_kind(kind);
    instruction.results = results;
    instruction
}

fn operator_import(
    mnemonic: &str,
    parameter_types: Vec<NyarType>,
    return_type: NyarType,
    _plan: TargetOperatorPlan<MsilOpcode>,
) -> BackendImport {
    BackendImport {
        index: ImportIndex::from_index(0).expect("import index"),
        link: ExternalImportLink::host(Some(Identifier::new("clr-operator")), vec![mnemonic.to_string()]),
        parameter_types,
        return_type,
    }
}

fn macro_plan(encoding: MsilOpcode, operand_type: TargetOperandType, result_type: TargetOperandType) -> TargetOperatorPlan<MsilOpcode> {
    TargetOperatorPlan {
        arity: 2,
        operand_type,
        result_type,
        recipe: TargetOperatorRecipe::Operands,
        encoding,
    }
}

/// `i64` 移位的冻结配方：先 `conv.i4` 截窄位移量，再执行移位。
const I64_SHIFT_STEPS: &[TargetOperandStep] = &[TargetOperandStep::Arguments, TargetOperandStep::NarrowTopI64ToI32];

fn shift_plan(encoding: MsilOpcode, operand_type: TargetOperandType, result_type: TargetOperandType) -> TargetOperatorPlan<MsilOpcode> {
    TargetOperatorPlan {
        arity: 2,
        operand_type,
        result_type,
        recipe: TargetOperatorRecipe::Steps(I64_SHIFT_STEPS),
        encoding,
    }
}

/// 构造只调用一个冻结 operator 的导出函数，降低成带 `managed_entry` 的托管模块。
fn lower_exported_operator(
    export: &str,
    parameter_types: Vec<NyarType>,
    result_type: NyarType,
    mnemonic: &str,
    operator: TargetOperatorPlan<MsilOpcode>,
) -> MsilModule {
    let operator_instance = ItemInstanceId::from_index(1).expect("operator instance");
    let value = ValueRef(parameter_types.len() as u32);
    let mut fixture = function("clr.unsigned.runtime", result_type.clone(), parameter_types.clone());
    fixture.value_types.insert(value, result_type.clone());
    let arguments = (0..parameter_types.len() as u32).map(|index| Operand::Value(ValueRef(index))).collect();
    fixture.blocks[0].instructions.push(instr(InstructionKind::Call { callee: Operand::Item(operator_instance), arguments }, vec![value]));
    fixture.blocks[0].terminator = Terminator::Return { value: Some(Operand::Value(value)) };

    let main = QualifiedName::new(vec![Identifier::new("main")]);
    let mut plan = TargetExecutionPlan::from_functions(BTreeMap::from([(main.clone(), fixture)]));
    plan.testing_module_name("clr_unsigned_runtime").testing_entry_operation(main);
    plan.testing_export_names_mut().insert(ItemInstanceId::from_index(0).expect("function instance"), export.to_string());
    plan.testing_register_import(operator_instance, operator_import(mnemonic, parameter_types.clone(), result_type, operator));
    let (module, _) = lower_fragment_to_clr_managed_surface(&FragmentSubmission::from_backend_plan(plan)).expect("managed surface must lower");
    module
}

/// 以可执行 PE flavor 写出托管镜像，并补 `dotnet` 启动所需的 runtimeconfig。
fn write_launcher(directory: &Path, artifact: &str, module: &MsilModule) {
    let bytes = PeWriter::new(PeWriterOptions {
        assembly_name: artifact.to_string(),
        module_name: artifact.to_string(),
        image_kind: ClrImageKind::Executable,
    })
    .write_module(module)
    .expect("CLR executable PE encoding");
    std::fs::write(directory.join(format!("{artifact}.dll")), bytes).expect("write CLR artifact");
    write_dotnet_runtime_config(directory, artifact).expect("write runtimeconfig");
}

fn run_clr(directory: &Path, artifact: &str, entry: &str, arguments: &[&str]) -> Output {
    suppress_host_error_dialogs();
    let mut command = Command::new("dotnet");
    command.current_dir(directory).arg("exec").arg(format!("{artifact}.dll")).arg(entry);
    for argument in arguments {
        command.arg(argument);
    }
    configure_quiet_host_command(&mut command);
    command.output().expect("dotnet exec")
}

fn assert_stdout(run: &Output, expected: &str, context: &str) {
    assert!(
        run.status.success(),
        "{context}: status={:?} stdout={} stderr={}",
        run.status.code(),
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&run.stdout).trim(), expected, "{context}: stdout");
}

fn assert_traps(run: &Output, exception: &str, context: &str) {
    assert!(!run.status.success(), "{context}：零除数必须失败，stdout={}", String::from_utf8_lossy(&run.stdout));
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(stderr.contains(exception), "{context}：stderr={stderr}");
}

fn dotnet_available() -> bool {
    suppress_host_error_dialogs();
    let mut command = Command::new("dotnet");
    command.arg("--version");
    configure_quiet_host_command(&mut command);
    command.output().map(|output| output.status.success()).unwrap_or(false)
}

/// `i64.div.un`/`i64.rem.un` 的高位边界必须在 CLR 上按无符号语义真跑出结果。
/// `-1` 是 `u64::MAX` 的十进制位模式。
#[test]
fn unsigned_i64_division_and_remainder_execute_on_host_clr() {
    if !dotnet_available() {
        eprintln!("skip clr host runtime: dotnet not on PATH");
        return;
    }
    let directory = tempfile::tempdir().expect("temp dir");

    let divide = lower_exported_operator(
        "divide",
        vec![LONG, LONG],
        LONG,
        "i64.div.un",
        macro_plan(MsilOpcode::DivUn, TargetOperandType::I64, TargetOperandType::I64),
    );
    write_launcher(directory.path(), "clr_u64_divide", &divide);
    let run = run_clr(directory.path(), "clr_u64_divide", "divide", &["-1", "2"]);
    assert_stdout(&run, "9223372036854775807", "u64::MAX/2");

    let run = run_clr(directory.path(), "clr_u64_divide", "divide", &["-1", "-1"]);
    assert_stdout(&run, "1", "u64::MAX/u64::MAX");

    // 无符号语义下 `INT64_MIN / -1` 是 `MAX/MAX`，不得出现有符号溢出异常。
    let run = run_clr(directory.path(), "clr_u64_divide", "divide", &["-9223372036854775808", "-1"]);
    assert_stdout(&run, "0", "INT64_MIN/-1 按无符号");

    let run = run_clr(directory.path(), "clr_u64_divide", "divide", &["-1", "0"]);
    assert_traps(&run, "DivideByZeroException", "u64 零除数");

    let remainder = lower_exported_operator(
        "remainder",
        vec![LONG, LONG],
        LONG,
        "i64.rem.un",
        macro_plan(MsilOpcode::RemUn, TargetOperandType::I64, TargetOperandType::I64),
    );
    write_launcher(directory.path(), "clr_u64_remainder", &remainder);
    let run = run_clr(directory.path(), "clr_u64_remainder", "remainder", &["-1", "3"]);
    assert_stdout(&run, "0", "u64::MAX%3");

    let run = run_clr(directory.path(), "clr_u64_remainder", "remainder", &["-1", "0"]);
    assert_traps(&run, "DivideByZeroException", "u64 零除数取余");
}

/// `i32.div.un`/`i32.rem.un`/`i32.shr.un` 的 `u32` 边界与 `&31` 截断。
#[test]
fn unsigned_i32_division_remainder_and_shift_execute_on_host_clr() {
    if !dotnet_available() {
        eprintln!("skip clr host runtime: dotnet not on PATH");
        return;
    }
    let directory = tempfile::tempdir().expect("temp dir");

    let divide = lower_exported_operator(
        "divide",
        vec![INTEGER, INTEGER],
        INTEGER,
        "i32.div.un",
        macro_plan(MsilOpcode::DivUn, TargetOperandType::I32, TargetOperandType::I32),
    );
    write_launcher(directory.path(), "clr_u32_divide", &divide);
    let run = run_clr(directory.path(), "clr_u32_divide", "divide", &["-1", "2"]);
    assert_stdout(&run, "2147483647", "u32::MAX/2");

    let run = run_clr(directory.path(), "clr_u32_divide", "divide", &["-1", "0"]);
    assert_traps(&run, "DivideByZeroException", "u32 零除数");

    let remainder = lower_exported_operator(
        "remainder",
        vec![INTEGER, INTEGER],
        INTEGER,
        "i32.rem.un",
        macro_plan(MsilOpcode::RemUn, TargetOperandType::I32, TargetOperandType::I32),
    );
    write_launcher(directory.path(), "clr_u32_remainder", &remainder);
    let run = run_clr(directory.path(), "clr_u32_remainder", "remainder", &["-1", "3"]);
    assert_stdout(&run, "0", "u32::MAX%3");

    let shift = lower_exported_operator(
        "shift",
        vec![INTEGER, INTEGER],
        INTEGER,
        "i32.shr.un",
        shift_plan(MsilOpcode::ShrUn, TargetOperandType::I32, TargetOperandType::I32),
    );
    write_launcher(directory.path(), "clr_u32_shift", &shift);
    let run = run_clr(directory.path(), "clr_u32_shift", "shift", &["-2147483648", "31"]);
    assert_stdout(&run, "1", "(1<<31)>>31");
    // 33 的低 5 位是 1：按 `&31` 截断后应右移 1 位。
    let run = run_clr(directory.path(), "clr_u32_shift", "shift", &["-2147483648", "33"]);
    assert_stdout(&run, "1073741824", "(1<<31)>>33 按 &31 截断");
}

/// `i64.shr.un` 的 `ConvI4 → shr.un` 配方必须真跑验证：CLR 的 `shr.un` 在 `int64`
/// 栈值上按低 6 位取位移量，`conv.i4` 只负责把 `i64` 位移量截窄成栈上的 `int32`。
#[test]
fn unsigned_i64_shift_masks_count_to_six_bits_on_host_clr() {
    if !dotnet_available() {
        eprintln!("skip clr host runtime: dotnet not on PATH");
        return;
    }
    let directory = tempfile::tempdir().expect("temp dir");

    let shift = lower_exported_operator(
        "shift",
        vec![LONG, LONG],
        LONG,
        "i64.shr.un",
        shift_plan(MsilOpcode::ShrUn, TargetOperandType::I64, TargetOperandType::I64),
    );
    write_launcher(directory.path(), "clr_u64_shift", &shift);

    let run = run_clr(directory.path(), "clr_u64_shift", "shift", &["-9223372036854775808", "63"]);
    assert_stdout(&run, "1", "(1<<63)>>63");
    let run = run_clr(directory.path(), "clr_u64_shift", "shift", &["-9223372036854775808", "127"]);
    assert_stdout(&run, "1", "(1<<63)>>127 按 &63 截断");

    let run = run_clr(directory.path(), "clr_u64_shift", "shift", &["-9223372036854775808", "64"]);
    assert_stdout(&run, "-9223372036854775808", "(1<<63)>>64 的 &63 归零");
}

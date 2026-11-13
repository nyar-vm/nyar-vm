//! 分支内赋值必须经 merge 块 phi 传给后继读取。
//!
//! `if` / `if let` 的两条 arm 都从分支前的绑定出发 lowering；若合并时直接恢复分支前
//! 绑定，arm 内的赋值在 merge 处被丢弃。下面的用例既断言 SSA 形状（merge 块参数），
//! 也把 fixture 编译到 wasm 后在 Node 里执行，直接断言运行期返回值。
//!
//! fixture 只用 `bool` 形参和 `if` / `while`：`bool` 在语言层是 `Boolean` 标量，
//! 赋值走 `StoreVar`，不依赖任何算子 adaptor 或聚合布局，因此能独立验证分支合并。

use nyar::HostProjectionBoundary;
use nyar_emitter::{CompiledProgram, FragmentSubmission, TargetExecutionPlan};
use nyar_types::pipeline::RepresentationPlanStage;
use rusty_valkyrie::{MirLowerer, ValkyrieCompiler, compiler::mir::ssa::MirValueOrigin};

const PRIMITIVES: &str = r#"
namespace std::primitive;
[primitive("std::primitive::bool")] structure bool { }
"#;

fn compile(source: &str) -> rusty_valkyrie::compiler::mir::ssa::MirModule {
    let source = format!("{PRIMITIVES}\n{source}");
    let hir = ValkyrieCompiler::new(rusty_valkyrie::compiler::types::SourceID { version_id: 9700 })
        .compile_source(&source)
        .expect("fixture 必须完成 HIR 分析");
    MirLowerer::lower_module_semantic(&hir)
}

fn function<'a>(module: &'a rusty_valkyrie::compiler::mir::ssa::MirModule, name: &str) -> &'a rusty_valkyrie::compiler::mir::ssa::MirFunction {
    module.functions.iter().find(|function| function.display_symbol.contains(name)).unwrap_or_else(|| panic!("fixture 缺少函数 `{name}`"))
}

fn origin<'a>(
    function: &'a rusty_valkyrie::compiler::mir::ssa::MirFunction,
    value: rusty_valkyrie::compiler::mir::ssa::MirValueRef,
) -> &'a MirValueOrigin {
    &function.values.iter().find(|entry| entry.id == value).unwrap_or_else(|| panic!("SSA 值 {value:?} 不在函数内")).origin
}

fn returned_value(function: &rusty_valkyrie::compiler::mir::ssa::MirFunction) -> rusty_valkyrie::compiler::mir::ssa::MirValueRef {
    let operand = function
        .blocks
        .iter()
        .find_map(|block| match &block.terminator {
            rusty_valkyrie::compiler::mir::ssa::MirTerminator::Return { value: Some(operand) } => Some(operand),
            _ => None,
        })
        .expect("fixture 必须有 return");
    let rusty_valkyrie::compiler::mir::ssa::MirOperand::Value(value) = operand
    else {
        panic!("return 必须读取 SSA 值而不是常量，实际 {operand:?}");
    };
    *value
}

const PICK_SOURCE: &str = r#"
micro pick(flag: bool, a: bool, b: bool) -> bool {
    let mut x: bool = a
    if flag {
        x = b
    }
    return x
}
"#;

const LOOP_PICK_SOURCE: &str = r#"
micro loop_pick(flag: bool, a: bool, b: bool) -> bool {
    let mut x: bool = a
    let mut go: bool = true
    while go {
        if flag {
            x = b
        }
        go = false
    }
    return x
}
"#;

/// `if` 内对已存在变量的赋值必须出现在 merge 块的 phi 参数里，后续读取必须读该参数。
#[test]
fn if_arm_assignment_reaches_merge_phi() {
    let module = compile(PICK_SOURCE);
    let function = function(&module, "pick");
    let merge = function.blocks.iter().find(|block| block.label == "merge").expect("if 必须有 merge 块");
    assert!(!merge.parameters.is_empty(), "arm 内赋值必须建立 merge phi，否则 merge 恢复分支前绑定");
    let parameter_name = merge
        .parameters
        .iter()
        .find_map(|parameter| match origin(function, *parameter) {
            MirValueOrigin::BlockParameter { name, .. } => Some(name.clone()),
            _ => None,
        })
        .expect("merge 参数必须是有名的 BlockParameter");
    assert_eq!(parameter_name, "x", "只有被 arm 改写的 `x` 需要 phi");

    match origin(function, returned_value(function)) {
        MirValueOrigin::BlockParameter { block, name } => {
            assert_eq!(*block, merge.id, "return 必须读取 merge 块的 phi");
            assert_eq!(name, "x");
        }
        other => panic!("return 读取了 arm 之外的旧值：{other:?}"),
    }
}

/// 循环体里 `if` 内改写的变量必须同时进入循环头参数，否则每次迭代都回到初值。
#[test]
fn if_arm_assignment_inside_loop_is_loop_carried() {
    let module = compile(LOOP_PICK_SOURCE);
    let function = function(&module, "loop_pick");
    let header = function.blocks.iter().find(|block| block.label == "loop_header").expect("while 必须有循环头");
    let carried: Vec<String> = header
        .parameters
        .iter()
        .filter_map(|parameter| match origin(function, *parameter) {
            MirValueOrigin::BlockParameter { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect();
    assert!(carried.iter().any(|name| name == "x"), "`if` 内改写的 x 必须是循环携带变量：{carried:?}");
    assert!(carried.iter().any(|name| name == "go"), "循环顶层的赋值必须是循环携带变量：{carried:?}");

    match origin(function, returned_value(function)) {
        MirValueOrigin::BlockParameter { block, name } => {
            assert_eq!(*block, header.id, "循环后读取的 x 必须是循环头 phi");
            assert_eq!(name, "x");
        }
        other => panic!("循环后读取了循环前的旧值：{other:?}"),
    }
}

const ELSE_IF_CHAIN_SOURCE: &str = r#"
micro chain_else_if(a: bool, b: bool) -> bool {
    let mut out: bool = true
    let mut go: bool = true
    while go {
        if a {
            out = false
        } else if b {
            out = true
        } else {
            go = false
        }
    }
    return out
}
"#;

const NESTED_IF_CHAIN_SOURCE: &str = r#"
micro chain_nested(a: bool, b: bool) -> bool {
    let mut out: bool = true
    let mut go: bool = true
    while go {
        if a {
            out = false
        } else {
            if b {
                out = true
            } else {
                go = false
            }
        }
    }
    return out
}
"#;

/// `else if` 链必须与语义等价的嵌套 `if` 得到同一组回边：只有循环体尾部的
/// 前向合并出口可以回跳循环头；`else` 臂不得把条件判断当成回跳目标。
#[test]
fn else_if_chain_matches_nested_if_back_edges() {
    let flat = compile(r#"
micro flat_chain(a: bool, b: bool) -> bool {
    if a {
        return true
    } else if b {
        return false
    } else {
        return true
    }
}
"#);
    let flat_function = function(&flat, "flat_chain");
    // `else if` 必须产生完整的条件分支链，而不是退化成单臂 `if`。
    let branch_count = flat_function
        .blocks
        .iter()
        .filter(|block| matches!(block.terminator, rusty_valkyrie::compiler::mir::ssa::MirTerminator::Branch { .. }))
        .count();
    assert_eq!(branch_count, 2, "`else if` 必须降低为两层条件分支，实际块结构：{:#?}", flat_function.blocks);
    let else_if = compile(ELSE_IF_CHAIN_SOURCE);
    let nested = compile(NESTED_IF_CHAIN_SOURCE);
    let else_if_function = function(&else_if, "chain_else_if");
    let nested_function = function(&nested, "chain_nested");
    for (label, function) in [("else-if", else_if_function), ("nested", nested_function)] {
        let header = function.blocks.iter().find(|block| block.label == "loop_header").expect("循环头");
        // 每条回跳循环头的边上实参数量必须与循环头参数一致；`else if` 臂合并出的
        // phi 必须随回边携带，否则下一轮读到的是循环前的旧值（表现为死循环）。
        for block in &function.blocks {
            if let rusty_valkyrie::compiler::mir::ssa::MirTerminator::Jump { target, arguments } = &block.terminator {
                if *target == header.id {
                    assert_eq!(
                        arguments.len(),
                        header.parameters.len(),
                        "{label} 回边 `{}` 的实参数量必须与循环头参数一致",
                        block.label
                    );
                }
            }
        }
    }
}

/// 把 fixture 编译到 wasm，在 Node 里对导出函数求值。
fn run_in_node(source: &str, entry: &str, script: &str) {
    let mir = compile(source);
    let canonical =
        rusty_valkyrie::compiler::emitter::pipeline::canonical_program_from_semantic_mir(&mir).expect("fixture MIR 必须满足 Canonical 合同");
    let representation =
        rusty_valkyrie::compiler::emitter::pipeline::CanonicalRepresentationPlanner.plan(&canonical).expect("fixture 必须形成表示计划");
    let program = CompiledProgram::new(canonical, representation).expect("表示输入必须一致");

    let roots: Vec<_> = program.canonical().mir.functions.keys().copied().collect();
    let fragment = program.canonical().linked.fragments.keys().next().expect("fixture 有片段").clone();
    let mut plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .expect("fixture 必须可准备为执行计划");
    let instance = mir
        .functions
        .iter()
        .find(|function| function.display_symbol.contains(entry))
        .and_then(|function| function.instance)
        .expect("fixture 有入口函数实例");
    plan.testing_export_names_mut().insert(instance, entry.to_owned());
    let submission = FragmentSubmission::from_backend_plan(plan);
    let (module, _) = nyar_emitter::lowering::backends::wasm::lower_fragment_to_wasm_module(&submission, HostProjectionBoundary::WasmJsGlue)
        .expect("fixture 必须编码为 wasm");

    let directory = tempfile::tempdir().expect("测试目录");
    let artifact = directory.path().join(format!("{entry}.wasm"));
    std::fs::write(&artifact, module.to_bytes().expect("wasm 编码成功")).expect("写入测试产物");
    let output = std::process::Command::new("node").args(["--input-type=module", "-e", script]).arg(&artifact).output().expect("Node 必须可用");
    assert!(output.status.success(), "Node 断言失败：{}", String::from_utf8_lossy(&output.stderr));
}

/// 运行期：`flag` 为真时必须返回 `b`，为假时返回 `a`。
#[test]
fn if_arm_assignment_observable_at_runtime() {
    run_in_node(
        PICK_SOURCE,
        "pick",
        "import {readFileSync} from 'node:fs'; \
         const mod=await WebAssembly.compile(readFileSync(process.argv[1])); const imports={}; for(const imp of WebAssembly.Module.imports(mod)){ if(!imports[imp.module]) imports[imp.module]={}; if(imp.kind==='function') imports[imp.module][imp.name]=()=>0; else if(imp.kind==='global') imports[imp.module][imp.name]=new WebAssembly.Global({value:'i32',mutable:true},0); else if(imp.kind==='memory') imports[imp.module][imp.name]=new WebAssembly.Memory({initial:1}); else imports[imp.module][imp.name]=new WebAssembly.Table({initial:0,element:'anyfunc'}); } \
         const instance=await WebAssembly.instantiate(mod, imports); \
         if(instance.exports.pick(1,1,0)!==0) throw new Error('flag=true 必须返回分支内的 b'); \
         if(instance.exports.pick(0,1,0)!==1) throw new Error('flag=false 必须返回初值 a');",
    );
}

/// 运行期：循环体内的 `if` 赋值必须对循环后的读取可见。
#[test]
fn if_arm_assignment_inside_loop_observable_at_runtime() {
    run_in_node(
        LOOP_PICK_SOURCE,
        "loop_pick",
        "import {readFileSync} from 'node:fs'; \
         const mod=await WebAssembly.compile(readFileSync(process.argv[1])); const imports={}; for(const imp of WebAssembly.Module.imports(mod)){ if(!imports[imp.module]) imports[imp.module]={}; if(imp.kind==='function') imports[imp.module][imp.name]=()=>0; else if(imp.kind==='global') imports[imp.module][imp.name]=new WebAssembly.Global({value:'i32',mutable:true},0); else if(imp.kind==='memory') imports[imp.module][imp.name]=new WebAssembly.Memory({initial:1}); else imports[imp.module][imp.name]=new WebAssembly.Table({initial:0,element:'anyfunc'}); } \
         const instance=await WebAssembly.instantiate(mod, imports); \
         if(instance.exports.loop_pick(1,1,0)!==0) throw new Error('循环体内 if 赋值必须生效'); \
         if(instance.exports.loop_pick(0,1,0)!==1) throw new Error('flag=false 必须返回初值 a');",
    );
}

/// 在独立 worker 里执行 wasm 导出函数，主线程超时即判定死循环。
///
/// `else if` 链若把 `else` 臂的错误目标连回条件判断，循环会一直转下去；
/// 普通断言只能看到进程挂起，因此必须由主线程 watchdog 终止并给出可诊断退出码。
fn run_in_node_with_watchdog(source: &str, entry: &str, args: &[i32], expected: i32) {
    let mir = compile(source);
    let canonical =
        rusty_valkyrie::compiler::emitter::pipeline::canonical_program_from_semantic_mir(&mir).expect("fixture MIR 必须满足 Canonical 合同");
    let representation =
        rusty_valkyrie::compiler::emitter::pipeline::CanonicalRepresentationPlanner.plan(&canonical).expect("fixture 必须形成表示计划");
    let program = CompiledProgram::new(canonical, representation).expect("表示输入必须一致");
    let roots: Vec<_> = program.canonical().mir.functions.keys().copied().collect();
    let fragment = program.canonical().linked.fragments.keys().next().expect("fixture 有片段").clone();
    let mut plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .expect("fixture 必须可准备为执行计划");
    let instance = mir
        .functions
        .iter()
        .find(|function| function.display_symbol.contains(entry))
        .and_then(|function| function.instance)
        .expect("fixture 有入口函数实例");
    plan.testing_export_names_mut().insert(instance, entry.to_owned());
    let submission = FragmentSubmission::from_backend_plan(plan);
    let (module, _) = nyar_emitter::lowering::backends::wasm::lower_fragment_to_wasm_module(&submission, HostProjectionBoundary::WasmJsGlue)
        .expect("fixture 必须编码为 wasm");

    let directory = tempfile::tempdir().expect("测试目录");
    let wasm = directory.path().join("fixture.wasm");
    std::fs::write(&wasm, module.to_bytes().expect("wasm 编码成功")).expect("写入 wasm");
    let harness = directory.path().join("harness.mjs");
    std::fs::write(&harness, WATCHDOG_HARNESS).expect("写入 watchdog harness");
    let arguments = format!("[{}]", args.iter().map(|value| value.to_string()).collect::<Vec<_>>().join(","));
    let output = std::process::Command::new("node")
        .arg(&harness)
        .arg(&wasm)
        .arg(entry)
        .arg(arguments)
        .arg(expected.to_string())
        .output()
        .expect("Node 必须可用");
    assert!(
        output.status.success(),
        "else-if 运行时断言失败（退出码 {:?}）：{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

const WATCHDOG_HARNESS: &str = r#"
import { readFileSync } from 'node:fs';
import { Worker } from 'node:worker_threads';
const [, , wasmPath, entry, argsJson, expectedRaw] = process.argv;
const workerSource = `
const { parentPort, workerData } = require('node:worker_threads');
(async () => {
  const mod = await WebAssembly.compile(workerData.bytes);
  const imports = {};
  for (const imp of WebAssembly.Module.imports(mod)) {
    if (!imports[imp.module]) imports[imp.module] = {};
    if (imp.kind === 'function') imports[imp.module][imp.name] = () => 0;
    else if (imp.kind === 'global') imports[imp.module][imp.name] = new WebAssembly.Global({ value: 'i32', mutable: true }, 0);
    else if (imp.kind === 'memory') imports[imp.module][imp.name] = new WebAssembly.Memory({ initial: 1 });
    else imports[imp.module][imp.name] = new WebAssembly.Table({ initial: 0, element: 'anyfunc' });
  }
  const instance = await WebAssembly.instantiate(mod, imports);
  parentPort.postMessage(instance.exports[workerData.entry](...workerData.args));
})();
`;
const worker = new Worker(workerSource, {
  eval: true,
  workerData: { bytes: new Uint8Array(readFileSync(wasmPath)), entry, args: JSON.parse(argsJson) },
});
const timer = setTimeout(() => {
  console.error('TIMEOUT: 导出函数未在 1500ms 内返回，判定控制流回边错误');
  process.exit(3);
}, 1500);
worker.on('message', (value) => {
  clearTimeout(timer);
  const normalized = typeof value === 'bigint' ? Number(value) : value;
  if (normalized !== Number(expectedRaw)) {
    console.error('WRONG ' + String(normalized));
    process.exit(4);
  }
  process.exit(0);
});
worker.on('error', (error) => {
  clearTimeout(timer);
  console.error(String(error));
  process.exit(5);
});
"#;

/// `else if` 的三条臂都必须在前向合并后回到循环头，而不是把条件判断当回边。
/// 输入命中 `else` 臂（该臂负责置 `go = false`）；若回边连错，watcher 会以超时失败。
#[test]
fn else_if_chain_terminates_at_runtime() {
    run_in_node_with_watchdog(ELSE_IF_CHAIN_SOURCE, "chain_else_if", &[0, 0], 1);
}

/// 同一语义的嵌套 `if` 形态必须与 `else if` 链结果一致。
#[test]
fn nested_if_chain_terminates_at_runtime() {
    run_in_node_with_watchdog(NESTED_IF_CHAIN_SOURCE, "chain_nested", &[0, 0], 1);
}

const DISCARDED_IF_SOURCE: &str = r#"
micro bool_result(flag: bool) -> bool {
    return flag
}

micro unit_result(flag: bool): unit {
    let ignored: bool = flag
}

micro statement_if_discards_arms(flag: bool, value: bool) -> bool {
    if flag {
        bool_result(value)
    } else {
        unit_result(value)
    }
    return value
}
"#;

/// 语句位置的 `if` 丢弃臂尾值：一条臂产出 `bool`、另一条产出 `unit` 时不得为
/// merge 块建值参数，否则只有单侧入边带实参，触发 SMIR007 `jump arity differs`。
#[test]
fn statement_if_arms_with_different_tail_types_do_not_merge_a_value() {
    let module = compile(DISCARDED_IF_SOURCE);
    let function = function(&module, "statement_if_discards_arms");
    let merge = function.blocks.iter().find(|block| block.label == "merge").expect("if 必须有 merge 块");
    assert!(merge.parameters.is_empty(), "语句位置 if 的 merge 块不得有值参数：{:?}", merge.parameters);
    rusty_valkyrie::compiler::emitter::pipeline::canonical_program_from_semantic_mir(&module)
        .expect("语句位置 if 的两条臂都必须以相同 arity 跳入 merge");
}

/// 运行期：两条臂都被执行到并正确汇合——`flag` 为真走 then，为假走 else。
#[test]
fn statement_if_discard_is_observable_at_runtime() {
    run_in_node(
        DISCARDED_IF_SOURCE,
        "statement_if_discards_arms",
        "import {readFileSync} from 'node:fs'; \
         const mod=await WebAssembly.compile(readFileSync(process.argv[1])); const imports={}; for(const imp of WebAssembly.Module.imports(mod)){ if(!imports[imp.module]) imports[imp.module]={}; if(imp.kind==='function') imports[imp.module][imp.name]=()=>0; else if(imp.kind==='global') imports[imp.module][imp.name]=new WebAssembly.Global({value:'i32',mutable:true},0); else if(imp.kind==='memory') imports[imp.module][imp.name]=new WebAssembly.Memory({initial:1}); else imports[imp.module][imp.name]=new WebAssembly.Table({initial:0,element:'anyfunc'}); } \
         const instance=await WebAssembly.instantiate(mod, imports); \
         if(instance.exports.statement_if_discards_arms(1,1)!==1) throw new Error('flag=true 必须走 then 臂'); \
         if(instance.exports.statement_if_discards_arms(0,1)!==1) throw new Error('flag=false 必须走 else 臂');",
    );
}

const ELSE_IF_TAIL_MISMATCH_SOURCE: &str = r#"
micro bool_result(flag: bool) -> bool {
    return flag
}

micro unit_result(flag: bool): unit {
    let ignored: bool = flag
}

micro statement_else_if_discards_arms(a: bool, b: bool) -> bool {
    if a {
        bool_result(a)
    } else if b {
        bool_result(b)
    } else {
        unit_result(b)
    }
    return true
}
"#;

/// 语句位置的 `else if` 链：内层 `if` 是 else 块的尾表达式，按值表达式 lowering 会把
/// `bool` 与 `unit` 强并到同一 merge 参数（SMIR007 block N）。丢弃规则必须沿嵌套链
/// 传递到内层 if，所有 merge 块都不得出现 `branch_result` 值参数。
#[test]
fn statement_else_if_chain_discards_nested_arm_values() {
    let module = compile(ELSE_IF_TAIL_MISMATCH_SOURCE);
    assert!(module.diagnostics.is_empty(), "语句位置 else-if 链不得产生 lowering 诊断：{:?}", module.diagnostics);
    let function = function(&module, "statement_else_if_discards_arms");
    for block in &function.blocks {
        if block.label == "merge" {
            assert!(
                block.parameters.is_empty(),
                "语句位置 else-if 的 merge 块不得有值参数：{} -> {:?}",
                block.label,
                block.parameters
            );
        }
    }
    rusty_valkyrie::compiler::emitter::pipeline::canonical_program_from_semantic_mir(&module)
        .expect("语句位置 else-if 链必须通过 Canonical 合同");
}

/// 运行期：三条臂都执行到并正确汇合，返回值不受臂尾值类型差异影响。
#[test]
fn statement_else_if_discard_is_observable_at_runtime() {
    run_in_node(
        ELSE_IF_TAIL_MISMATCH_SOURCE,
        "statement_else_if_discards_arms",
        "import {readFileSync} from 'node:fs'; \
         const mod=await WebAssembly.compile(readFileSync(process.argv[1])); const imports={}; for(const imp of WebAssembly.Module.imports(mod)){ if(!imports[imp.module]) imports[imp.module]={}; if(imp.kind==='function') imports[imp.module][imp.name]=()=>0; else if(imp.kind==='global') imports[imp.module][imp.name]=new WebAssembly.Global({value:'i32',mutable:true},0); else if(imp.kind==='memory') imports[imp.module][imp.name]=new WebAssembly.Memory({initial:1}); else imports[imp.module][imp.name]=new WebAssembly.Table({initial:0,element:'anyfunc'}); } \
         const instance=await WebAssembly.instantiate(mod, imports); \
         if(instance.exports.statement_else_if_discards_arms(1,1)!==1) throw new Error('a 为真必须走 then 臂'); \
         if(instance.exports.statement_else_if_discards_arms(0,1)!==1) throw new Error('b 为真必须走 else-if 臂'); \
         if(instance.exports.statement_else_if_discards_arms(0,0)!==1) throw new Error('两条都为假必须走 else 臂');",
    );
}

const VALUE_IF_MISMATCH_SOURCE: &str = r#"
micro bool_result(flag: bool) -> bool {
    return flag
}

micro unit_result(flag: bool): unit {
    let ignored: bool = flag
}

micro value_if_with_unit_arm(flag: bool, value: bool) -> bool {
    let chosen: bool = if flag { bool_result(value) } else { unit_result(value) }
    return chosen
}
"#;

/// 反例：值位置的 `if` 仍要求两臂类型一致，`unit` 臂不得冒充 `bool`。
/// 收紧语句位置的丢弃规则不能把值位置的类型检查一起放宽；失败必须是带源码 span
/// 与分支坐标的 lowering 诊断，而不是只有 block 编号的 SMIR007。
#[test]
fn value_position_if_with_mismatched_arm_type_reports_structured_diagnostic() {
    use rusty_valkyrie::compiler::mir::MirDiagnostic;
    use rusty_valkyrie::compiler::mir::validation::MirValidationError;

    let module = compile(VALUE_IF_MISMATCH_SOURCE);
    let error = rusty_valkyrie::compiler::mir::validation::validate_module(&module)
        .expect_err("值位置 if 的 `unit` 臂不得冒充 `bool`");
    let MirValidationError::DiagnosticRejected { diagnostic } = &error
    else {
        panic!("必须是 lowering 诊断而不是控制流合同失败：{error:?}");
    };
    let MirDiagnostic::UnsupportedExpression { span, kind } = diagnostic
    else {
        panic!("必须是 UnsupportedExpression 诊断：{diagnostic:?}");
    };
    assert!(kind.contains("分支尾值类型不一致"), "诊断必须点名分支尾值类型不一致：{kind}");
    assert!(!span.span.is_empty(), "诊断必须携带非空源码 span：{:?}", span);
}

/// 反例：人为构造的跳边实参数量与目标块参数数量不一致时，Semantic MIR 校验必须拒绝。
/// 收紧语句位置 `if` 的汇合建模不能演变成"arity 校验被放宽"。
#[test]
fn hand_built_jump_arity_mismatch_is_still_rejected() {
    use rusty_valkyrie::{
        Identifier, MirBlock, MirBlockRef, MirFunction, MirOperand, MirTerminator, MirValue, MirValueOrigin, MirValueRef, NamePath,
        mir::MirModule, types::hir::ValkyrieType, validate_semantic_module,
    };

    let stray = MirValueRef(0);
    let mut module: MirModule = rusty_valkyrie::compiler::mir::ssa::test_support::lower_test_module(Vec::new(), Vec::new());
    module.functions.push(MirFunction {
        display_symbol: "fixture.jump_arity".into(),
        symbol: NamePath::new(vec![Identifier::new("fixture"), Identifier::new("jump_arity")]),
        declaration: None,
        instance: None,
        return_type: ValkyrieType::Unit,
        param_types: Vec::new(),
        value_types: [(stray, ValkyrieType::Boolean)].into_iter().collect(),
        entry: MirBlockRef(0),
        values: vec![MirValue { id: stray, origin: MirValueOrigin::Temporary }],
        blocks: vec![
            MirBlock {
                id: MirBlockRef(0),
                label: "entry".into(),
                parameters: Vec::new(),
                instructions: Vec::new(),
                terminator: MirTerminator::Jump { target: MirBlockRef(1), arguments: vec![MirOperand::Value(stray)] },
            },
            MirBlock {
                id: MirBlockRef(1),
                label: "plain_target".into(),
                parameters: Vec::new(),
                instructions: Vec::new(),
                terminator: MirTerminator::Return { value: None },
            },
        ],
    });

    let error = validate_semantic_module(&module).expect_err("跳边实参数与目标块参数数不一致必须拒绝");
    assert_eq!(error.code, "SMIR007", "必须落在 SMIR007，实际 {error:?}");
    assert!(error.detail.contains("arity"), "必须报 arity 不一致，实际 {}", error.detail);
    assert_eq!(error.function, "fixture.jump_arity");
}

use rusty_valkyrie::compiler::mir::ssa::test_support::{block, expr, TestMirBuilder};
use rusty_valkyrie::compiler::types::hir::{
    HirBlock, HirExpr, HirExprKind, HirIdentifier, HirLiteral, HirPattern, HirStatement, HirStatementKind, ValkyrieType,
};
use rusty_valkyrie::compiler::types::SourceID;
use rusty_valkyrie::{Identifier, MirOperand, SourceSpan};

fn bool_literal(value: bool) -> HirExpr {
    expr(HirExprKind::Literal(HirLiteral::Bool(value)))
}

fn bool_let(name: &str, value: bool, mutable: bool) -> HirStatement {
    let span = test_span();
    let initializer = bool_literal(value);
    HirStatement {
        kind: HirStatementKind::Let {
            is_mutable: mutable,
            pattern: HirPattern::Variable(HirIdentifier { name: Identifier::new(name), shadow_index: 0, span: test_span() }),
            initializer: Some(Box::new(initializer)),
            ty: Some(ValkyrieType::Boolean),
            origin: Default::default(),
        },
        span,
    }
}

fn test_span() -> SourceSpan {
    SourceSpan::new(SourceID::default(), 0, 0)
}

fn bool_assign(name: &str, value: bool) -> HirExpr {
    expr(HirExprKind::Assign { target: Identifier::new(name), value: Box::new(bool_literal(value)) })
}

/// 块表达式（HIR `Block`）：`{ if condition { <name> = value } }`。
fn conditional_block(condition: bool, name: &str, value: bool) -> HirExpr {
    let assignment = HirStatement { kind: HirStatementKind::Expr(Box::new(bool_assign(name, value))), span: test_span() };
    let conditional = expr(HirExprKind::If {
        condition: Box::new(bool_literal(condition)),
        then_branch: Box::new(block(vec![assignment], None)),
        else_branch: None,
    });
    let statement = HirStatement { kind: HirStatementKind::Expr(Box::new(conditional)), span: test_span() };
    expr(HirExprKind::Block(Box::new(block(vec![statement], None))))
}

fn read_binding(builder: &mut TestMirBuilder, name: &str) -> Option<u32> {
    let identifier = HirIdentifier { name: Identifier::new(name), shadow_index: 0, span: test_span() };
    match builder.lower_expr_to_operand(&expr(HirExprKind::Variable(identifier))) {
        MirOperand::Value(value) => Some(value.0),
        _ => None,
    }
}

/// HIR `Block` 表达式的块作用域只隐藏块内新声明；对块外可变绑定的写回
/// 必须在块退出后对后继读取可见。for-in 脱糖正是把迭代器与循环包进这样的块，
/// 循环内的赋值曾因此被恢复成块前的初始槽（`remove-duplicates` 返回 0）。
#[test]
fn block_expression_writeback_is_visible_to_subsequent_read() {
    let mut builder = TestMirBuilder::new();
    builder.lower_statement(&bool_let("x", true, true));
    let before = read_binding(&mut builder, "x").expect("`x` 必须是 SSA 绑定");
    builder.lower_expr_to_operand(&conditional_block(true, "x", false));
    let after = read_binding(&mut builder, "x").expect("块退出后 `x` 必须仍可读");
    assert_ne!(before, after, "块内写回被块退出时的作用域恢复丢弃，后继读取回到初始槽");
}

/// 反例：块内同名 `let` 是新的作用域绑定，不得泄漏为块外 `x` 的写回。
#[test]
fn block_expression_shadowing_does_not_overwrite_outer_binding() {
    let mut builder = TestMirBuilder::new();
    builder.lower_statement(&bool_let("x", true, true));
    let before = read_binding(&mut builder, "x").expect("`x` 必须是 SSA 绑定");
    let inner = expr(HirExprKind::Block(Box::new(block(vec![bool_let("x", false, false)], None))));
    builder.lower_expr_to_operand(&inner);
    let after = read_binding(&mut builder, "x").expect("块退出后 `x` 必须仍可读");
    assert_eq!(before, after, "块内同名 `let` 不得改写块外绑定");
}

/// 矩阵：嵌套块逐层退出时都要把块外写回传递出去。
#[test]
fn block_expression_nested_blocks_propagate_writeback() {
    use rusty_valkyrie::compiler::mir::ssa::test_support::block;
    let mut builder = TestMirBuilder::new();
    builder.lower_statement(&bool_let("x", true, true));
    let before = read_binding(&mut builder, "x").expect("`x` 必须是 SSA 绑定");
    let inner = conditional_block(true, "x", false);
    let statement = HirStatement { kind: HirStatementKind::Expr(Box::new(inner)), span: test_span() };
    let outer = expr(HirExprKind::Block(Box::new(block(vec![statement], None))));
    builder.lower_expr_to_operand(&outer);
    assert_ne!(read_binding(&mut builder, "x"), Some(before), "嵌套块退出时写回被丢弃");
}

/// 矩阵：块内新增声明只隐藏该名字；其它块外绑定的写回仍必须保留。
#[test]
fn block_expression_local_declaration_does_not_suppress_outer_writeback() {
    use rusty_valkyrie::compiler::mir::ssa::test_support::block;
    let mut builder = TestMirBuilder::new();
    builder.lower_statement(&bool_let("x", true, true));
    let before = read_binding(&mut builder, "x").expect("`x` 必须是 SSA 绑定");
    let write = HirStatement { kind: HirStatementKind::Expr(Box::new(bool_assign("x", false))), span: test_span() };
    let inner = expr(HirExprKind::Block(Box::new(block(vec![bool_let("y", true, false), write], None))));
    builder.lower_expr_to_operand(&inner);
    assert_ne!(read_binding(&mut builder, "x"), Some(before), "块内局部声明不得抑制其它块外绑定的写回");
}

/// 反向风险：块内新声明只属于块作用域，退出后不得泄漏成外层可读绑定。
#[test]
fn block_expression_local_declaration_does_not_leak() {
    use rusty_valkyrie::compiler::mir::ssa::test_support::block;
    let mut builder = TestMirBuilder::new();
    let inner = expr(HirExprKind::Block(Box::new(block(vec![bool_let("y", true, false)], None))));
    builder.lower_expr_to_operand(&inner);
    assert_eq!(read_binding(&mut builder, "y"), None, "块内 `let y` 退出后仍可读，说明新声明泄漏到外层绑定");
}

fn nullable_bool_let(name: &str, mutable: bool) -> HirStatement {
    HirStatement {
        kind: HirStatementKind::Let {
            is_mutable: mutable,
            pattern: HirPattern::Variable(HirIdentifier { name: Identifier::new(name), shadow_index: 0, span: test_span() }),
            initializer: Some(Box::new(expr(HirExprKind::Literal(HirLiteral::Null)))),
            ty: Some(ValkyrieType::Nullable(Box::new(ValkyrieType::Boolean))),
            origin: Default::default(),
        },
        span: test_span(),
    }
}

fn try_scope(body: HirBlock) -> HirExpr {
    expr(HirExprKind::TryScope { is_optional: false, is_forced: false, result_type: None, body: Box::new(body) })
}

/// 残余形态：没有 `?` 提前出口的 try 作用域只有单一 fallthrough 前驱，等价于普通块，
/// 块外写回必须继续传播，不能再退回入口槽。
#[test]
fn try_scope_without_early_exit_propagates_outer_writeback() {
    let mut builder = TestMirBuilder::new();
    builder.lower_statement(&bool_let("x", true, true));
    let before = read_binding(&mut builder, "x").expect("`x` 必须是 SSA 绑定");
    let write = HirStatement { kind: HirStatementKind::Expr(Box::new(bool_assign("x", false))), span: test_span() };
    builder.lower_expr_to_operand(&try_scope(block(vec![write], None)));
    assert_ne!(read_binding(&mut builder, "x"), Some(before), "无提前出口的 try 作用域必须像普通块一样保留块外写回");
    assert!(builder.diagnostics().is_empty(), "无提前出口的 try 写回不应产生诊断：{:?}", builder.diagnostics());
}

/// 残余形态：`?` 可以在块外写回之前退出 `try_exit`，写回值没有被线程化成
/// `try_exit` 的块参数；此形态当前无法表达，必须结构化拒绝而不是静默恢复入口槽。
#[test]
fn try_scope_writeback_with_early_exit_is_rejected() {
    let mut builder = TestMirBuilder::new();
    builder.lower_statement(&bool_let("x", true, true));
    builder.lower_statement(&nullable_bool_let("n", false));
    let write = HirStatement { kind: HirStatementKind::Expr(Box::new(bool_assign("x", false))), span: test_span() };
    let propagate = HirStatement {
        kind: HirStatementKind::Expr(Box::new(expr(HirExprKind::TryPropagate {
            inner: Box::new(expr(HirExprKind::Variable(HirIdentifier { name: Identifier::new("n"), shadow_index: 0, span: test_span() }))),
            resolved_ok_variant: None,
        }))),
        span: test_span(),
    };
    builder.lower_expr_to_operand(&try_scope(block(vec![write, propagate], None)));
    let rejected = builder.diagnostics().iter().any(|diagnostic| {
        matches!(
            diagnostic,
            rusty_valkyrie::MirDiagnostic::UnsupportedExpression { kind, .. }
                if kind.contains("try scope writes outer binding `x`") && kind.contains("not threaded")
        )
    });
    assert!(rejected, "带 `?` 提前出口的 try 写回必须 fail-closed，实际诊断：{:?}", builder.diagnostics());
}

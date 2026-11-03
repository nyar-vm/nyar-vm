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
    plan.testing_wasm_export_names_mut().insert(instance, entry.to_owned());
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
    plan.testing_wasm_export_names_mut().insert(instance, entry.to_owned());
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
/// 收紧语句位置的丢弃规则不能把值位置的类型检查一起放宽。
#[test]
fn value_position_if_with_mismatched_arm_type_still_fails() {
    let module = compile(VALUE_IF_MISMATCH_SOURCE);
    let error = rusty_valkyrie::compiler::emitter::pipeline::canonical_program_from_semantic_mir(&module)
        .expect_err("值位置 if 的 `unit` 臂不得冒充 `bool`");
    let message = format!("{error:?}");
    assert!(
        message.contains("SMIR007") || message.contains("CAN010") || message.contains("arity") || message.contains("type"),
        "必须保持结构化失败，实际：{message}"
    );
}

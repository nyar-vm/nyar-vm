//! `while true { 赋值; if cond { break } }` 的 break 出边必须把循环携带变量的
//! 当前新值传入 exit 侧 phi。
//!
//! 修复前：break 的 `Jump { exit }` 只带 break 值、不带携带变量；循环后的读取
//! 绑到 header 参数（上一轮迭代入口值），break 路径静默读到过期值。修复后：
//! exit 块为每个携带名建立 phi 参数，break 边传入 break 点新值，header 条件为
//! 假的正常退出边经 stub 块（`Jump { exit, header 参数 }`）传入入口值。
//!
//! 用例同时断言 SSA 形状（exit 参数、break/stub 入边实参、return 读取的 phi）
//! 与 Node 实跑 wasm 的运行期返回值；携带形态覆盖 `StoreVar`（标量赋值）与
//! `AggregateCopy`（值存储类型赋值）两种。
//!
//! fixture 只用 `bool` 标量、`(bool, bool)` 元组与 `if` / `while`：不依赖任何
//! 算子 adaptor，独立验证循环 break 出边的 SSA 合并。

use nyar::HostProjectionBoundary;
use nyar_emitter::{CompiledProgram, FragmentSubmission, TargetExecutionPlan};
use nyar_types::pipeline::RepresentationPlanStage;
use rusty_valkyrie::{MirLowerer, ValkyrieCompiler, compiler::mir::ssa::MirValueOrigin};

const PRIMITIVES: &str = r#"
namespace core::primitive;
[primitive("core::primitive::bool")] structure bool { }
"#;

fn compile(source: &str) -> rusty_valkyrie::compiler::mir::ssa::MirModule {
    let source = format!("{PRIMITIVES}\n{source}");
    let hir = ValkyrieCompiler::new(rusty_valkyrie::compiler::types::SourceID { version_id: 9701 })
        .compile_source(&source)
        .expect("fixture 必须完成 HIR 分析");
    MirLowerer::lower_module_semantic(&hir)
}

fn function<'a>(module: &'a rusty_valkyrie::compiler::mir::ssa::MirModule, name: &str) -> &'a rusty_valkyrie::compiler::mir::ssa::MirFunction {
    module.functions.iter().find(|function| function.display_symbol.contains(name)).unwrap_or_else(|| panic!("fixture 缺少函数 `{name}`"))
}

fn block<'a>(function: &'a rusty_valkyrie::compiler::mir::ssa::MirFunction, label: &str) -> &'a rusty_valkyrie::compiler::mir::ssa::MirBlock {
    function.blocks.iter().find(|block| block.label == label).unwrap_or_else(|| panic!("fixture 缺少 `{label}` 块"))
}

fn origin<'a>(
    function: &'a rusty_valkyrie::compiler::mir::ssa::MirFunction,
    value: rusty_valkyrie::compiler::mir::ssa::MirValueRef,
) -> &'a MirValueOrigin {
    &function.values.iter().find(|entry| entry.id == value).unwrap_or_else(|| panic!("SSA 值 {value:?} 不在函数内")).origin
}

/// 块参数对应的携带变量名（只认 BlockParameter 来源）。
fn parameter_names(
    function: &rusty_valkyrie::compiler::mir::ssa::MirFunction,
    block: &rusty_valkyrie::compiler::mir::ssa::MirBlock,
) -> Vec<String> {
    block
        .parameters
        .iter()
        .filter_map(|parameter| match origin(function, *parameter) {
            MirValueOrigin::BlockParameter { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

/// 所有以 `Jump` 进入 `target` 的入边（源块 id + 实参）。
fn incoming_jumps(
    function: &rusty_valkyrie::compiler::mir::ssa::MirFunction,
    target: rusty_valkyrie::compiler::mir::ssa::MirBlockRef,
) -> Vec<(u32, Vec<rusty_valkyrie::compiler::mir::ssa::MirOperand>)> {
    function
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            rusty_valkyrie::compiler::mir::ssa::MirTerminator::Jump { target: edge, arguments } if *edge == target => {
                Some((block.id.0, arguments.clone()))
            }
            _ => None,
        })
        .collect()
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

/// Floyd 判别式：`while true { 赋值; if cond { break } }`，break 在携带变量
/// 已被改写之后离开循环。
const MEET_FLIP_SOURCE: &str = r#"
micro meet_flip(a: bool, b: bool) -> bool {
    let mut x: bool = a
    let mut parity: bool = false
    while true {
        x = if parity { b } else { a }
        if parity {
            break
        }
        parity = true
    }
    return x
}
"#;

/// 同一循环既有 break 出边又有正常退出（条件转假）出边。
const FLIP_OR_DRAIN_SOURCE: &str = r#"
micro flip_or_drain(flag: bool, a: bool, b: bool) -> bool {
    let mut x: bool = a
    let mut go: bool = true
    while go {
        if flag {
            x = b
            break
        }
        go = false
    }
    return x
}
"#;

/// 值存储类型（tuple）经 `AggregateCopy` 赋值的携带变量。
const PAIR_FLIP_SOURCE: &str = r#"
micro pair_flip(a: bool, b: bool) -> (bool, bool) {
    let mut pair: (bool, bool) = (a, b)
    let mut parity: bool = false
    while true {
        pair = (b, a)
        if parity {
            break
        }
        parity = true
    }
    return pair
}
"#;

/// break 出边与正常退出 stub 都必须按 exit 参数 arity 传入携带变量。
#[test]
fn break_edge_carries_loop_carried_variables_into_exit_phi() {
    let module = compile(MEET_FLIP_SOURCE);
    let function = function(&module, "meet_flip");
    let header = block(function, "loop_header");
    let exit = block(function, "loop_exit");
    let carried = parameter_names(function, exit);
    assert!(carried.iter().any(|name| name == "x"), "break 携带的 x 必须有 exit phi：{carried:?}");
    assert!(carried.iter().any(|name| name == "parity"), "循环体内改写的 parity 必须有 exit phi：{carried:?}");

    // exit 的每条 Jump 入边实参数必须与参数数一致（SMIR007 合同）。
    let edges = incoming_jumps(function, exit.id);
    assert!(edges.len() >= 2, "break 边与正常退出 stub 边都必须进入 exit，实际 {edges:?}");
    for (source, arguments) in &edges {
        assert_eq!(arguments.len(), exit.parameters.len(), "块 {source} 进入 exit 的实参数必须等于 exit 参数数");
        for argument in arguments {
            let rusty_valkyrie::compiler::mir::ssa::MirOperand::Value(value) = argument
            else {
                continue;
            };
            assert!(function.value_types.contains_key(value), "exit 入边实参必须有 SSA 类型");
        }
    }

    // break 边（来自循环体区域，非 stub）传入的 x 必须是本轮 StoreVar 新值，
    // 而不是 header 参数（上一轮入口值）。
    let stub = block(function, "loop_normal_exit");
    let break_edge = edges.iter().find(|(source, _)| *source != stub.id.0).expect("必须有 break 入边");
    let x_argument_index = carried.iter().position(|name| name == "x").expect("exit 参数必须含 x");
    let x_argument = match &break_edge.1[x_argument_index] {
        rusty_valkyrie::compiler::mir::ssa::MirOperand::Value(value) => *value,
        other => panic!("break 边必须传 SSA 值，实际 {other:?}"),
    };
    assert!(
        !matches!(origin(function, x_argument), MirValueOrigin::BlockParameter { block, .. } if *block == header.id),
        "break 边的 x 必须是 break 点新值而不是 header 参数"
    );

    // stub 边（正常退出）传入的必须是 header 参数值。
    let stub_edge =
        incoming_jumps(function, exit.id).into_iter().find(|(source, _)| *source == stub.id.0).expect("正常退出必须经 stub 边进入 exit");
    let stub_x = match &stub_edge.1[x_argument_index] {
        rusty_valkyrie::compiler::mir::ssa::MirOperand::Value(value) => *value,
        other => panic!("stub 边必须传 SSA 值，实际 {other:?}"),
    };
    assert!(
        matches!(origin(function, stub_x), MirValueOrigin::BlockParameter { block, .. } if *block == header.id),
        "正常退出边必须传 header 参数"
    );

    // 循环后的读取必须绑到 exit phi，而不是 header 参数。
    match origin(function, returned_value(function)) {
        MirValueOrigin::BlockParameter { block, name } => {
            assert_eq!(*block, exit.id, "return 必须读取 exit 块的 phi");
            assert_eq!(name, "x");
        }
        other => panic!("循环后读取了 break 路径外的旧绑定：{other:?}"),
    }
}

/// `AggregateCopy` 形态的携带变量（tuple 赋值）同样必须进入 exit phi。
#[test]
fn aggregate_copy_assignment_is_loop_carried_into_exit_phi() {
    let module = compile(PAIR_FLIP_SOURCE);
    let function = function(&module, "pair_flip");
    let exit = block(function, "loop_exit");
    let carried = parameter_names(function, exit);
    assert!(carried.iter().any(|name| name == "pair"), "仅经 AggregateCopy 改写的 pair 必须是循环携带变量并有 exit phi：{carried:?}");
    let edges = incoming_jumps(function, exit.id);
    for (source, arguments) in &edges {
        assert_eq!(arguments.len(), exit.parameters.len(), "块 {source} 进入 exit 的实参数必须等于 exit 参数数");
    }
    let header = block(function, "loop_header");
    assert!(parameter_names(function, header).iter().any(|name| name == "pair"), "pair 必须同时是 header phi（continue/back-edge 携带）");
    match origin(function, returned_value(function)) {
        MirValueOrigin::BlockParameter { block, name } => {
            assert_eq!(*block, exit.id, "return 必须读取 exit 块的 pair phi");
            assert_eq!(name, "pair");
        }
        other => panic!("循环后读取了 break 路径外的旧绑定：{other:?}"),
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

/// 运行期：break 路径必须返回 break 点的新值 `b`，而不是上一轮入口值 `a`。
#[test]
fn break_carried_value_observable_at_runtime() {
    run_in_node(
        MEET_FLIP_SOURCE,
        "meet_flip",
        "import {readFileSync} from 'node:fs'; \
         const {instance}=await WebAssembly.instantiate(readFileSync(process.argv[1])); \
         if(instance.exports.meet_flip(1,0)!==0) throw new Error('break 路径必须返回循环体内改写后的 b'); \
         if(instance.exports.meet_flip(0,1)!==1) throw new Error('break 路径必须返回改写后的 b=1');",
    );
}

/// 运行期：break 出边返回改写值；条件转假的正常退出返回最后一轮入口值。
#[test]
fn normal_exit_and_break_edges_both_observable_at_runtime() {
    run_in_node(
        FLIP_OR_DRAIN_SOURCE,
        "flip_or_drain",
        "import {readFileSync} from 'node:fs'; \
         const {instance}=await WebAssembly.instantiate(readFileSync(process.argv[1])); \
         if(instance.exports.flip_or_drain(1,1,0)!==0) throw new Error('break 路径必须返回分支内改写后的 b'); \
         if(instance.exports.flip_or_drain(0,1,0)!==1) throw new Error('正常退出必须返回初值 a');",
    );
}

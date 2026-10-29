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
namespace core::primitive;
[primitive("core::primitive::bool")] structure bool { }
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
         const {instance}=await WebAssembly.instantiate(readFileSync(process.argv[1])); \
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
         const {instance}=await WebAssembly.instantiate(readFileSync(process.argv[1])); \
         if(instance.exports.loop_pick(1,1,0)!==0) throw new Error('循环体内 if 赋值必须生效'); \
         if(instance.exports.loop_pick(0,1,0)!==1) throw new Error('flag=false 必须返回初值 a');",
    );
}

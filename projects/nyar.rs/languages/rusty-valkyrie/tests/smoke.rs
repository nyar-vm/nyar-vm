//! rusty-valkyrie 的最小冒烟门。
//!
//! 重型管线/后端/Asgard/Legion 套件已迁往 `valkyrie.rs` 的下游 harness。
//! 本 crate 只保留最基本的“前端可解析、编译器不 panic”检查，避免测试门禁
//! 反向约束架构重构。

use rusty_valkyrie::{ValkyrieCompiler, parser::parse_source};

#[test]
fn parses_a_minimal_function() {
    let root = parse_source("micro answer(): i32 {\n    return 42\n}\n").expect("minimal source must parse");
    assert_eq!(root.items.len(), 1, "one function declaration must survive parsing");
}

#[test]
fn minimal_entry_compiles() {
    let source = "[main] micro main() { }";
    let groups = [rusty_valkyrie::CompilerSourceGroup::single_source("smoke", "smoke", source).as_entry()];
    let program = ValkyrieCompiler::default().compile_source_groups_to_program_with_host_bindings(&groups, &[], &[], "wasm");
    assert!(program.is_ok(), "minimal entry must compile without a structured failure: {program:?}");
}

/// 端到端最小门：源码 → Valkyrie MIR → 语言 × 目标适配器 → Wasm 产物。
///
/// 该用例覆盖正式 `AdaptedFragmentInput` 接入路径，确保语义载荷归还语言后
/// 仍能真实产出后端制品，而不是只通过手工构造计划的 lowering 测试。
/// 按目标 lane 的宿主调用约定构造执行命令。
///
/// 这里只知道外部工具的命令行形状（`java -jar` / `dotnet exec` / `node`），
/// 不推断语言或产物语义；产物路径来自后端 run contract。
fn host_command(invocation: &str, artifact: &std::path::Path) -> std::process::Command {
    match invocation {
        "java" => {
            let mut command = std::process::Command::new("java");
            command.arg("-jar").arg(artifact);
            command
        }
        "dotnet" => {
            let mut command = std::process::Command::new("dotnet");
            command.arg("exec").arg(artifact);
            command
        }
        "node" => {
            let mut command = std::process::Command::new("node");
            command.arg(artifact);
            command
        }
        other => panic!("unsupported host invocation `{other}`"),
    }
}

/// 源码 → 适配器 → 目标产物 → 宿主执行的统一最小门。
///
/// `invocation` 为 `None` 时取该 lane 的第一条运行契约（原生镜像的入口就是
/// 产物本身，没有外部解释器命令）。
fn compile_and_run_on_host(target: &str, invocation: Option<&str>) {
    use nyar::ClrSuspendStrategy;
    use nyar_emitter::nyar_backend_wasi::WasmPackageKind;
    use rusty_valkyrie::compiler::emitter::pipeline::{CompilerBuildContext, compile_source_groups_to_artifacts};

    let groups = vec![rusty_valkyrie::CompilerSourceGroup::single_source("smoke", "smoke", "[main] micro main() { }").as_entry()];
    let input = rusty_valkyrie::compiler::hir::lowering::CompilerSourceInput::application(groups);
    let context = CompilerBuildContext::new(
        "smoke-arch",
        nyar::CanonicalTarget::parse(target).unwrap_or_else(|error| panic!("target `{target}`: {error}")),
        ClrSuspendStrategy::from_runtime_async_flag(false),
        WasmPackageKind::Binary,
    );
    let dir = tempfile::tempdir().expect("temp dir");
    // CLR 的框架依赖镜像需要 runtime config 才能被 `dotnet exec` 装载。
    let report = compile_source_groups_to_artifacts(&ValkyrieCompiler::default(), &input, &context, dir.path(), "smoke", false, true)
        .unwrap_or_else(|error| panic!("`{target}` artifact must be produced through the adapter: {error}"));
    assert!(!report.driver.artifacts.artifacts.is_empty(), "`{target}` adapter path must emit at least one artifact");

    let contract = match invocation {
        Some(invocation) => report
            .driver
            .run_contracts
            .iter()
            .find(|contract| contract.invocation == invocation)
            .unwrap_or_else(|| panic!("`{target}` must emit a `{invocation}` run contract, got {:?}", report.driver.run_contracts)),
        None => report
            .driver
            .run_contracts
            .first()
            .unwrap_or_else(|| panic!("`{target}` must emit a run contract")),
    };
    // 运行契约的 `validate` 是该 lane 声明的宿主命令形状（`java -cp ... <launcher>`、
    // `dotnet exec <artifact>`）；测试按契约执行，不自行猜参数或入口名。
    let mut tokens = contract.validate.split_whitespace();
    let program = tokens.next().unwrap_or_else(|| panic!("`{target}` run contract has an empty validate command"));
    // 宿主工具（java/dotnet/node）按名字从 PATH 解析；原生镜像的入口是输出目录内
    // 的产物本身，必须用绝对路径，不能依赖 PATH 或调用方 cwd。
    let in_output_dir = dir.path().join(program);
    let program_path = if in_output_dir.exists() { in_output_dir } else { std::path::PathBuf::from(program) };
    let mut command = std::process::Command::new(&program_path);
    command.args(tokens);
    command.current_dir(dir.path());
    let output = command.output().unwrap_or_else(|error| panic!("spawn `{}`: {error}", program_path.display()));
    assert!(
        output.status.success(),
        "`{target}` host run failed for `{}`: status={:?} stderr={}",
        contract.physical_entry,
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn minimal_entry_runs_on_jvm() {
    compile_and_run_on_host("jvm", Some("java"));
}

#[test]
fn minimal_entry_runs_on_clr() {
    compile_and_run_on_host("clr", Some("dotnet"));
}

/// native lane 当前只闭合 `x86_64` + Windows + PE。
///
/// 宿主是 Windows 时真实执行产物；其它宿主必须给出结构化「无该容器实现」失败，
/// 不能借用其它平台的镜像格式，也不能用跳过冒充通过。
#[test]
fn minimal_entry_runs_on_native() {
    if cfg!(windows) {
        compile_and_run_on_host("native", None);
        return;
    }
    use nyar::ClrSuspendStrategy;
    use nyar_emitter::nyar_backend_wasi::WasmPackageKind;
    use rusty_valkyrie::compiler::emitter::pipeline::{CompilerBuildContext, compile_source_groups_to_artifacts};

    let groups = vec![rusty_valkyrie::CompilerSourceGroup::single_source("smoke", "smoke", "[main] micro main() { }").as_entry()];
    let input = rusty_valkyrie::compiler::hir::lowering::CompilerSourceInput::application(groups);
    let context = CompilerBuildContext::new(
        "native",
        nyar::CanonicalTarget::parse("native").expect("host native target"),
        ClrSuspendStrategy::from_runtime_async_flag(false),
        WasmPackageKind::Binary,
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let error = compile_source_groups_to_artifacts(&ValkyrieCompiler::default(), &input, &context, dir.path(), "smoke", false, true)
        .expect_err("非 Windows 宿主的 native lane 必须结构化拒绝");
    let message = error.to_string();
    assert!(message.contains("容器实现") || message.contains("尚未实现"), "unexpected native error: {message}");
}

/// WASI lane 真实装载：源码 → `.wasi` component → `wasmtime run`。
///
/// 入口声明 `[utf8]` argv 形参，迫使入口包装体走
/// `wasi:cli/environment#get-arguments` 的 canonical ABI 合成路径，而不是
/// 用空数组或缺省值绕过：宿主必须把真实实参 lift 进 guest 线性内存，再由
/// 包装体构建 GC 数组。夹具没有 std 包（数组下标/`length` 都是 std 单例），
/// 因此这里只断言派生路径被真实执行，数值比较交给下游 harness。
#[test]
fn minimal_entry_runs_on_wasi_with_real_argv() {
    use nyar::ClrSuspendStrategy;
    use nyar_emitter::nyar_backend_wasi::WasmPackageKind;
    use rusty_valkyrie::compiler::emitter::pipeline::{CompilerBuildContext, compile_source_groups_to_artifacts};

    let source = "[main]\nmicro main(args: [utf8]) {\n}\n";
    let groups = vec![rusty_valkyrie::CompilerSourceGroup::single_source("smoke", "smoke", source).as_entry()];
    let input = rusty_valkyrie::compiler::hir::lowering::CompilerSourceInput::application(groups);
    let context = CompilerBuildContext::new(
        "wasi",
        nyar::CanonicalTarget::parse("wasi").expect("wasi target"),
        ClrSuspendStrategy::from_runtime_async_flag(false),
        WasmPackageKind::Binary,
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let report = compile_source_groups_to_artifacts(&ValkyrieCompiler::default(), &input, &context, dir.path(), "smoke", false, false)
        .expect("argv entry must produce a WASI component through the adapter");
    let contract = report
        .driver
        .run_contracts
        .iter()
        .find(|contract| contract.invocation == "wasmtime")
        .unwrap_or_else(|| panic!("wasi target must emit a wasmtime run contract, got {:?}", report.driver.run_contracts));

    // 组件必须真的导入 argv 接口，否则入口包装体只是拿到默认空数组。
    let component_path = dir.path().join(&contract.physical_entry);
    let wit_dump = std::process::Command::new("wasm-tools")
        .args(["component", "wit", component_path.to_string_lossy().as_ref()])
        .output()
        .expect("spawn wasm-tools component wit");
    assert!(wit_dump.status.success(), "stderr={}", String::from_utf8_lossy(&wit_dump.stderr));
    let wit_text = String::from_utf8(wit_dump.stdout).expect("utf8 component wit");
    assert!(wit_text.contains("wasi:cli/environment"), "component must import `wasi:cli/environment`, wit={wit_text}");
    assert!(wit_text.contains("get-arguments"), "component must import `get-arguments`, wit={wit_text}");

    let mut tokens = contract.validate.split_whitespace();
    let program = tokens.next().expect("wasmtime run contract has an empty validate command");
    let mut command = std::process::Command::new(program);
    command.args(tokens).arg("smoke-argv-probe").current_dir(dir.path());
    let output = command.output().unwrap_or_else(|error| panic!("spawn `{program}`: {error}"));
    assert!(
        output.status.success(),
        "`wasi` host run failed for `{}`: status={:?} stderr={}",
        contract.physical_entry,
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// OVM 用仓库内 OdinVM 真实装载并执行 `.odin`，不依赖外部 `odin` 工具链。
#[test]
fn minimal_entry_runs_on_ovm() {
    use nyar::ClrSuspendStrategy;
    use nyar_emitter::nyar_backend_wasi::WasmPackageKind;
    use rusty_valkyrie::compiler::emitter::pipeline::{CompilerBuildContext, compile_source_groups_to_artifacts};

    let groups = vec![rusty_valkyrie::CompilerSourceGroup::single_source("smoke", "smoke", "[main] micro main() { }").as_entry()];
    let input = rusty_valkyrie::compiler::hir::lowering::CompilerSourceInput::application(groups);
    let context = CompilerBuildContext::new(
        "ovm",
        nyar::CanonicalTarget::parse("ovm").expect("ovm target"),
        ClrSuspendStrategy::from_runtime_async_flag(false),
        WasmPackageKind::Binary,
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let report = compile_source_groups_to_artifacts(&ValkyrieCompiler::default(), &input, &context, dir.path(), "smoke", false, false)
        .expect("minimal source must produce an OVM artifact through the adapter");
    let contract = report
        .driver
        .run_contracts
        .iter()
        .find(|contract| contract.invocation == "ovm")
        .expect("ovm target must emit an `ovm` run contract");
    let bytes = std::fs::read(dir.path().join(&contract.physical_entry)).expect("read .odin artifact");
    let mut vm = odin_vm::OdinVm::new();
    let loaded = vm.load(&bytes).expect("OdinVM must load the emitted module");
    vm.run(&loaded, &contract.logical_entry, Vec::new())
        .unwrap_or_else(|error| panic!("OdinVM run `{}` failed: {error}", contract.logical_entry));
}

#[test]
fn minimal_entry_emits_wasm_artifact() {
    use nyar::ClrSuspendStrategy;
    use nyar_emitter::nyar_backend_wasi::WasmPackageKind;
    use rusty_valkyrie::compiler::emitter::pipeline::{CompilerBuildContext, compile_source_groups_to_artifacts};

    let groups = vec![rusty_valkyrie::CompilerSourceGroup::single_source("smoke", "smoke", "[main] micro main() { }").as_entry()];
    let input = rusty_valkyrie::compiler::hir::lowering::CompilerSourceInput::application(groups);
    let context = CompilerBuildContext::new(
        "wasm32",
        nyar::CanonicalTarget::parse("node").expect("node target"),
        ClrSuspendStrategy::from_runtime_async_flag(false),
        WasmPackageKind::Binary,
    );
    let dir = tempfile::tempdir().expect("temp dir");
    let report = compile_source_groups_to_artifacts(
        &ValkyrieCompiler::default(),
        &input,
        &context,
        dir.path(),
        "smoke",
        false,
        false,
    )
    .expect("minimal source must produce a target artifact through the adapter");
    assert!(!report.driver.artifacts.artifacts.is_empty(), "adapter path must emit at least one artifact");

    // 真实宿主装载：把适配器产出的 Wasm + glue 交给 Node 执行，拒绝「只生成文件、
    // 从不加载」的假交付。run contract 由后端按入口身份生成，不由测试拼名字。
    let contract = report
        .driver
        .run_contracts
        .iter()
        .find(|contract| contract.invocation == "node")
        .expect("node target must emit a node run contract");
    let entry = dir.path().join(&contract.physical_entry);
    let output = host_command("node", &entry).current_dir(dir.path()).output().expect("spawn node for emitted wasm artifact");
    assert!(
        output.status.success(),
        "node failed for `{}`: status={:?} stderr={}",
        contract.physical_entry,
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

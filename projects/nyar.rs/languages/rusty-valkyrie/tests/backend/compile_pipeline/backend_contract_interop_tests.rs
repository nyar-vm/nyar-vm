use nyar_types::SourceID;

use rusty_valkyrie::{
    HirError,
    compiler::{emitter::backend_contract::interop::BackendContractError, hir::ValkyrieCompiler},
    function_interop_contract, *,
};

use crate::primitives::{SourceFixtureCompile, with_primitives};

#[test]
fn maps_c_binding_attribute_to_win32_host_link() {
    let compiler = ValkyrieCompiler::new(SourceID { version_id: 900 });
    let module = compiler
        .compile_fixture_source(
            r#"
[c("kernel32", "WriteFile")]
micro helper(message: utf8): unit {
    return;
}
"#,
        )
        .unwrap();
    let contract = function_interop_contract(&module.functions[0]).expect("expected interop contract");
    assert!(contract.matches_host_platform("win32"));
    assert_eq!(contract.locator_segments(), &["kernel32", "WriteFile"]);
}

#[test]
fn maps_syscall_binding_to_neutral_host_link() {
    let compiler = ValkyrieCompiler::new(SourceID { version_id: 901 });
    let module = compiler
        .compile_fixture_source(
            r#"
[syscall(1)]
micro helper(message: utf8): unit {
    return;
}
"#,
        )
        .unwrap();
    let contract = function_interop_contract(&module.functions[0]).expect("expected interop contract");
    assert!(contract.matches_boundary("host"));
    assert!(contract.platform_tag.is_none());
    assert_eq!(contract.locator_segments(), &["syscall", "1"]);
}

#[test]
fn maps_abstract_syscall_binding_and_call_edge() {
    let compiler = ValkyrieCompiler::new(SourceID { version_id: 902 });
    let module = compiler
        .compile_fixture_source(
            r#"
[syscall(1)]
micro console_write(message: utf8): i32;

[main]
micro main() -> i64 {
    console_write("hello from linux")
    return 0;
}
"#,
        )
        .unwrap();
    let write = module.functions.iter().find(|function| function.name.as_str() == "console_write").expect("console_write");
    assert!(write.is_abstract);
    let contract = function_interop_contract(write).expect("expected syscall contract");
    assert_eq!(contract.locator_segments(), &["syscall", "1"]);

    let build_output = compiler
        .compile_source_to_program(&with_primitives(
            r#"
[syscall(1)]
micro console_write(message: utf8): i32;

[main]
micro main() -> i64 {
    console_write("hello from linux")
    return 0;
}
"#,
        ))
        .unwrap();
    let fragment = build_output.canonical().linked.fragments.values().next().expect("Canonical 片段");
    assert_eq!(fragment.external_imports.len(), 1, "links={:?}", fragment.external_imports);
    assert_eq!(fragment.external_call_edges.len(), 1, "edges={:?}", fragment.external_call_edges);
}

#[test]
fn bare_abstract_declaration_has_no_import_contract() {
    let compiler = ValkyrieCompiler::new(SourceID { version_id: 903 });
    let module = compiler
        .compile_fixture_source(
            r#"
micro declaration(message: utf8): unit;
"#,
        )
        .unwrap();
    let declaration = module.functions.iter().find(|function| function.name.as_str() == "declaration").expect("declaration");
    assert!(declaration.is_abstract);
    assert!(function_interop_contract(declaration).is_none());
    let output = compiler.compile_source_to_program("micro declaration(message: utf8): unit;").expect("declaration output");
    let semantic_mir = rusty_valkyrie::compiler::mir::MirLowerer::lower_module_semantic(
        &compiler.compile_fixture_source("micro declaration(message: utf8): unit;").expect("declaration source"),
    );
    assert!(semantic_mir.external_calls.is_empty());
}

#[test]
fn host_contract_requires_explicit_annotation() {
    let compiler = ValkyrieCompiler::new(SourceID { version_id: 904 });
    let module = compiler
        .compile_fixture_source(
            r#"
[host_contract]
micro declaration(message: utf8): unit;
"#,
        )
        .unwrap();
    let declaration = module.functions.iter().find(|function| function.name.as_str() == "declaration").expect("declaration");
    let contract = function_interop_contract(declaration).expect("explicit host contract");
    assert!(contract.matches_boundary("host"));
    assert!(contract.locator_segments().is_empty());
    let output =
        compiler.compile_source_to_program("[host_contract] micro declaration(message: utf8): unit;").expect("host declaration output");
    let semantic_mir = rusty_valkyrie::compiler::mir::MirLowerer::lower_module_semantic(
        &compiler.compile_fixture_source("[host_contract] micro declaration(message: utf8): unit;").expect("host declaration source"),
    );
    assert_eq!(
        semantic_mir.external_calls.iter().filter(|contract| contract.symbol.name().as_str() == "declaration").count(),
        1,
        "user host contracts must not be conflated with compiler workload phase callables"
    );
}

#[test]
fn maps_c_libc_binding_to_linux_gnu_host_link() {
    let compiler = ValkyrieCompiler::new(SourceID { version_id: 910 });
    let module = compiler
        .compile_fixture_source(
            r#"
[c("libc", "write")]
micro helper(message: utf8): unit {
    return;
}
"#,
        )
        .unwrap();
    let contract = function_interop_contract(&module.functions[0]).expect("expected interop contract");
    assert!(contract.matches_host_platform("linux-gnu"));
    assert_eq!(contract.locator_segments(), &["libc", "write"]);
}

#[test]
fn maps_c_libsystem_binding_to_darwin_host_link() {
    let compiler = ValkyrieCompiler::new(SourceID { version_id: 911 });
    let module = compiler
        .compile_fixture_source(
            r#"
[c("libSystem", "write")]
micro helper(message: utf8): unit {
    return;
}
"#,
        )
        .unwrap();
    let contract = function_interop_contract(&module.functions[0]).expect("expected interop contract");
    assert!(contract.matches_host_platform("darwin"));
    assert_eq!(contract.locator_segments(), &["libSystem", "write"]);
}

#[test]
fn collapses_host_import_shapes_into_same_neutral_contract() {
    for (version_id, tag, arguments) in [
        (801, "clr", r#""mscorlib", "System.Console", "WriteLine""#),
        (802, "wasm", r#""env", "memory""#),
        (803, "wasi", r#""wasi:io/streams", "blocking-write-and-flush""#),
        (804, "wasi", r#""wasi:clocks/monotonic-clock", "now""#),
        (805, "jvm", r#""java/lang/System", "currentTimeMillis""#),
        (806, "com", r#""Excel.Application", "Visible", "set""#),
        (810, "js_builtin", r#""console.log""#),
        (811, "ovm", r#""utf8_to_utf16""#),
    ] {
        let compiler = ValkyrieCompiler::new(SourceID { version_id });
        let module = compiler
            .compile_fixture_source(&format!(
                r#"
[{tag}({arguments})]
micro helper(message: utf16) {{
    return;
}}
"#
            ))
            .unwrap();
        let contract = function_interop_contract(&module.functions[0]).expect("expected interop contract");

        assert!(contract.matches_boundary("host"));
        assert!(contract.matches_platform_tag(tag), "expected platform_tag={tag}, got {:?}", contract.platform_tag);
        assert_eq!(contract.locator_segments().len(), module.functions[0].annotations[0].arguments.len());
    }
}

#[test]
fn maps_wasm_operator_binding_to_wasm_operator_host_link() {
    let compiler = ValkyrieCompiler::new(SourceID { version_id: 912 });
    let module = compiler
        .compile_fixture_source(
            r#"
[wasm_operator("i32.add")]
micro i32_add(a: i32, b: i32): i32;
"#,
        )
        .unwrap();
    let contract = function_interop_contract(&module.functions[0]).expect("expected wasm opcode contract");
    assert!(contract.matches_boundary("host"));
    assert!(contract.matches_platform_tag("wasm-operator"));
    assert_eq!(contract.locator_segments(), &["i32.add"]);
}

#[test]
fn rejects_wasm_operator_binding_with_non_single_segment_locator() {
    for (version_id, argument) in [(913, r#""i32", "add""#), (914, "32")] {
        let compiler = ValkyrieCompiler::new(SourceID { version_id });
        let module = compiler
            .compile_fixture_source(&format!(
                r#"
[wasm_operator({argument})]
micro i32_add(a: i32, b: i32): i32;
"#
            ))
            .unwrap();
        assert!(
            function_interop_contract(&module.functions[0]).is_none(),
            "wasm_operator must require exactly one string mnemonic, got {argument}"
        );
    }
}

#[test]
fn maps_ovm_operator_binding_to_ovm_operator_host_link() {
    let compiler = ValkyrieCompiler::new(SourceID { version_id: 915 });
    let module = compiler
        .compile_fixture_source(
            r#"
[ovm_operator("i32.add")]
micro i32_add(a: i32, b: i32): i32;
"#,
        )
        .unwrap();
    let contract = function_interop_contract(&module.functions[0]).expect("expected vm opcode contract");
    assert!(contract.matches_boundary("host"));
    assert!(contract.matches_platform_tag("ovm-operator"));
    assert_eq!(contract.locator_segments(), &["i32.add"]);
}

#[test]
fn rejects_ovm_operator_binding_with_non_single_segment_locator() {
    for (version_id, argument) in [(916, r#""i32", "add""#), (917, "32")] {
        let compiler = ValkyrieCompiler::new(SourceID { version_id });
        let module = compiler
            .compile_fixture_source(&format!(
                r#"
[ovm_operator({argument})]
micro i32_add(a: i32, b: i32): i32;
"#
            ))
            .unwrap();
        assert!(
            function_interop_contract(&module.functions[0]).is_none(),
            "ovm_operator must require exactly one string mnemonic, got {argument}"
        );
    }
}

#[test]
fn maps_jvm_operator_binding_to_jvm_operator_host_link() {
    let compiler = ValkyrieCompiler::new(SourceID { version_id: 920 });
    let module = compiler
        .compile_fixture_source(
            r#"
[jvm_operator("long.divide")]
micro i64_div(a: i64, b: i64): i64;
"#,
        )
        .unwrap();
    let contract = function_interop_contract(&module.functions[0]).expect("expected jvm operator contract");
    assert!(contract.matches_boundary("host"));
    assert!(contract.matches_platform_tag("jvm-operator"));
    assert_eq!(contract.locator_segments(), &["long.divide"]);
}

#[test]
fn maps_clr_operator_binding_to_clr_operator_host_link() {
    let compiler = ValkyrieCompiler::new(SourceID { version_id: 921 });
    let module = compiler
        .compile_fixture_source(
            r#"
[clr_operator("i64.div")]
micro i64_div(a: i64, b: i64): i64;
"#,
        )
        .unwrap();
    let contract = function_interop_contract(&module.functions[0]).expect("expected clr operator contract");
    assert!(contract.matches_boundary("host"));
    assert!(contract.matches_platform_tag("clr-operator"));
    assert_eq!(contract.locator_segments(), &["i64.div"]);
}

#[test]
fn rejects_jvm_and_clr_operator_bindings_with_non_single_segment_locator() {
    for (version_id, attribute, argument) in
        [(922, "jvm_operator", r#""i32", "add""#), (923, "clr_operator", r#""i64", "div""#), (924, "jvm_operator", "32")]
    {
        let compiler = ValkyrieCompiler::new(SourceID { version_id });
        let module = compiler
            .compile_fixture_source(&format!(
                r#"
[{attribute}({argument})]
micro numeric_add(a: i64, b: i64): i64;
"#
            ))
            .unwrap();
        assert!(
            function_interop_contract(&module.functions[0]).is_none(),
            "`[{attribute}]` must require exactly one string mnemonic, got {argument}"
        );
    }
}

/// 旧 `opcode` 属性不得再产生任何目标绑定：迁移后不保留兼容别名。
#[test]
fn legacy_opcode_attributes_do_not_bind_target_contracts() {
    for (version_id, attribute) in [(918, "wasm_opcode"), (919, "vm_opcode")] {
        let compiler = ValkyrieCompiler::new(SourceID { version_id });
        let module = compiler
            .compile_fixture_source(&format!(
                r#"
[{attribute}("i32.add")]
micro i32_add(a: i32, b: i32): i32;
"#
            ))
            .unwrap();
        assert!(function_interop_contract(&module.functions[0]).is_none(), "`[{attribute}]` 必须不再产生目标绑定");
    }
}

/// 旧 `[vm(...)]` 宿主标签不得再产生外部导入合同：OVM 宿主边界统一为 `[ovm]`。
#[test]
fn vm_surface_tag_does_not_bind_host_contract() {
    let compiler = ValkyrieCompiler::new(SourceID { version_id: 920 });
    let module = compiler
        .compile_fixture_source(
            r#"
[vm("i64_add")]
micro i64_add(a: i64, b: i64): i64;
"#,
        )
        .unwrap();
    assert!(function_interop_contract(&module.functions[0]).is_none(), "`[vm]` 必须不再产生宿主绑定，OVM 宿主边界只接受 `[ovm]`");
}

/// `[intrinsic]` 注解已随 builtin 旁路整条退役：任何携带该注解的声明——
/// 抽象无体、带参、带体、裸注解——都必须被 backend contract 显式拒绝，
/// 不静默忽略、不降级成空体函数，也不从函数名推导目标。
#[test]
fn any_intrinsic_annotated_declaration_fails_closed() {
    let sources = [
        (
            921,
            r#"
[intrinsic]
micro __i64_add(lhs: i64, rhs: i64): i64;
"#,
        ),
        (
            922,
            r#"
[intrinsic("f64.add")]
micro __f64_add(lhs: f64, rhs: f64): f64;
"#,
        ),
        (
            923,
            r#"
[intrinsic("builtin.array.length")]
micro frozen_length(xs: [i64]): usize {
    return frozen_length(xs)
}
"#,
        ),
        (
            924,
            r#"
[intrinsic]
micro helper(): i64 {
    return 0
}
"#,
        ),
    ];
    for (version_id, source) in sources {
        let compiler = ValkyrieCompiler::new(SourceID { version_id });
        let error = compiler.compile_fixture_source(source).expect_err("携带 `[intrinsic]` 注解的声明必须失败关闭");
        assert!(
            matches!(error, HirError::BackendContractRejected(BackendContractError::IntrinsicAnnotationRetired { .. })),
            "version_id={version_id} 必须命中 IntrinsicAnnotationRetired，got {error:?}"
        );
        let rendered = error.to_string();
        assert!(rendered.contains("retired `[intrinsic]` annotation"), "诊断消息必须指出注解已退役: {rendered}");
        assert!(rendered.contains("bypass is removed"), "诊断消息必须指出 builtin 旁路已整体移除: {rendered}");
        let row: nyar_types::DiagnosticRow = error.into();
        assert!(row.code.contains("backend.contract.rejected"), "expected backend contract rejection, got code={}", row.code);
    }
}

#[test]
fn rejects_wasi_preview1_surface_imports() {
    for (version_id, source) in [
        (
            807,
            r#"
[wasi("wasi_snapshot_preview1", "fd_write")]
micro console_write_line(message: utf8): unit;
"#,
        ),
        (
            808,
            r#"
[wasi("fd_write")]
micro console_write_line(message: utf8): unit;
"#,
        ),
        (
            809,
            r#"
[wasi("fd_read")]
micro console_read(message: utf8): unit;
"#,
        ),
    ] {
        let compiler = ValkyrieCompiler::new(SourceID { version_id });
        let error = compiler.compile_fixture_source(source).expect_err("preview1 fd I/O should be rejected");
        let row: nyar_types::DiagnosticRow = error.into();
        assert!(
            row.code.contains("lowering_failed") || row.code.contains("unresolved") || row.code.contains("contract"),
            "preview1 fd I/O should be rejected, got code={}",
            row.code
        );
    }
}

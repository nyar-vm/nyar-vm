use rusty_valkyrie::{ValkyrieCompiler, compiler::mir::MirLowerer};

const HOST_CONTRACT_SOURCE: &str = "[primitive(\"std::primitive::i32\")] structure i32 { } \
    [host_contract] micro foreign(value: i32) -> i32 { } \
    micro caller(value: i32) -> i32 { return foreign(value) }";

/// 数值原始类型不是语言内建，必须绑定到源码声明身份。
const PRIMITIVES: &str = "[primitive(\"std::primitive::i32\")] structure i32 { } ";

#[test]
fn source_import_declaration_has_signature_but_no_fabricated_body() {
    let output = ValkyrieCompiler::default().compile_source_to_program(HOST_CONTRACT_SOURCE).expect("显式 import 与普通调用沿真实构建入口闭合");
    let mir = MirLowerer::lower_module_semantic(
        &ValkyrieCompiler::default().compile_source(HOST_CONTRACT_SOURCE).expect("source for declaration contract"),
    );
    assert_eq!(mir.functions.len(), 1);
    assert_eq!(mir.external_calls.len(), 1);
    assert!(mir.functions.iter().all(|function| function.display_symbol != mir.external_calls[0].symbol.to_string()));
    let canonical = output.canonical();
    assert_eq!(canonical.mir.functions.len(), 1);
    assert_eq!(canonical.linked.item_instances.len(), 2);
    assert_eq!(canonical.linked.imports.len(), 1);
    let import = canonical.linked.imports.values().next().unwrap();
    let signature = &canonical.linked.item_instances[&import.callee];
    assert_eq!(import.parameter_types, signature.parameter_types);
    assert_eq!(import.return_type, signature.return_type);
    assert!(!canonical.mir.functions.contains_key(&import.callee));
}

#[test]
fn source_bare_declaration_cannot_supply_an_executable_call_target() {
    ValkyrieCompiler::default()
        .compile_source_to_program("micro caller(value: i32) -> i32 { return missing(value) }")
        .expect_err("无定义且无显式 import 的调用不能成功");
}

#[test]
fn source_entry_declaration_requires_a_function_body() {
    // Oak 允许无函数体的 `micro` 作为声明（HIR 标记 abstract），但入口必须能产生可执行体；
    // 该声明不得成为可执行程序。
    let error = ValkyrieCompiler::default()
        .compile_source_to_program(&crate::primitives::with_primitives("[main] micro entry() -> i32;"))
        .expect_err("入口声明没有函数体不得产生可执行程序");
    assert!(
        error.to_string().contains("CAN042") && error.to_string().contains("callable"),
        "无函数体的入口必须因缺少 callable 合同而结构化失败: {error}"
    );
}

#[test]
fn every_test_annotation_becomes_one_export_contract() {
    let compiler = ValkyrieCompiler::default();
    for (source, expected) in [
        ("[test] micro only() -> i32 { return 1 }", vec!["only"]),
        ("[test] micro alpha() -> i32 { return 1 } [test] micro beta() -> i32 { return 2 }", vec!["alpha", "beta"]),
        (
            "[test] micro alpha() -> i32 { return 1 } [test] micro beta() -> i32 { return 2 } [test] micro gamma() -> i32 { return 3 }",
            vec!["alpha", "beta", "gamma"],
        ),
    ] {
        let hir = compiler.compile_source(&format!("{PRIMITIVES}{source}")).expect("测试源码必须通过 HIR");
        let mir = MirLowerer::lower_module_semantic(&hir);
        let names = mir.exports.iter().map(|contract| contract.exported_name.clone()).collect::<Vec<_>>();
        assert_eq!(names, expected, "每个 `[test]` 必须按声明顺序产出一条导出合同: source={source}");
        assert!(mir.exports.iter().all(|contract| contract.partition == "default"), "测试导出必须落在 default 分区: {names:?}");
    }
}

#[test]
fn bom_prefixed_test_file_keeps_all_test_contracts() {
    let compiler = ValkyrieCompiler::default();
    // BOM 必须仍在文件首位：声明追加在 BOM 之后，不改变该用例的 BOM 前提。
    let source = format!(
        "\u{feff}{PRIMITIVES}\n[test]\nmicro alpha() -> i32 {{ return 1 }}\n[test]\nmicro beta() -> i32 {{ return 2 }}\n"
    );
    let hir = compiler.compile_source(&source).expect("带 BOM 的测试文件必须通过 HIR");
    let mir = MirLowerer::lower_module_semantic(&hir);
    let names = mir.exports.iter().map(|contract| contract.exported_name.clone()).collect::<Vec<_>>();
    assert_eq!(names, vec!["alpha", "beta"], "UTF-8 BOM 不得吞掉首个 `[test]` 的合同: {names:?}");
}

#[test]
fn namespaced_test_file_with_backticked_names_keeps_first_test_contract() {
    let compiler = ValkyrieCompiler::default();
    let source = format!("{PRIMITIVES}\nnamespace suite;\n\n# 首测说明注释\n# 第二行注释\n\n[test]\nmicro `test alpha`() -> i32 {{ return 1 }}\n\n[test]\nmicro `test beta`() -> i32 {{ return 2 }}\n\n[test]\nmicro `test gamma`() -> i32 {{ return 3 }}\n");
    let hir = compiler.compile_source(&source).expect("真实测试文件形态必须通过 HIR");
    let mir = MirLowerer::lower_module_semantic(&hir);
    let names = mir.exports.iter().map(|contract| contract.exported_name.clone()).collect::<Vec<_>>();
    assert_eq!(
        names,
        vec!["test alpha", "test beta", "test gamma"],
        "namespace 头 + 注释 + 反引号测试名不得让首个 `[test]` 丢失: {names:?}"
    );
}

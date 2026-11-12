//! 调用解析失败的诊断事实：真实 callee 形态、按文件的源码身份、失败原因分类。
//!
//! 合同：未解析调用诊断必须携带（a）用户可读的 callee（`set.is_empty`、`next`，
//! 计算得到的函数值统一 `<fn value>`，禁止内部占位符）；（b）调用点所在文件的
//! 真实路径（多文件组不得整组共享入口文件身份）；（c）结构化失败原因标签。

use nyar_types::DiagnosticArg;
use rusty_valkyrie::{
    CompilerSourceGroup, ValkyrieCompiler,
    compiler::diagnostics::pending,
    hir::lowering::DEFAULT_COMPILE_ARCH,
};

/// 取字符串参数；缺失即测试失败。
fn string_arg<'a>(diagnostic: &'a rusty_valkyrie::compiler::diagnostics::pending::CompileDiagnostic, key: &str) -> &'a str {
    match diagnostic.row.args.get(key) {
        Some(DiagnosticArg::String(value)) => value.as_str(),
        other => panic!("arg `{key}` missing or not a string: {other:?}"),
    }
}

/// 编译（必然失败）并取回全部未解析调用诊断的 (selector, reason, source_id)。
fn unresolved_calls(lib: &str, app: &str) -> Vec<(String, String, String)> {
    pending::clear_pending_diagnostics();
    let groups = crate::primitives::groups_with_primitives(vec![
        CompilerSourceGroup::single_source("std", "std", lib),
        CompilerSourceGroup::single_source("app", "app", app).as_entry().with_direct_dependencies(vec!["std".into()]),
    ]);
    let _ = ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH);
    let collected = pending::take_pending_diagnostics();
    collected
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.row.code == "valkyrie.hir.unresolved_call")
        .map(|diagnostic| {
            (
                string_arg(diagnostic, "selector").to_string(),
                string_arg(diagnostic, "reason").to_string(),
                diagnostic.row.source.id.clone(),
            )
        })
        .collect()
}

#[test]
fn free_function_missing_reports_name_not_found_with_real_span() {
    let diagnostics = unresolved_calls(
        "namespace std;",
        "namespace app;\n[export] micro probe() -> i64 {\n    return no_such_fn(1)\n}\n",
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let (selector, reason, source) = &diagnostics[0];
    assert_eq!(selector, "no_such_fn");
    assert_eq!(reason, "name-not-found");
    assert_eq!(source, "app.v");
}

#[test]
fn method_on_known_receiver_without_candidate_reports_no_method_candidate() {
    let diagnostics = unresolved_calls(
        "namespace std;",
        "namespace app;\n[export] micro probe(xs: i64) -> bool {\n    return xs.is_empty()\n}\n",
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let (selector, reason, source) = &diagnostics[0];
    assert_eq!(selector, "xs.is_empty", "成员调用须给出点形态，不得是占位符");
    assert_eq!(reason, "no-method-candidate");
    assert_eq!(source, "app.v");
}

#[test]
fn unresolved_call_in_second_file_reports_second_file_identity() {
    let diagnostics = unresolved_calls(
        "namespace std.primitive;\n[primitive(\"std::primitive::i64\")] structure i64 { }\n\nnamespace std;\nmicro helper(x: i64) -> i64 { return x }",
        "namespace app;\nusing std;\nusing std.primitive;\n[export] micro probe() -> i64 {\n    let seed: i64 = helper(1)\n    return no_such_fn(seed)\n}\n",
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let (selector, reason, source) = &diagnostics[0];
    assert_eq!(selector, "no_such_fn");
    assert_eq!(reason, "name-not-found");
    assert_eq!(source, "app.v", "调用点在入口文件，不得标成依赖文件");
}

#[test]
fn no_diagnostic_contains_internal_placeholder() {
    let diagnostics = unresolved_calls(
        "namespace std;",
        "namespace app;\n[export] micro probe(make: micro(i64) -> i64) -> i64 {\n    return make(1) + make(2).frobnicate()\n}\n",
    );
    for (selector, _, _) in &diagnostics {
        assert!(!selector.contains("<computed>"), "占位符不得出现在诊断: {diagnostics:?}");
    }
}

/// turbofish 失败时诊断标签必须保留 peel 后的 callee 与显式实参，不得只剩 `<fn value>`。
#[test]
fn turbofish_failure_label_keeps_callee_and_explicit_arguments() {
    let diagnostics = unresolved_calls(
        "namespace std;",
        "namespace app;\n[export] micro probe() -> i64 {\n    return no_such::<i64>(1)\n}\n",
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let (selector, reason, source) = &diagnostics[0];
    assert_eq!(selector, "no_such::<i64>", "callee 必须显示 peel 后的名字与显式实参");
    assert_eq!(reason, "name-not-found");
    assert_eq!(source, "app.v");
}

/// 显式泛型实参数量不符必须自成一类结构化原因，不得退化成笼统的「无一候选」。
#[test]
fn explicit_generic_arity_mismatch_is_its_own_reason() {
    let diagnostics = unresolved_calls(
        "namespace std;\nmicro take<T>(value: T) -> T { return value }",
        "namespace app;\n[export] micro probe() -> i64 {\n    return take::<i64, bool>(1)\n}\n",
    );
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let (selector, reason, _) = &diagnostics[0];
    assert_eq!(selector, "take::<i64, bool>");
    assert_eq!(reason, "explicit-generic-arguments-mismatch");
}

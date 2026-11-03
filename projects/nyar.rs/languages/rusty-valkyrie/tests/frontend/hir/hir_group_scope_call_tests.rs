//! 同一源码组内的文件作用域可见性合同。
//!
//! 一个源码组就是一次 package 编译单元：组内各命名空间的顶层声明共享文件作用域，
//! 可以互相以单段短名无限定引用（与组内名义类型的短名解析一致）。跨组仍然只承认
//! 显式 `using` 绑定与已声明依赖组的导出，不允许靠末段回退把别的组拉进候选集。
//!
//! 本文件锁定 std 的 `panic` 形状：`std.types` 声明、`std.collection.test` 无限定调用。

use rusty_valkyrie::hir::lowering::DEFAULT_COMPILE_ARCH;
use rusty_valkyrie::{CompilerSourceFile, CompilerSourceGroup, ValkyrieCompiler};

fn single_group(files: Vec<CompilerSourceFile>) -> Vec<CompilerSourceGroup> {
    vec![CompilerSourceGroup::from_source_files("std", "std", files, Vec::new()).as_entry()]
}

fn compile(groups: &[CompilerSourceGroup]) -> Result<(), String> {
    ValkyrieCompiler::default().resolve_source_groups(groups, DEFAULT_COMPILE_ARCH).map(|_| ()).map_err(|error| format!("{error}"))
}

#[test]
fn same_group_cross_namespace_generic_free_call_resolves() {
    let files = vec![
        CompilerSourceFile {
            path: "types.v".into(),
            source: "namespace std.types;\nmicro panic_stub<T>(message: T): void { raise message }\n".into(),
        },
        CompilerSourceFile {
            path: "test.v".into(),
            source: "namespace std.collection.test;\nmicro helper(): i32 { return 0 }\n[test]\nmicro run(): void { panic_stub(\"x\") }\n".into(),
        },
    ];
    compile(&single_group(files)).expect("同一源码组内其它命名空间的泛型自由函数必须能以单段短名解析");
}

#[test]
fn same_group_unknown_symbol_still_reports_no_callable_in_scope() {
    let files = vec![CompilerSourceFile {
        path: "test.v".into(),
        source: "namespace std.collection.test;\nmicro helper(): i32 { return 0 }\n[test]\nmicro run(): void { not_there() }\n".into(),
    }];
    let error = compile(&single_group(files)).expect_err("组内也不存在的名字必须 fail-closed");
    assert!(error.contains("no callable with this name is in scope"), "unexpected diagnostic: {error}");
}

#[test]
fn cross_group_symbol_without_declared_dependency_still_reports_no_callable_in_scope() {
    let groups = [
        CompilerSourceGroup::single_source("std", "std", "namespace std.types;\nmicro panic_stub<T>(message: T): void { raise message }\n"),
        CompilerSourceGroup::single_source(
            "app",
            "app",
            "namespace app;\nmicro helper(): i32 { return 0 }\n[main]\nmicro run(): void { panic_stub(1) }\n",
        )
        .as_entry(),
    ];
    let error = compile(&groups).expect_err("未声明依赖的跨组符号不得进入候选集");
    assert!(error.contains("no callable with this name is in scope"), "unexpected diagnostic: {error}");
}

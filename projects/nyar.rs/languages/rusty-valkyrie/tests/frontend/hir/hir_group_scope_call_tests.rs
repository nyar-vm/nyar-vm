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

/// 组名不参与声明身份：这里故意让源组名 `app-package` 与源码 namespace 无关，
/// 保证成员体的解析只可能命中 `app.deep.nested` 这一条真实声明路径。
fn package_group(files: Vec<CompilerSourceFile>) -> Vec<CompilerSourceGroup> {
    vec![CompilerSourceGroup::from_source_files("app", "app-package", files, Vec::new()).as_entry()]
}

/// 正例：class 声明体方法调用同组模块级 `micro`，多级 namespace、被调方无 `[export]`。
///
/// 候选符号由 `app.deep.nested` 生成，调用点必须按同一声明 namespace 解析；
/// 组名 `app-package` 既不等于首段也不构成前缀，任何按组名回退的分支都不得命中。
#[test]
fn class_method_same_group_free_call_uses_declared_namespace() {
    let files = vec![CompilerSourceFile {
        path: "solution.v".into(),
        source: "namespace app.deep.nested;\nmicro helper(): i64 { return 0 }\nclass Solution {\n    [export(case: \"camelCase\")]\n    micro run(): i64 { return helper() }\n}\n"
            .into(),
    }];
    compile(&package_group(files)).expect("class 方法体必须按声明 namespace 解析同组自由函数");
}

/// 正例（结构体方法路径，与 `resolve.rs` 的 `module.structs` 分支同源）：
/// `structure` 方法调用同组模块级 `micro`。
///
/// `structure` 成员上方的属性在表面 AST 里归属聚合本身，不会提升成成员入口，
/// 因此入口放在模块级 `[test]`，用接收者调用 demand 结构体方法体。
#[test]
fn structure_method_same_group_free_call_uses_declared_namespace() {
    let files = vec![CompilerSourceFile {
        path: "solution.v".into(),
        source: "namespace app.deep.nested;\nmicro helper(): i64 { return 0 }\nstructure Holder {\n    value: i32\n\n    micro run(self): i64 { return helper() }\n}\n[test]\nmicro drive(holder: Holder): i64 { return holder.run() }\n"
            .into(),
    }];
    compile(&package_group(files)).expect("结构体方法体必须按声明 namespace 解析同组自由函数");
}

/// 反例：方法体里真正不存在的名字仍必须结构化失败，不能被新作用域兜底。
#[test]
fn class_method_unknown_symbol_still_reports_no_callable_in_scope() {
    let files = vec![CompilerSourceFile {
        path: "solution.v".into(),
        source:
            "namespace app.deep.nested;\nclass Solution {\n    [export(case: \"camelCase\")]\n    micro run(): i32 { return missing() }\n}\n"
                .into(),
    }];
    let error = compile(&package_group(files)).expect_err("方法体内不存在的名字必须 fail-closed");
    assert!(error.contains("no callable with this name is in scope"), "unexpected diagnostic: {error}");
}

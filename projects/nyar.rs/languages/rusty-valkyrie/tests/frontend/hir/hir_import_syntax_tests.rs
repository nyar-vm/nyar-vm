//! `using` 导入语法事实：解析层对无法识别的导入语法（如已废弃的 `using!`）不报错，
//! 只产出空路径 `UsingDeclaration`；HIR 必须 fail-closed，禁止把它翻译成空 `HirImport`。

use rusty_valkyrie::{HirError, SourceID, ValkyrieCompiler, frontend::ast::StatementNode, parser::parse_source};

fn parsed_using_paths(source: &str) -> Vec<Vec<String>> {
    let root = parse_source(source).unwrap_or_else(|error| panic!("源码必须可解析（形态：`{source}`），got {error}"));
    root.items
        .iter()
        .filter_map(|item| match item {
            StatementNode::Using(using) => Some(using.path.parts.iter().map(|part| part.name.clone()).collect()),
            _ => None,
        })
        .collect()
}

#[test]
fn live_dot_using_parses_into_non_empty_path() {
    assert_eq!(
        parsed_using_paths("namespace probe;\nusing std.collection::{ArrayList};\n"),
        vec![vec!["std".to_string(), "collection".to_string()]]
    );
}

#[test]
fn hierarchical_using_parses_into_full_path() {
    assert_eq!(
        parsed_using_paths("namespace probe;\nusing std::collection::{ArrayList};\n"),
        vec![vec!["std".to_string(), "collection".to_string()]]
    );
}

#[test]
fn using_bang_parses_into_full_path_without_diagnostic() {
    // 解析层事实：`using!` 的 `!` 不进入路径，导入声明保留完整路径且不产出解析错误；
    // 主空间限定由 HIR lowering 的放置校验负责，解析层不代判。
    assert_eq!(
        parsed_using_paths("namespace probe;\nusing! std::collection::{ArrayList};\n"),
        vec![vec!["std".to_string(), "collection".to_string()]]
    );
}

#[test]
fn rejects_using_bang_outside_primary_namespace() {
    // `using!` 只属于主空间 `namespace!`：普通 namespace 下的 `using!` 必须结构化失败。
    let error = ValkyrieCompiler::new(SourceID::default())
        .compile_source("namespace probe;\nusing! std::collection::{ArrayList};\n")
        .expect_err("非主空间的 `using!` 必须结构化失败");
    assert!(matches!(error, HirError::PrimaryImportOutsidePrimaryNamespace { .. }), "got {error:?}");
}

#[test]
fn primary_using_bang_lowers_to_module_scoped_import() {
    // 主空间 `using!` 进入 unit 共享表，不得与文件局部 `using` 混表。
    let hir = ValkyrieCompiler::new(SourceID::default())
        .compile_source("namespace! probe;\nusing! std::collection::{ArrayList};\n")
        .expect("主空间 `using!` 必须可编译");
    assert!(hir.files[0].imports.is_empty(), "`using!` 不得混入文件局部导入表");
    assert_eq!(hir.primary_imports.len(), 1);
    assert_eq!(
        hir.primary_imports[0].path.parts().iter().map(|part| part.as_str()).collect::<Vec<_>>(),
        vec!["std", "collection"]
    );
}

#[test]
fn live_using_lowers_to_non_empty_hir_import() {
    let hir = ValkyrieCompiler::new(SourceID::default())
        .compile_source("namespace probe;\nusing std.collection::{ArrayList};\n")
        .expect("活语法导入必须可编译");
    assert_eq!(hir.files.len(), 1, "单源码编译必须产出一个文件作用域");
    assert_eq!(hir.files[0].imports.len(), 1, "既有一个导入声明，就必须产出一个 `HirImport`");
    let import = &hir.files[0].imports[0];
    assert_eq!(import.path.parts().iter().map(|part| part.as_str()).collect::<Vec<_>>(), vec!["std", "collection"], "导入路径不得被截断或清空");
    assert_eq!(import.bindings.iter().map(|binding| binding.name.as_str()).collect::<Vec<_>>(), vec!["ArrayList"]);
}

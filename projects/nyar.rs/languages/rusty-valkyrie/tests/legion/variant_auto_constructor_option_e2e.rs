//! `[constructor]` 端到端工程：`unite Option<T>` 的构造器入口与字面量入口经
//! `legion test` 内置 OVM 真跑。
//!
//! 覆盖：
//! - `Some(41)` 括号构造入口；
//! - `Some { value: 7 }` 字面量入口（有无 `[constructor]` 都必须保留）；
//! - 同一函数内交叉使用两种入口，`case` 分支行为一致；
//! - 无 `[constructor]` 的 `unite` 仅用字面量构造，证明同工程互不影响。
//!
//! 测试臂使用语句形式 `match`（先写结果变量再 `return`）：当前 Semantic MIR
//! 合同要求函数返回类型显式匹配，`match` 作为尾表达式产值的形状不在本特性范围内。

use std::path::Path;

use rusty_valkyrie::legion::{
    cmds::test_engine::{compile_external_test_session, run_external_test_in_session},
    workspace::planner::WorkspaceResolver,
};

use crate::support::create_smoke_project_with_manifest;

const SUITE_SOURCE: &str = r#"
unite Option<T> {
    [constructor]
    Some {
        value: T,
    },
    None,
}

unite Optional<T> {
    Present {
        value: T,
    },
    Absent,
}

[test]
micro constructor_entry_round_trips() -> i64 {
    let argument: i64 = 41
    let value: Option<i64> = Some(argument)
    let mut outcome: i64 = 1
    match value {
        case Some(inner):
            outcome = inner
        case None:
            outcome = 1
    }
    return outcome
}

[test]
micro literal_entry_round_trips() -> i64 {
    let argument: i64 = 7
    let value: Option<i64> = Some { value: argument }
    let mut outcome: i64 = 1
    match value {
        case Some(inner):
            outcome = inner
        case None:
            outcome = 1
    }
    return outcome
}

[test]
micro both_entries_interchange_in_same_function() -> i64 {
    let bracketed_argument: i64 = 41
    let literal_argument: i64 = 7
    let bracketed: Option<i64> = Some(bracketed_argument)
    let literal: Option<i64> = Some { value: literal_argument }
    let mut outcome: i64 = 1
    match bracketed {
        case Some(inner):
            outcome = inner
        case None:
            outcome = 1
    }
    match literal {
        case Some(inner):
            outcome = inner
        case None:
            outcome = 1
    }
    return outcome
}

[test]
micro unite_without_constructor_keeps_literal_entry() -> i64 {
    let argument: i64 = 9
    let value: Optional<i64> = Present { value: argument }
    let mut outcome: i64 = 1
    match value {
        case Present(inner):
            outcome = inner
        case Absent:
            outcome = 1
    }
    return outcome
}
"#;

fn write_test_suite(project_dir: &Path) {
    let test_dir = project_dir.join("test");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("suite.v"), SUITE_SOURCE).unwrap();
}

#[test]
fn variant_auto_constructor_entries_execute_in_builtin_ovm() {
    let fixture = create_smoke_project_with_manifest(
        "legion-variant-constructor",
        r#"{
    name: "variant_auto_constructor_app",
    build: [ { target: "ovm" } ]
}
"#,
        r#"[main]
micro main() -> i64 {
    return 0
}
"#,
    );
    write_test_suite(&fixture.project_dir);

    let workspace = WorkspaceResolver::discover_for_project(&fixture.project_dir).expect("discover workspace");
    let session =
        compile_external_test_session(&workspace, &fixture.project_dir, "legion").expect("构造器与字面量入口必须完成 `legion test` 构建");

    for entry in [
        "constructor_entry_round_trips",
        "literal_entry_round_trips",
        "both_entries_interchange_in_same_function",
        "unite_without_constructor_keeps_literal_entry",
    ] {
        let outcome = run_external_test_in_session(&workspace, &session, entry, &[], false);
        assert!(outcome.success, "内置 OVM 执行 `{entry}` 失败: {:?}", outcome.error);
        assert!(!outcome.is_compile_error, "`{entry}` 不应是编译期失败: {:?}", outcome.error);
        assert_eq!(outcome.error, None);
    }
}

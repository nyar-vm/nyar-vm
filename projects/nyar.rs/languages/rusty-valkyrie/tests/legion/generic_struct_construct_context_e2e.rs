//! 裸泛型构造借用外层期望类型的端到端工程。
//!
//! 覆盖 `let node: Node<T> = Node { value, next: self.head, prev: Nothing }`：
//! 结构构造的字段上下文必须按构造点的外层期望（`Node<T>`）投影，
//! 不能让声明期字段类型里的泛型 binder 冻结空变体 `Nothing`，
//! 否则同一泛型会在构造匹配里变成两个 rigid binder 而被淘汰。

use std::path::Path;

use rusty_valkyrie::legion::{
    cmds::test_engine::{compile_external_test_session, run_external_test_in_session},
    workspace::planner::WorkspaceResolver,
};

use crate::support::create_smoke_project_with_manifest;

const SUITE_SOURCE: &str = r#"
unite Maybe<T> {
    Just {
        value: T,
    },
    Nothing,
}

structure Node<T> {
    value: T
    next: Maybe<Node<T>>
    prev: Maybe<Node<T>>
}

structure List<T> {
    head: Maybe<Node<T>>
    tail: Maybe<Node<T>>
    size: i64
}

imply List<T> {
    micro push_front(mut self, value: T) -> unit {
        let node: Node<T> = Node {
            value: value,
            next: self.head,
            prev: Nothing,
        }
        self.head = Just { value: node }
        self.size = 1
    }
}

[test]
micro generic_struct_construct_binds_expected_type() -> i64 {
    let mut list: List<i64> = List {
        head: Nothing,
        tail: Nothing,
        size: 0,
    }
    list.push_front(7)
    return list.size
}
"#;

fn write_test_suite(project_dir: &Path) {
    let test_dir = project_dir.join("test");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("suite.v"), SUITE_SOURCE).unwrap();
}

#[test]
fn generic_struct_construct_uses_expected_type_in_builtin_ovm() {
    let fixture = create_smoke_project_with_manifest(
        "legion-generic-construct-context",
        r#"{
    name: "generic_construct_context_app",
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
    let session = match compile_external_test_session(&workspace, &fixture.project_dir, "legion") {
        Ok(session) => session,
        Err(error) => panic!("裸泛型构造必须借用外层期望类型完成解析: {error:?}"),
    };

    let outcome = run_external_test_in_session(&workspace, &session, "generic_struct_construct_binds_expected_type", &[], false);
    assert!(outcome.success, "内置 OVM 执行失败: {:?}", outcome.error);
    assert!(!outcome.is_compile_error, "不应是编译期失败: {:?}", outcome.error);
    assert_eq!(outcome.error, None);
}

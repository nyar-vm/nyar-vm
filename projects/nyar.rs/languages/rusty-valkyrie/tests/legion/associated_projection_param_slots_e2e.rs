//! 泛型形参槽位上的关联投影端到端工程。
//!
//! 覆盖：
//! - `run_source<S>(source: S, settings: S::Settings)` 在调用点把 `S` 钉成具体类型后，
//!   槽位投影按 `S` 的 trait 实现见证落成具体类型并通过实参匹配；
//! - 同一形状嵌在容器类型参数里（`Holder<S::Settings>`）同样归约；
//! - 反例：把不满足该投影的实参传进槽位必须仍被淘汰，不能退化成“未知即通过”。

use std::path::Path;

use rusty_valkyrie::legion::{
    cmds::test_engine::{compile_external_test_session, run_external_test_in_session},
    workspace::planner::WorkspaceResolver,
};

use crate::support::create_smoke_project_with_manifest;

const SUITE_SOURCE: &str = r#"
trait SettingsSource {
    type Settings;
    type Marker;

    micro read(self, settings: Settings) -> i64
}

structure SourceA {
    base: i64
}

structure SettingsA {
    width: i64
}

structure Holder<T> {
    value: T
}

imply SourceA: SettingsSource {
    type Settings = SettingsA;
    type Marker = i64;

    micro read(self, settings: SettingsA) -> i64 {
        return settings.width
    }
}

micro run_source<S>(source: S, settings: S::Settings) -> i64
    where S: SettingsSource
{
    return source.read(settings)
}

micro run_nested<S>(source: S, holder: Holder<S::Settings>) -> i64
    where S: SettingsSource
{
    return source.read(holder.value)
}

[test]
micro projection_slot_binds_concrete_impl() -> i64 {
    let source: SourceA = SourceA { base: 1 }
    let settings: SettingsA = SettingsA { width: 2 }
    return run_source(source, settings)
}

[test]
micro nested_projection_slot_binds_concrete_impl() -> i64 {
    let source: SourceA = SourceA { base: 4 }
    let settings: SettingsA = SettingsA { width: 5 }
    let holder: Holder<SettingsA> = Holder { value: settings }
    return run_nested(source, holder)
}
"#;

const NEGATIVE_SOURCE: &str = r#"
trait SettingsSource {
    type Settings;

    micro read(self, settings: Settings) -> i64
}

structure SourceA {
    base: i64
}

structure SettingsA {
    width: i64
}

imply SourceA: SettingsSource {
    type Settings = SettingsA;

    micro read(self, settings: SettingsA) -> i64 {
        return settings.width
    }
}

micro run_source<S>(source: S, settings: S::Settings) -> i64
    where S: SettingsSource
{
    return source.read(settings)
}

[main]
micro main() -> i64 {
    let source: SourceA = SourceA { base: 1 }
    return run_source(source, source)
}
"#;

fn write_test_suite(project_dir: &Path) {
    let test_dir = project_dir.join("test");
    std::fs::create_dir_all(&test_dir).unwrap();
    std::fs::write(test_dir.join("suite.v"), SUITE_SOURCE).unwrap();
}

#[test]
fn associated_projection_param_slots_execute_in_builtin_ovm() {
    let fixture = create_smoke_project_with_manifest(
        "legion-associated-projection-slots",
        r#"{
    name: "associated_projection_slots",
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
    // 解析阶段的判据是「调用不再以 unresolved_call 失败」：SMIR 之后的身份表/目标合同
    // 缺口属于另一层，这里不把它们混进本特性的断言。
    let session = match compile_external_test_session(&workspace, &fixture.project_dir, "legion") {
        Ok(session) => Some(session),
        Err(error) => {
            let message = format!("{error:?}");
            assert!(
                !message.contains("unresolved call `run_source`"),
                "关联投影槽位 `run_source` 不应在解析阶段失败: {message}"
            );
            assert!(
                !message.contains("unresolved call `run_nested`"),
                "嵌套关联投影槽位 `run_nested` 不应在解析阶段失败: {message}"
            );
            None
        }
    };

    if let Some(session) = session {
        for entry in ["projection_slot_binds_concrete_impl", "nested_projection_slot_binds_concrete_impl"] {
            let outcome = run_external_test_in_session(&workspace, &session, entry, &[], false);
            assert!(outcome.success, "内置 OVM 执行 `{entry}` 失败: {:?}", outcome.error);
            assert!(!outcome.is_compile_error, "`{entry}` 不应是编译期失败: {:?}", outcome.error);
            assert_eq!(outcome.error, None);
        }
    }
}

#[test]
fn wrong_projection_slot_argument_is_rejected() {
    let fixture = create_smoke_project_with_manifest(
        "legion-associated-projection-negative",
        r#"{
    name: "associated_projection_negative",
    build: [ { target: "ovm" } ]
}
"#,
        NEGATIVE_SOURCE,
    );

    let workspace = WorkspaceResolver::discover_for_project(&fixture.project_dir).expect("discover workspace");
    let error = match compile_external_test_session(&workspace, &fixture.project_dir, "legion") {
        Ok(_) => panic!("不满足投影槽位的实参必须被淘汰"),
        Err(error) => error,
    };
    let message = format!("{error:?}");
    assert!(
        message.contains("unresolved call `run_source`"),
        "反例必须以未解析调用失败，实际: {message}"
    );
}

//! `Fine` 变体构造解析回归探针：`Result<T, E>` 的 `Fine(payload)` 在 T 为多字段
//! 名义结构时必须解析，与单字段 / 全整型形态同构。

use rusty_valkyrie::{CompilerSourceGroup, HirError, ValkyrieCompiler, hir::lowering::DEFAULT_COMPILE_ARCH};

use crate::primitives::{SourceFixtureCompile, groups_with_primitives};

const RESULT_DECL: &str = r#"
[tag(ResultKind)]
unite Result<T, E> {
    [tag(0)]
    [constructor]
    Fine { value: T },
    [tag(1)]
    [constructor]
    Fail { error: E },
}
"#;

fn compile(source: &str) -> Result<(), HirError> {
    ValkyrieCompiler::default().compile_fixture_source(source).map(|_| ())
}

#[test]
fn fine_with_multi_field_struct_payload_resolves() {
    let source = format!(
        "{RESULT_DECL}
structure LocalTriple {{ a: i64, b: i64, c: i64 }}
micro probe(payload: LocalTriple) -> Result<LocalTriple, i64> {{
    return Fine(payload)
}}"
    );
    compile(&source).expect("多字段名义结构载荷的 `Fine(payload)` 必须解析");
}

#[test]
fn fine_with_single_field_struct_payload_resolves() {
    let source = format!(
        "{RESULT_DECL}
structure LocalSingleField {{ a: i64 }}
micro probe(payload: LocalSingleField) -> Result<LocalSingleField, i64> {{
    return Fine(payload)
}}"
    );
    compile(&source).expect("单字段名义结构载荷的 `Fine(payload)` 必须解析");
}

#[test]
fn fine_with_integer_payload_resolves() {
    let source = format!(
        "{RESULT_DECL}
micro probe(payload: i64) -> Result<i64, i64> {{
    return Fine(payload)
}}"
    );
    compile(&source).expect("整型载荷的 `Fine(payload)` 必须解析");
}

#[test]
fn fail_with_multi_field_struct_payload_resolves() {
    let source = format!(
        "{RESULT_DECL}
structure LocalTriple {{ a: i64, b: i64, c: i64 }}
micro probe(payload: LocalTriple) -> Result<i64, LocalTriple> {{
    return Fail(payload)
}}"
    );
    compile(&source).expect("多字段名义结构载荷的 `Fail(payload)` 必须解析");
}

// ---------- 跨包形态：Result 来自依赖组 ----------

const STD_GROUP: &str = r#"
namespace std.types;

[tag(ResultKind)]
unite Result<T, E> {
    [tag(0)]
    [constructor]
    Fine { value: T },
    [tag(1)]
    [constructor]
    Fail { error: E },
}
"#;

fn compile_groups(consumer: &str) -> Result<(), String> {
    let groups = groups_with_primitives(vec![
        CompilerSourceGroup::single_source("std", "std", STD_GROUP),
        CompilerSourceGroup::single_source("app", "app", consumer).as_entry().with_direct_dependencies(vec!["std".into()]),
    ]);
    ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH).map(|_| ()).map_err(|error| format!("{error}"))
}

#[test]
fn cross_group_fine_with_multi_field_struct_payload_resolves() {
    let consumer = r#"
namespace app;

using std.types;

structure LocalTriple { a: i64, b: i64, c: i64 }

[export] micro probe(payload: LocalTriple) -> Result<LocalTriple, i64> {
    return Fine(payload)
}
"#;
    compile_groups(consumer).expect("跨包 `Fine(多字段结构)` 必须解析");
}

#[test]
fn cross_group_fine_with_integer_payload_resolves() {
    let consumer = r#"
namespace app;

using std.types;

[export] micro probe(payload: i64) -> Result<i64, i64> {
    return Fine(payload)
}
"#;
    compile_groups(consumer).expect("跨包 `Fine(i64)` 必须解析");
}

#[test]
fn cross_group_fine_with_single_field_struct_payload_resolves() {
    let consumer = r#"
namespace app;

using std.types;

structure LocalSingleField { a: i64 }

[export] micro probe(payload: LocalSingleField) -> Result<LocalSingleField, i64> {
    return Fine(payload)
}
"#;
    compile_groups(consumer).expect("跨包 `Fine(单字段结构)` 必须解析");
}

#[test]
fn cross_group_fail_with_multi_field_struct_payload_resolves() {
    let consumer = r#"
namespace app;

using std.types;

structure LocalTriple { a: i64, b: i64, c: i64 }

[export] micro probe(payload: LocalTriple) -> Result<i64, LocalTriple> {
    return Fail(payload)
}
"#;
    compile_groups(consumer).expect("跨包 `Fail(多字段结构)` 必须解析");
}

// ---------- 字段初始化形态：泛型函数体内的 `Variant { field: value }` ----------

#[test]
fn field_init_variant_construction_inside_generic_function_resolves() {
    let source = r#"
[tag(DataOutcomeKind)]
unite DataOutcome<T> {
    [tag(0)]
    [constructor]
    Fine { value: T },
    [tag(1)]
    [constructor]
    Failed { diagnostics: i64 },
}

micro data_outcome_of<T>(value: T) -> DataOutcome<T> {
    return Fine { value: value }
}
"#;
    compile(source).expect("泛型函数体内 `Fine { value: value }` 必须解析");
}

#[test]
fn untagged_field_init_variant_construction_inside_generic_function_resolves() {
    let source = r#"
unite DataOutcome<T> {
    [constructor]
    Fine { value: T },
    [constructor]
    Partial { value: T, diagnostics: [i64] },
    [constructor]
    Failed { diagnostics: [i64] },
}

micro data_outcome_of<T>(value: T) -> DataOutcome<T> {
    return Fine { value: value }
}
"#;
    compile(source).expect("无 tag 判别属性的 `Fine { value: value }` 必须解析");
}

#[test]
fn real_data_outcome_shape_resolves() {
    let source = r#"
namespace std.data;

using std.diagnostic::{Diagnostic, Error, Span, severity_is_error};

unite DataOutcome<T> {
    [constructor]
    Fine { value: T }
    [constructor]
    Partial { value: T, diagnostics: [Diagnostic] }
    [constructor]
    Failed { diagnostics: [Diagnostic] }
}

[export]
micro data_outcome_of<T>(value: T) -> DataOutcome<T> {
    return Fine {
        value: value
    }
}

micro data_outcome_diagnostics<T>(outcome: DataOutcome<T>) -> [Diagnostic] {
    match outcome {
        case Fine(payload):
            []
        case Partial(payload):
            payload.diagnostics
        case Failed(payload):
            payload.diagnostics
    }
}
"#;
    let diagnostic = r#"
namespace std.diagnostic;

structure Diagnostic { message: i64 }
structure Span { start: i64 }
structure Error { code: i64 }

micro severity_is_error(diagnostic: Diagnostic) -> bool {
    return true
}
"#;
    let groups = groups_with_primitives(vec![
        CompilerSourceGroup::single_source("std.diagnostic", "std.diagnostic", diagnostic),
        CompilerSourceGroup::single_source("std.data", "std.data", source).as_entry().with_direct_dependencies(vec!["std.diagnostic".into()]),
    ]);
    ValkyrieCompiler::default()
        .resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH)
        .map(|_| ())
        .expect("真实 `DataOutcome` 形态必须解析");
}

#[test]
fn generic_structure_construction_with_unit_variant_field_resolves() {
    let source = r#"
[primitive("std::primitive::i64")]
structure i64 { }

unite DataCacheSlot<T> {
    [constructor]
    Filled { value: T },
    Empty,
}

structure DataCache<T> {
    source_digest: i64,
    value: DataCacheSlot<T>,
}

micro data_cache_empty<T>() -> DataCache<T> {
    return DataCache { source_digest: 0, value: Empty }
}
"#;
    compile(source).expect("泛型结构体裸名构造 + 裸 unit 变体字段必须解析");
}

#[test]
fn two_same_named_variant_constructors_resolve_by_return_type() {
    let source = r#"
unite Result<T, E> {
    [constructor]
    Fine { value: T },
    [constructor]
    Fail { error: E },
}

unite DataOutcome<T> {
    [constructor]
    Fine { value: T },
    [constructor]
    Failed { diagnostics: i64 },
}

micro data_outcome_of<T>(value: T) -> DataOutcome<T> {
    return Fine { value: value }
}
"#;
    compile(source).expect("同组两个 `Fine` 变体构造必须按返回类型消歧");
}

#[test]
fn cross_group_two_same_named_variant_constructors_resolve() {
    let std_types = r#"
namespace std.types;

unite Result<T, E> {
    [constructor]
    Fine { value: T },
    [constructor]
    Fail { error: E },
}
"#;
    let std_data = r#"
namespace std.data;

using std.types;

unite DataOutcome<T> {
    [constructor]
    Fine { value: T },
    [constructor]
    Failed { diagnostics: i64 },
}

[export]
micro data_outcome_of<T>(value: T) -> DataOutcome<T> {
    return Fine { value: value }
}
"#;
    let groups = groups_with_primitives(vec![
        CompilerSourceGroup::single_source("std.types", "std.types", std_types),
        CompilerSourceGroup::single_source("std.data", "std.data", std_data).as_entry().with_direct_dependencies(vec!["std.types".into()]),
    ]);
    ValkyrieCompiler::default()
        .resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH)
        .map(|_| ())
        .expect("跨组两个 `Fine` 变体构造必须按返回类型消歧");
}

#[test]
fn cross_group_field_init_variant_construction_inside_generic_function_resolves() {
    let std_group = r#"
namespace std.data;

[tag(DataOutcomeKind)]
unite DataOutcome<T> {
    [tag(0)]
    [constructor]
    Fine { value: T },
    [tag(1)]
    [constructor]
    Failed { diagnostics: i64 },
}

micro data_outcome_of<T>(value: T) -> DataOutcome<T> {
    return Fine { value: value }
}
"#;
    let consumer = r#"
namespace app;

using std.data;

structure LocalTriple { a: i64, b: i64, c: i64 }

[export] micro probe(payload: LocalTriple) -> DataOutcome<LocalTriple> {
    return data_outcome_of(payload)
}
"#;
    let groups = groups_with_primitives(vec![
        CompilerSourceGroup::single_source("std", "std", std_group),
        CompilerSourceGroup::single_source("app", "app", consumer).as_entry().with_direct_dependencies(vec!["std".into()]),
    ]);
    ValkyrieCompiler::default()
        .resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH)
        .map(|_| ())
        .expect("跨包泛型字段初始化构造必须解析");
}

#[test]
fn nested_variant_constructor_narrows_by_outer_field_type_when_names_collide() {
    // `decode_char_at` 的真实形态：多个 unite 都有 `EndOfInput`，外层 `Fail { error: ... }`
    // 的字段类型（`E = Utf8DecodeError`）必须把内层构造收窄到唯一候选；
    // 变体字段签名缺失时同名变体并列成歧义。
    let std_types = r#"
namespace std.types;

unite Result<T, E> {
    [constructor]
    Fine { value: T },
    [constructor]
    Fail { error: E },
}

unite Utf8DecodeError {
    [constructor]
    EndOfInput { position: u64 },
    [constructor]
    Truncated { position: u64 },
}

unite Utf16DecodeError {
    [constructor]
    EndOfInput { position: u64 },
}
"#;
    let consumer = r#"
namespace app;

using std.types::{Result, Fine, Fail, Utf8DecodeError};

[export] micro decode(start: u64) -> Result<i64, Utf8DecodeError> {
    return Fail { error: EndOfInput { position: start } }
}
"#;
    let groups = groups_with_primitives(vec![
        CompilerSourceGroup::single_source("std.types", "std.types", std_types),
        CompilerSourceGroup::single_source("app", "app", consumer).as_entry().with_direct_dependencies(vec!["std.types".into()]),
    ]);
    ValkyrieCompiler::default()
        .resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH)
        .map(|_| ())
        .expect("同名内层变体必须按外层字段类型收窄到唯一候选");
}

#[test]
fn tuple_literal_argument_binds_each_formal_slot_type() {
    // `(first as char, 1)` 的整型字面量位必须由形式参数的元组槽类型（`u64`）定型，
    // 否则字面量默认宽度会让返回类型与声明不一致。
    let source = r#"
unite Result<T, E> {
    [constructor]
    Fine { value: T },
    [constructor]
    Fail { error: E },
}

micro decode(first: u32) -> Result<(char, u64), i64> {
    return Fine { value: (first as char, 1) }
}
"#;
    compile(source).expect("元组字面量实参必须逐位置按形式参数槽类型定型");
}

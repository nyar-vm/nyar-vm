//! 回归：文本别名作为 sum 载荷时，`SumNew` 的载荷/结果类型必须与声明的实例化结果
//! 处于同一值空间（`utf8` 别名 ↔ 内建 `Utf8` carrier）。
//!
//! 复现 `leetcode.v` `result-utf8-payload` 的 SMIR006：
//! `Result<i64, utf8>` 在 `Fail("boom")` 构造时报
//! `sum construction contract disagrees with declared sum metadata`。

use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

use crate::primitives::groups_with_primitives;

const STD_TYPES: &str = r#"
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

imply Result<T, E> {
    micro fold<U>(self, fine: micro(T) -> U, fail: micro(E) -> U): U {
        match self {
            case Fine(value):
                return fine(value)
            case Fail(error):
                return fail(error)
        }
    }
}

namespace std.primitive;

[primitive("std::primitive::i64")]
structure i64 { }

imply i64 {
    infix `+`(self, rhs: Self): Self { return self }
}
"#;

const STD_TEXT: &str = r#"
namespace std.text;

[primitive("std::text::utf8")]
class Utf8Text {
    _repr: [u8]
}

type utf8 = Utf8Text;
"#;

fn compile_with_std(app: &str) -> Result<(), String> {
    let groups = groups_with_primitives(vec![
        CompilerSourceGroup::single_source("std.types", "std.types", STD_TYPES),
        CompilerSourceGroup::single_source("std.text", "std.text", STD_TEXT).with_direct_dependencies(vec!["std.types".into()]),
        CompilerSourceGroup::single_source("app", "app", app).as_entry().with_direct_dependencies(vec!["std.types".into(), "std.text".into()]),
    ]);
    ValkyrieCompiler::default()
        .compile_source_groups_to_program(&groups)
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

#[test]
fn result_text_payload_construction_matches_declared_sum() {
    let app = r#"
namespace probe.app;

using std.types
using std.text

[export(name: "resultFoldUtf8")]
micro result_fold_utf8() -> i64 {
    let failed: Result<i64, utf8> = Fail("boom")
    let fine: Result<i64, utf8> = Fine(42)
    let a: i64 = failed.fold(fine_value, fail_value)
    let b: i64 = fine.fold(fine_value, fail_value)
    return a + b
}

micro fine_value(value: i64) -> i64 { return value }
micro fail_value(message: utf8) -> i64 { return -1 }
"#;
    let result = compile_with_std(app);
    assert!(result.is_ok(), "文本载荷 sum 构造必须通过 Semantic MIR 合同，实际: {result:?}");
}

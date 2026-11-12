//! 标量比较解析合同：`char`/`u8` 字面量比较成立，`char`/`u8` 与整型混比结构化失败。
//!
//! 合同（用户裁定）：
//! - `'<'` 归 `char`、`b'<'` 归 `u8`；`char == char`、`u8 == u8` 走 `std::operator` 的
//!   `Equal` 声明解析，编译器不内建比较分支。
//! - `char`/`u8` 与 `u32`/`i32` 值混比不许隐式成立，诊断须提示显式转换或按码点比较。
//! - 文本索引链按裁定形态解析：基数 `⁅ ⁆` 归 `u8`（impl 内 `byte_at(...).unwrap()`），
//!   序数 `[ ]` 归 `char`（impl 内 `character_at(...).unwrap()`），返回值类型传播到比较点。

use rusty_valkyrie::{
    CompilerSourceGroup, ValkyrieCompiler,
    compiler::diagnostics::pending,
    hir::lowering::DEFAULT_COMPILE_ARCH,
};

/// 模拟 std：比较/索引算子与文本类型，全部按裁定形态声明。
const STD: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

namespace std.operator;

trait Equal {
    infix `==`(self, other: Self): bool

    infix `!=`(self, other: Self): bool
}

trait Cardinal {
    type Element;

    suffix `⁅ ⁆`(self, offset: u64): Element

    suffix `⁅ ⁆=`(mut self, offset: u64, value: Element)
}

trait Ordinal {
    type Index;
    type Element;

    suffix `[ ]`(self, ordinal: Self::Index): Element

    suffix `[ ]=`(mut self, ordinal: Self::Index, value: Element)
}

namespace std.types;

micro panic<T>(msg: T): void {
    raise msg
}

namespace std.text;

[tag(OptionKind)]
unite Option<T> {
    [tag(0)]
    Some { value: T },
    [tag(1, default)]
    None,
}

imply Option<T> {
    micro unwrap(self): T {
        match self {
            case Some(value): value
            case None: std::types::panic("unwrap on `None` value")
        }
    }
}

[primitive("std::text::char")]
structure char {

}

imply char: std::operator::Equal {
    infix `==`(self, rhs: Self): bool { return true }

    infix `!=`(self, rhs: Self): bool { return true }
}

namespace std.primitive;

[primitive("std::primitive::u8")]
structure u8 {

}

imply u8: std::operator::Equal {
    infix `==`(self, rhs: Self): bool { return true }

    infix `!=`(self, rhs: Self): bool { return true }
}

namespace std.text;

[primitive("std::text::utf8")]
structure Utf8Text {
    _repr: [u8]
}

type utf8 = Utf8Text;

imply Utf8Text {
    micro byte_at(self, offset: u64): Option<u8> {
        let empty: Option<u8> = None
        return empty
    }

    micro character_at(self, index: u64): Option<char> {
        let empty: Option<char> = None
        return empty
    }

    micro concat(self, other: utf8): utf8 {
        return self
    }
}

imply Utf8Text: std::operator::Cardinal {
    type Element = u8;

    suffix `⁅ ⁆`(self, offset: u64): u8 {
        return self.byte_at(offset).unwrap()
    }

    suffix `⁅ ⁆=`(mut self, offset: u64, value: u8): unit {
        std::types::panic("mock: no write path")
    }
}

imply Utf8Text: std::operator::Ordinal {
    type Index = u64;
    type Element = char;

    suffix `[ ]`(self, ordinal: u64): char {
        return self.character_at(ordinal).unwrap()
    }

    suffix `[ ]=`(mut self, ordinal: u64, value: char): unit {
        std::types::panic("mock: no write path")
    }
}

imply Utf8Text: std::operator::Equal {
    infix `==`(self, rhs: Self): bool { return true }

    infix `!=`(self, rhs: Self): bool { return true }
}

namespace std.collection;

type Equal = std::operator::Equal;

class Bag<T> {
    _items: [T]
}

imply Bag<T> {
    micro put(self, value: T): bool
        where T: Equal
    {
        return true
    }
}
"#;

fn compile(consumer: &str) -> Result<(), String> {
    pending::clear_pending_diagnostics();
    let groups = crate::primitives::groups_with_primitives(vec![
        CompilerSourceGroup::single_source("std", "std", STD),
        CompilerSourceGroup::single_source("app", "app", consumer).as_entry().with_direct_dependencies(vec!["std".into()]),
    ]);
    let result = ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH);
    let collected = pending::take_pending_diagnostics();
    match result {
        Ok(_) if collected.total() == 0 => Ok(()),
        Ok(_) => Err(format!("编译成功但诊断汇非空: {collected:?}")),
        Err(error) => Err(format!("{error}; diagnostics: {collected:?}")),
    }
}

// ---------- 正例：同型标量比较 ----------

#[test]
fn char_equal_char_literal_resolves() {
    let consumer = r#"
namespace app;

[export] micro probe(c: char) -> bool {
    return c == '<'
}
"#;
    compile(consumer).expect("char == char 字面量必须解析");
}

#[test]
fn char_not_equal_char_literal_resolves() {
    let consumer = r#"
namespace app;

[export] micro probe(c: char) -> bool {
    return c != '<'
}
"#;
    compile(consumer).expect("char != char 字面量必须解析");
}

#[test]
fn byte_equal_byte_literal_resolves() {
    let consumer = r#"
namespace app;

[export] micro probe(b: u8) -> bool {
    return b == b'<'
}
"#;
    compile(consumer).expect("u8 == u8 字面量必须解析");
}

// ---------- 正例：文本索引链（裁定形态） ----------

#[test]
fn text_cardinal_byte_compares_with_byte_literal() {
    let consumer = r#"
namespace app;

using std.text;

[export] micro probe(text: Utf8Text, offset: u64) -> bool {
    return text⁅offset⁆ == b'<'
}
"#;
    compile(consumer).expect("`text⁅offset⁆ == b'<'`（u8 == u8）必须解析");
}

#[test]
fn text_ordinal_char_compares_with_char_literal() {
    let consumer = r#"
namespace app;

using std.text;

[export] micro probe(text: Utf8Text, index: u64) -> bool {
    return text[index] == '<'
}
"#;
    compile(consumer).expect("`text[index] == '<'`（char == char）必须解析");
}

// ---------- 正例：模式里的标量字面量 ----------

#[test]
fn match_with_char_literal_patterns_resolves() {
    let consumer = r#"
namespace app;

[export] micro probe(c: char) -> i32 {
    match c {
        case '<': return 1
        else: return 0
    }
}
"#;
    compile(consumer).expect("match 臂上的 char 字面量模式必须解析");
}

#[test]
fn match_option_char_with_some_char_literal_pattern_resolves() {
    let consumer = r#"
namespace app;

using std.text;

[export] micro probe(optional: Option<char>) -> i32 {
    match optional {
        case Some('<'): return 1
        case None: return 0
    }
}
"#;
    compile(consumer).expect("`Some('<')` 提取器模式必须解析");
}

#[test]
fn match_option_char_with_else_arm_resolves() {
    let consumer = r#"
namespace app;

using std.text;

[export] micro probe(optional: Option<char>) -> i32 {
    match optional {
        case Some('<'): return 1
        else: return 0
    }
}
"#;
    compile(consumer).expect("`Some('<')` + `else` 必须解析");
}

// ---------- 正例：字符串字面量实参特化 ----------

#[test]
fn string_literal_binds_aliased_utf8_formal() {
    let consumer = r#"
namespace app;

using std.text;

[export] micro probe(a: Utf8Text) -> Utf8Text {
    return a.concat("x")
}
"#;
    compile(consumer).expect("字符串字面量必须特化到别名解析后的 `utf8` 形参");
}

// ---------- 反例：混比结构化失败 ----------

#[test]
fn char_equal_u32_value_fails_with_structured_diagnostic() {
    let consumer = r#"
namespace app;

[export] micro probe(c: char, x: u32) -> bool {
    return c == x
}
"#;
    let failure = compile(consumer).expect_err("char 与 u32 值混比必须失败");
    assert!(failure.contains("valkyrie.hir.unresolved_call"), "必须是结构化未解析调用诊断: {failure}");
    assert!(failure.contains("mixed-scalar-comparison"), "必须给出混比分类: {failure}");
    assert!(failure.contains("cast explicitly"), "必须提示显式转换: {failure}");
}

#[test]
fn char_equal_int_literal_fails_with_structured_diagnostic() {
    let consumer = r#"
namespace app;

[export] micro probe(c: char) -> bool {
    return c == 60
}
"#;
    let failure = compile(consumer).expect_err("char 与整数字面量混比必须失败");
    assert!(failure.contains("valkyrie.hir.unresolved_call"), "必须是结构化未解析调用诊断: {failure}");
}

#[test]
fn byte_equal_i32_value_fails_with_structured_diagnostic() {
    let consumer = r#"
namespace app;

[export] micro probe(b: u8, n: i32) -> bool {
    return b == n
}
"#;
    let failure = compile(consumer).expect_err("u8 与 i32 值混比必须失败");
    assert!(failure.contains("valkyrie.hir.unresolved_call"), "必须是结构化未解析调用诊断: {failure}");
}

#[test]
fn string_literal_rejects_non_text_formal() {
    let consumer = r#"
namespace app;

micro take_i32(v: i32) -> bool { return true }

[export] micro probe() -> bool {
    return take_i32("x")
}
"#;
    let failure = compile(consumer).expect_err("字符串字面量不得塞进非文本形参");
    assert!(failure.contains("valkyrie.hir.unresolved_call"), "必须是结构化未解析调用诊断: {failure}");
}

#[test]
fn string_literal_rejects_mismatched_receiver_binder() {
    let consumer = r#"
namespace app;

using std.collection::Bag;

[export] micro probe(bag: Bag<u8>) -> bool {
    return bag.put("x")
}
"#;
    let failure = compile(consumer).expect_err("receiver 已钉 `u8` 时字符串字面量必须失败闭合");
    assert!(failure.contains("valkyrie.hir.unresolved_call"), "必须是结构化未解析调用诊断: {failure}");
}

//! 双操作数算子与成员调用的解析回归：两个**变量**之间的 `infix`/`prefix`/`suffix`
//! 必须像「变量 + 字面量」一样解析到 imply 成员，不得在候选纳入阶段丢候选。
//!
//! 镜像 std 形制：`structure usize` + `imply usize` 运算符成员 + `[host_contract]`，
//! provider 经 `[host_provider]` + `[ovm]` 挂在独立 adaptor 组（与 std.adaptor.ovm 同构）。

use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

const CORE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::usize")]
structure usize { }

trait Equal {
    infix `==`(self, rhs: Self): bool
}

imply usize {
    infix `+`(self, rhs: Self): Self { __usize_add(self, rhs) }
    infix `-`(self, rhs: Self): Self { __usize_sub(self, rhs) }
    infix `*`(self, rhs: Self): Self { __usize_mul(self, rhs) }
    infix `/`(self, rhs: Self): Self { __usize_div(self, rhs) }
    infix `<`(self, rhs: Self): bool { __usize_lt(self, rhs) }
    infix `>`(self, rhs: Self): bool { rhs < self }
    infix `<=`(self, rhs: Self): bool { !(rhs < self) }
    infix `>=`(self, rhs: Self): bool { !(self < rhs) }
}

[primitive("std::primitive::bool")]
structure bool { }

imply bool {
    prefix `!`(self): bool { __bool_not(self) }
}

imply usize: Equal {
    infix `==`(self, rhs: Self): bool { __usize_eq(self, rhs) }
}

[host_contract]
private micro __usize_add(lhs: usize, rhs: usize): usize

[host_contract]
private micro __usize_sub(lhs: usize, rhs: usize): usize

[host_contract]
private micro __usize_mul(lhs: usize, rhs: usize): usize

[host_contract]
private micro __usize_div(lhs: usize, rhs: usize): usize

[host_contract]
private micro __usize_lt(lhs: usize, rhs: usize): bool

[host_contract]
private micro __usize_eq(lhs: usize, rhs: usize): bool

[host_contract]
private micro __bool_not(x: bool): bool

namespace std.collection;

class ArrayList<T> {
    _length: usize
}

imply ArrayList<T> {
    micro push(mut self, value: T): unit {
        self._length = self._length + 1
    }

    suffix `⁅ ⁆`(self, offset: usize): T {
        return self.nth(offset)
    }

    micro nth(self, index: usize): T {
        return self⁅index⁆
    }

    micro length(self): usize {
        return self._length
    }
}

[tag(OptionKind)]
unite Option<T> {
    [tag(0)]
    Some { value: T },
    [tag(1, default)]
    None,
}

micro panic<T>(msg: T): void {
}

micro Some<T>(value: T) -> Option<T> {
    let wrapped: Option<T> = Some(value)
    return wrapped
}

imply Option<T> {
    micro unwrap(self): T {
        match self {
            case Some(value):
                value
            case None:
                panic("unwrap on `None` value")
        }
    }
}

class ArrayListIterator<T> {
    _list: ArrayList<T>
    _index: usize
}

imply ArrayList<T>: std::iterator::IntoIterator {
    type Item = T;
    type Iter = ArrayListIterator<T>;

    micro into_iterator(self): ArrayListIterator<T> {
        return ArrayListIterator { _list: self, _index: 0 }
    }
}

imply ArrayListIterator<T>: std::iterator::Iterator {
    type Item = T;

    micro has_next(self): bool {
        return self._index < self._list.length()
    }

    micro next(mut self): Option<T> {
        if !self.has_next() {
            return None
        }
        let value: T = self._list⁅self._index⁆
        self._index = self._index + 1
        return Some(value)
    }
}

namespace std.iterator;

trait Iterator {
    type Item;

    micro has_next(self): bool

    micro next(mut self): Option<Item>
}

trait IntoIterator {
    type Item;
    type Iter: Iterator<Item=Self::Item>;

    micro into_iterator(self): Self::Iter
}
"#;

const ADAPTOR: &str = r#"
namespace std.adaptor.ovm;

[host_provider(std::primitive::__usize_add)]
[ovm("i64_add")]
micro usize_add(lhs: usize, rhs: usize): usize

[host_provider(std::primitive::__usize_sub)]
[ovm("i64_sub")]
micro usize_sub(lhs: usize, rhs: usize): usize

[host_provider(std::primitive::__usize_mul)]
[ovm("i64_mul")]
micro usize_mul(lhs: usize, rhs: usize): usize

[host_provider(std::primitive::__usize_div)]
[ovm("i64_div_u")]
micro usize_div(lhs: usize, rhs: usize): usize

[host_provider(std::primitive::__usize_lt)]
[ovm("i64_lt_u")]
micro usize_lt(lhs: usize, rhs: usize): bool

[host_provider(std::primitive::__usize_eq)]
[ovm("i64_eq")]
micro usize_eq(lhs: usize, rhs: usize): bool

[host_provider(std::primitive::__bool_not)]
[ovm("i32_eqz")]
micro bool_not(x: bool): bool
"#;

fn provider_bindings() -> Vec<rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding> {
    use rusty_valkyrie::compiler::semantic::closure::micro_declaration_key;
    let core_ns = ["std".to_owned(), "primitive".to_owned()];
    let adaptor_ns = ["std".to_owned(), "adaptor".to_owned(), "ovm".to_owned()];
    [("__usize_add", "usize_add"), ("__usize_sub", "usize_sub"), ("__usize_mul", "usize_mul"), ("__usize_div", "usize_div"), ("__usize_lt", "usize_lt"), ("__usize_eq", "usize_eq"), ("__bool_not", "bool_not")]
        .into_iter()
        .map(|(contract, provider)| rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding {
            contract: micro_declaration_key("core", "core.v", &core_ns, contract),
            provider: micro_declaration_key("adaptor", "adaptor.v", &adaptor_ns, provider),
            required_capabilities: vec![],
            foreign_bindings: vec![],
        })
        .collect()
}

fn compile(app: &str) -> Result<(), String> {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", CORE),
        CompilerSourceGroup::single_source("adaptor", "adaptor", ADAPTOR).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "demo", app).as_entry().with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ];
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &provider_bindings(), "native")
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// 双变量四则与比较：两个**变量**之间的 `infix` 必须解析到 imply 成员。
#[test]
fn two_variable_arithmetic_and_comparison_operators_resolve() {
    let app = r#"
namespace demo;

[export]
micro multiply(a: usize, b: usize) -> usize {
    return a * b
}

[export]
micro add(a: usize, b: usize) -> usize {
    return a + b
}

[export]
micro sub(a: usize, b: usize) -> usize {
    return a - b
}

[export]
micro divide(a: usize, b: usize) -> usize {
    return a / b
}

[export]
micro lt(a: usize, b: usize) -> bool {
    return a < b
}

[export]
micro gt(a: usize, b: usize) -> bool {
    return a > b
}

[export]
micro le(a: usize, b: usize) -> bool {
    return a <= b
}

[export]
micro ge(a: usize, b: usize) -> bool {
    return a >= b
}

[export]
micro eq(a: usize, b: usize) -> bool {
    return a == b
}
"#;
    compile(app).expect("双变量 `+ - * / < > <= >= ==` 必须解析到 imply 成员");
}

/// `prefix !` 作用于函数调用结果同样必须解析。
#[test]
fn prefix_not_on_call_result_resolves() {
    let app = r#"
namespace demo;

micro is_small(a: usize, b: usize) -> bool {
    return a < b
}

[export]
micro is_not_small(a: usize, b: usize) -> bool {
    return !is_small(a, b)
}
"#;
    compile(app).expect("`prefix !` 作用于函数调用必须解析");
}

/// 递归泛型函数体内的 `push`（成员调用，owner 是泛型聚合）与 `suffix ⁅ ⁆` 必须解析。
#[test]
fn recursive_generic_push_and_zero_based_subscript_resolve() {
    let app = r#"
namespace demo;

[export]
micro fill<T>(mut list: ArrayList<T>, value: T, n: usize): unit {
    if n > 0 {
        list.push(value)
        fill(list, value, n - 1)
    }
}

[export]
micro pick_two<T>(list: ArrayList<T>, i: usize, j: usize): T {
    return list⁅j - i⁆
}
"#;
    compile(app).expect("递归泛型内的 `push` 与 `suffix ⁅ ⁆` 必须解析");
}

/// D1：`loop x in ArrayList` 的迭代项必须是 `T`，不得是 `Option<T>`。
#[test]
fn loop_in_iteration_item_is_element_type() {
    let app = r#"
namespace demo;

[export]
micro sum_list(xs: ArrayList<usize>) -> usize {
    let mut total: usize = 0
    loop x in xs {
        total = total + x
    }
    return total
}
"#;
    compile(app).expect("`loop x in ArrayList` 的迭代项必须是元素类型 `T`（D1）");
}

/// D1 反例：没有 `IntoIterator` 实现的集合必须结构化失败，不得把集合自身当元素类型静默通过。
#[test]
fn loop_in_non_iterable_source_fails() {
    let app = r#"
namespace demo;

[export]
micro sum_number(n: usize) -> usize {
    let mut total: usize = 0
    loop x in n {
        total = total + x
    }
    return total
}
"#;
    let error = compile(app).expect_err("没有 `IntoIterator` 实现的 `usize` 必须结构化失败（D1 反例）");
    assert!(error.contains("unresolved call") && error.contains("demo.v"), "脱糖后的调用必须结构化失败并带源码坐标：{error}");
}

/// 相互递归的自由函数：调用点在双方声明之间必须双向解析。
#[test]
fn mutually_recursive_free_functions_resolve() {
    let app = r#"
namespace demo;

micro is_even(n: usize) -> bool {
    if n == 0 {
        return true
    }
    return is_odd(n - 1)
}

micro is_odd(n: usize) -> bool {
    if n == 0 {
        return false
    }
    return is_even(n - 1)
}

[export]
micro check(n: usize) -> bool {
    return is_even(n)
}
"#;
    compile(app).expect("相互递归的自由函数必须解析");
}

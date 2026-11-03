//! 双操作数算子与成员调用的解析回归：两个**变量**之间的 `infix`/`prefix`/`suffix`
//! 必须像「变量 + 字面量」一样解析到 imply 成员，不得在候选纳入阶段丢候选。
//!
//! 镜像 std 形制：`[primitive] structure u64` + `type usize = u64` 指针宽度别名 +
//! `imply u64` 运算符成员 + `[host_contract]`，provider 经 `[host_provider]` + `[ovm]`
//! 挂在独立 adaptor 组（与 std.adaptor.ovm 同构）。

use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

const CORE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::u64")]
structure u64 { }

type usize = u64;

trait Equal {
    infix `==`(self, rhs: Self): bool
}

imply u64 {
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

imply u64: Equal {
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

namespace std.collection.array;

micro new<T>(): [T] {
    return []
}

micro plain(): usize {
    return 0
}

[host_contract]
micro get<T>(array: [T], index: usize): T

[host_contract]
micro set<T>(array: [T], index: usize, value: T): unit

namespace std.types;

micro panic<T>(msg: T): void {
    raise msg
}

namespace std.operator;

trait Cardinal {
    type Element;

    suffix `⁅ ⁆`(self, offset: usize): Element

    suffix `⁅ ⁆=`(mut self, offset: usize, value: Element)
}

namespace std.collection;

class Nums<T> {
    _items: [T]
}

imply Nums<T> {
    micro length(self): usize {
        return 0
    }
}

imply Nums<T>: std::operator::Cardinal {
    type Element = T;

    suffix `⁅ ⁆`(self, offset: usize): T {
        if offset >= self.length() {
            std::types::panic("Cardinal ⁅ ⁆ 越界：offset 超出元素个数")
        }
        return std::collection::array::get(self._items, offset)
    }

    suffix `⁅ ⁆=`(mut self, offset: usize, value: T): unit {
        if offset >= self.length() {
            std::types::panic("Cardinal ⁅ ⁆= 越界：offset 超出元素个数")
        }
        std::collection::array::set(self._items, offset, value)
    }
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

[host_provider(std::collection::array::get)]
[ovm("array_get")]
micro array_host_get<T>(array: [T], index: usize): T

[host_provider(std::collection::array::set)]
[ovm("array_set")]
micro array_host_set<T>(array: [T], index: usize, value: T): unit
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
        .chain(
            [("get", "array_host_get"), ("set", "array_host_set")].into_iter().map(|(contract, provider)| {
                rusty_valkyrie::compiler::emitter::pipeline::CompilerHostProviderBinding {
                    contract: micro_declaration_key("core", "core.v", &["std".to_owned(), "collection".to_owned(), "array".to_owned()], contract),
                    provider: micro_declaration_key("adaptor", "adaptor.v", &adaptor_ns, provider),
                    required_capabilities: vec![],
                    foreign_bindings: vec![],
                }
            }),
        )
        .collect()
}

fn compile(app: &str) -> Result<(), String> {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "core", CORE),
        CompilerSourceGroup::single_source("adaptor", "adaptor", ADAPTOR).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "demo", app).as_entry().with_direct_dependencies(vec!["core".into(), "adaptor".into()]),
    ];
    ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups, &provider_bindings(), &[], "native")
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
/// D5：限定路径 turbofish `std::collection::array::new::<usize>` 必须解析到依赖声明并完成实例化。
///
/// 与 `std::collection::array::new::<T>` 语料同形：显式泛型实参是**具体类型**，
/// 调用点必须绑定依赖组声明，不得被拆成 `<` / `>` 比较链。
#[test]
fn qualified_turbofish_call_resolves() {
    let app = r#"
namespace demo;

[export]
micro fresh() -> [usize] {
    return std::collection::array::new::<usize>()
}
"#;
    compile(app).expect("`std::collection::array::new::<usize>()` turbofish 必须解析");
}

/// D5 反例：显式泛型实参个数与声明 binder 个数不等时必须结构化失败。
#[test]
fn qualified_turbofish_arity_mismatch_fails() {
    let app = r#"
namespace demo;

[export]
micro fresh() -> [usize] {
    return std::collection::array::new::<usize, bool>()
}
"#;
    let error = compile(app).expect_err("显式泛型实参个数不匹配必须结构化失败");
    assert!(error.contains("unresolved call"), "expected unresolved call, got: {error}");
}

/// D5 反例：非泛型限定函数附显式泛型实参必须结构化失败。
#[test]
fn turbofish_on_non_generic_qualified_call_fails() {
    let app = r#"
namespace demo;

[export]
micro fresh() -> usize {
    return std::collection::array::plain::<usize>()
}
"#;
    let error = compile(app).expect_err("非泛型函数附 turbofish 必须结构化失败");
    assert!(error.contains("unresolved call"), "expected unresolved call, got: {error}");
}

/// 统一访问：字段形态的零参成员 `.length` 与调用形态 `.length()` 必须等价。
#[test]
fn field_shaped_zero_arg_member_matches_call() {
    let app = r#"
namespace demo;

[export]
micro len_value(xs: ArrayList<usize>) -> usize {
    return xs.length
}

[export]
micro len_call(xs: ArrayList<usize>) -> usize {
    return xs.length()
}
"#;
    compile(app).expect("`.length` 与 `.length()` 必须等价");
}

/// 统一访问反例：既不是聚合字段、也没有零参方法的成员必须结构化失败。
#[test]
fn unknown_member_without_method_or_field_fails() {
    let app = r#"
namespace demo;

[export]
micro len_value(xs: ArrayList<usize>) -> usize {
    return xs.not_a_member
}
"#;
    let error = compile(app).expect_err("未声明的成员必须结构化失败");
    assert!(error.contains("SMIR007") && error.contains("demo::len_value"), "失败必须带结构化 MIR 码与函数身份：{error}");
}
/// 开放实例可达性：泛型体内 `std::collection::array::new::<T>()` 铸造的开放克隆，
/// 被具体实例 `fill::<usize>` 的调用链引用时，必须随链接迁入消费模块。
#[test]
fn open_instance_clone_reaches_consumer_module() {
    let app = r#"
namespace demo;

micro fill<T>(): [T] {
    return std::collection::array::new::<T>()
}

[export]
micro fresh() -> [usize] {
    return fill::<usize>()
}
"#;
    compile(app).expect("开放克隆必须随具体实例的引用闭包迁入消费模块");
}

/// 开放实例可达性反例：泛型体内不存在的依赖成员不得因根集合放宽而静默通过。
#[test]
fn open_instance_clone_unresolved_member_still_fails() {
    let app = r#"
namespace demo;

micro fill<T>(): [T] {
    return std::collection::array::missing::<T>()
}

[export]
micro fresh() -> [usize] {
    return fill::<usize>()
}
"#;
    let error = compile(app).expect_err("不存在的依赖成员必须结构化失败");
    assert!(error.contains("unresolved call"), "expected unresolved call, got: {error}");
}

/// D12'：`raise` 的名义载体（`usize` 形参）resume 块参数必须按 `Resume = !` 冻结为 `void`，不得无类型。
#[test]
fn panic_raise_nominal_payload_keeps_never_resume() {
    let app = r#"
namespace demo;

[export]
micro boom() -> unit {
    std::types::panic::<usize>(0)
}
"#;
    compile(app).expect("名义载体的 raise resume 参数必须按 `!` 冻结");
}

/// D12'：`raise "..."` 的 UTF-8 字面量载体不是名义类型，resume 参数必须保留随载荷推断的类型。
///
/// 与 `Option::unwrap` / `char_at` 越界分支同形：`panic("...")` 出现在 match 臂。
#[test]
fn panic_raise_literal_payload_assigns_resume_type() {
    let app = r#"
namespace demo;

[export]
micro unwrap_opt(x: Option<usize>) -> usize {
    match x {
        case Some(value): value
        case None: std::types::panic("unwrap on None")
    }
}
"#;
    compile(app).expect("字面量载体的 raise resume 参数必须带 SSA 类型");
}

/// D12'：`panic(error)` 的载体是调用方泛型 binder 时，resume 参数同样必须带 SSA 类型。
///
/// 与 `Result::unwrap_fail` 同形：开放克隆进消费模块后仍要过 SMIR001。
#[test]
fn panic_raise_open_binder_payload_assigns_resume_type() {
    let app = r#"
namespace demo;

micro propagate<E>(error: E) -> unit {
    std::types::panic::<E>(error)
}

[export]
micro boom() -> unit {
    propagate::<usize>(1)
}
"#;
    compile(app).expect("开放 binder 载体的 raise resume 参数必须带 SSA 类型");
}

/// D12' 反例：`raise` 修复不得掩盖类型检查——函数体只剩 `panic` 而声明返回 `usize` 必须结构化失败。
#[test]
fn panic_only_body_return_mismatch_fails() {
    let app = r#"
namespace demo;

[export]
micro boom() -> usize {
    std::types::panic("boom")
}
"#;
    let error = compile(app).expect_err("`panic` 独占函数体与声明返回类型不一致必须结构化失败");
    assert!(error.contains("SMIR007"), "expected SMIR007 return mismatch, got: {error}");
}

/// D12''：`Cardinal` trait 实现的 `⁅ ⁆` 经 `[host_contract]` 泛型 `get` 委托时，
/// 索引读取必须解析到具体实例，`let` 初始化类型与声明标注一致。
#[test]
fn cardinal_index_over_host_contract_get_compiles() {
    let app = r#"
namespace demo;

[export]
micro first(nums: Nums<usize>) -> usize {
    let x: usize = nums⁅0⁆
    return x
}
"#;
    compile(app).expect("`⁅ ⁆` 经 host contract `get` 的索引读取必须编译");
}

/// D12''：元素类型是 `Option<T>` 时，索引结果链上调用 `unwrap` 必须解析。
#[test]
fn cardinal_index_result_unwraps_when_element_is_option() {
    let app = r#"
namespace demo;

[export]
micro first(nums: Nums<Option<usize>>) -> usize {
    return nums⁅0⁆.unwrap()
}
"#;
    compile(app).expect("`Option<T>` 元素的 `⁅ ⁆` 结果必须能链上 `unwrap`");
}

/// D12''：`⁅ ⁆=` 索引写入经 `[host_contract]` 泛型 `set` 委托时必须解析到具体实例。
#[test]
fn cardinal_index_assign_over_host_contract_set_compiles() {
    let app = r#"
namespace demo;

[export]
micro swap(mut nums: Nums<usize>) -> unit {
    nums⁅0⁆ = nums⁅1⁆
}
"#;
    compile(app).expect("`⁅ ⁆=` 经 host contract `set` 的索引写入必须编译");
}

/// D12'' 反例：元素类型不是 `Option` 时索引结果上的 `unwrap` 必须结构化失败。
#[test]
fn cardinal_index_unwrap_on_plain_element_fails() {
    let app = r#"
namespace demo;

[export]
micro first(nums: Nums<usize>) -> usize {
    return nums⁅0⁆.unwrap()
}
"#;
    let error = compile(app).expect_err("非 `Option` 元素上的 `unwrap` 必须结构化失败");
    assert!(error.contains("unresolved call"), "expected unresolved call, got: {error}");
}

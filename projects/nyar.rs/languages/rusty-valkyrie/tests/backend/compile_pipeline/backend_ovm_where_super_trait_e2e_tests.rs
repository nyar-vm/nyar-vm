//! 具体实参上的 super trait `where` 证据闭合：V 源 → OVM 模块 → 真实执行。
//!
//! 靶子是 `satisfy_explicit_type_at` 对「带 super trait 的 trait」无条件拒绝：
//! `Comparable: Equal` 在具体实参 `i32` 上永远求不到 evidence，于是
//! - 外部调用点（`sort` / `BinaryHeap::push`）淘汰候选 → `no-viable-overload`；
//! - 克隆体内部成员调用（`sift_up` / `insert_non_full`）无法铸造实例 → `InstanceNotClosed`。
//!
//! 夹具按 std 形制分三组：core 只放 primitive 与 `i32.lt` 助记符，lib 放
//! `Comparable: Equal` 合同、`sort`、`BinaryHeap<T>`，app 只放 `[main]` 入口。
//! 泛型函数必须来自依赖组才会在固定点里单态化（与 `legion` 规划 std 的方式一致）。
//! 比较走 `[ovm_operator("i32.lt")]`，不引入宿主绑定。
#![cfg(feature = "ovm-lane")]

use nyar_emitter::{FragmentSubmission, TargetExecutionPlan};
use acorn_odin::encode_module;
use odin_vm::{OdinVm, Value, executor::Executor, module::LoadedModule};
use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler};

const PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::bool")]
structure bool { }

namespace demo.collection;

[host_contract]
[ovm_operator("i32.lt")]
micro i32_lt(a: i32, b: i32): bool
"#;

/// `Comparable: Equal` 与 std 同形：`sort` 与 `BinaryHeap<T>` 的公共成员都在
/// 具体实参 `i32` 上要求 `Comparable`，超级 trait `Equal` 的 evidence 必须逐层闭合。
const LIBRARY: &str = r#"
namespace demo.collection;

trait Equal {
    micro equal(self, other: Self) -> bool
}

trait Comparable: Equal {
    micro less(self, other: Self) -> bool
}

imply i32: Equal {
    micro equal(self, other: Self) -> bool {
        return i32_lt(self, other)
    }
}

imply i32: Comparable {
    micro less(self, other: Self) -> bool {
        return i32_lt(self, other)
    }
}

class ArrayList<T> {
    first: T
    second: T
}

imply ArrayList<T> {
    micro new(first: T, second: T): Self {
        return Self { first: first, second: second }
    }
}

[export]
micro sort<X>(list: ArrayList<X>) -> X
    where X: Comparable
{
    if list.first.less(list.second) {
        return list.first
    }
    return list.second
}

# `Equal` 只有抽象方法；必须有一个真实消费点，否则闭包规划会把合同整条裁掉，
# 让 `Comparable: Equal` 的 super trait 身份在后续 lowering 里消失。
[export]
micro equal_probe(a: i32, b: i32) -> bool {
    return a.equal(b)
}

class BinaryHeap<T> {
    top: T
}

imply BinaryHeap<T> {
    micro new(top: T): Self {
        return Self { top: top }
    }

    micro push(mut self, value: T): unit
        where T: Comparable
    {
        if value.less(self.top) {
            self.top = value
        }
    }

    micro pop(mut self): T
        where T: Comparable
    {
        return self.top
    }
}
"#;

const SORT_MAIN: &str = r#"
namespace demo.app;

using demo.collection.{ArrayList, sort, equal_probe};

[main]
micro main() -> i32 {
    let left: i32 = 3
    let right: i32 = 3
    let same: bool = equal_probe(left, right)
    let list: ArrayList<i32> = ArrayList.new(9, 3)
    let sorted: i32 = sort(list)
    return sorted
}
"#;

const HEAP_MAIN: &str = r#"
namespace demo.app;

using demo.collection.{BinaryHeap, equal_probe};

[main]
micro main() -> i32 {
    let left: i32 = 3
    let right: i32 = 3
    let same: bool = equal_probe(left, right)
    let heap: BinaryHeap<i32> = BinaryHeap.new(9)
    heap.push(3)
    heap.push(7)
    let popped: i32 = heap.pop()
    return popped
}
"#;

const BOTH_MAIN: &str = r#"
namespace demo.app;

using demo.collection.{ArrayList, sort, BinaryHeap, equal_probe};

[main]
micro main() -> i32 {
    let left: i32 = 3
    let right: i32 = 3
    let same: bool = equal_probe(left, right)

    let list: ArrayList<i32> = ArrayList.new(9, 3)
    let sorted: i32 = sort(list)

    let heap: BinaryHeap<i32> = BinaryHeap.new(9)
    heap.push(3)
    heap.push(7)
    let popped: i32 = heap.pop()

    # `sort` 与 `BinaryHeap` 共享同一处 witness 修复：两者都必须实例化。
    return sorted
}
"#;

fn groups(main: &str) -> Vec<CompilerSourceGroup> {
    vec![
        CompilerSourceGroup::single_source("core", "core", PRIMITIVES),
        CompilerSourceGroup::single_source("lib", "lib", LIBRARY).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "app", main).as_entry().with_direct_dependencies(vec!["core".into(), "lib".into()]),
    ]
}

fn compile_and_run(main: &str) -> Result<Value, String> {
    let program = ValkyrieCompiler::default()
        .compile_source_groups_to_program_with_host_bindings(&groups(main), &[], &[], "native")
        .map_err(|error| error.to_string())?;
    let fragment = program.canonical().linked.fragments.keys().next().expect("测试程序具有片段").clone();
    let roots = program.canonical().mir.functions.keys().copied().collect::<Vec<_>>();
    let plan = TargetExecutionPlan::from_compiled_program(&program, &fragment, nyar::TheoryBundle::default(), &roots)
        .map_err(|error| error.to_string())?;
    let module = nyar_emitter::lowering::backends::ovm::lower_fragment_to_nyar_module(&FragmentSubmission::from_backend_plan(plan))
        .map_err(|error| error.to_string())?;
    let bytes = encode_module(&module);
    let loaded = LoadedModule::from_bytes(&bytes).map_err(|error| error.to_string())?;
    let entry = loaded
        .exports
        .iter()
        .find(|export| export.symbol_name.ends_with("main"))
        .ok_or_else(|| format!("必须导出 main 入口: {:?}", loaded.exports))?;
    Executor::new().run(&loaded, entry.function_index as usize, Vec::new()).map_err(|error| error.to_string())
}

/// `sort(ArrayList<i32>)` 的返回门控直接消费具体代入后的 `where` 见证；
/// 未闭合时编译边界即 `no-viable-overload`。
#[test]
fn concrete_sort_where_witness_closes_and_executes() {
    let value = compile_and_run(SORT_MAIN).expect("`sort` 的具体实参 `where` 证据必须闭合并在 OVM 内执行");
    assert_eq!(value, Value::I32(3), "`sort(ArrayList.new(9, 3))` 必须返回较小元素 3");
}

/// `BinaryHeap<i32>` 的 `push` / `pop` 都带 `where T: Comparable`：两个公共成员
/// 都必须铸造成闭合实例并在运行时真实比较（push 3、push 7 后 pop 仍为 3）。
#[test]
fn concrete_binary_heap_where_witness_closes_and_executes() {
    let value = compile_and_run(HEAP_MAIN).expect("`BinaryHeap<i32>` 的 `push` / `pop` 证据必须闭合并在 OVM 内执行");
    assert_eq!(value, Value::I32(3), "push 3、push 7 后 pop 必须仍是较小元素 3");
}

/// 合并链路：`sort` 与 `BinaryHeap` 同时在一个模块内闭合，证明两者共享同一处修复。
#[test]
fn sort_and_binary_heap_where_witnesses_close_together() {
    let value = compile_and_run(BOTH_MAIN).expect("`sort` 与 `BinaryHeap` 必须同时闭合");
    assert_eq!(value, Value::I32(3), "两条链路都必须在 OVM 内真实执行");
}

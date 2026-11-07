//! `[T].into_iterator().collect()` 的方法解析合同。
//!
//! 同一 trait、同一容器、同一 `collect` 声明，在元素类型来自本组与来自依赖组时
//! 必须给出同一解析结果；`collect` 是依赖组导出的自由函数（首参 `self`），
//! 方法位置解析不得要求 consumer 额外补 `using`。依赖组没有该声明时，失败必须
//! 落成点名成员与接收者的结构化诊断，不得静默退化成无候选。

use rusty_valkyrie::{
    CompilerSourceFile, CompilerSourceGroup, HirError, ValkyrieCompiler,
    compiler::{diagnostics::pending, types::hir::HirModule},
    hir::lowering::DEFAULT_COMPILE_ARCH,
};

const PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::u64")]
structure u64 { }

type usize = u64;

[primitive("std::primitive::bool")]
structure bool { }
"#;

/// 容器与迭代器实现：与 `std` 的真实布局一致，一个文件只声明一个命名空间。
const CONTAINER: &str = r#"
namespace std.collection;

class ArrayList<T> {
    _length: usize
}

imply ArrayList<T> {
    micro length(self): usize {
        return self._length
    }
}

structure ArrayListIterator<T> {
    _list: ArrayList<T>
    _index: usize
}

imply ArrayList<T>: std::iterator::IntoIterator {
    type Iter = ArrayListIterator<T>;

    micro into_iterator(self): ArrayListIterator<T> {
        return ArrayListIterator::<T> { _list: self, _index: 0 }
    }
}

imply ArrayListIterator<T>: std::iterator::Iterator {
    micro has_next(self): bool {
        return false
    }
}
"#;

/// 迭代器合同；与 `collect` 分文件，贴近 std 真实的多文件同 namespace 布局。
const ITERATOR: &str = r#"
namespace std.iterator;

trait Iterator {
    micro has_next(self): bool
}

trait IntoIterator {
    type Iter: Iterator;

    micro into_iterator(self): Self::Iter
}
"#;

/// 首参 `self` 的自由函数：只能由方法位置调用命中，不产生名字引用。
const COLLECT: &str = r#"
namespace std.iterator;

micro collect<I>(self: I) -> I {
    return self
}
"#;

const ELEMENT_TYPE: &str = r#"
namespace std.types;

structure Column {
    _marker: bool
}
"#;

fn library_sources(include_element: bool, include_collect: bool) -> Vec<CompilerSourceFile> {
    let mut sources = vec![
        CompilerSourceFile { path: "primitives.v".into(), source: PRIMITIVES.into() },
        CompilerSourceFile { path: "container.v".into(), source: CONTAINER.into() },
        CompilerSourceFile { path: "iterator.v".into(), source: ITERATOR.into() },
    ];
    if include_element {
        sources.push(CompilerSourceFile { path: "types.v".into(), source: ELEMENT_TYPE.into() });
    }
    if include_collect {
        sources.push(CompilerSourceFile { path: "collect.v".into(), source: COLLECT.into() });
    }
    sources
}

fn compile(include_element_in_library: bool, include_collect: bool, consumer: &str) -> Result<Vec<HirModule>, HirError> {
    let groups = [
        CompilerSourceGroup::from_source_files("std", "std", library_sources(include_element_in_library, include_collect), Vec::new()),
        CompilerSourceGroup::single_source("app", "app", consumer).as_entry().with_direct_dependencies(vec!["std".into()]),
    ];
    ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH)
}

const CONSUMER_OWN_ELEMENT: &str = r#"
namespace app;

structure Column {
    _marker: bool
}

[export] micro run(values: ArrayList<Column>) -> unit {
    let iterator = values.into_iterator().collect()
    return
}
"#;

const CONSUMER_DEPENDENCY_ELEMENT: &str = r#"
namespace app;

[export] micro run(values: ArrayList<Column>) -> unit {
    let iterator = values.into_iterator().collect()
    return
}
"#;

#[test]
fn iterator_collect_resolves_for_element_type_in_consumer_group() {
    compile(false, true, CONSUMER_OWN_ELEMENT).expect("同组元素类型上的 `into_iterator().collect()` 必须解析");
}

#[test]
fn iterator_collect_resolves_for_element_type_in_dependency_group() {
    compile(true, true, CONSUMER_DEPENDENCY_ELEMENT).expect("依赖组元素类型上的 `into_iterator().collect()` 必须解析");
}

#[test]
fn iterator_collect_absent_from_dependency_reports_structured_diagnosis() {
    pending::clear_pending_diagnostics();
    let error = compile(true, false, CONSUMER_DEPENDENCY_ELEMENT).expect_err("依赖组未声明 `collect` 时调用必须失败");
    assert!(matches!(error, HirError::UnresolvedCall { .. }), "{error:?}");
    let collected = pending::take_pending_diagnostics();
    let diagnostic = collected
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.row.code == "valkyrie.hir.unresolved_call")
        .expect("失败必须落成 `unresolved_call` 诊断");
    assert_eq!(
        match diagnostic.row.args.get("reason") {
            Some(nyar_types::DiagnosticArg::String(reason)) => reason.as_str(),
            other => panic!("reason 缺失或不是字符串: {other:?}"),
        },
        "no-method-candidate",
        "依赖组没有该声明时必须点名成员与接收者，不得只报笼统无候选"
    );
    assert!(diagnostic.message.contains("`collect`"), "诊断必须点名成员: {}", diagnostic.message);
    let selector = match diagnostic.row.args.get("selector") {
        Some(nyar_types::DiagnosticArg::String(selector)) => selector.as_str(),
        other => panic!("selector 缺失或不是字符串: {other:?}"),
    };
    assert!(
        selector.starts_with("values.into_iterator(...)") && selector.ends_with(".collect"),
        "诊断标签必须保留接收者调用与成员名，不得退化成 `<fn value>`: {selector}"
    );
}

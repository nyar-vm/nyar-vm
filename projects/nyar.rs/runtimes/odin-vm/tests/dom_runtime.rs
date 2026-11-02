//! `asgard.render` 内存 DOM 宿主：合同操作的堆表示与读出 ABI 测试。

use nyar_gc::{ObjectHeap, ObjectPayload, Value};
use odin_vm::{
    dom_runtime::{self, DOM_NODE_LAYOUT_ID, DomNode},
    error::OdinRuntimeError,
    host::{HostOp, execute_host_op, parse_host_op},
};

fn s(text: &str) -> Value {
    Value::String(text.to_string())
}

#[test]
fn host_symbols_resolve_to_dom_ops() {
    assert_eq!(parse_host_op("dom_create_element"), Some(HostOp::DomCreateElement));
    assert_eq!(parse_host_op("dom_create_text"), Some(HostOp::DomCreateText));
    assert_eq!(parse_host_op("dom_append"), Some(HostOp::DomAppend));
    assert_eq!(parse_host_op("dom_set_attr"), Some(HostOp::DomSetAttr));
    assert_eq!(parse_host_op("dom_get_attr"), Some(HostOp::DomGetAttr));
    assert_eq!(parse_host_op("dom_set_text"), Some(HostOp::DomSetText));
    assert_eq!(parse_host_op("dom_create_fragment"), None, "未知符号必须保持未绑定");
}

#[test]
fn build_tree_and_read_it_back_with_typed_layout_identity() {
    let mut heap = ObjectHeap::new();
    let root = execute_host_op(HostOp::DomCreateElement, &[s("div")], &mut heap).expect("create root");
    let child = execute_host_op(HostOp::DomCreateElement, &[s("span")], &mut heap).expect("create child");
    execute_host_op(HostOp::DomSetAttr, &[child.clone(), s("class"), s("hero")], &mut heap).expect("set class");
    let text = execute_host_op(HostOp::DomCreateText, &[s("hello")], &mut heap).expect("create text");
    execute_host_op(HostOp::DomAppend, &[child.clone(), text], &mut heap).expect("append text");
    execute_host_op(HostOp::DomAppend, &[root.clone(), child], &mut heap).expect("append child");

    let tree = dom_runtime::read_tree(&heap, &root).expect("read tree");
    assert_eq!(
        tree,
        DomNode::Element {
            tag: "div".into(),
            attrs: vec![],
            children: vec![DomNode::Element {
                tag: "span".into(),
                attrs: vec![("class".into(), "hero".into())],
                children: vec![DomNode::Text("hello".into())],
                text: String::new(),
            }],
            text: String::new(),
        }
    );
}

#[test]
fn set_attr_overwrites_same_name_and_get_attr_reads_back() {
    let mut heap = ObjectHeap::new();
    let node = dom_runtime::create_element(&mut heap, "a").expect("create");
    dom_runtime::set_attr(&mut heap, &node, "href", "/one").expect("set one");
    dom_runtime::set_attr(&mut heap, &node, "href", "/two").expect("set two");
    dom_runtime::set_attr(&mut heap, &node, "title", "t").expect("set title");
    assert_eq!(dom_runtime::get_attr(&heap, &node, "href").expect("get href"), s("/two"));
    assert_eq!(dom_runtime::get_attr(&heap, &node, "title").expect("get title"), s("t"));
    assert_eq!(dom_runtime::get_attr(&heap, &node, "missing").expect("get missing"), s(""), "未设置的属性返回空串");

    let tree = dom_runtime::read_tree(&heap, &node).expect("read");
    match tree {
        DomNode::Element { attrs, .. } => assert_eq!(attrs, vec![("href".into(), "/two".into()), ("title".into(), "t".into())]),
        other => panic!("expected element, got {other:?}"),
    }
}

#[test]
fn set_text_replaces_children() {
    let mut heap = ObjectHeap::new();
    let node = dom_runtime::create_element(&mut heap, "p").expect("create");
    let child = dom_runtime::create_text(&mut heap, "old").expect("text");
    dom_runtime::append(&mut heap, &node, &child).expect("append");
    dom_runtime::set_text(&mut heap, &node, "new").expect("set text");
    let tree = dom_runtime::read_tree(&heap, &node).expect("read");
    match tree {
        DomNode::Element { children, text, .. } => {
            assert!(children.is_empty(), "set_text 必须清空既有子节点: {children:?}");
            assert_eq!(text, "new");
        }
        other => panic!("expected element, got {other:?}"),
    }
}

#[test]
fn invalid_handles_and_arguments_fail_structured() {
    let mut heap = ObjectHeap::new();
    let node = dom_runtime::create_element(&mut heap, "div").expect("create");
    let text = dom_runtime::create_text(&mut heap, "t").expect("text");

    let err = dom_runtime::append(&mut heap, &text, &node).expect_err("文本节点不能作父节点");
    assert!(matches!(err, OdinRuntimeError::TypeMismatch { .. }));

    let err = dom_runtime::append(&mut heap, &node, &Value::I32(7)).expect_err("整数不是 DOM 句柄");
    assert!(matches!(err, OdinRuntimeError::TypeMismatch { .. }));

    let err = dom_runtime::set_attr(&mut heap, &node, "", "v").expect_err("空属性名必须拒绝");
    assert!(matches!(err, OdinRuntimeError::TypeMismatch { .. }));

    let err = dom_runtime::create_element(&mut heap, "").expect_err("空标签名必须拒绝");
    assert!(matches!(err, OdinRuntimeError::TypeMismatch { .. }));

    let err = execute_host_op(HostOp::DomCreateElement, &[Value::I32(1)], &mut heap).expect_err("非字符串标签必须拒绝");
    assert!(matches!(err, OdinRuntimeError::TypeMismatch { .. }));
}

#[test]
fn foreign_layout_object_is_not_a_dom_handle() {
    let mut heap = ObjectHeap::new();
    let plain = Value::Object(heap.alloc_layout_object(0, 1));
    let err = dom_runtime::append(&mut heap, &plain, &plain).expect_err("外来布局对象不得冒充 DOM 句柄");
    assert!(matches!(err, OdinRuntimeError::TypeMismatch { .. }));

    let dom = dom_runtime::create_element(&mut heap, "div").expect("create");
    if let Value::Object(id) = &dom {
        match heap.get(*id) {
            Some(ObjectPayload::LayoutObject { layout_id, .. }) => assert_eq!(*layout_id, DOM_NODE_LAYOUT_ID),
            other => panic!("expected layout object, got {other:?}"),
        }
    }
}

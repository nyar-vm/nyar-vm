//! SSG 内存 DOM：`asgard.render` 宿主合同的 VM 私有表示。
//!
//! DOM 节点是 GC 堆中的布局对象；布局身份、槽位序与读出器唯一归本模块所有，
//! 序列化语义（HTML escape、void 元素、作用域前缀）不归 VM。
//! 句柄即 `Value::Object`——类型化的堆引用，不是整数编号。

use nyar_gc::{ObjectHeap, ObjectId, ObjectPayload};

use crate::{array_runtime, error::OdinRuntimeError, value::Value};

/// `HostElement` 的 VM 私有布局身份。模块布局表由编译器从 0 起顺序分配，
/// 不会占用该值；读出器以此校验对象身份，不按槽位猜类型。
/// 该 id 只作身份标记：DOM 对象直接以 `heap.alloc` 创建，
/// 不经过 `alloc_layout_object`/`register_layout`（后者会按 id 扩容描述表）。
/// GC 追踪对 `LayoutObject` 全覆盖所有槽位，不依赖描述表。
pub const DOM_NODE_LAYOUT_ID: u32 = u32::MAX;

/// 节点种类：元素节点。
pub const DOM_KIND_ELEMENT: i32 = 1;
/// 节点种类：文本节点。
pub const DOM_KIND_TEXT: i32 = 2;

const SLOT_KIND: usize = 0;
const SLOT_TAG: usize = 1;
const SLOT_ATTRS: usize = 2;
const SLOT_CHILDREN: usize = 3;
const SLOT_TEXT: usize = 4;

/// 读出器递归上限：与 SSG 准入门的嵌套上限同量级，超出一律结构化失败。
const MAX_READ_TREE_DEPTH: usize = 256;

/// 从堆还原的显式类型化 DOM 树（SSG 读出 ABI 的产物形态）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomNode {
    /// 元素节点：标签、属性键值（插入序）、子节点（插入序）与文本内容。
    Element {
        /// 宿主标签名。
        tag: String,
        /// 属性键值对，保持设置顺序；同名属性以最后一次写入为准。
        attrs: Vec<(String, String)>,
        /// 子节点，保持追加顺序。
        children: Vec<DomNode>,
        /// `dom_set_text` 写入的文本内容（写入会清空既有子节点）。
        text: String,
    },
    /// 文本节点。
    Text(String),
}

fn invalid_handle(actual: String) -> OdinRuntimeError {
    OdinRuntimeError::TypeMismatch { expected: "asgard.render HostElement", actual }
}

fn read_string(value: &Value, what: &'static str) -> Result<String, OdinRuntimeError> {
    match value {
        Value::String(text) => Ok(text.clone()),
        other => Err(OdinRuntimeError::TypeMismatch { expected: what, actual: other.type_name().to_string() }),
    }
}

/// 校验值为 DOM 节点并返回 (kind, ObjectId)。
fn dom_node_parts(heap: &ObjectHeap, value: &Value) -> Result<(i32, ObjectId), OdinRuntimeError> {
    let Value::Object(object_id) = value
    else {
        return Err(invalid_handle(value.type_name().to_string()));
    };
    match heap.get(*object_id) {
        Some(ObjectPayload::LayoutObject { layout_id, slots }) if *layout_id == DOM_NODE_LAYOUT_ID => match slots.get(SLOT_KIND) {
            Some(Value::I32(kind)) => Ok((*kind, *object_id)),
            _ => Err(OdinRuntimeError::ModuleLoad("DOM node missing kind slot".into())),
        },
        Some(_) => Err(invalid_handle("non-DOM object".into())),
        None => Err(OdinRuntimeError::ModuleLoad(format!("invalid object id {object_id}"))),
    }
}

fn require_element(heap: &ObjectHeap, value: &Value) -> Result<ObjectId, OdinRuntimeError> {
    let (kind, object_id) = dom_node_parts(heap, value)?;
    if kind != DOM_KIND_ELEMENT {
        return Err(OdinRuntimeError::TypeMismatch { expected: "element HostElement", actual: format!("DOM node kind {kind}") });
    }
    Ok(object_id)
}

fn set_slot(heap: &mut ObjectHeap, object_id: ObjectId, slot: usize, value: Value) -> Result<(), OdinRuntimeError> {
    heap.set_field(object_id, slot, value).map_err(|reason| match reason {
        "field slot out of range" => OdinRuntimeError::FieldSlotOutOfRange(slot as i32),
        "object not found" => OdinRuntimeError::ModuleLoad(format!("invalid object id {object_id}")),
        other => OdinRuntimeError::TypeMismatch { expected: "DOM node", actual: other.to_string() },
    })
}

fn fresh_node(heap: &mut ObjectHeap, kind: i32, tag: &str, text: &str) -> Result<Value, OdinRuntimeError> {
    let attrs = array_runtime::build_fixed_array(heap, 0, &[]);
    let children = array_runtime::build_fixed_array(heap, 0, &[]);
    Ok(Value::Object(heap.alloc(ObjectPayload::LayoutObject {
        layout_id: DOM_NODE_LAYOUT_ID,
        slots: vec![Value::I32(kind), Value::String(tag.to_string()), attrs, children, Value::String(text.to_string())],
    })))
}

/// `dom_create_element(tag)`：创建元素节点。空标签名结构化拒绝。
pub fn create_element(heap: &mut ObjectHeap, tag: &str) -> Result<Value, OdinRuntimeError> {
    if tag.is_empty() {
        return Err(OdinRuntimeError::TypeMismatch { expected: "non-empty element tag", actual: "empty string".into() });
    }
    fresh_node(heap, DOM_KIND_ELEMENT, tag, "")
}

/// `dom_create_text(text)`：创建文本节点。
pub fn create_text(heap: &mut ObjectHeap, text: &str) -> Result<Value, OdinRuntimeError> {
    fresh_node(heap, DOM_KIND_TEXT, "", text)
}

/// `dom_append(parent, child)`：向元素末尾追加子节点，保持插入顺序。
/// 父节点必须是元素节点，子节点必须是 DOM 节点；违规结构化失败。
pub fn append(heap: &mut ObjectHeap, parent: &Value, child: &Value) -> Result<Value, OdinRuntimeError> {
    let parent_id = require_element(heap, parent)?;
    dom_node_parts(heap, child)?;
    let children = read_slot(heap, parent_id, SLOT_CHILDREN)?;
    let Value::Object(children_id) = children
    else {
        return Err(OdinRuntimeError::ModuleLoad("DOM node children slot is not an object".into()));
    };
    heap.append_field(children_id, child.clone()).map_err(|reason| match reason {
        "object not found" => OdinRuntimeError::ModuleLoad(format!("invalid object id {children_id}")),
        other => OdinRuntimeError::TypeMismatch { expected: "layout array", actual: other.to_string() },
    })?;
    Ok(Value::Null)
}

/// `dom_set_attr(node, name, value)`：设置元素属性；同名覆盖，否则追加。
/// 空属性名结构化拒绝。
pub fn set_attr(heap: &mut ObjectHeap, node: &Value, name: &str, value: &str) -> Result<Value, OdinRuntimeError> {
    if name.is_empty() {
        return Err(OdinRuntimeError::TypeMismatch { expected: "non-empty attribute name", actual: "empty string".into() });
    }
    let node_id = require_element(heap, node)?;
    let attrs = read_slot(heap, node_id, SLOT_ATTRS)?;
    let Value::Object(attrs_id) = attrs
    else {
        return Err(OdinRuntimeError::ModuleLoad("DOM node attrs slot is not an object".into()));
    };
    let len = match heap.get(attrs_id) {
        Some(ObjectPayload::LayoutObject { slots, .. }) => slots.len(),
        _ => return Err(OdinRuntimeError::ModuleLoad(format!("invalid object id {attrs_id}"))),
    };
    for index in 0..len {
        let pair = array_runtime::array_get(heap, &attrs, &Value::I32(index as i32))?;
        let existing = array_runtime::array_get(heap, &pair, &Value::I32(0))?;
        if existing == Value::String(name.to_string()) {
            if let Value::Object(pair_id) = pair {
                set_slot(heap, pair_id, 1, Value::String(value.to_string()))?;
            }
            return Ok(Value::Null);
        }
    }
    let pair = array_runtime::build_fixed_array(heap, 0, &[Value::String(name.to_string()), Value::String(value.to_string())]);
    heap.append_field(attrs_id, pair).map_err(|reason| match reason {
        "object not found" => OdinRuntimeError::ModuleLoad(format!("invalid object id {attrs_id}")),
        other => OdinRuntimeError::TypeMismatch { expected: "layout array", actual: other.to_string() },
    })?;
    Ok(Value::Null)
}

/// `dom_get_attr(node, name)`：读取元素属性；未设置返回空串。
pub fn get_attr(heap: &ObjectHeap, node: &Value, name: &str) -> Result<Value, OdinRuntimeError> {
    let node_id = require_element(heap, node)?;
    let attrs = read_slot(heap, node_id, SLOT_ATTRS)?;
    let len = match &attrs {
        Value::Object(attrs_id) => match heap.get(*attrs_id) {
            Some(ObjectPayload::LayoutObject { slots, .. }) => slots.len(),
            _ => return Err(OdinRuntimeError::ModuleLoad(format!("invalid object id {attrs_id}"))),
        },
        _ => return Err(OdinRuntimeError::ModuleLoad("DOM node attrs slot is not an object".into())),
    };
    for index in 0..len {
        let pair = array_runtime::array_get(heap, &attrs, &Value::I32(index as i32))?;
        let existing = array_runtime::array_get(heap, &pair, &Value::I32(0))?;
        if existing == Value::String(name.to_string()) {
            return array_runtime::array_get(heap, &pair, &Value::I32(1));
        }
    }
    Ok(Value::String(String::new()))
}

/// `dom_set_text(node, text)`：设置元素文本内容并清空既有子节点。
pub fn set_text(heap: &mut ObjectHeap, node: &Value, text: &str) -> Result<Value, OdinRuntimeError> {
    let node_id = require_element(heap, node)?;
    let empty = array_runtime::build_fixed_array(heap, 0, &[]);
    set_slot(heap, node_id, SLOT_CHILDREN, empty)?;
    set_slot(heap, node_id, SLOT_TEXT, Value::String(text.to_string()))?;
    Ok(Value::Null)
}

fn read_slot(heap: &ObjectHeap, object_id: ObjectId, slot: usize) -> Result<Value, OdinRuntimeError> {
    match heap.get(object_id) {
        Some(ObjectPayload::LayoutObject { slots, .. }) => slots.get(slot).cloned().ok_or(OdinRuntimeError::FieldSlotOutOfRange(slot as i32)),
        Some(_) => Err(OdinRuntimeError::TypeMismatch { expected: "layout object", actual: "non-layout object".into() }),
        None => Err(OdinRuntimeError::ModuleLoad(format!("invalid object id {object_id}"))),
    }
}

/// 从堆还原显式类型化的 DOM 树：先校验布局身份，再按合同槽位序读出。
/// 不是从线性内存按偏移猜测；非法句柄与越出深度上限一律结构化失败。
pub fn read_tree(heap: &ObjectHeap, root: &Value) -> Result<DomNode, OdinRuntimeError> {
    read_tree_at(heap, root, 0)
}

fn read_tree_at(heap: &ObjectHeap, value: &Value, depth: usize) -> Result<DomNode, OdinRuntimeError> {
    if depth > MAX_READ_TREE_DEPTH {
        return Err(OdinRuntimeError::ModuleLoad(format!("DOM tree exceeds read limit {MAX_READ_TREE_DEPTH}")));
    }
    let (kind, object_id) = dom_node_parts(heap, value)?;
    if kind == DOM_KIND_TEXT {
        let text = read_string(&read_slot(heap, object_id, SLOT_TEXT)?, "text node content")?;
        return Ok(DomNode::Text(text));
    }
    if kind != DOM_KIND_ELEMENT {
        return Err(OdinRuntimeError::ModuleLoad(format!("unknown DOM node kind {kind}")));
    }
    let tag = read_string(&read_slot(heap, object_id, SLOT_TAG)?, "element tag")?;
    let text = read_string(&read_slot(heap, object_id, SLOT_TEXT)?, "element text")?;
    let attrs_value = read_slot(heap, object_id, SLOT_ATTRS)?;
    let children_value = read_slot(heap, object_id, SLOT_CHILDREN)?;

    let mut attrs = Vec::new();
    if let Value::Object(attrs_id) = attrs_value {
        let len = match heap.get(attrs_id) {
            Some(ObjectPayload::LayoutObject { slots, .. }) => slots.len(),
            _ => return Err(OdinRuntimeError::ModuleLoad(format!("invalid object id {attrs_id}"))),
        };
        for index in 0..len {
            let pair = array_runtime::array_get(heap, &attrs_value, &Value::I32(index as i32))?;
            let name = read_string(&array_runtime::array_get(heap, &pair, &Value::I32(0))?, "attribute name")?;
            let attr_value = read_string(&array_runtime::array_get(heap, &pair, &Value::I32(1))?, "attribute value")?;
            attrs.push((name, attr_value));
        }
    }

    let mut children = Vec::new();
    if let Value::Object(children_id) = children_value {
        let len = match heap.get(children_id) {
            Some(ObjectPayload::LayoutObject { slots, .. }) => slots.len(),
            _ => return Err(OdinRuntimeError::ModuleLoad(format!("invalid object id {children_id}"))),
        };
        for index in 0..len {
            let child = array_runtime::array_get(heap, &children_value, &Value::I32(index as i32))?;
            children.push(read_tree_at(heap, &child, depth + 1)?);
        }
    }

    Ok(DomNode::Element { tag, attrs, children, text })
}

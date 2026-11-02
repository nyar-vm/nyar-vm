//! DOM 树 → HTML 的交付序列化。
//!
//! 序列化是交付适配（escape、void 元素、作用域前缀），归 Rust 交付层；
//! 渲染语义（树长什么样）已经全部在 V 侧完成。透明片段容器
//! （`asgard:fragment`）在这里拆包，不留下标签。

use std::fmt::Write as _;

use oak_awsl::is_html_void_element;
use odin_vm::dom_runtime::DomNode;

use crate::asgard::codegen::ssg_ovm::emit::FRAGMENT_TAG;

/// 把读出 ABI 还原的 DOM 树序列化为 HTML 片段。
pub(crate) fn serialize_body(root: &DomNode) -> String {
    let mut out = String::new();
    emit_node(&mut out, root);
    out
}

fn emit_node(out: &mut String, node: &DomNode) {
    match node {
        DomNode::Text(text) => out.push_str(&escape_html(text)),
        DomNode::Element { tag, attrs, children, text } => {
            if tag == FRAGMENT_TAG {
                for child in children {
                    emit_node(out, child);
                }
                return;
            }
            write!(out, "<{tag}").expect("写入内存字符串不可失败");
            for (name, value) in attrs {
                // 与迁移前交付形状一致：空值属性不输出（`class`/`href` 例外保留）。
                if value.is_empty() && name != "class" && name != "href" {
                    continue;
                }
                write!(out, " {name}=\"{}\"", escape_attr(value)).expect("写入内存字符串不可失败");
            }
            if children.is_empty() && text.is_empty() && is_html_void_element(tag) {
                out.push_str(" />");
                return;
            }
            out.push('>');
            if !text.is_empty() {
                out.push_str(&escape_html(text));
            }
            for child in children {
                emit_node(out, child);
            }
            write!(out, "</{tag}>").expect("写入内存字符串不可失败");
        }
    }
}

/// 组件名转 kebab-case（作用域 class 前缀的组成规则，与迁移前一致）。
pub(crate) fn to_kebab_case(name: &str) -> String {
    let mut out = String::new();
    for (index, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if index > 0 {
                out.push('-');
            }
            out.push(ch.to_ascii_lowercase());
        }
        else if ch == '_' {
            out.push('-');
        }
        else {
            out.push(ch);
        }
    }
    out
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// 属性值转义（供本模块与外层包装使用）。
pub(crate) fn escape_attr(text: &str) -> String {
    escape_html(text).replace('"', "&quot;")
}

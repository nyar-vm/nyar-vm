use rusty_valkyrie::compiler::syntax::oak::{
    ast::{Statement, StatementNode, TemplateNode, Trait},
    parse_source,
};

use rusty_valkyrie::compiler::hir::lowering::{
    HirError,
    errors::{TemplateConstruct, TemplateFailure},
    expand_templates_in_root,
};

#[test]

fn expands_module_level_arch_match() {
    let source = r#"<% match arch %>

<% case "wasm32" %>

micro main() -> i32 { return 23 }

<% else %>

micro main() -> i32 { return 0 }

<% end %>"#;

    let mut root = parse_source(source).expect("parse");

    expand_templates_in_root(&mut root, source, "wasm32").expect("expand");

    assert_eq!(root.items.len(), 1);

    assert!(matches!(root.items[0], StatementNode::Micro(_)));
}

#[test]

fn parses_template_match_inside_micro_body_before_expand() {
    let source = r#"micro main() -> i32 {

<% match arch %>

<% case "wasm32" %>

return 23

<% else %>

return 0

<% end %>

}"#;

    let root = parse_source(source).expect("parse");

    let StatementNode::Micro(micro) = &root.items[0]
    else {
        panic!("expected micro")
    };

    assert_eq!(micro.body.statements.len(), 1, "micro body should contain one template statement");

    let Statement::Template(template) = &micro.body.statements[0]
    else {
        panic!("expected template statement")
    };

    let TemplateNode::Match(match_node) = template.as_ref()
    else {
        panic!("expected match template")
    };

    assert_eq!(match_node.arms.len(), 2);

    assert_eq!(match_node.arms[0].body.len(), 1, "wasm32 arm body should contain parsed `return` statement");
}

#[test]

fn expands_arch_match_inside_micro_body() {
    let source = r#"micro main() -> i32 {

<% match arch %>

<% case "wasm32" %>

return 23

<% else %>

return 0

<% end %>

}"#;

    let mut root = parse_source(source).expect("parse");

    expand_templates_in_root(&mut root, source, "wasm32").expect("expand");

    let StatementNode::Micro(micro) = &root.items[0]
    else {
        panic!("expected micro")
    };

    assert!(!micro.body.statements.is_empty());

    assert!(!matches!(micro.body.statements[0], Statement::Template(_)));
}

#[test]

fn expands_arch_if_predicate() {
    let source = r#"micro main() -> i32 {

<% if arch == "wasm32" %>

return 42

<% else %>

return 0

<% end %>

}"#;

    let mut root = parse_source(source).expect("parse");

    expand_templates_in_root(&mut root, source, "wasm32").expect("expand");

    let StatementNode::Micro(micro) = &root.items[0]
    else {
        panic!("expected micro")
    };

    assert_eq!(micro.body.statements.len(), 1);

    let mut root_native = parse_source(source).expect("parse");

    expand_templates_in_root(&mut root_native, source, "native").expect("expand");

    let StatementNode::Micro(micro_native) = &root_native.items[0]
    else {
        panic!("expected micro")
    };

    assert_eq!(micro_native.body.statements.len(), 1);
}

#[test]
fn expands_const_loop_unroll() {
    let source = r#"micro main() -> i32 {
<% loop _ in 0..2 %>
return 1
<% end %>
}"#;
    let mut root = parse_source(source).expect("parse");
    expand_templates_in_root(&mut root, source, "native").expect("expand");
    let StatementNode::Micro(micro) = &root.items[0]
    else {
        panic!("expected micro")
    };
    assert_eq!(micro.body.statements.len(), 2, "loop 0..2 should unroll two `return` statements");
    assert!(micro.body.statements.iter().all(|stmt| !matches!(stmt, Statement::Template(_))));
}

#[test]
fn expands_fragment_arch_as_string_literal() {
    let source = r#"micro main() -> i32 {
<% arch %>
}"#;
    let mut root = parse_source(source).expect("parse");
    expand_templates_in_root(&mut root, source, "wasm32").expect("expand");
    let StatementNode::Micro(micro) = &root.items[0]
    else {
        panic!("expected micro")
    };
    assert_eq!(micro.body.statements.len(), 1);
    assert!(matches!(micro.body.statements[0], Statement::Expression(_)));
}

#[test]
fn expands_nested_if_inside_const_loop() {
    let source = r#"micro main() -> i32 {
<% loop _ in 0..1 %>
<% if arch == "wasm32" %>
return 7
<% end %>
<% end %>
}"#;
    let mut root = parse_source(source).expect("parse");
    expand_templates_in_root(&mut root, source, "wasm32").expect("expand");
    let StatementNode::Micro(micro) = &root.items[0]
    else {
        panic!("expected micro")
    };
    assert_eq!(micro.body.statements.len(), 1);
    assert!(!matches!(micro.body.statements[0], Statement::Template(_)));
}

#[test]
fn expands_arch_if_else_if_predicate() {
    let source = r#"micro main() -> i32 {
<% if arch == "wasm32" %>
return 1
<% else if arch == "native" %>
return 2
<% else %>
return 3
<% end %>
}"#;
    let mut root = parse_source(source).expect("parse");
    expand_templates_in_root(&mut root, source, "native").expect("expand");
    let StatementNode::Micro(micro) = &root.items[0]
    else {
        panic!("expected micro")
    };
    assert_eq!(micro.body.statements.len(), 1);
}

#[test]
fn expands_false_if_to_empty_body() {
    let source = r#"micro main() -> i32 {
<% if arch == "wasm32" %>
return 1
<% end %>
}"#;
    let mut root = parse_source(source).expect("parse");
    expand_templates_in_root(&mut root, source, "native").expect("expand");
    let StatementNode::Micro(micro) = &root.items[0]
    else {
        panic!("expected micro")
    };
    assert!(micro.body.statements.is_empty());
}

#[test]
fn expands_trait_body_template_members_in_source_order() {
    let source = r#"trait T {
    micro before(self) -> bool
<% match arch %>
<% case "wasm32" %>
    micro inside(self) -> bool
<% end %>
    micro after(self) -> bool
}"#;
    let mut root = parse_source(source).expect("parse");
    expand_templates_in_root(&mut root, source, "wasm32").expect("expand");
    let StatementNode::Trait(trait_declaration) = &root.items[0]
    else {
        panic!("expected trait")
    };
    assert!(trait_declaration.body.templates.is_empty(), "body templates should be consumed");
    let names: Vec<&str> = trait_declaration.body.methods.iter().map(|method| method.name.name.as_str()).collect();
    assert_eq!(names, ["before", "inside", "after"], "template member must be restored at its source position");
}

#[test]
fn expands_gated_iterator_style_trait_template_for_non_clr_arch() {
    let source = r#"namespace std.iterator;
trait Iterator {
    type Item;
    micro has_next(self): bool
    micro next(mut self): Option<Item>
    <% match arch %>
    <% case "clr" %>
    <% else %>
    micro map<U>(self, f: micro(Item) -> U) -> MapIterator<Item, U, Self> {
        return MapIterator { _iter: self, _mapper: f }
    }
    micro filter(self, pred: micro(Item) -> bool) -> FilterIterator<Item, Self> {
        return FilterIterator { _iter: self, _predicate: pred }
    }
    micro filter_map<U>(self, f: micro(Item) -> Option<U>) -> FilterMapIterator<Item, U, Self> {
        return FilterMapIterator { _iter: self, _mapper: f }
    }
    micro skip_while(self, pred: micro(Item) -> bool) -> SkipWhileIterator<Item, Self> {
        return SkipWhileIterator { _iter: self, _predicate: pred, _skipped: false }
    }
    micro take_while(self, pred: micro(Item) -> bool) -> TakeWhileIterator<Item, Self> {
        return TakeWhileIterator { _iter: self, _predicate: pred, _done: false }
    }
    <% end %>
}"#;
    let mut root = parse_source(source).expect("parse");
    expand_templates_in_root(&mut root, source, "ovm").expect("expand");
    let trait_declaration = root
        .items
        .iter()
        .find_map(|item| match item {
            StatementNode::Trait(trait_declaration) => Some(trait_declaration.as_ref()),
            _ => None,
        })
        .expect("trait");
    let names: Vec<&str> = trait_declaration.body.methods.iter().map(|method| method.name.name.as_str()).collect();
    assert_eq!(names, ["has_next", "next", "map", "filter", "filter_map", "skip_while", "take_while"]);
}

#[test]
fn rejects_non_micro_template_member_in_trait_body() {
    let source = r#"trait T {
<% match arch %>
<% case "wasm32" %>
structure S { x: i32 }
<% end %>
}"#;
    let mut root = parse_source(source).expect("parse");
    let error = expand_templates_in_root(&mut root, source, "wasm32").expect_err("non-micro template member must fail");
    assert!(matches!(error, HirError::TemplateExpansionFailed { .. }));
    let message = error.to_string();
    assert!(message.contains("declaration body"), "unexpected message: {message}");
    assert!(message.contains("structure"), "unexpected message: {message}");
}

/// 与 `valkyrie.v` 的 `projects/std/source/iterator/Iterator.v` 逐字一致，仅补 `namespace std.iterator;` 头。
const REAL_ITERATOR_SOURCE: &str = r#"namespace std.iterator;

⍝ 迭代器 trait
⍝ 定义按顺序产出元素的统一接口。
trait Iterator {
    type Item;

    ⍝ 判断当前迭代器是否还有下一个元素。
    micro has_next(self): bool
    ⍝ 返回下一个元素。
    ⍝ 当迭代结束时返回 `None`，否则返回 `Some(Item)`。
    ⍝ 这里不能设计成 `Item?`，因为当 `Item` 自身允许为 `null` 时，
    ⍝ `Item?` 无法区分“产出了一个 `null`”与“迭代已经结束”这两种语义。
    micro next(mut self): Option<Item>

    micro enumerate(self) -> EnumerateIterator<Item, Self> {
        return EnumerateIterator {
            _iter: self,
            _ordinal: 1,
        }
    }

    micro skip(self, count: usize) -> SkipIterator<Item, Self> {
        return SkipIterator {
            _iter: self,
            _count: count,
            _skipped: 0,
        }
    }

    micro take(self, count: usize) -> TakeIterator<Item, Self> {
        return TakeIterator {
            _iter: self,
            _count: count,
            _taken: 0,
        }
    }

    # Function-valued adapters (`filter` / `map` / …) need CLR DynamicInvoke for
    # field calls like `self._predicate(item)`. Gate them off the CLR seed path until
    # that ABI is wired; other backends keep the full adaptor surface.
    <% match arch %>
    <% case "clr" %>
    <% else %>
    micro map<U>(self, f: micro(Item) -> U) -> MapIterator<Item, U, Self> {
        return MapIterator {
            _iter: self,
            _mapper: f,
        }
    }

    micro filter(self, pred: micro(Item) -> bool) -> FilterIterator<Item, Self> {
        return FilterIterator {
            _iter: self,
            _predicate: pred,
        }
    }

    micro filter_map<U>(self, f: micro(Item) -> Option<U>) -> FilterMapIterator<Item, U, Self> {
        return FilterMapIterator {
            _iter: self,
            _mapper: f,
        }
    }

    micro skip_while(self, pred: micro(Item) -> bool) -> SkipWhileIterator<Item, Self> {
        return SkipWhileIterator {
            _iter: self,
            _predicate: pred,
            _skipped: false,
        }
    }

    micro take_while(self, pred: micro(Item) -> bool) -> TakeWhileIterator<Item, Self> {
        return TakeWhileIterator {
            _iter: self,
            _predicate: pred,
            _done: false,
        }
    }
    <% end %>
}
"#;

fn find_trait_declaration(items: &[StatementNode]) -> Option<&Trait> {
    items.iter().find_map(|item| match item {
        StatementNode::Trait(trait_declaration) => Some(trait_declaration.as_ref()),

        StatementNode::Namespace(namespace) => namespace.body.as_ref().and_then(|body| find_trait_declaration(&body.items)),

        _ => None,
    })
}

fn expanded_real_iterator_methods(arch: &str) -> Vec<String> {
    let mut root = parse_source(REAL_ITERATOR_SOURCE).expect("parse real Iterator.v");
    expand_templates_in_root(&mut root, REAL_ITERATOR_SOURCE, arch).expect("expand real Iterator.v");
    let trait_declaration = find_trait_declaration(&root.items).expect("Iterator trait");
    assert!(trait_declaration.body.templates.is_empty(), "declaration body templates must be consumed");
    trait_declaration.body.methods.iter().map(|method| method.name.name.clone()).collect()
}

#[test]
fn expands_real_iterator_trait_methods_for_ovm_and_wasm() {
    let adapters = ["map", "filter", "filter_map", "skip_while", "take_while"];

    let mut expected = vec!["has_next", "next", "enumerate", "skip", "take"];
    expected.extend(adapters);

    let ovm = expanded_real_iterator_methods("ovm");
    assert_eq!(ovm, expected, "ovm keeps the full adaptor surface");

    let wasm = expanded_real_iterator_methods("wasm32");
    assert_eq!(wasm, expected, "wasm32 keeps the full adaptor surface");

    let clr = expanded_real_iterator_methods("clr");
    assert_eq!(clr, ["has_next", "next", "enumerate", "skip", "take"], "clr gates the function-valued adaptors");
    assert!(!clr.iter().any(|name| name == "arch"), "the `match arch` scrutinee must not become a method");
}

#[test]
fn rejects_arch_match_without_matching_or_fallback_arm() {
    let source = r#"trait T {
<% match arch %>
<% case "clr" %>
micro ping() -> i64
<% end %>
}"#;
    let mut root = parse_source(source).expect("parse");
    let error = expand_templates_in_root(&mut root, source, "wasm32").expect_err("arch without a matching arm must fail");
    match error {
        HirError::TemplateExpansionFailed { construct, failure, .. } => {
            assert_eq!(construct, TemplateConstruct::Match);
            assert_eq!(failure, TemplateFailure::NoMatchingArchArm { arch: "wasm32".to_string() });
        }
        other => panic!("unexpected error: {other:?}"),
    }
}


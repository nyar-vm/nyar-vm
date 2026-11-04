//! 可复现的 `let set` 绑定探针。
//!
//! 只读 HIR 的公开结构，不含任何编译器内部插桩。resolver 的局部表是
//! `pub(crate)`，探针不伪造它，而是给出绑定可见性的可观测后果：
//! `let` 的 pattern 必须是 `Variable(set)`，且 `set.length()` 的成员候选
//! 必须由这个绑定名解析出 `DisjointSet.length` 的声明身份。

use rusty_valkyrie::{
    SourceID, ValkyrieCompiler,
    compiler::types::hir::{HirExprKind, HirModule, HirPattern, HirStatementKind},
};

const SOURCE: &str = r#"
[primitive("std::primitive::i32")] structure i32 { }

micro probe(input: DisjointSet<i32>) -> i32 {
    let set: DisjointSet<i32> = input
    return set.length()
}

class DisjointSet<T> {
    _impl: i32
    micro length(self) -> i32 { return 0 }
}
"#;

fn main() {
    let module: HirModule = ValkyrieCompiler::new(SourceID { version_id: 9001 })
        .compile_source(SOURCE)
        .expect("probe source must reach HIR");
    let probe = module.functions.iter().find(|function| function.name.as_str() == "probe").expect("`probe` must be a module function");

    let mut let_names = Vec::new();
    for statement in &probe.body.statements {
        let HirStatementKind::Let { pattern, ty, initializer, .. } = &statement.kind else { continue };
        if let HirPattern::Variable(identifier) = pattern {
            let_names.push(identifier.name.to_string());
        }
        println!("let.pattern={pattern:?}");
        println!("let.declared_type={ty:?}");
        println!("let.initializer={:?}", initializer.as_ref().map(|expr| expr.kind.callable_label()));
    }

    let tail = probe.body.expr.as_deref().expect("`probe` must end with a tail expression");
    let HirExprKind::Return(Some(returned)) = &tail.kind else {
        panic!("expected tail return expression, got {tail:?}");
    };
    let HirExprKind::Call { callee, args, resolved } = &returned.kind else {
        panic!("expected `set.length()` call, got {returned:?}");
    };
    let has_receiver = matches!(callee.kind, HirExprKind::FieldAccess { .. });
    let resolved = resolved.as_ref().expect("`set.length()` must resolve at HIR");
    println!("call.callee={} call.has_receiver={has_receiver}", callee.kind.callable_label());
    println!("call.arguments={}", args.len());
    println!("call.resolved.symbol={}", resolved.symbol);
    println!("call.resolved.declaration={:?}", resolved.declaration);
    println!("call.resolved.domain={:?}", resolved.domain);
    println!("call.resolved.return_type={:?}", resolved.return_type);
    println!("let.bound_names={let_names:?}");

    assert!(let_names.iter().any(|name| name == "set"), "`let set` must bind a named variable, got {let_names:?}");
    assert!(has_receiver, "`set.length()` must be a member call with a receiver");
    assert_eq!(resolved.symbol.to_string(), "DisjointSet.length", "member call must bind the `DisjointSet.length` declaration");
    assert!(resolved.declaration.is_some(), "instance method resolution must carry the source declaration identity");
    println!("binding_visible=true");
}

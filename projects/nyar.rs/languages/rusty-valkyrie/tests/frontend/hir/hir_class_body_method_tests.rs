//! 类/结构体声明体的方法入口：必须在 HIR 边界结构化拒绝。
//!
//! 固有方法只有 `imply Foo<T> { ... }` 一个载体；类体方法一旦写进去，
//! 调用点曾静默跌成 MIR 的 `UnresolvedCallableIdentity`，没有任何前端诊断。

use rusty_valkyrie::ValkyrieCompiler;

#[test]
fn class_body_method_is_rejected_with_a_structured_diagnostic() {
    let source = r#"
namespace app;

class Foo<T> {
    _value: T

    micro bar(self) -> i32 { return 0 }
}
"#;
    let error = ValkyrieCompiler::default().compile_source(source).expect_err("类体方法必须在 HIR 边界被拒绝");
    let message = error.to_string();
    assert!(message.contains("imply"), "诊断必须直接给出 `imply` 载体：{message}");
    assert!(message.contains("bar"), "诊断必须点名违规方法：{message}");
    assert!(message.contains("Foo"), "诊断必须点名声明体 owner：{message}");
}

#[test]
fn imply_block_methods_stay_available_and_struct_bodies_remain_empty() {
    let source = r#"
namespace app;

class Foo<T> {
    _value: T
}

imply Foo<T> {
    micro bar(self): T { return self._value }
}
"#;
    let hir = ValkyrieCompiler::default().compile_source(source).expect("`imply` 块方法必须继续可用");
    assert!(hir.structs.iter().all(|item| item.methods.is_empty()), "类/结构体声明体不得再产出方法");
    assert!(
        hir.impls.iter().any(|block| block.methods.iter().any(|method| method.name.as_str() == "bar")),
        "`imply` 块方法必须保留在 impl 声明表里"
    );
}

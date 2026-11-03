//! `impl` 体内 `Self` 与显式 impl target 类型的等价性。

use rusty_valkyrie::{ValkyrieCompiler, compiler::types::hir::HirStatementKind, types::hir::ValkyrieType};

fn compile(source: &str) -> rusty_valkyrie::types::hir::HirModule {
    ValkyrieCompiler::default().compile_source(source).expect("impl 体内 `Self` 源码必须完成 HIR")
}

const SELF_BODY_SOURCE: &str = r#"
class Holder<T> {
    _value: T
}

imply Holder<T> {
    micro new(value: T): Self {
        return Holder { _value: value }
    }

    micro get(self) -> T {
        return self._value
    }

    micro store(mut self, value: T): unit {
        self._value = value
    }

    micro rebuild_self(value: T): Self {
        let mut r: Self = Self::new(value)
        r.store(value)
        return r
    }

    micro rebuild_explicit(value: T): Self {
        let mut r: Holder<T> = Self::new(value)
        r.store(value)
        return r
    }
}
"#;

const SELF_UNKNOWN_MEMBER_SOURCE: &str = r#"
class Holder<T> {
    _value: T
}

imply Holder<T> {
    micro new(value: T): Self {
        return Holder { _value: value }
    }

    micro store(mut self, value: T): unit {
        self._value = value
    }

    micro rebuild_self(value: T): Self {
        let mut r: Self = Self::new(value)
        r.missing_member(value)
        return r
    }
}
"#;

/// 取 impl 方法体里第一条带显式类型标注的 `let` 类型。
fn impl_method_let_type(module: &rusty_valkyrie::types::hir::HirModule, method_name: &str) -> ValkyrieType {
    let method = module
        .impls
        .iter()
        .flat_map(|impl_block| impl_block.methods.iter())
        .find(|method| method.name.as_str() == method_name)
        .expect("impl 方法必须存在");
    method
        .body
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            HirStatementKind::Let { ty, .. } => ty.clone(),
            _ => None,
        })
        .expect("方法体必须有显式类型标注的 let")
}

/// `Self` 在 impl 体内必须已代入 impl 头部 target（含泛型实参）：
/// 局部变量因此与显式写 `Holder<T>` 同型，成员调用才能解析到 owner 的候选。
#[test]
fn impl_body_self_local_equals_explicit_impl_target_type() {
    let module = compile(SELF_BODY_SOURCE);
    let self_ty = impl_method_let_type(&module, "rebuild_self");
    let explicit_ty = impl_method_let_type(&module, "rebuild_explicit");
    assert!(!matches!(self_ty, ValkyrieType::SelfType), "impl 体内 `Self` 不得保持未代入形态: {self_ty:?}");
    assert_eq!(self_ty, explicit_ty, "`Self` 与显式 impl target 类型必须在 impl 体内同型");
}

/// 反例：把接收者写成 `Self` 不会放宽成员解析——不存在的方法仍必须 unresolved。
#[test]
fn self_local_unknown_member_is_still_unresolved() {
    let failure = ValkyrieCompiler::default()
        .compile_source(SELF_UNKNOWN_MEMBER_SOURCE)
        .expect_err("`Self` 接收者上不存在的方法必须解析失败");
    let message = format!("{failure}");
    assert!(message.contains("unresolved"), "必须是 unresolved 结构化诊断，实际: {message}");
}

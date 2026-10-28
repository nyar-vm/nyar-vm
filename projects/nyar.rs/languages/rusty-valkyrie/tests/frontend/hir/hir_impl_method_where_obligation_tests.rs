//! `imply` 成员方法在宿主泛型参数上调用带 `where` 约束的兄弟方法。
//!
//! `imply Table<K, V>` 的泛型环境没有任何 `where`，成员 `get` 调用
//! `find_slot(self, key: K) where K: Equal, K: Hash` 时，`K` 仍是调用方的
//! 刚性泛型参数。约束 evidence 只能在实例化时按具体 `K` 履行，因此解析阶段
//! 必须接受该调用，而不是把它降成 `<computed>`。

use rusty_valkyrie::{
    ValkyrieCompiler,
    types::hir::{HirExpr, HirExprKind, HirFunction, HirModule, HirStatementKind, ValkyrieType},
};

fn impl_method<'a>(module: &'a HirModule, name: &str) -> &'a HirFunction {
    module
        .impls
        .iter()
        .flat_map(|implementation| implementation.methods.iter())
        .find(|method| method.name.as_str() == name)
        .unwrap_or_else(|| panic!("imply method `{name}` 必须出现在 HIR"))
}

fn first_statement_expr(method: &HirFunction) -> &HirExpr {
    let from_statements = method.body.statements.iter().find_map(|statement| match &statement.kind {
        HirStatementKind::Let { initializer: Some(initializer), .. } => Some(initializer.as_ref()),
        HirStatementKind::Expr(expression) => Some(expression.as_ref()),
        _ => None,
    });
    from_statements.or(method.body.expr.as_deref()).expect("方法体必须包含调用语句")
}

#[test]
fn impl_method_resolves_bounded_sibling_on_open_generic_target() {
    let compiler = ValkyrieCompiler::default();
    let module = compiler
        .compile_source(
            r#"
trait Equal { micro equal(self, other: Self): bool }
trait Hash { micro hash(self): usize }
class Table<K, V> { }
imply Table<K, V> {
    micro get(self, key: K): bool {
        return self.find_slot(key)
    }
    micro find_slot(self, key: K): bool where K: Equal, K: Hash {
        return true
    }
}
"#,
        )
        .expect("开放泛型目标上的 where 义务必须由实例化履行，不能在解析阶段拒绝");

    let get = impl_method(&module, "get");
    let HirExprKind::Return(Some(call)) = &first_statement_expr(get).kind
    else {
        panic!("get 必须返回 find_slot 的调用结果")
    };
    let HirExprKind::Call { resolved: Some(resolved), .. } = &call.kind
    else {
        panic!("`self.find_slot(key)` 必须形成已解析调用合同")
    };
    assert_eq!(resolved.symbol.to_string(), "Table.find_slot");
    assert_eq!(resolved.return_type, ValkyrieType::Boolean);

    // 约束目标必须携带 `imply Table<K, V>` 冻结出的 binder；残留无 binder 的 `Generic`
    // 会让调用点代入与 evidence 查找永远对不上。
    let find_slot = impl_method(&module, "find_slot");
    let target = &find_slot.where_constraints[0].target;
    assert!(
        matches!(target, ValkyrieType::Generic(generic) if generic.binder.is_some()),
        "imply 方法的 where 目标必须绑定 owner 泛型参数，got {target:?}"
    );
}

#[test]
fn generic_field_projection_substitutes_declaration_arguments() {
    let compiler = ValkyrieCompiler::default();
    let module = compiler
        .compile_source(
            r#"
trait Equal { infix `==`(self, rhs: Self): bool }
structure Entry<K, V> {
    key: K
    value: V
    hash: usize
}
class Table<K, V> { }
imply Table<K, V> {
    micro same(self, key: K, entry: Entry<K, V>): bool where K: Equal {
        return entry.key == key
    }
}
"#,
        )
        .expect("泛型实例的成员字段类型必须代入声明实参，否则 `K: Equal` 的 evidence 对不上");

    let same = impl_method(&module, "same");
    let HirExprKind::Return(Some(comparison)) = &first_statement_expr(same).kind
    else {
        panic!("same 必须返回相等比较结果")
    };
    let HirExprKind::Call { resolved: Some(resolved), .. } = &comparison.kind
    else {
        panic!("`entry.key == key` 必须解析到 `Equal.infix ==`，got {:?}", comparison.kind)
    };
    assert_eq!(resolved.symbol.to_string(), "Equal.infix ==");
}

#[test]
fn concrete_target_without_evidence_is_still_rejected() {
    let compiler = ValkyrieCompiler::default();
    assert!(
        compiler
            .compile_source(
                r#"
trait Equal { micro equal(self, other: Self): bool }
trait Hash { micro hash(self): usize }
class Table<K, V> { }
imply Table<K, V> {
    micro find_slot(self, key: K): bool where K: Equal, K: Hash {
        return true
    }
}
micro caller(table: Table<bool, bool>, key: bool): bool {
    return table.find_slot(key)
}
"#,
            )
            .is_err(),
        "具体类型缺少 Equal/Hash evidence 时不得解析成功"
    );
}

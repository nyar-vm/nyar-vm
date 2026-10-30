//! 数组 push 的前端身份只来自源码声明（普通声明 callable，不再是公共结构操作身份）。
//!
//! `std/source/collection/Array.v` 的 `push` 声明是唯一前端来源；
//! `[intrinsic("builtin.array.push")]` 注解已退役，不再产生 `BuiltinStructureOp` 身份
//! （与 `array.insert` 等不产生身份的注解先例一致），push 调用按声明签名走普通候选匹配。
//! 删除该声明后调用必须结构化失败，且失败不得靠名字表或默认值兜底。

use rusty_valkyrie::{
    HirError, ValkyrieCompiler,
    types::hir::{HirExprKind, HirFunction, HirStatementKind},
};

/// 与 std 声明同形的唯一前端来源；注解路径保留拼写但不再产生公共结构身份。
const ARRAY_PUSH_INTRINSIC_DECLARATION: &str = "[intrinsic(\"builtin.array.push\")] micro push<T>(array: [T], value: T) -> [T] { } ";

const GROW: &str = "micro grow(text: utf8) -> [utf8] { let mut result: [utf8] = [] push(result, text) return result }";

const QUALIFIED_GROW: &str = "micro grow(text: utf8) -> [utf8] { let mut result: [utf8] = [] builtin.array.push(result, text) return result }";

fn find_function<'a>(module: &'a rusty_valkyrie::compiler::types::hir::HirModule, name: &str) -> &'a HirFunction {
    module.functions.iter().find(|function| function.name.as_str() == name).unwrap_or_else(|| panic!("缺少 `{name}` 声明"))
}

fn push_call(function: &HirFunction) -> &rusty_valkyrie::compiler::types::hir::HirResolvedCall {
    for statement in &function.body.statements {
        if let HirStatementKind::Expr(expression) = &statement.kind
            && let HirExprKind::Call { resolved: Some(resolved), .. } = &expression.kind
        {
            return resolved;
        }
    }
    panic!("expected a resolved push call: {:?}", function.body);
}

#[test]
fn declared_array_push_binds_declared_callable_without_builtin_identity() {
    let compiler = ValkyrieCompiler::default();
    let hir = compiler.compile_source(&format!("{ARRAY_PUSH_INTRINSIC_DECLARATION}{GROW}")).expect("源码内声明的数组 push 结构操作必须解析");

    let declaration = find_function(&hir, "push").declaration;
    let resolved = push_call(find_function(&hir, "grow"));
    assert_eq!(resolved.intrinsic, None, "builtin.array.push 注解不再产生公共结构身份；push 必须是普通声明 callable");
    assert_eq!(resolved.declaration, declaration, "调用必须绑定到源码声明的完整身份，而不是前端合成候选");
    assert_eq!(resolved.symbol.parts().last().map(|part| part.as_str()), Some("push"), "符号必须是声明名，而不是合成路径: {}", resolved.symbol);
}

#[test]
fn undeclared_array_push_intrinsic_fails_without_frontend_synthesis() {
    for probe in [GROW, QUALIFIED_GROW] {
        let error = ValkyrieCompiler::default().compile_source_to_program(probe).expect_err("缺少 intrinsic 声明时数组 push 调用不得成功");
        let rendered = format!("{error:?}");
        assert!(matches!(error, HirError::UnresolvedCall { .. }), "缺少声明必须结构化失败为 UnresolvedCall: {rendered}");
        assert!(rendered.contains("push"), "失败必须指出未解析的 push 调用: {rendered}");
    }
}

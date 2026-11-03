//! 跨源码组限定 turbofish 泛型调用的 frontend 守卫：
//! `std::collection::array::new::<T>(n)` 的声明在依赖组，显式泛型实参又是调用方
//! 自己的泛型参数 `T` 时，调用必须绑定到依赖声明身份，不能被判成 `<computed>`。

use rusty_valkyrie::{
    ValkyrieCompiler,
    compiler::{
        hir::lowering::CompilerSourceGroup,
        types::hir::{HirExpr, HirExprKind, HirFunction, HirModule, HirPattern, HirStatementKind},
    },
};

/// 数值原始类型名属于 `std/primitive` 的声明族；`usize` 是生产 std 的指针宽度别名。
const PRIMITIVES: &str = concat!(
    "namespace std::primitive;\n",
    "[primitive(\"std::primitive::u64\")] structure u64 { }\n",
    "[primitive(\"std::primitive::i32\")] structure i32 { }\n",
    "type usize = u64;\n",
    "imply u64 {\n",
    "  infix `+`(self, rhs: Self): Self { return rhs }\n",
    "}"
);

/// 依赖组：`[host_contract]` 无体泛型声明，显式泛型实参由调用方绑定。
const ARRAY_CONTRACT: &str = "namespace std.collection.array;\n\
    [host_contract]\n\
    micro new<T>(length: usize): [T]";

/// 调用方组：用自己的泛型参数 `T` 显式指定依赖声明的泛型实参。
const CALLER: &str = concat!(
    "namespace std.collection;\n",
    "[host_contract]\n",
    "micro fill<T>(value: T, n: usize): [T] {\n",
    "  let old_len: usize = n\n",
    "  let new_array: [T] = std::collection::array::new::<T>(old_len + 1)\n",
    "  return new_array\n",
    "}\n",
    "[main] micro main() -> i32 {\n",
    "  fill(1, 2)\n",
    "  return 0\n",
    "}"
);

fn groups() -> Vec<CompilerSourceGroup> {
    vec![
        CompilerSourceGroup::single_source("core", "core", PRIMITIVES),
        CompilerSourceGroup::single_source("std", "std", ARRAY_CONTRACT).with_direct_dependencies(vec!["core".into()]),
        CompilerSourceGroup::single_source("app", "demo", CALLER)
            .as_entry()
            .with_direct_dependencies(vec!["core".into(), "std".into()]),
    ]
}

fn find_function<'a>(module: &'a HirModule, name: &str) -> &'a HirFunction {
    module.functions.iter().find(|function| function.name.as_str() == name).unwrap_or_else(|| panic!("缺少 `{name}` 声明"))
}

fn let_initializer<'a>(function: &'a HirFunction, name: &str) -> &'a HirExpr {
    function
        .body
        .statements
        .iter()
        .find_map(|statement| match &statement.kind {
            HirStatementKind::Let { pattern: HirPattern::Variable(bound), initializer, .. } if bound.name.as_str() == name => initializer.as_ref(),
            _ => None,
        })
        .unwrap_or_else(|| panic!("缺少 `{name}` 初始化表达式"))
}

fn find_owner<'a>(module: &'a HirModule, name: &str) -> Option<&'a HirModule> {
    if module.functions.iter().any(|function| function.name.as_str() == name) {
        return Some(module);
    }
    module.submodules.iter().find_map(|submodule| find_owner(submodule, name))
}

/// 依赖组声明的 `[host_contract]` 泛型函数被外部组以 `::<T>` 调用时必须解析成功。
#[test]
fn qualified_turbofish_generic_call_binds_dependency_declaration() {
    let groups = groups();
    let modules = ValkyrieCompiler::default()
        .resolve_source_groups(&groups, "native")
        .expect("跨组限定路径 + 泛型参数 turbofish 调用必须完成 HIR");
    let caller = modules
        .iter()
        .find_map(|module| find_owner(module, "fill"))
        .unwrap_or_else(|| panic!("缺少调用方组 HIR: {:?}", modules.iter().map(|module| module.name.to_string()).collect::<Vec<_>>()));
    let dependency = modules.iter().find_map(|module| find_owner(module, "new")).expect("缺少依赖组 HIR");
    let declaration = find_function(dependency, "new").declaration.expect("依赖声明必须冻结声明身份");
    let expression = let_initializer(find_function(caller, "fill"), "new_array");
    let HirExprKind::Call { resolved, .. } = &expression.kind
    else {
        panic!("跨组限定路径 + turbofish 必须落在 Call，不得只剩比较链: {expression:?}");
    };
    assert!(
        resolved.as_ref().is_some_and(|resolved| resolved.declaration == Some(declaration)),
        "调用必须绑定依赖组 `new` 的声明身份: {resolved:?}"
    );
}

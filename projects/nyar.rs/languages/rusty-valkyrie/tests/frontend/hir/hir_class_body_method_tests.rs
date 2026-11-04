//! 类/结构体声明体的方法入口合同。
//!
//! 声明体方法 = 该聚合 `Nominal` 上的固有实例方法；没有接收者参数的成员按静态成员处理，
//! MIR 仍发射普通 `Invoke`（与 `imply` 成员同一条调用形态）。
//! 泛型 owner 的声明体方法暂不支持：这是暂时的实现边界（实例闭合入口尚未覆盖成员），
//! 不是语言禁令，因此仍保持结构化拒绝并点名 owner 与成员。

use rusty_valkyrie::{
    ValkyrieCompiler,
    compiler::mir::validation::validate_semantic_module,
    mir::ssa::{MirFunction, MirLowerer, MirOperand, MirOperation},
    types::hir::{HirExpr, HirExprKind, HirFunction, HirModule, HirResolvedCall, HirStatementKind, ValkyrieType},
};

/// `two-sum` 形：单态类体方法由自由函数按接收者调用，整个链路必须完成 HIR → MIR。
const MONOMORPHIC_SOURCE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

namespace app;

class Solution {
    micro twoSum(self, offset: i32) -> i32 {
        return offset
    }
}

micro solve(solution: Solution, offset: i32) -> i32 {
    return solution.twoSum(offset)
}
"#;

fn function<'a>(module: &'a HirModule, name: &str) -> &'a HirFunction {
    module.functions.iter().find(|function| function.name.as_str() == name).unwrap_or_else(|| panic!("{name}"))
}

fn struct_method<'a>(module: &'a HirModule, owner: &str, name: &str) -> &'a HirFunction {
    module
        .structs
        .iter()
        .find(|item| item.name.as_str() == owner)
        .and_then(|item| item.methods.iter().find(|method| method.name.as_str() == name))
        .unwrap_or_else(|| panic!("{owner}.{name}"))
}

fn first_resolved_call(function: &HirFunction) -> Option<&HirResolvedCall> {
    fn visit(expression: &HirExpr) -> Option<&HirResolvedCall> {
        match &expression.kind {
            HirExprKind::Call { resolved: Some(resolved), .. } => Some(resolved),
            HirExprKind::Return(Some(value)) => visit(value),
            _ => None,
        }
    }
    for statement in &function.body.statements {
        let expression = match &statement.kind {
            HirStatementKind::Let { initializer: Some(expression), .. } => expression,
            HirStatementKind::Expr(expression) => expression,
            _ => continue,
        };
        if let Some(resolved) = visit(expression) {
            return Some(resolved);
        }
    }
    function.body.expr.as_ref().and_then(|expression| visit(expression))
}

fn call_targets(mir_function: &MirFunction) -> Vec<MirOperand> {
    mir_function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match &instruction.kind {
            MirOperation::Call { callee, .. } => Some(callee.clone()),
            _ => None,
        })
        .collect()
}

/// 单态类体方法：HIR 产出实例身份，接收者调用解析到该实例，MIR 发射普通 `Invoke`。
#[test]
fn monomorphic_class_body_method_is_callable_and_lowers_to_invoke() {
    let hir = ValkyrieCompiler::default().compile_source(MONOMORPHIC_SOURCE).expect("单态类体方法必须完成 HIR");
    let method = struct_method(&hir, "Solution", "twoSum");
    let instance = method.instance.expect("单态 owner 的声明体方法必须有 callable 实例");
    assert_eq!(method.declaration.is_some(), true, "声明体方法必须冻结声明身份");

    let resolved = first_resolved_call(function(&hir, "solve")).unwrap_or_else(|| {
        panic!(
            "`solution.twoSum(offset)` 必须解析成调用合同: failures={:?} body={:?}",
            hir.call_resolution_failures,
            function(&hir, "solve").body
        )
    });
    assert_eq!(resolved.instance, Some(instance), "接收者调用必须绑定声明体方法自己的实例: {resolved:?}");
    assert!(resolved.has_receiver, "首参 `self` 必须是接收者，不能被当成普通实参");

    let mir = MirLowerer::lower_module_semantic(&hir);
    assert!(validate_semantic_module(&mir).is_ok(), "声明体方法的 MIR 必须通过合同校验: {:?}", mir.diagnostics);
    let solve = mir.functions.iter().find(|function| function.display_symbol.contains("solve")).expect("solve MIR 函数");
    let targets = call_targets(solve);
    assert!(targets.contains(&MirOperand::Callable(instance)), "调用点必须发射到声明体方法的实例: {targets:?}");

    let lowered = mir
        .functions
        .iter()
        .find(|function| function.display_symbol.contains("Solution.twoSum"))
        .expect("声明体方法必须有自己的 MIR 实体");
    assert!(
        matches!(lowered.param_types.first(), Some(ValkyrieType::Nominal { .. })),
        "声明体方法的接收者必须是 owner `Nominal`，实际 {:?}",
        lowered.param_types.first()
    );
}

/// 泛型 owner 的声明体方法仍结构化拒绝：诊断必须点名 owner 与成员，并说明是暂时边界。
#[test]
fn generic_owner_class_body_method_is_rejected_with_a_structured_diagnostic() {
    let source = r#"
namespace app;

class Foo<T> {
    _value: T

    micro bar(self) -> i32 { return 0 }
}
"#;
    let error = ValkyrieCompiler::default().compile_source(source).expect_err("泛型 owner 的类体方法必须被拒绝");
    let message = error.to_string();
    assert!(message.contains("泛型"), "诊断必须点明这是泛型 owner 边界：{message}");
    assert!(message.contains("bar"), "诊断必须点名违规方法：{message}");
    assert!(message.contains("Foo"), "诊断必须点名声明体 owner：{message}");
    assert!(!message.contains("imply"), "诊断不得再声称固有方法只有 `imply` 一个载体：{message}");
}

/// `imply` 块方法不回退：仍然解析到 impl 声明表，声明体方法表保持为空。
#[test]
fn imply_block_methods_stay_available_and_remain_the_only_carrier() {
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
    assert!(hir.structs.iter().all(|item| item.methods.is_empty()), "`imply` 方法不得回流进声明体方法表");
    assert!(
        hir.impls.iter().any(|block| block.methods.iter().any(|method| method.name.as_str() == "bar")),
        "`imply` 块方法必须保留在 impl 声明表里"
    );
}

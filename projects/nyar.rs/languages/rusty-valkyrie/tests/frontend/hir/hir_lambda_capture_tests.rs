//! lambda 捕获事实与冻结环境身份（闭包 P1）的 HIR 合同。
//!
//! 锁定五条事实：
//! 1. 捕获列表记录写标记：lambda 体内写入外层绑定 `is_write = true`，只读为 `false`。
//! 2. 每个 lambda 的捕获环境是独立的 nominal 声明，不同 lambda 不共享声明/字段 id。
//! 3. 无捕获 lambda 不产生环境聚合，合成函数照常存在。
//! 4. 同一泛型 owner 的不同实例（`make<i32>` / `make<bool>`）环境类型实参不同，
//!    不共享同一 `Nominal` 类型。
//! 5. lambda 只被间接调用时，合成函数仍作为根边进入物化可达集，不会被 DCE 掉。

use rusty_valkyrie::{
    CompilerSourceGroup, ValkyrieCompiler,
    compiler::hir::{compute_hir_materialization_gate, lowering::DEFAULT_COMPILE_ARCH},
    types::hir::{HirBlock, HirCapture, HirExpr, HirExprKind, HirLambdaIdentity, HirModule, HirStatementKind, ValkyrieType},
};

#[derive(Debug, Clone)]
struct LambdaFacts {
    captures: Vec<HirCapture>,
    hoisted: Option<HirLambdaIdentity>,
}

fn compile(source: &str) -> HirModule {
    ValkyrieCompiler::default().compile_source(source).expect("lambda 捕获源码必须完成 HIR")
}

fn collect_lambdas(block: &HirBlock, out: &mut Vec<LambdaFacts>) {
    for statement in &block.statements {
        match &statement.kind {
            HirStatementKind::Let { initializer, .. } => {
                if let Some(initializer) = initializer {
                    collect_lambdas_in_expr(initializer, out);
                }
            }
            HirStatementKind::Expr(expr) => collect_lambdas_in_expr(expr, out),
        }
    }
    if let Some(expr) = &block.expr {
        collect_lambdas_in_expr(expr, out);
    }
}

fn collect_lambdas_in_expr(expr: &HirExpr, out: &mut Vec<LambdaFacts>) {
    match &expr.kind {
        HirExprKind::Lambda { captures, hoisted, .. } => {
            out.push(LambdaFacts { captures: captures.clone(), hoisted: hoisted.clone() });
        }
        HirExprKind::Call { callee, args, .. } => {
            collect_lambdas_in_expr(callee, out);
            for arg in args {
                collect_lambdas_in_expr(&arg.value, out);
            }
        }
        HirExprKind::Return(Some(value)) | HirExprKind::Assign { value, .. } | HirExprKind::Raise(value) => {
            collect_lambdas_in_expr(value, out)
        }
        HirExprKind::Block(block) => collect_lambdas(block, out),
        _ => {}
    }
}

fn lambdas_of_function(module: &HirModule, name: &str) -> Vec<LambdaFacts> {
    let function = module.functions.iter().find(|function| function.name.as_str() == name).expect(name);
    let mut facts = Vec::new();
    collect_lambdas(&function.body, &mut facts);
    facts
}

fn environment_type(identity: &HirLambdaIdentity) -> &ValkyrieType {
    identity.environment.as_ref().and_then(|environment| environment.ty.as_ref()).expect("捕获环境必须有冻结类型")
}

fn environment_arguments(environment: &rusty_valkyrie::types::hir::HirLambdaEnvironment) -> Vec<ValkyrieType> {
    match environment.ty.as_ref().expect("捕获环境必须有冻结类型") {
        ValkyrieType::Nominal { arguments, .. } => arguments.clone(),
        other => panic!("环境类型必须是 Nominal，实际 {other:?}"),
    }
}

const WRITE_CAPTURE_SOURCE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

micro apply(visitor: micro(i32) -> i32) -> i32 {
    return visitor(1)
}

micro run() -> i32 {
    let mut total: i32 = 0
    apply(micro(item: i32) -> i32 {
        total = item
        return total
    })
    return total
}
"#;

/// 写标记必须进入捕获事实：P3 可变捕获 cell 依赖它，丢失就是静默算错。
#[test]
fn lambda_captures_record_write_flag() {
    let facts = lambdas_of_function(&compile(WRITE_CAPTURE_SOURCE), "run");
    assert_eq!(facts.len(), 1, "run 体内只有一个 lambda");
    let facts = &facts[0];
    assert_eq!(facts.captures.len(), 1, "只捕获 total：{:?}", facts.captures);
    let capture = &facts.captures[0];
    assert_eq!(capture.identifier.name.as_str(), "total");
    assert!(capture.is_write, "lambda 体内写入 total，捕获必须标记 is_write");
    assert!(capture.is_mutable, "外层 total 是 mut 绑定");

    let identity = facts.hoisted.as_ref().expect("lambda 必须被提升");
    let environment = identity.environment.as_ref().expect("有捕获就必须合成环境聚合");
    assert_eq!(environment.field_ids.len(), 1, "环境字段 id 与捕获列表一一对应");
    let field = match environment_type(identity) {
        ValkyrieType::Nominal { declaration, .. } => *declaration,
        other => panic!("环境类型必须是 Nominal，实际 {other:?}"),
    };
    assert_eq!(environment.ty.as_ref().and_then(|ty| match ty {
        ValkyrieType::Nominal { declaration, .. } => Some(*declaration),
        _ => None,
    }), Some(field));
}

const READ_CAPTURE_SOURCE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

micro apply(visitor: micro(i32) -> i32) -> i32 {
    return visitor(1)
}

micro run() -> i32 {
    let total: i32 = 7
    apply(micro(item: i32) -> i32 {
        return total
    })
    return total
}
"#;

/// 只读捕获不得被误标为写入。
#[test]
fn lambda_readonly_capture_is_not_write() {
    let facts = lambdas_of_function(&compile(READ_CAPTURE_SOURCE), "run");
    let facts = &facts[0];
    assert_eq!(facts.captures.len(), 1);
    let capture = &facts.captures[0];
    assert_eq!(capture.identifier.name.as_str(), "total");
    assert!(!capture.is_write, "只读捕获 is_write 必须为 false");
    assert!(!capture.is_mutable, "外层 total 不是 mut 绑定");
}

const TWO_CAPTURE_SOURCE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

micro apply(visitor: micro(i32) -> i32) -> i32 {
    return visitor(1)
}

micro run() -> i32 {
    let a: i32 = 1
    let b: i32 = 2
    apply(micro(item: i32) -> i32 {
        return a
    })
    apply(micro(item: i32) -> i32 {
        return b
    })
    return a
}
"#;

/// 每个 lambda 的环境声明唯一：不得按形状复用同一 nominal/字段 id。
#[test]
fn lambda_environment_declaration_is_unique_per_lambda() {
    let facts = lambdas_of_function(&compile(TWO_CAPTURE_SOURCE), "run");
    assert_eq!(facts.len(), 2, "两个独立 lambda");
    let first = facts[0].hoisted.as_ref().and_then(|identity| identity.environment.as_ref()).expect("第一个 lambda 环境");
    let second = facts[1].hoisted.as_ref().and_then(|identity| identity.environment.as_ref()).expect("第二个 lambda 环境");
    assert_ne!(first.struct_name, second.struct_name, "每个 lambda 必须合成独立环境声明");
    assert_ne!(first.field_ids, second.field_ids, "不同环境声明的字段 id 不得复用");
    let first_ty = environment_type(facts[0].hoisted.as_ref().expect("第一个 lambda 必须提升"));
    let second_ty = environment_type(facts[1].hoisted.as_ref().expect("第二个 lambda 必须提升"));
    assert_ne!(first_ty, second_ty, "两个 lambda 的环境类型不得共享");
}

const CAPTURE_FREE_SOURCE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

micro apply(visitor: micro(i32) -> i32) -> i32 {
    return visitor(1)
}

micro run() -> i32 {
    return apply(micro(item: i32) -> i32 {
        return item
    })
}
"#;

/// 无捕获 lambda 不产生环境聚合，但合成函数身份照常冻结。
#[test]
fn capture_free_lambda_has_no_environment() {
    let module = compile(CAPTURE_FREE_SOURCE);
    let facts = lambdas_of_function(&module, "run");
    assert_eq!(facts.len(), 1, "run 体内只有一个 lambda");
    let identity = facts[0].hoisted.as_ref().expect("lambda 必须被提升");
    assert!(identity.environment.is_none(), "无捕获不得生成环境聚合");
    assert!(identity.declaration.is_some(), "合成函数必须进入声明表");
    assert!(identity.instance.is_some(), "合成函数必须冻结实例身份");
    assert!(
        !module.structs.iter().any(|structure| structure.name.as_str().starts_with("__lambda_env_")),
        "无捕获 lambda 不得留下环境结构"
    );
}

const GENERIC_OWNER_SOURCE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::bool")]
structure bool { }

micro make<T>(seed: T) -> micro() -> T {
    return micro() -> T {
        return seed
    }
}

micro run() -> i32 {
    let f: micro() -> i32 = make(3)
    let g: micro() -> bool = make(true)
    let a: i32 = f()
    let b: bool = g()
    return a
}
"#;

/// 同一泛型 owner 的不同实例环境类型实参必须不同：冻结的是声明身份，
/// 实例化代入的是类型实参，不能共享同一份 `Nominal`。
#[test]
fn generic_owner_instances_do_not_share_environment_type() {
    let module = compile(GENERIC_OWNER_SOURCE);
    let instances: Vec<_> = module
        .instantiated_callables
        .iter()
        .filter(|callable| callable.function.name.as_str() == "make")
        .collect();
    assert_eq!(instances.len(), 2, "make 必须为 i32/bool 各铸一个实例");

    let mut environments = Vec::new();
    for instance in &instances {
        let mut facts = Vec::new();
        collect_lambdas(&instance.function.body, &mut facts);
        assert_eq!(facts.len(), 1, "make 实例体内有一个 lambda");
        environments.push(facts[0].hoisted.as_ref().and_then(|identity| identity.environment.clone()).expect("泛型 owner lambda 必须有环境"));
    }

    let first = environment_arguments(&environments[0]);
    let second = environment_arguments(&environments[1]);
    assert_eq!(first.len(), 1, "owner 泛型只有一个实参");
    assert_eq!(second.len(), 1, "owner 泛型只有一个实参");
    assert_ne!(first, second, "不同实例的环境类型实参必须不同：{first:?} vs {second:?}");
    assert_eq!(
        environments[0].ty.as_ref().and_then(|ty| match ty {
            ValkyrieType::Nominal { declaration, .. } => Some(*declaration),
            _ => None,
        }),
        environments[1].ty.as_ref().and_then(|ty| match ty {
            ValkyrieType::Nominal { declaration, .. } => Some(*declaration),
            _ => None,
        }),
        "冻结的环境声明身份必须唯一，只允许类型实参不同"
    );
}

const INDIRECT_LAMBDA_SOURCE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

micro apply(visitor: micro(i32) -> i32) -> i32 {
    return visitor(1)
}

[export] micro run() -> i32 {
    return apply(micro(item: i32) -> i32 {
        return item
    })
}
"#;

/// lambda 只被间接调用：创建点必须作为根边把合成函数带进可达集，否则运行期
/// 间接调用找不到目标。DCE 不得裁掉它。
#[test]
fn indirectly_called_lambda_synthetic_function_is_reachable() {
    let groups = vec![CompilerSourceGroup::single_source("app", "app", INDIRECT_LAMBDA_SOURCE).as_entry()];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, DEFAULT_COMPILE_ARCH).expect("lambda 源码必须完成 HIR 物化");
    let gate = compute_hir_materialization_gate(&modules, &[]).expect("门计算必须结构化成功");

    let run = modules
        .iter()
        .flat_map(|module| module.functions.iter())
        .find(|function| function.name.as_str() == "run")
        .expect("run 声明");
    assert!(gate.reachable_declarations().contains(&run.declaration.expect("run 声明身份")), "导出入口必须可达");

    let lambda = modules
        .iter()
        .flat_map(|module| module.functions.iter())
        .find(|function| function.name.as_str().starts_with("__lambda_"))
        .expect("合成 lambda 函数");
    let lambda_declaration = lambda.declaration.expect("合成 lambda 必须有冻结声明身份");
    assert!(
        gate.reachable_declarations().contains(&lambda_declaration),
        "lambda 创建点必须把合成函数作为根边带入可达集，实际可达集：{:?}",
        gate.reachable_declarations()
    );
}

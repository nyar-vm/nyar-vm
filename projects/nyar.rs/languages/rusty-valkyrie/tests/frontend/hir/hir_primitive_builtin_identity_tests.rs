//! 消费者包对 `std::primitive` 内建标量的身份解析：注解必须折叠回内建类型身份。
//!
//! `[primitive("std::primitive::f64")] structure f64 { }` 是宿主绑定声明，
//! 不得在注解位置产生第二个名义身份；注解、表达式与 `using` 三条路径必须得到
//! 同一个内建 `ValkyrieType`。

use rusty_valkyrie::{CompilerSourceGroup, ValkyrieCompiler, types::hir::ValkyrieType};

const CORE: &str = r#"
namespace std.primitive;

[primitive("std::primitive::i32")]
structure i32 { }

[primitive("std::primitive::f64")]
structure f64 { }

[primitive("std::primitive::bool")]
structure bool { }
"#;

const APP: &str = r#"
namespace demo.app;

using std.primitive::{f64};

micro one() -> f64 {
    return 1.0
}

micro probe_let() -> f64 {
    let value: f64 = 1.0
    return value
}

[main]
micro main() -> f64 {
    return one()
}
"#;

fn app_module() -> rusty_valkyrie::types::hir::HirModule {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "std.primitive", CORE),
        CompilerSourceGroup::single_source("app", "demo.app", APP).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, "native").expect("消费者包必须完成 HIR 解析");
    modules.into_iter().last().expect("消费者包模块")
}

const ANNOTATION_ONLY: &str = r#"
namespace demo.ann;

using std.primitive::{f64};

micro passthrough(value: f64) -> f64 {
    return value
}

micro chain(value: f64) -> f64 {
    return passthrough(value)
}

micro seed() -> f64 {
    return seed()
}

[main]
micro main() -> f64 {
    return chain(seed())
}
"#;

#[test]
fn annotation_only_module_reveals_identity_shape() {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "std.primitive", CORE),
        CompilerSourceGroup::single_source("app", "demo.ann", ANNOTATION_ONLY).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, "native");
    match modules {
        Ok(modules) => {
            let module = modules.into_iter().last().expect("消费者包模块");
            let passthrough = module.functions.iter().find(|function| function.name.as_str() == "passthrough").expect("`passthrough` 声明");
            assert_eq!(passthrough.return_type, ValkyrieType::Float64, "注解身份: {:?}", passthrough.return_type);
        }
        Err(error) => panic!("消费者包必须完成 HIR 解析: {error:?}"),
    }
}

#[test]
fn consumer_primitive_annotation_folds_to_builtin_identity() {
    let groups = vec![
        CompilerSourceGroup::single_source("core", "std.primitive", CORE),
        CompilerSourceGroup::single_source("app", "demo.app", APP).as_entry().with_direct_dependencies(vec!["core".into()]),
    ];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, "native");
    match modules {
        Ok(modules) => {
            let module = modules.into_iter().last().expect("消费者包模块");
            let one = module.functions.iter().find(|function| function.name.as_str() == "one").expect("`one` 声明");
            assert_eq!(one.return_type, ValkyrieType::Float64, "注解 `f64` 必须解析到内建 `Float64`，不得是 `Nominal`");
        }
        Err(error) => panic!("消费者包必须完成 HIR 解析: {error:?}"),
    }
}

const SAME_MODULE: &str = r#"
namespace demo.same;

[primitive("std::primitive::f64")]
structure f64 { }

micro one() -> f64 {
    return 1.0
}

[main]
micro main() -> f64 {
    return one()
}
"#;

/// 对照：声明与使用同在一个模块时同样必须折叠到内建身份。
#[test]
fn same_module_primitive_annotation_folds_to_builtin_identity() {
    let groups = vec![CompilerSourceGroup::single_source("app", "demo.same", SAME_MODULE).as_entry()];
    let modules = ValkyrieCompiler::default().resolve_source_groups(&groups, "native").expect("同模块声明必须完成 HIR 解析");
    let module = modules.into_iter().last().expect("同模块");
    let one = module.functions.iter().find(|function| function.name.as_str() == "one").expect("`one` 声明");
    assert_eq!(one.return_type, ValkyrieType::Float64, "同模块注解 `f64` 也必须折叠到内建身份");
}

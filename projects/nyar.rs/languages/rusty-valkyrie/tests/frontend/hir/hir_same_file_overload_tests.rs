//! 同文件同名原顶层 `micro` 组：声明身份按「文件 + 名字」折叠，但需求事实必须并集保留。
//!
//! 合同（保守上界）：同一文件内同名同 arity、只差参数类型的顶层 `micro` 是合法重载组。
//! 闭包按声明身份去重**不得**丢掉组内任一重载签名/体内的引用事实；否则只被另一个重载
//! 引用的类型聚合会被裁出闭包，该重载体内的运算符候选随之消失，形成「同名候选无匹配」
//! 的伪重载失败。拆分文件只是让身份不同、掩盖了缺陷，不是正确修法。

use rusty_valkyrie::{
    CompilerSourceFile, CompilerSourceGroup, ValkyrieCompiler, compiler::diagnostics::pending, hir::lowering::DEFAULT_COMPILE_ARCH,
};

const PRIMITIVES: &str = r#"
namespace std.primitive;

[primitive("std::primitive::bool")]
structure bool { }

[primitive("std::primitive::i64")]
structure i64 { }

imply i64 {
    infix `<`(self, rhs: Self): bool {
        return true
    }
}

[primitive("std::primitive::f64")]
structure f64 { }

imply f64 {
    infix `<`(self, rhs: Self): bool {
        return true
    }

    prefix `-`(self): Self {
        return self
    }
}
"#;

const FLOAT_ABS: &str = r#"
namespace std.math;

micro abs(value: f64): f64 {
    if value < 0.0 {
        return -value
    }

    return value
}
"#;

const INTEGER_ABS: &str = r#"
namespace std.math;

micro abs(value: i64): i64 {
    if value < 0 {
        return value
    }

    return value
}
"#;

const INTEGER_CONSUMER: &str = r#"
namespace app;

[export] micro probe(x: i64) -> i64 {
    return std::math::abs(x)
}
"#;

fn compile(files: Vec<(String, String)>, consumer: &str) -> Result<(), String> {
    pending::clear_pending_diagnostics();
    let std_group = CompilerSourceGroup::from_source_files(
        "std",
        "std",
        files.into_iter().map(|(path, source)| CompilerSourceFile { path, source }).collect(),
        Vec::new(),
    );
    let app_group = CompilerSourceGroup::single_source("app", "app", consumer).as_entry().with_direct_dependencies(vec!["std".into()]);
    let result = ValkyrieCompiler::default().resolve_source_groups(&[std_group, app_group], DEFAULT_COMPILE_ARCH);
    let collected = pending::take_pending_diagnostics();
    match result {
        Ok(_) if collected.total() == 0 => Ok(()),
        Ok(_) => Err(format!("编译成功但诊断汇非空: {collected:?}")),
        Err(error) => Err(format!("{error}; diagnostics: {collected:?}")),
    }
}

fn split_math_group() -> Vec<(String, String)> {
    vec![
        ("source/primitive/all.v".to_string(), PRIMITIVES.to_string()),
        ("source/math/float.v".to_string(), FLOAT_ABS.to_string()),
        ("source/math/integer.v".to_string(), INTEGER_ABS.to_string()),
    ]
}

fn same_file_math_group() -> Vec<(String, String)> {
    vec![
        ("source/primitive/all.v".to_string(), PRIMITIVES.to_string()),
        ("source/math/basic.v".to_string(), format!("{FLOAT_ABS}\n{INTEGER_ABS}")),
    ]
}

// ---------- 正例：同文件同名重载组 ----------

#[test]
fn same_file_same_name_micros_keep_all_overload_demands() {
    compile(same_file_math_group(), INTEGER_CONSUMER).expect("同文件同名重载组必须完成调用解析");
}

#[test]
fn split_file_same_name_micros_keep_all_overload_demands() {
    compile(split_math_group(), INTEGER_CONSUMER).expect("分文件同名重载组必须完成调用解析");
}

// ---------- 反例：同名但无匹配实参仍结构化失败 ----------

#[test]
fn same_file_overload_group_still_rejects_unknown_argument_type() {
    let consumer = r#"
namespace app;

[export] micro probe(flag: bool) -> bool {
    let value: i64 = std::math::abs(flag)
    return value == value
}
"#;
    let failure = compile(same_file_math_group(), consumer).expect_err("实参类型不属于任何重载时必须失败");
    assert!(failure.contains("valkyrie.hir.unresolved_call"), "必须是结构化未解析调用诊断: {failure}");
    assert!(failure.contains("no-viable-overload"), "必须给出同名候选无匹配分类: {failure}");
}

//! 生成单元标量载体的两条结构化路径：std 可见时只导入，封闭单元用独立文件。
//!
//! 载体形态由生成组的显式依赖在构建源码之前判定一次（`scalar_bootstrap_for`），
//! 生成文本与附加文件随后一并产出；不再先造载体再按字符串前缀裁剪。

use nyar_types::compiler_input::CompilerSourceGroup;
use oak_awsl::AwslParser;
use rusty_valkyrie::asgard::{
    awsl::{LoweredComponent, LoweringOptions, lower_component},
    codegen::{ScalarBootstrap, build_awsl_wasm_source},
    compile::scalar_bootstrap_for,
};

const FIXTURE: &str = r#"<widget demo>
<Column><Text>{label}</Text></Column>
</widget>
<script>
let mut label: utf8 = ""
</script>"#;

fn lowered() -> LoweredComponent {
    let root = AwslParser::parse_root(FIXTURE).expect("parse");
    lower_component(&root, "demo", "demo.awsl", &LoweringOptions::default()).expect("lower demo")
}

/// 正例：`std` 进入生成组可见依赖时，生成源码只 `using std.primitive;`，不重声明标量。
#[test]
fn std_visible_generated_unit_imports_primitive_without_redeclaring() {
    let generated = build_awsl_wasm_source(&[lowered()], ScalarBootstrap::StdVisible).expect("生成 V 源码");
    assert!(generated.source.contains("using std.primitive;"), "std 可见时生成源码必须显式导入标量声明:\n{}", generated.source);
    assert!(!generated.source.contains("[primitive("), "std 可见时生成源码不得重声明标量载体:\n{}", generated.source);
    assert!(generated.extra_sources.is_empty(), "std 可见时生成组不得附带标量载体文件");
}

/// 反例：封闭单元（无 std）时载体落在独立文件，生成代码文件自身不含 `std.primitive` 命名空间。
#[test]
fn closed_unit_carries_scalar_bootstrap_in_separate_file() {
    let generated = build_awsl_wasm_source(&[lowered()], ScalarBootstrap::ClosedUnit).expect("生成 V 源码");
    assert!(!generated.source.contains("namespace std.primitive;"), "封闭单元的生成代码文件不得自带标量命名空间:\n{}", generated.source);
    assert!(!generated.source.contains("[primitive("), "封闭单元的生成代码文件不得自带标量载体:\n{}", generated.source);
    assert_eq!(generated.extra_sources.len(), 1, "封闭单元必须随组提交唯一标量载体文件");
    let scalars = &generated.extra_sources[0];
    assert_eq!(scalars.path, "asgard/generated.scalars.v", "标量载体必须有独立文件身份");
    assert!(scalars.source.contains("namespace std.primitive;"), "载体文件必须占据身份的规范命名空间:\n{}", scalars.source);
    assert!(
        scalars.source.contains(r#"[primitive("std::primitive::i32")]"#),
        "载体文件必须声明 `i32` 的 primitive 身份:\n{}",
        scalars.source
    );
    assert!(!scalars.source.contains("structure bool"), "`bool` 是编码型内建，载体不得重声明");
}

/// 形态判定与依赖挂接同一事实：只有 `std` 组对应的依赖键进入生成组时才判为可见。
#[test]
fn bootstrap_shape_follows_std_in_generated_dependencies() {
    let std_group = CompilerSourceGroup::single_source("key-std", "std", "namespace! std;");
    let app_group = CompilerSourceGroup::single_source("key-app", "app", "");
    let groups = [app_group, std_group];
    assert_eq!(scalar_bootstrap_for(&groups, &[]), ScalarBootstrap::ClosedUnit, "没有显式依赖时生成组是封闭单元");
    assert_eq!(
        scalar_bootstrap_for(&groups, &["key-app".to_string()]),
        ScalarBootstrap::ClosedUnit,
        "非 std 依赖不得让标量声明进入可见面"
    );
    assert_eq!(
        scalar_bootstrap_for(&groups, &["key-std".to_string()]),
        ScalarBootstrap::StdVisible,
        "std 显式依赖必须判为可见"
    );
}

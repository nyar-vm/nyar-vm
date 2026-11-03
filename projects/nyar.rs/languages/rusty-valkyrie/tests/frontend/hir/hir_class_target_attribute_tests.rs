//! 类级目标类型映射属性的 HIR 保留与形态合同测试。

use rusty_valkyrie::{
    HirError,
    compiler::{
        emitter::backend_contract::interop::BackendContractError,
        hir::ValkyrieCompiler,
        types::hir::{HirAttribute, HirExprKind, HirLiteral, HirStringSegment},
    },
};

const PRIORITY_QUEUE: &str = r#"
[clr("System.Collections", "System.Collections.Generic.PriorityQueue`2")]
[jvm("java.util.PriorityQueue")]
class PriorityQueue<T> {
}
"#;

fn string_argument(attribute: &HirAttribute, index: usize) -> String {
    let HirExprKind::Literal(HirLiteral::String(literal)) = &attribute.arguments[index].value.kind
    else {
        panic!("expected string literal argument {index}");
    };
    literal
        .segments
        .iter()
        .map(|segment| match segment {
            HirStringSegment::Text(text) => text.as_str(),
            _ => panic!("expected plain text segment in string literal argument"),
        })
        .collect()
}

#[test]
fn class_target_attributes_are_retained_on_the_hir_struct() {
    let module = ValkyrieCompiler::default().compile_source(PRIORITY_QUEUE).expect("带目标映射属性的 class 必须完成 HIR lowering");
    let queue = module.structs.iter().find(|item| item.name.as_str() == "PriorityQueue").expect("PriorityQueue 声明");
    assert_eq!(queue.annotations.len(), 2);

    let clr = &queue.annotations[0];
    assert_eq!(clr.name.parts()[0].as_str(), "clr");
    assert_eq!(clr.arguments.len(), 2);
    assert_eq!(string_argument(clr, 0), "System.Collections");
    assert_eq!(string_argument(clr, 1), "System.Collections.Generic.PriorityQueue`2");

    let jvm = &queue.annotations[1];
    assert_eq!(jvm.name.parts()[0].as_str(), "jvm");
    assert_eq!(jvm.arguments.len(), 1);
    assert_eq!(string_argument(jvm, 0), "java.util.PriorityQueue");
}

#[test]
fn unrelated_class_attributes_are_retained_without_shape_validation() {
    let module = ValkyrieCompiler::default()
        .compile_source("[serializable]\n[my_custom_attribute]\nclass Plain {\n}\n")
        .expect("非目标映射的类级属性必须原样保留且不触发形态校验");
    let plain = module.structs.iter().find(|item| item.name.as_str() == "Plain").expect("Plain 声明");
    let names = plain.annotations.iter().map(|attribute| attribute.name.parts()[0].as_str()).collect::<Vec<_>>();
    assert_eq!(names, ["serializable", "my_custom_attribute"]);
}

#[test]
fn jvm_type_attribute_with_extra_segment_is_rejected() {
    let error = ValkyrieCompiler::default()
        .compile_source("[jvm(\"java.util.PriorityQueue\", \"add\")]\nclass Wrong {\n}\n")
        .expect_err("类级 `[jvm]` 多段必须 fail-closed");
    match error {
        HirError::BackendContractRejected(BackendContractError::MalformedTypeTargetAttribute { attribute, actual, .. }) => {
            assert_eq!(attribute, "jvm");
            assert_eq!(actual, 2);
        }
        other => panic!("期望 MalformedTypeTargetAttribute，got {other:?}"),
    }
}

#[test]
fn clr_type_attribute_with_single_segment_is_rejected() {
    let error = ValkyrieCompiler::default()
        .compile_source("[clr(\"System.Collections\")]\nclass Wrong {\n}\n")
        .expect_err("类级 `[clr]` 少段必须 fail-closed");
    match error {
        HirError::BackendContractRejected(BackendContractError::MalformedTypeTargetAttribute { attribute, actual, .. }) => {
            assert_eq!(attribute, "clr");
            assert_eq!(actual, 1);
        }
        other => panic!("期望 MalformedTypeTargetAttribute，got {other:?}"),
    }
}

#[test]
fn empty_type_target_segment_is_rejected() {
    let error =
        ValkyrieCompiler::default().compile_source("[jvm(\"\")]\nclass Wrong {\n}\n").expect_err("类级目标映射属性空串必须 fail-closed");
    match error {
        HirError::BackendContractRejected(BackendContractError::MalformedTypeTargetAttribute { attribute, actual, .. }) => {
            assert_eq!(attribute, "jvm");
            assert_eq!(actual, 1);
        }
        other => panic!("期望 MalformedTypeTargetAttribute，got {other:?}"),
    }
}

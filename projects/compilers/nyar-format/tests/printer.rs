//! 平台 printer 注册表测试（VON / 语言中立错误路径）。

use oak_von::language::value::{VonField, VonObject};

use nyar_format::{
    formatter::{FormatError, FormatOptions, PrintStyle},
    printer::{print_document, print_von},
};
use oak_von::language::value::VonValue;

#[test]
fn von_print_via_registry() {
    let value = VonValue::Object(VonObject { fields: vec![VonField { name: "x".into(), value: VonValue::Number(1.0) }] });
    let out = print_von(&value, PrintStyle::Compact, &FormatOptions::default()).unwrap();
    assert_eq!(out, "{x=1}");
    let pretty = print_von(&value, PrintStyle::Indented, &FormatOptions::default()).unwrap();
    assert!(pretty.contains('x'));
}

#[test]
fn wrong_document_type() {
    let value = VonValue::Null;
    let err = print_document("wat", &value, PrintStyle::Indented, &FormatOptions::default()).unwrap_err();
    assert!(matches!(err, FormatError::WrongDocument { .. }));
}

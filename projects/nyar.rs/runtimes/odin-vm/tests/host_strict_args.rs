//! 第 2b 层 C：存量宿主符号缺参严格化。
//!
//! 合同：任何有固定元数的宿主符号缺参时一律 `StackUnderflow`，不得用默认值
//! （`0`/`false`/`Null`/空串）继续执行。`print` 保持变参语义（唯一例外）。
//!
//! 本文件同时充当改动前后的行为对照：正例证明补全参数后执行结果不受影响。

use odin_vm::{
    ObjectHeap, Value,
    error::OdinRuntimeError,
    host::{HostOp, execute_host_op},
};

fn assert_underflow(result: Result<Value, OdinRuntimeError>, context: &str) {
    match result {
        Ok(value) => panic!("{context}: 缺参必须失败，实际成功返回 {value:?}"),
        Err(OdinRuntimeError::StackUnderflow) => {}
        Err(other) => panic!("{context}: 期望 StackUnderflow，实际 {other:?}"),
    }
}

fn missing(op: HostOp, provided: usize, context: &str) {
    // 用 `I64` 载体填充已提供的槽位：载体正确，只有元数不足。
    let args = vec![Value::I64(1); provided];
    let mut heap = ObjectHeap::new();
    assert_underflow(execute_host_op(op, &args, &mut heap), context);
}

#[test]
fn i64_binary_ops_require_both_arguments() {
    let ops = [
        HostOp::I64Add,
        HostOp::I64Sub,
        HostOp::I64Mul,
        HostOp::I64Div,
        HostOp::I64Rem,
        HostOp::I64DivU,
        HostOp::I64RemU,
        HostOp::I64Eq,
        HostOp::I64Ne,
        HostOp::I64Lt,
        HostOp::I64Le,
        HostOp::I64Gt,
        HostOp::I64Ge,
        HostOp::I64LtU,
        HostOp::I64And,
        HostOp::I64Or,
        HostOp::I64Xor,
        HostOp::I64Shl,
        HostOp::I64Shr,
        HostOp::I64ShrU,
    ];
    for op in ops {
        missing(op, 1, &format!("{op:?} 单参"));
        missing(op, 0, &format!("{op:?} 零参"));
    }
}

#[test]
fn i64_unary_ops_require_one_argument() {
    missing(HostOp::I64Neg, 0, "I64Neg 零参");
    missing(HostOp::I32ToI64, 0, "I32ToI64 零参");
}

#[test]
fn i32_ops_require_both_arguments() {
    for op in [HostOp::I32Div, HostOp::I32DivU, HostOp::I32RemU] {
        missing(op, 1, &format!("{op:?} 单参"));
        missing(op, 0, &format!("{op:?} 零参"));
    }
    missing(HostOp::I32ToI64, 0, "I32ToI64 零参");
}

#[test]
fn bool_ops_require_arguments() {
    missing(HostOp::BoolNot, 0, "BoolNot 零参");
    for op in [HostOp::BoolAnd, HostOp::BoolOr] {
        let mut heap = ObjectHeap::new();
        assert_underflow(execute_host_op(op, &[Value::Bool(true)], &mut heap), &format!("{op:?} 单参"));
        let mut heap = ObjectHeap::new();
        assert_underflow(execute_host_op(op, &[], &mut heap), &format!("{op:?} 零参"));
    }
}

#[test]
fn array_ops_require_arguments() {
    missing(HostOp::ArrayNew, 0, "ArrayNew 零参");
    missing(HostOp::ArrayLength, 0, "ArrayLength 零参");

    let mut heap = ObjectHeap::new();
    let array = execute_host_op(HostOp::ArrayNew, &[Value::I32(3)], &mut heap).expect("构造数组");
    assert_underflow(execute_host_op(HostOp::ArrayGet, &[array.clone()], &mut heap), "ArrayGet 缺下标");
    assert_underflow(execute_host_op(HostOp::ArrayGet, &[], &mut heap), "ArrayGet 零参");
    assert_underflow(execute_host_op(HostOp::ArraySet, &[array.clone()], &mut heap), "ArraySet 缺下标/值");
    assert_underflow(execute_host_op(HostOp::ArraySet, &[array.clone(), Value::I32(0)], &mut heap), "ArraySet 缺值");
    assert_underflow(execute_host_op(HostOp::ArraySet, &[], &mut heap), "ArraySet 零参");
}

#[test]
fn dom_ops_require_arguments() {
    missing(HostOp::DomCreateElement, 0, "DomCreateElement 零参");
    missing(HostOp::DomCreateText, 0, "DomCreateText 零参");

    let mut heap = ObjectHeap::new();
    let element = execute_host_op(HostOp::DomCreateElement, &[Value::String("div".to_string())], &mut heap).expect("创建元素");
    let text = execute_host_op(HostOp::DomCreateText, &[Value::String("hi".to_string())], &mut heap).expect("创建文本");
    assert_underflow(execute_host_op(HostOp::DomAppend, &[element.clone()], &mut heap), "DomAppend 缺子节点");
    assert_underflow(execute_host_op(HostOp::DomAppend, &[], &mut heap), "DomAppend 零参");
    assert_underflow(execute_host_op(HostOp::DomSetAttr, &[element.clone(), Value::String("id".to_string())], &mut heap), "DomSetAttr 缺值");
    assert_underflow(execute_host_op(HostOp::DomSetAttr, &[], &mut heap), "DomSetAttr 零参");
    assert_underflow(execute_host_op(HostOp::DomGetAttr, &[element.clone()], &mut heap), "DomGetAttr 缺属性名");
    assert_underflow(execute_host_op(HostOp::DomGetAttr, &[], &mut heap), "DomGetAttr 零参");
    assert_underflow(execute_host_op(HostOp::DomSetText, &[element], &mut heap), "DomSetText 缺文本");
    assert_underflow(execute_host_op(HostOp::DomSetText, &[], &mut heap), "DomSetText 零参");
    // 正例：文本节点本身是合法载体，补全参数后追加成功。
    let mut heap = ObjectHeap::new();
    let element = execute_host_op(HostOp::DomCreateElement, &[Value::String("div".to_string())], &mut heap).expect("创建元素");
    let text = execute_host_op(HostOp::DomCreateText, &[Value::String("hi".to_string())], &mut heap).expect("创建文本");
    assert!(execute_host_op(HostOp::DomAppend, &[element, text], &mut heap).is_ok());
}

#[test]
fn string_and_console_ops_require_arguments() {
    for provided in [0, 1] {
        let args = vec![Value::String("a".to_string()); provided];
        let mut heap = ObjectHeap::new();
        assert_underflow(execute_host_op(HostOp::StringConcat, &args, &mut heap), &format!("StringConcat {provided} 参"));
    }
    missing(HostOp::ConsoleLog, 0, "ConsoleLog 零参");
}

#[test]
fn strict_ops_still_execute_with_full_arguments() {
    let mut heap = ObjectHeap::new();
    assert_eq!(execute_host_op(HostOp::I64Add, &[Value::I64(2), Value::I64(3)], &mut heap).expect("i64_add"), Value::I64(5));
    assert_eq!(execute_host_op(HostOp::I32ToI64, &[Value::I32(7)], &mut heap).expect("i32_to_i64"), Value::I64(7));
    assert_eq!(execute_host_op(HostOp::I32Div, &[Value::I32(7), Value::I32(2)], &mut heap).expect("i32_div"), Value::I32(3));
    assert_eq!(execute_host_op(HostOp::BoolNot, &[Value::Bool(true)], &mut heap).expect("bool_not"), Value::Bool(false));
    assert_eq!(execute_host_op(HostOp::BoolAnd, &[Value::Bool(true), Value::Bool(false)], &mut heap).expect("bool_and"), Value::Bool(false));
    assert_eq!(
        execute_host_op(HostOp::StringConcat, &[Value::String("a".to_string()), Value::String("b".to_string())], &mut heap)
            .expect("string_concat"),
        Value::String("ab".to_string())
    );

    let array = execute_host_op(HostOp::ArrayNew, &[Value::I32(2)], &mut heap).expect("array.new");
    assert_eq!(execute_host_op(HostOp::ArrayLength, &[array.clone()], &mut heap).expect("array.length"), Value::I32(2));
    execute_host_op(HostOp::ArraySet, &[array.clone(), Value::I32(1), Value::I32(9)], &mut heap).expect("array.set");
    assert_eq!(execute_host_op(HostOp::ArrayGet, &[array, Value::I32(1)], &mut heap).expect("array.get"), Value::I32(9));
}

#[test]
fn print_remains_variadic() {
    // `print` 是唯一保留的变参入口：零参打印空行并返回 Null，不属于「缺参补默认值」。
    let mut heap = ObjectHeap::new();
    assert_eq!(execute_host_op(HostOp::Print, &[], &mut heap).expect("print 零参"), Value::Null);
}

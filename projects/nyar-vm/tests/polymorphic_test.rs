use nyar_vm::bytecode::compiler::NyarBackend;
use nyar_vm::vm::core::NyarVM;
use chomsky_extract::IKunTree;
use nyar_vm::bytecode::opcode::Opcode;
use nyar_vm::bytecode::instruction::Instruction;

fn run_tree(tree: &IKunTree) -> nyar_vm::vm::value::Value {
    let mut backend = NyarBackend::new();
    let mut code = backend.lower_tree(tree).unwrap();
    if code.last() != Some(&(Opcode::Return as u8)) {
        backend.emit(Instruction::Return, &mut code);
    }
    let module = backend.finish_with_code(code);
    let mut vm = NyarVM::new();
    let module_idx = vm.load_module(module);
    vm.execute(module_idx, 0).expect("Execution failed")
}

#[test]
fn test_int_overflow_to_bigint() {
    // i64::MAX + 1
    let tree = IKunTree::Extension(
        "add".to_string(),
        vec![
            IKunTree::Constant(i64::MAX),
            IKunTree::Constant(1),
        ],
    );
    let result = run_tree(&tree);
    assert!(result.is_bigint());
    assert_eq!(result.try_as_bigint().unwrap().0.to_string(), "9223372036854775808");
}

#[test]
fn test_mixed_int_float() {
    // 10 + 20.5
    let tree = IKunTree::Extension(
        "add".to_string(),
        vec![
            IKunTree::Constant(10),
            IKunTree::FloatConstant(20.5f64.to_bits()),
        ],
    );
    let result = run_tree(&tree);
    assert!(result.is_f64());
    assert_eq!(result.as_f64(), 30.5);
}

#[test]
fn test_string_concat() {
    // "hello" + " world"
    let tree = IKunTree::Extension(
        "add".to_string(),
        vec![
            IKunTree::StringConstant("hello".to_string()),
            IKunTree::StringConstant(" world".to_string()),
        ],
    );
    let result = run_tree(&tree);
    assert!(result.is_string());
    assert_eq!(result.try_as_str().unwrap(), "hello world");
}

#[test]
fn test_bigint_arithmetic() {
    // (i64::MAX + 1) * 2
    let bigint_tree = IKunTree::Extension(
        "add".to_string(),
        vec![
            IKunTree::Constant(i64::MAX),
            IKunTree::Constant(1),
        ],
    );
    let tree = IKunTree::Extension(
        "mul".to_string(),
        vec![
            bigint_tree,
            IKunTree::Constant(2),
        ],
    );
    let result = run_tree(&tree);
    assert!(result.is_bigint());
    assert_eq!(result.try_as_bigint().unwrap().0.to_string(), "18446744073709551616");
}

#[test]
fn test_bigint_comparison() {
    // (i64::MAX + 1) > i64::MAX
    let bigint_tree = IKunTree::Extension(
        "add".to_string(),
        vec![
            IKunTree::Constant(i64::MAX),
            IKunTree::Constant(1),
        ],
    );
    let tree = IKunTree::Extension(
        "gt".to_string(),
        vec![
            bigint_tree,
            IKunTree::Constant(i64::MAX),
        ],
    );
    let result = run_tree(&tree);
    assert!(result.is_bool());
    assert!(result.as_bool());
}

#[test]
fn test_bigint_bitwise() {
    // (1 << 70) | 1
    let shl_tree = IKunTree::Extension(
        "bit_shl".to_string(),
        vec![
            IKunTree::Constant(1),
            IKunTree::Constant(70),
        ],
    );
    let tree = IKunTree::Extension(
        "bit_or".to_string(),
        vec![
            shl_tree,
            IKunTree::Constant(1),
        ],
    );
    let result = run_tree(&tree);
    assert!(result.is_bigint());
    // 2^70 + 1 = 1180591620717411303424 + 1 = 1180591620717411303425
    assert_eq!(result.try_as_bigint().unwrap().0.to_string(), "1180591620717411303425");
}

#[test]
fn test_unary_neg_bigint() {
    // -(i64::MAX + 1)
    let bigint_tree = IKunTree::Extension(
        "add".to_string(),
        vec![
            IKunTree::Constant(i64::MAX),
            IKunTree::Constant(1),
        ],
    );
    let tree = IKunTree::Extension(
        "neg".to_string(),
        vec![bigint_tree],
    );
    let result = run_tree(&tree);
    assert!(result.is_bigint());
    assert_eq!(result.try_as_bigint().unwrap().0.to_string(), "-9223372036854775808");
}

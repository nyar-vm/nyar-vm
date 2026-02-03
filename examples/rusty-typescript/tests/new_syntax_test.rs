use rusty_typescript::RustyTypescriptFrontend;
use nyar_types::NyarFrontend;
use nyar_vm::vm::interpreter::NyarVM;

#[test]
fn test_if_statement() {
    let frontend = RustyTypescriptFrontend::new();
    let source = "let x = 10; let y = 0; if (x > 5) { y = 1; } else { y = 2; } y;";

    let module = frontend.compile_to_nyar(source).expect("Compilation failed");
    let mut vm = NyarVM::new();
    vm.load_module(module.into());

    let result = vm.execute(0, 0).expect("Execution failed");
    assert_eq!(unsafe { result.as_int() }, 1);
}

#[test]
fn test_while_loop() {
    let frontend = RustyTypescriptFrontend::new();
    let source = "let i = 0; let sum = 0; while (i < 10) { sum = sum + i; i = i + 1; } sum;";

    let module = frontend.compile_to_nyar(source).expect("Compilation failed");
    let mut vm = NyarVM::new();
    vm.load_module(module.into());

    let result = vm.execute(0, 0).expect("Execution failed");
    assert_eq!(unsafe { result.as_int() }, 45);
}

#[test]
fn test_arrow_function() {
    let frontend = RustyTypescriptFrontend::new();
    let source = "let add = (a, b) => a + b; add(10, 20);";

    let module = frontend.compile_to_nyar(source).expect("Compilation failed");
    let mut vm = NyarVM::new();
    vm.load_module(module.into());

    let result = vm.execute(0, 0).expect("Execution failed");
    assert_eq!(unsafe { result.as_int() }, 30);
}

#[test]
fn test_array_literal() {
    let frontend = RustyTypescriptFrontend::new();
    let source = "let arr = [1, 2, 3]; arr[1];";

    let module = frontend.compile_to_nyar(source).expect("Compilation failed");
    let mut vm = NyarVM::new();
    vm.load_module(module.into());

    let result = vm.execute(0, 0).expect("Execution failed");
    assert_eq!(unsafe { result.as_int() }, 2);
}

#[test]
fn test_object_literal() {
    let frontend = RustyTypescriptFrontend::new();
    let source = "let obj = { x: 10, y: 20 }; obj.x + obj.y;";

    let module = frontend.compile_to_nyar(source).expect("Compilation failed");
    let mut vm = NyarVM::new();
    vm.load_module(module.into());

    let result = vm.execute(0, 0).expect("Execution failed");
    assert_eq!(unsafe { result.as_int() }, 30);
}

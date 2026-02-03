use chomsky_extract::IKunTree;
use nyar_vm::bytecode::compiler::NyarBackend;
use nyar_vm::vm::core::NyarVM;
use nyar_vm::vm::value::Value;

#[test]
fn test_logical_and_short_circuit() {
    let mut backend = NyarBackend::new();
    
    // true && true -> true
    let tree = IKunTree::Module(
        "test".to_string(),
        vec![IKunTree::Export(
            "main".to_string(),
            Box::new(IKunTree::Lambda(
                vec![],
                Box::new(IKunTree::Extension(
                    "and".to_string(),
                    vec![
                        IKunTree::Constant(1),
                        IKunTree::Constant(1),
                    ],
                )),
            )),
        )],
    );
    
    backend.lower_tree(&tree).unwrap();
    let artifact = backend.finish();
    let mut vm = NyarVM::new();
    vm.load_module(artifact);
    let res = vm.execute_symbol(&"test:main".into(), vec![]).unwrap();
    assert_eq!(res.as_int(), 1);

    // true && false -> false
    let mut backend = NyarBackend::new();
    let tree = IKunTree::Module(
        "test".to_string(),
        vec![IKunTree::Export(
            "main".to_string(),
            Box::new(IKunTree::Lambda(
                vec![],
                Box::new(IKunTree::Extension(
                    "and".to_string(),
                    vec![
                        IKunTree::Constant(1),
                        IKunTree::Constant(0),
                    ],
                )),
            )),
        )],
    );
    backend.lower_tree(&tree).unwrap();
    let artifact = backend.finish();
    let mut vm = NyarVM::new();
    vm.load_module(artifact);
    let res = vm.execute_symbol(&"test:main".into(), vec![]).unwrap();
    assert_eq!(res.as_int(), 0);

    // false && (panic) -> false (short-circuit)
    let mut backend = NyarBackend::new();
    let tree = IKunTree::Module(
        "test".to_string(),
        vec![IKunTree::Export(
            "main".to_string(),
            Box::new(IKunTree::Lambda(
                vec![],
                Box::new(IKunTree::Extension(
                    "and".to_string(),
                    vec![
                        IKunTree::Constant(0),
                        IKunTree::CrossLangCall {
                            language: "nyar".to_string(),
                            module_path: "std".to_string(),
                            function_name: "panic".to_string(),
                            arguments: vec![IKunTree::StringConstant("should not happen".to_string())],
                        },
                    ],
                )),
            )),
        )],
    );
    backend.lower_tree(&tree).unwrap();
    let artifact = backend.finish();
    let mut vm = NyarVM::new();
    vm.load_module(artifact);
    let res = vm.execute_symbol(&"test:main".into(), vec![]).unwrap();
    assert_eq!(res.as_int(), 0);
}

#[test]
fn test_logical_or_short_circuit() {
    let mut backend = NyarBackend::new();
    
    // false || false -> false
    let tree = IKunTree::Module(
        "test".to_string(),
        vec![IKunTree::Export(
            "main".to_string(),
            Box::new(IKunTree::Lambda(
                vec![],
                Box::new(IKunTree::Extension(
                    "or".to_string(),
                    vec![
                        IKunTree::Constant(0),
                        IKunTree::Constant(0),
                    ],
                )),
            )),
        )],
    );
    backend.lower_tree(&tree).unwrap();
    let artifact = backend.finish();
    let mut vm = NyarVM::new();
    vm.load_module(artifact);
    let res = vm.execute_symbol(&"test:main".into(), vec![]).unwrap();
    assert_eq!(res.as_int(), 0);

    // false || true -> true
    let mut backend = NyarBackend::new();
    let tree = IKunTree::Module(
        "test".to_string(),
        vec![IKunTree::Export(
            "main".to_string(),
            Box::new(IKunTree::Lambda(
                vec![],
                Box::new(IKunTree::Extension(
                    "or".to_string(),
                    vec![
                        IKunTree::Constant(0),
                        IKunTree::Constant(1),
                    ],
                )),
            )),
        )],
    );
    backend.lower_tree(&tree).unwrap();
    let artifact = backend.finish();
    let mut vm = NyarVM::new();
    vm.load_module(artifact);
    let res = vm.execute_symbol(&"test:main".into(), vec![]).unwrap();
    assert_eq!(res.as_int(), 1);

    // true || (panic) -> true (short-circuit)
    let mut backend = NyarBackend::new();
    let tree = IKunTree::Module(
        "test".to_string(),
        vec![IKunTree::Export(
            "main".to_string(),
            Box::new(IKunTree::Lambda(
                vec![],
                Box::new(IKunTree::Extension(
                    "or".to_string(),
                    vec![
                        IKunTree::Constant(1),
                        IKunTree::CrossLangCall {
                            language: "nyar".to_string(),
                            module_path: "std".to_string(),
                            function_name: "panic".to_string(),
                            arguments: vec![IKunTree::StringConstant("should not happen".to_string())],
                        },
                    ],
                )),
            )),
        )],
    );
    backend.lower_tree(&tree).unwrap();
    let artifact = backend.finish();
    let mut vm = NyarVM::new();
    vm.load_module(artifact);
    let res = vm.execute_symbol(&"test:main".into(), vec![]).unwrap();
    assert_eq!(res.as_int(), 1);
}

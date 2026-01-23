use mini_typescript::project::ProjectLoader;
use nyar_vm::vm::interpreter::NyarVM;
use std::path::Path;

#[test]
fn test_multi_file_project() {
    let project_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("multi-file-test");
    
    let mut loader = ProjectLoader::new(&project_dir);
    let modules = loader.load_project(&project_dir).expect("Failed to load project");
    
    assert!(modules.len() >= 2, "Should have loaded at least 2 modules");
    
    let mut vm = NyarVM::new();
    // Load all modules into VM
    for module in modules {
        vm.load_module(module).expect("Failed to load module into VM");
    }
    
    // Run the main module (last one loaded by ProjectLoader usually, or we can find it)
    // Actually ProjectLoader loads entry point first, but load_file_recursive pushes to modules vec.
    // So entry point is modules[0].
    
    vm.run_module(0).expect("Failed to run main module");
}

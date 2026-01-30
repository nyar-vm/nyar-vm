use crate::bytecode::compiler::NyarBackend;
use crate::vm::interpreter::NyarVM;
use nyar_types::{NyarError, NyarFrontend};
use std::fs;
use std::path::Path;

/// Nyar 驱动程序
#[derive(Default)]
pub struct NyarDriver;

impl NyarDriver {
    /// 创建一个新的驱动程序
    pub fn new() -> Self {
        Self
    }

    /// 运行源代码文件
    pub fn run_source<F: NyarFrontend>(&self, frontend: &F, path: &Path) -> Result<(), NyarError> {
        let source = fs::read_to_string(path).map_err(NyarError::from)?;
        let ast = frontend.parse(&source)?;
        let tree = frontend.lower(&ast)?;
        let mut backend = NyarBackend::new();
        backend.lower_tree(&tree)?;
        let module = backend.finish();
        let mut vm = NyarVM::new();
        let module_idx = vm.load_module(module);
        vm.execute(module_idx, 0).map(|_| ()).map_err(NyarError::from)
    }

    /// 运行源代码字符串
    pub fn run_code<F: NyarFrontend>(&self, frontend: &F, source: &str) -> Result<(), NyarError> {
        let ast = frontend.parse(source)?;
        let tree = frontend.lower(&ast)?;
        let mut backend = NyarBackend::new();
        backend.lower_tree(&tree)?;
        let module = backend.finish();
        let mut vm = NyarVM::new();
        let module_idx = vm.load_module(module);
        vm.execute(module_idx, 0).map(|_| ()).map_err(NyarError::from)
    }

    /// AOT 编译到原生可执行文件
    pub fn compile_to_native<F: NyarFrontend>(
        &self,
        frontend: &F,
        source_path: &Path,
        output_path: &Path,
    ) -> Result<(), NyarError> {
        let source = fs::read_to_string(source_path).map_err(NyarError::from)?;
        let ast = frontend.parse(&source)?;
        let tree = frontend.lower(&ast)?;
        
        // TODO: 使用 nyar-aot 进行原生代码生成
        println!("AOT: Compiling IKunTree to native at {:?}", output_path);
        println!("IKunTree name: {}", tree.name);
        
        Err(NyarError::Compile("AOT compilation to native backend is not yet fully integrated".to_string()))
    }

    /// AOT 编译到 WASM
    pub fn compile_to_wasm<F: NyarFrontend>(
        &self,
        frontend: &F,
        source_path: &Path,
        output_path: &Path,
    ) -> Result<(), NyarError> {
        let source = fs::read_to_string(source_path).map_err(NyarError::from)?;
        let ast = frontend.parse(&source)?;
        let tree = frontend.lower(&ast)?;
        
        // TODO: 使用 nyar-aot 进行 WASM 生成
        println!("AOT: Compiling IKunTree to WASM at {:?}", output_path);
        println!("IKunTree name: {}", tree.name);
        
        Err(NyarError::Compile("AOT compilation to WASM backend is not yet fully integrated".to_string()))
    }
}

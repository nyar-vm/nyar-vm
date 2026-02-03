use crate::bytecode::compiler::NyarBackend;
use crate::vm::core::NyarVM;
use nyar_types::{NyarError, NyarFrontend, QualifiedName};
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
        let main_name = QualifiedName::new(vec!["main".to_string()]);
        if vm.execute_symbol(&main_name, vec![]).is_ok() {
            Ok(())
        } else {
            vm.execute(module_idx, 0)
                .map(|_| ())
                .map_err(NyarError::from)
        }
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
        vm.execute(module_idx, 0)
            .map(|_| ())
            .map_err(NyarError::from)
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

        eprintln!("DEBUG: IKunTree: {:#?}", tree);

        let _aot: crate::aot::NyarAot<chomsky_uir::ConstraintAnalysis> =
            crate::aot::NyarAot::new();
        let backend = crate::aot::NativeBackend::new();

        // 这里的 tree 是 IKunTree，需要转换成 IKun 才能传给 aot.compile
        // 或者我们直接调用 backend.generate 如果不需要优化的话
        // 为了简单起见，我们先直接调用 backend.generate

        use chomsky_extract::Backend;
        let artifact = backend
            .generate(&tree)
            .map_err(|e| NyarError::Compile(format!("Backend error: {:?}", e)))?;

        match artifact {
            chomsky_extract::BackendArtifact::Binary(bytes) => {
                fs::write(output_path, bytes).map_err(NyarError::from)?;
                println!("AOT: Compiled to native at {:?}", output_path);
                Ok(())
            }
            _ => Err(NyarError::Compile("Unexpected artifact type".to_string())),
        }
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

        println!("AOT: Compiling IKunTree to WASM at {:?}", output_path);

        use chomsky::adapters::GaiaWasiAdapter;
        use chomsky_extract::Backend;
        let adapter = GaiaWasiAdapter;
        let artifact = adapter
            .generate(&tree)
            .map_err(|e| NyarError::Compile(format!("{:?}", e)))?;

        match artifact {
            chomsky_extract::BackendArtifact::Binary(bytes) => {
                fs::write(output_path, bytes).map_err(NyarError::from)?;
            }
            _ => return Err(NyarError::Compile("Expected binary artifact".to_string())),
        }

        Ok(())
    }

    /// 编译到 JVM .class 文件
    pub fn compile_to_jvm<F: NyarFrontend>(
        &self,
        frontend: &F,
        source_path: &Path,
        output_path: &Path,
    ) -> Result<(), NyarError> {
        use chomsky::adapters::GaiaJvmAdapter;
        use chomsky_extract::Backend;

        let source = fs::read_to_string(source_path).map_err(NyarError::from)?;
        let ast = frontend.parse(&source)?;
        let tree = frontend.lower(&ast)?;

        println!("JVM: Compiling IKunTree to JVM at {:?}", output_path);

        let adapter = GaiaJvmAdapter;
        let artifact = adapter
            .generate(&tree)
            .map_err(|e| NyarError::Compile(format!("{:?}", e)))?;

        match artifact {
            chomsky_extract::BackendArtifact::Binary(bytes) => {
                fs::write(output_path, bytes).map_err(NyarError::from)?;
            }
            _ => return Err(NyarError::Compile("Expected binary artifact".to_string())),
        }

        Ok(())
    }

    /// 编译到 CLR 程序集
    pub fn compile_to_clr<F: NyarFrontend>(
        &self,
        frontend: &F,
        source_path: &Path,
        output_path: &Path,
    ) -> Result<(), NyarError> {
        // TODO: 完善 CLR 适配器并在这里调用
        let source = fs::read_to_string(source_path).map_err(NyarError::from)?;
        let ast = frontend.parse(&source)?;
        let _tree = frontend.lower(&ast)?;

        println!("CLR: Compiling IKunTree to CLR at {:?}", output_path);
        Err(NyarError::Compile("CLR backend is not yet fully integrated".to_string()))
    }
}

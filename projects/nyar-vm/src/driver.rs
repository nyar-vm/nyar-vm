use crate::bytecode::compiler::NyarBackend;
use crate::vm::core::NyarVM;
use nyar_types::{NyarError, NyarFrontend, QualifiedName};
use oak_core::source::Source;
use oak_vfs::{Vfs, WritableVfs};
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

    /// 获取默认的 VFS
    pub fn default_vfs(&self) -> oak_vfs::DiskVfs {
        oak_vfs::DiskVfs::new()
    }

    /// 运行源代码文件
    pub fn run_source<F, V>(&self, frontend: &F, vfs: &V, uri: &str) -> Result<(), NyarError>
    where
        F: NyarFrontend,
        V: Vfs,
    {
        let source = vfs
            .get_source(uri)
            .ok_or_else(|| NyarError::Compile(format!("Source not found: {}", uri)))?;
        let content = source.get_text_from(0);
        let ast = frontend.parse(&content)?;
        let tree = frontend.lower(&ast, vfs)?;
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
    pub fn run_code<F, V>(&self, frontend: &F, vfs: &V, source: &str) -> Result<(), NyarError>
    where
        F: NyarFrontend,
        V: Vfs,
    {
        let ast = frontend.parse(source)?;
        let tree = frontend.lower(&ast, vfs)?;
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
    pub fn compile_to_native<F, V>(
        &self,
        frontend: &F,
        vfs: &V,
        source_uri: &str,
        output_uri: &str,
    ) -> Result<(), NyarError>
    where
        F: NyarFrontend,
        V: WritableVfs,
    {
        let source = vfs
            .get_source(source_uri)
            .ok_or_else(|| NyarError::Compile(format!("Source not found: {}", source_uri)))?;
        let content = source.get_text_from(0);
        let ast = frontend.parse(&content)?;
        let tree = frontend.lower(&ast, vfs)?;

        eprintln!("DEBUG: IKunTree: {:#?}", tree);

        let _aot: crate::aot::NyarAot<chomsky_uir::ConstraintAnalysis> =
            crate::aot::NyarAot::new();
        let backend = crate::aot::NativeBackend::new();

        use chomsky_extract::Backend;
        let artifact = backend
            .generate(&tree)
            .map_err(|e| NyarError::Compile(format!("Backend error: {:?}", e)))?;

        match artifact {
            chomsky_extract::BackendArtifact::Binary(bytes) => {
                let content = String::from_utf8_lossy(&bytes).to_string();
                vfs.write_file(output_uri, content.into());
                println!("AOT: Compiled to native at {}", output_uri);
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
        let vfs = self.default_vfs();
        let source = fs::read_to_string(source_path).map_err(NyarError::from)?;
        let ast = frontend.parse(&source)?;

        println!("AOT: Compiling to WASM at {:?}", output_path);

        let artifact = frontend.compile_to_gaia(&ast, &vfs, "wasm32-wasi")?;

        match artifact {
            chomsky_extract::BackendArtifact::Binary(bytes) => {
                fs::write(output_path, bytes).map_err(NyarError::from)?;
            }
            chomsky_extract::BackendArtifact::Collection(files) => {
                if let Some(bytes) = files.get("main.wasm") {
                    fs::write(output_path, bytes).map_err(NyarError::from)?;
                } else if let Some((_, bytes)) = files.iter().next() {
                    fs::write(output_path, bytes).map_err(NyarError::from)?;
                }
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

        let vfs = self.default_vfs();
        let source = fs::read_to_string(source_path).map_err(NyarError::from)?;
        let ast = frontend.parse(&source)?;
        let tree = frontend.lower(&ast, &vfs)?;

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
        let vfs = self.default_vfs();
        let source = fs::read_to_string(source_path).map_err(NyarError::from)?;
        let ast = frontend.parse(&source)?;
        let _tree = frontend.lower(&ast, &vfs)?;

        println!("CLR: Compiling IKunTree to CLR at {:?}", output_path);
        Err(NyarError::Compile("CLR backend is not yet fully integrated".to_string()))
    }
}

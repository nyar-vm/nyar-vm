use crate::bytecode::compiler::NyarBackend;
use crate::vm::core::NyarVM;
use nyar_types::{NyarError, NyarFrontend, QualifiedName};
use oak_core::source::Source;
use oak_vfs::{Vfs, WritableVfs};

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
        let module_idx = vm.load_module(module, uri.to_string());
        let main_name = QualifiedName::new(vec!["main".to_string()]);
        if vm.execute_symbol(&main_name, vec![]).is_ok() {
            Ok(())
        } else {
            let chunk_idx = vm.get_module(module_idx).chunks.len().saturating_sub(1);
            vm.execute(module_idx, chunk_idx)
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
        let module_idx = vm.load_module(module, "code".to_string());
        let main_name = QualifiedName::new(vec!["main".to_string()]);
        if vm.execute_symbol(&main_name, vec![]).is_ok() {
            Ok(())
        } else {
            let chunk_idx = vm.get_module(module_idx).chunks.len().saturating_sub(1);
            vm.execute(module_idx, chunk_idx)
                .map(|_| ())
                .map_err(NyarError::from)
        }
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
    pub fn compile_to_wasm<F, V>(
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

        println!("AOT: Compiling to WASM at {}", output_uri);

        let artifact = frontend.compile_to_gaia(&ast, vfs, "wasm32-wasi")?;

        match artifact {
            chomsky_extract::BackendArtifact::Binary(bytes) => {
                let content = String::from_utf8_lossy(&bytes).to_string();
                vfs.write_file(output_uri, content.into());
            }
            chomsky_extract::BackendArtifact::Collection(files) => {
                if let Some(bytes) = files.get("main.wasm") {
                    let content = String::from_utf8_lossy(bytes).to_string();
                    vfs.write_file(output_uri, content.into());
                } else if let Some((_, bytes)) = files.iter().next() {
                    let content = String::from_utf8_lossy(bytes).to_string();
                    vfs.write_file(output_uri, content.into());
                }
            }
            _ => return Err(NyarError::Compile("Expected binary artifact".to_string())),
        }

        Ok(())
    }

    /// 编译到 JVM .class 文件
    pub fn compile_to_jvm<F, V>(
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
        #[cfg(feature = "jvm")]
        {
            use chomsky::adapters::GaiaJvmAdapter;
            use chomsky_extract::Backend;

            let source = vfs
                .get_source(source_uri)
                .ok_or_else(|| NyarError::Compile(format!("Source not found: {}", source_uri)))?;
            let content = source.get_text_from(0);
            let ast = frontend.parse(&content)?;
            let tree = frontend.lower(&ast, vfs)?;

            println!("JVM: Compiling IKunTree to JVM at {}", output_uri);

            let adapter = GaiaJvmAdapter;
            let artifact = adapter
                .generate(&tree)
                .map_err(|e| NyarError::Compile(format!("{:?}", e)))?;

            match artifact {
                chomsky_extract::BackendArtifact::Binary(bytes) => {
                    let content = String::from_utf8_lossy(&bytes).to_string();
                    vfs.write_file(output_uri, content.into());
                }
                _ => return Err(NyarError::Compile("Expected binary artifact".to_string())),
            }

            Ok(())
        }
        #[cfg(not(feature = "jvm"))]
        {
            let _ = (frontend, vfs, source_uri, output_uri);
            Err(NyarError::Compile("JVM backend is not enabled".to_string()))
        }
    }

    /// 编译到 CLR 程序集
    pub fn compile_to_clr<F, V>(
        &self,
        frontend: &F,
        vfs: &V,
        source_uri: &str,
        _output_uri: &str,
    ) -> Result<(), NyarError>
    where
        F: NyarFrontend,
        V: Vfs,
    {
        let source = vfs
            .get_source(source_uri)
            .ok_or_else(|| NyarError::Compile(format!("Source not found: {}", source_uri)))?;
        let content = source.get_text_from(0);
        let ast = frontend.parse(&content)?;
        let _tree = frontend.lower(&ast, vfs)?;

        println!("CLR: Compiling IKunTree to CLR at {}", _output_uri);
        Err(NyarError::Compile("CLR backend is not yet fully integrated".to_string()))
    }
}

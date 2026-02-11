use crate::bytecode::compiler::NyarBackend;
use crate::vm::core::NyarVM;
use nyar_types::{HasDebugInfo, IKun, NyarError, NyarFrontend, QualifiedName};
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
        oak_vfs::DiskVfs::new(std::env::current_dir().unwrap_or_default())
    }

    /// 运行源代码文件
    pub fn run_source<F, V, A>(&self, frontend: &F, vfs: &V, uri: &str) -> Result<(), NyarError>
    where
        F: NyarFrontend<A>,
        V: Vfs,
        A: chomsky_uir::Analysis<IKun>,
        A::Data: HasDebugInfo,
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
        let module_idx = vm.load_named_module(module, uri.to_string());
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
    pub fn run_code<F, V, A>(&self, frontend: &F, vfs: &V, source: &str) -> Result<(), NyarError>
    where
        F: NyarFrontend<A>,
        V: Vfs,
        A: chomsky_uir::Analysis<IKun>,
        A::Data: HasDebugInfo,
    {
        let ast = frontend.parse(source)?;
        let tree = frontend.lower(&ast, vfs)?;
        let mut backend = NyarBackend::new();
        backend.lower_tree(&tree)?;
        let module = backend.finish();
        let mut vm = NyarVM::new();
        let module_idx = vm.load_named_module(module, "code".to_string());
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
}

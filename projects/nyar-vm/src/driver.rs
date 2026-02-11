use crate::bytecode::format::NyarModule;
use crate::vm::core::NyarVM;
use nyar_types::{NyarError, QualifiedName};

/// Nyar 驱动程序
#[derive(Default)]
pub struct NyarDriver;

impl NyarDriver {
    /// 创建一个新的驱动程序
    pub fn new() -> Self {
        Self
    }

    /// 运行已编译的模块
    pub fn run_module(&self, module: NyarModule, main_module_name: &str) -> Result<(), NyarError> {
        let mut vm = NyarVM::new();
        // 这里只是加载模块，假设 NyarModule 包含所有需要的信息
        // 实际上可能需要一个 module name
        let module_idx = vm.load_named_module(module, main_module_name.to_string());
        
        let main_name = QualifiedName::new(vec!["main".to_string()]);
        // 尝试执行 main 函数
        if vm.execute_symbol(&main_name, vec![]).is_ok() {
            Ok(())
        } else {
            // 如果没有 main 函数，执行最后一个 chunk (通常是 top-level code)
            let chunk_idx = vm.get_module(module_idx).chunks.len().saturating_sub(1);
            vm.execute(module_idx, chunk_idx)
                .map(|_| ())
                .map_err(NyarError::from)
        }
    }
}

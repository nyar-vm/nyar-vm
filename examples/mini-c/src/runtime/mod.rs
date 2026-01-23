use chomsky_uast::UastNode;
use gaia_jit::GaiaJit;
use gaia_assembler::program::GaiaModule;

pub struct MiniCRuntime {
    jit: GaiaJit,
}

impl MiniCRuntime {
    pub fn new() -> Self {
        Self {
            jit: GaiaJit::new(),
        }
    }

    pub fn execute(&mut self, uast: UastNode) -> Result<(), String> {
        // 1. 将 UAST 转换为 Gaia Module (指令 IR)
        // 这里通常需要一个转换器，类似于 rusty-c 中的 GaiaTranslator
        let module = self.translate_to_gaia(uast)?;

        // 2. 使用 JIT 执行
        self.jit.load_module(module).map_err(|e| format!("JIT load error: {:?}", e))?;
        self.jit.run("main").map_err(|e| format!("Execution error: {:?}", e))?;

        Ok(())
    }

    fn translate_to_gaia(&self, _uast: UastNode) -> Result<GaiaModule, String> {
        // 这是一个复杂的转换过程，目前先返回一个空的模块作为演示
        // 实际开发中会复用 GaiaTranslator 的逻辑
        Ok(GaiaModule::new("mini-c-module"))
    }
}

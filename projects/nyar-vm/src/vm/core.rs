use crate::bytecode::format::NyarcModule;
use crate::vm::effects::HandlerFrame;
use crate::vm::ffi::FFIRegistry;
use crate::vm::value::{Value, Frame};
use crate::vm::NyarError;
use nyar_gc::{MarkContext, NyarGc, Trace};


use nyar_types::QualifiedName;


pub trait JitProvider: Send + Sync {
    fn try_execute(
        &self,
        vm: &mut NyarVM,
        module_idx: usize,
        chunk_idx: usize,
    ) -> Option<Result<Value, NyarError>>;
    fn osr(
        &self,
        vm: &NyarVM,
        module_idx: usize,
        chunk_idx: usize,
        target: u32,
    ) -> Result<*const u8, NyarError>;
}

pub struct NyarVM {
    pub gc: NyarGc,
    pub stack: Vec<Value>,
    pub sp: usize,
    pub frames: Vec<Frame>,
    pub modules: Vec<NyarcModule>,
    pub handler_stack: Vec<HandlerFrame>,
    #[allow(clippy::type_complexity)]
    pub stdout: Option<Box<dyn Fn(&str)>>,
    pub trace_log: std::cell::RefCell<Vec<String>>,
    pub ffi: FFIRegistry,
    pub symbol_table: std::collections::HashMap<QualifiedName, (usize, u16)>, // (module_idx, chunk_idx)
    pub builtins: std::collections::HashMap<QualifiedName, Value>,
    pub jit: Option<std::sync::Arc<dyn JitProvider>>,
    pub local_hotness: u8,
    pub last_gc_count: u64,
}

impl Trace for NyarVM {
    fn trace(&self, ctx: &mut MarkContext) {
        for i in 0..self.sp {
            self.stack[i].trace(ctx);
        }
        for frame in &self.frames {
            frame.trace(ctx);
        }
        for builtin in self.builtins.values() {
            builtin.trace(ctx);
        }
    }
}

impl NyarVM {
    pub fn new() -> Self {
        let mut vm = Self {
            gc: NyarGc::new(),
            stack: Vec::with_capacity(64),
            sp: 0,
            frames: Vec::new(),
            modules: Vec::new(),
            handler_stack: Vec::new(),
            stdout: None,
            trace_log: std::cell::RefCell::new(Vec::new()),
            ffi: FFIRegistry::new(),
            symbol_table: std::collections::HashMap::new(),
            builtins: std::collections::HashMap::new(),
            jit: None,
            local_hotness: 0,
            last_gc_count: 0,
        };
        vm.register_builtins();
        vm
    }

    pub fn push(&mut self, v: Value) -> Result<(), NyarError> {
        if self.sp >= self.stack.len() {
            self.stack.push(v)
        } else {
            self.stack[self.sp] = v
        }
        self.sp += 1;
        
        // Implicitly check for GC on significant stack growth
        if self.sp % 256 == 0 {
            if nyar_gc::runtime::GC_STOP_THE_WORLD.load(std::sync::atomic::Ordering::Acquire) {
                self.gc.flush_thread_local();
            }
        }
        Ok(())
    }

    pub fn pop(&mut self) -> Result<Value, NyarError> {
        if self.sp == 0 {
            Err(self.error(nyar_types::VmErrorKind::StackUnderflow))
        } else {
            self.sp -= 1;
            Ok(self.stack[self.sp])
        }
    }

    pub fn error(&self, kind: nyar_types::VmErrorKind) -> NyarError {
        let code = match &kind {
            nyar_types::VmErrorKind::StackUnderflow => 0x1001,
            nyar_types::VmErrorKind::IndexOutOfBounds(_) => 0x1002,
            nyar_types::VmErrorKind::ModuleNotFound(_) => 0x1003,
            nyar_types::VmErrorKind::ChunkNotFound { .. } => 0x1004,
            nyar_types::VmErrorKind::SymbolNotFound(_) => 0x1005,
            nyar_types::VmErrorKind::NoActiveFrame => 0x1006,
            nyar_types::VmErrorKind::LimitExceeded => 0x1007,
            nyar_types::VmErrorKind::Halt => 0x1008,
            nyar_types::VmErrorKind::ImplNotFound { .. } => 0x1009,
            nyar_types::VmErrorKind::UnhandledEffect(_) => 0x100A,
            nyar_types::VmErrorKind::TypeMismatch { .. } => 0x100B,
            nyar_types::VmErrorKind::YieldAsync => 0x100C,
            nyar_types::VmErrorKind::InvalidOpcode(_) => 0x100D,
            nyar_types::VmErrorKind::DivisionByZero => 0x100E,
            nyar_types::VmErrorKind::InvalidContinuation => 0x100F,
            nyar_types::VmErrorKind::FutureFailed => 0x1010,
            nyar_types::VmErrorKind::RuntimeError(_) => 0x1011,
        };
        let location = self.frames.last().map(|f| f.location).unwrap_or_default();
        NyarError::new(code, nyar_types::NyarErrorKind::Vm(kind), location)
    }

    pub fn peek_at(&self, depth: usize) -> Result<Value, NyarError> {
        if depth >= self.sp {
            Err(self.error(nyar_types::VmErrorKind::StackUnderflow))
        } else {
            Ok(self.stack[self.sp - 1 - depth])
        }
    }

    pub fn swap_with(&mut self, depth: usize) -> Result<(), NyarError> {
        if depth >= self.sp {
            Err(self.error(nyar_types::VmErrorKind::StackUnderflow))
        } else {
            let top = self.sp - 1;
            let idx = self.sp - 1 - depth;
            self.stack.swap(top, idx);
            Ok(())
        }
    }

    pub fn print_line(&self, msg: &str) {
        if let Some(cb) = &self.stdout {
            cb(msg);
        } else {
            println!("{}", msg);
        }
        self.trace_log.borrow_mut().push(msg.to_string());
    }

    pub fn log(&self, msg: &str) {
        self.print_line(msg);
    }

    pub fn get_module(&self, idx: usize) -> &NyarcModule {
        &self.modules[idx]
    }

    pub fn get_traceback_summary(&self) -> String {
        let mut res = String::new();
        for (i, f) in self.frames.iter().enumerate().rev().take(10) {
            let func_name = if let Some(closure) = f.closure.try_as_closure() {
                format!("Closure#{}", closure.func)
            } else if let Some(chunk_idx) = f.chunk_idx {
                // Try to find a symbol for this chunk
                self.symbol_table
                    .iter()
                    .find(|(_, &(m_idx, c_idx))| m_idx == f.module_idx && c_idx as usize == chunk_idx)
                    .map(|(name, _)| name.to_string())
                    .unwrap_or_else(|| format!("Chunk#{}", chunk_idx))
            } else {
                "Anonymous".to_string()
            };

            let info = format!(
                "  [frame {}] {} at {}\n",
                i, func_name, f.location
            );
            res.push_str(&info);
        }
        if self.frames.len() > 10 {
            res.push_str(&format!("  ... ({} more frames)\n", self.frames.len() - 10));
        }
        res
    }

    pub fn decay_hotness(&self) {
        for module in &self.modules {
            for chunk in &module.chunks {
                let old = chunk.hotness.load(std::sync::atomic::Ordering::Relaxed);
                chunk
                    .hotness
                    .store(old >> 1, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }

    pub fn print_traceback(&self, err: &NyarError) {
        self.print_line("Traceback (most recent call last):");
        let start = if self.frames.len() > 20 {
            self.print_line(&format!("... ({} frames omitted)", self.frames.len() - 20));
            self.frames.len() - 20
        } else {
            0
        };
        for (i, f) in self.frames.iter().enumerate().skip(start) {
            let func_name = if let Some(closure) = f.closure.try_as_closure() {
                format!("Closure#{}", closure.func)
            } else if let Some(chunk_idx) = f.chunk_idx {
                self.symbol_table
                    .iter()
                    .find(|(_, &(m_idx, c_idx))| m_idx == f.module_idx && c_idx as usize == chunk_idx)
                    .map(|(name, _)| name.to_string())
                    .unwrap_or_else(|| format!("Chunk#{}", chunk_idx))
            } else {
                "Anonymous".to_string()
            };

            let info = format!(
                "  [frame {}] {} at {}",
                i, func_name, f.location
            );
            self.print_line(&info);
        }
        self.print_line(&format!("{}", err));
    }

    pub fn dump_symbol_table(&self) -> String {
        let mut res = String::new();
        res.push_str("Symbol Table Dump:\n");
        let mut symbols: Vec<_> = self.symbol_table.iter().collect();
        symbols.sort_by(|a, b| a.0.to_string().cmp(&b.0.to_string()));

        for (name, &(m_idx, c_idx)) in symbols {
            res.push_str(&format!(
                "  {} -> Module {}, Chunk {}\n",
                name, m_idx, c_idx
            ));
        }
        res
    }

    pub fn load_module(&mut self, module: NyarcModule) -> usize {
        let module_idx = self.modules.len();

        // Update symbol table with exports from this module
        for export in &module.exports {
            self.symbol_table
                .insert(export.symbol.clone(), (module_idx, export.chunk_idx));
        }

        self.modules.push(module);
        module_idx
    }

    pub fn call_closure_sync(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, NyarError> {
        let target_depth = self.frames.len();
        let argc = args.len() as u16;
        for arg in args {
            self.push(arg)?;
        }
        self.push(callee)?;
        self.execute_call_closure(argc)?;
        while self.frames.len() > target_depth {
            self.execute_step()?;
        }
        self.pop()
    }
}

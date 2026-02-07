
use crate::vm::effects::HandlerFrame;
use crate::vm::ffi::FFIRegistry;
use crate::vm::value::{Value, Frame};
use crate::vm::NyarError;
use nyar_gc::{MarkContext, NyarGc, Trace};
use std::sync::Arc;
use dashmap::DashMap;

use nyar_types::QualifiedName;
use crate::bytecode::format::NyarcModule;

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

pub struct NyarEnv {
    pub modules: DashMap<usize, NyarcModule>,
    pub module_names: DashMap<usize, String>,
    pub symbol_table: DashMap<QualifiedName, (usize, u16)>,
    pub builtins: DashMap<QualifiedName, Value>,
    pub next_module_idx: std::sync::atomic::AtomicUsize,
}

impl NyarEnv {
    pub fn new() -> Self {
        Self {
            modules: DashMap::new(),
            module_names: DashMap::new(),
            symbol_table: DashMap::new(),
            builtins: DashMap::new(),
            next_module_idx: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

pub struct NyarVM {
    pub gc: Arc<NyarGc>,
    pub env: Arc<NyarEnv>,
    pub stack: Vec<Value>,
    pub sp: usize,
    pub frames: Vec<Frame>,
    pub handler_stack: Vec<HandlerFrame>,
    pub trace_log: Arc<std::sync::Mutex<Vec<String>>>,
    pub ffi: FFIRegistry,
    pub jit: Option<std::sync::Arc<dyn JitProvider>>,
    pub local_hotness: u8,
    pub last_gc_count: u64,
    pub current_waker: Option<std::task::Waker>,
    pub network: crate::vm::net::NetworkContext,
    pub platform: Arc<dyn crate::vm::platform::NyarPlatform>,
    pub effect_handler: Option<Arc<dyn crate::vm::effects::EffectHandler>>,
}

impl Trace for NyarVM {
    fn trace(&self, ctx: &mut MarkContext) {
        for i in 0..self.sp {
            self.stack[i].trace(ctx);
        }
        for frame in &self.frames {
            frame.trace(ctx);
        }
        for r in self.env.builtins.iter() {
            r.value().trace(ctx);
        }
    }
}

impl NyarVM {
    pub fn new() -> Self {
        let mut vm = Self {
            gc: Arc::new(NyarGc::new()),
            env: Arc::new(NyarEnv::new()),
            stack: Vec::with_capacity(64),
            sp: 0,
            frames: Vec::new(),
            handler_stack: Vec::new(),
            trace_log: Arc::new(std::sync::Mutex::new(Vec::new())),
            ffi: FFIRegistry::new(),
            jit: None,
            local_hotness: 0,
            last_gc_count: 0,
            current_waker: None,
            network: crate::vm::net::NetworkContext::new(),
            platform: Arc::new(crate::vm::platform::StubPlatform),
            effect_handler: None,
        };
        vm.ffi.register_std();
        vm
    }

    pub fn spawn_child(&self) -> Self {
        Self {
            gc: self.gc.clone(),
            env: self.env.clone(),
            stack: Vec::with_capacity(64),
            sp: 0,
            frames: Vec::new(),
            handler_stack: Vec::new(),
            trace_log: self.trace_log.clone(),
            ffi: self.ffi.clone(),
            jit: self.jit.clone(),
            local_hotness: 0,
            last_gc_count: self.last_gc_count,
            current_waker: self.current_waker.clone(),
            network: self.network.clone(),
            platform: self.platform.clone(),
            effect_handler: self.effect_handler.clone(),
        }
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

    pub fn log(&self, msg: &str) {
        self.platform.stdout_write(&format!("{}\n", msg));
    }

    pub fn print_line(&self, msg: &str) {
        self.platform.stdout_write(&format!("{}\n", msg));
    }

    pub fn get_module(&self, idx: usize) -> dashmap::mapref::one::Ref<usize, NyarcModule> {
        self.env.modules.get(&idx).expect("Module not found")
    }

    pub fn gc_stats(&self) -> (usize, u64) {
        let allocated = self.gc.allocated_bytes.load(std::sync::atomic::Ordering::Relaxed);
        let collections = self.gc.total_collections.load(std::sync::atomic::Ordering::Relaxed);
        (allocated, collections)
    }

    pub fn get_traceback_summary(&self) -> String {
        let mut res = String::new();
        for (i, f) in self.frames.iter().enumerate().rev().take(10) {
            let func_name = if let Some(closure) = f.closure.try_as_closure() {
                format!("Closure#{}", closure.func)
            } else if let Some(chunk_idx) = f.chunk_idx {
                // Try to find a symbol for this chunk
                self.env.symbol_table
                    .iter()
                    .find(|r| {
                        let (m_idx, c_idx) = *r.value();
                        m_idx == f.module_idx && c_idx as usize == chunk_idx
                    })
                    .map(|r| r.key().to_string())
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
        for module in self.env.modules.iter() {
            for chunk in &module.value().chunks {
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
                self.env.symbol_table
                    .iter()
                    .find(|r| {
                        let (m_idx, c_idx) = *r.value();
                        m_idx == f.module_idx && c_idx as usize == chunk_idx
                    })
                    .map(|r| r.key().to_string())
                    .unwrap_or_else(|| format!("Chunk#{}", chunk_idx))
            } else {
                "Anonymous".to_string()
            };

            let info = format!(
                "  [frame {}] {} at {}:{}",
                i,
                func_name,
                self.env.module_names.get(&(f.location.source_id as usize)).map(|r| r.value().clone()).unwrap_or_else(|| format!("source:{}", f.location.source_id)),
                f.location.offset
            );
            self.print_line(&info);
        }
        self.print_line(&format!("{}", err));
    }

    pub fn dump_symbol_table(&self) -> String {
        let mut res = String::new();
        res.push_str("Symbol Table Dump:\n");
        let mut symbols: Vec<_> = self.env.symbol_table.iter().map(|r| (r.key().clone(), *r.value())).collect();
        symbols.sort_by(|a, b| a.0.to_string().cmp(&b.0.to_string()));

        for (name, (m_idx, c_idx)) in symbols {
            res.push_str(&format!(
                "  {} -> Module {}, Chunk {}\n",
                name, m_idx, c_idx
            ));
        }
        res
    }

    pub fn load_module(&mut self, module: NyarcModule) -> usize {
        self.load_named_module(module, "anonymous".to_string())
    }

    pub fn load_named_module(&mut self, module: NyarcModule, name: String) -> usize {
        let module_idx = self.env.next_module_idx.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        // Update symbol table with exports from this module
        for export in &module.exports {
            self.env.symbol_table
                .insert(export.symbol.clone(), (module_idx, export.chunk_idx));
        }

        self.env.modules.insert(module_idx, module);
        self.env.module_names.insert(module_idx, name);
        module_idx
    }

    pub fn call_closure_sync(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, NyarError> {
        let target_depth = self.frames.len();
        let argc = args.len() as u16;
        self.push(callee)?;
        for arg in args {
            self.push(arg)?;
        }
        self.execute_call_closure(argc)?;
        while self.frames.len() > target_depth {
            self.execute_step()?;
        }
        self.pop()
    }

    pub fn resume_continuation_sync(&mut self, continuation: Value, result: Value) -> Result<Value, NyarError> {
        let target_depth = self.frames.len();
        if let Some(cont) = continuation.try_as_continuation() {
            // Restore frames
            self.frames.extend(cont.frames.clone());
            // Restore stack slice
            for val in &cont.stack_slice {
                self.push(*val)?;
            }
            // Push the result of resume
            self.push(result)?;
        } else {
            return Err(self.error(nyar_types::VmErrorKind::InvalidOpcode(0x14)));
        }

        while self.frames.len() > target_depth {
            self.execute_step()?;
        }
        self.pop()
    }
}

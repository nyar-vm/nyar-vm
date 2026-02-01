use crate::bytecode::format::NyarcModule;
use crate::vm::effects::HandlerFrame;
use crate::vm::ffi::FFIRegistry;
use crate::vm::value::{Value, Frame};
use crate::vm::VmError;
use nyar_gc::{MarkContext, NyarGc, Trace};


use nyar_types::{EffectInfo, QualifiedName, SourceLocation};


pub trait JitProvider: Send + Sync {
    fn try_execute(
        &self,
        vm: &mut NyarVM,
        module_idx: usize,
        chunk_idx: usize,
    ) -> Option<Result<Value, VmError>>;
    fn osr(
        &self,
        vm: &NyarVM,
        module_idx: usize,
        chunk_idx: usize,
        target: u32,
    ) -> Result<*const u8, VmError>;
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

    pub fn push(&mut self, v: Value) -> Result<(), VmError> {
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

    pub fn pop(&mut self) -> Result<Value, VmError> {
        if self.sp == 0 {
            println!("VM: STACK UNDERFLOW!");
            Err(VmError::StackUnderflow)
        } else {
            self.sp -= 1;
            Ok(self.stack[self.sp])
        }
    }

    pub fn peek_at(&self, depth: usize) -> Result<Value, VmError> {
        if depth >= self.sp {
            Err(VmError::StackUnderflow)
        } else {
            Ok(self.stack[self.sp - 1 - depth])
        }
    }

    pub fn swap_with(&mut self, depth: usize) -> Result<(), VmError> {
        if depth >= self.sp {
            Err(VmError::StackUnderflow)
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

    pub fn get_traceback_summary(&self) -> String {
        let mut res = String::new();
        for (i, f) in self.frames.iter().enumerate().rev().take(10) {
            let info = match f.chunk_idx {
                Some(ci) => format!(
                    "  frame {}: module={}, chunk={}, ip={}\n",
                    i, f.module_idx, ci, f.ip
                ),
                None => format!(
                    "  frame {}: module={}, chunk=<entry>, ip={}\n",
                    i, f.module_idx, f.ip
                ),
            };
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

    pub fn print_traceback(&self, err: &VmError) {
        self.print_line("Traceback (most recent call last):");
        let start = if self.frames.len() > 20 {
            self.print_line(&format!("... ({} frames omitted)", self.frames.len() - 20));
            self.frames.len() - 20
        } else {
            0
        };
        for (i, f) in self.frames.iter().enumerate().skip(start) {
            let info = match f.chunk_idx {
                Some(ci) => format!(
                    "frame {}: module={}, chunk={}, ip={}",
                    i, f.module_idx, ci, f.ip
                ),
                None => format!(
                    "frame {}: module={}, chunk=<entry>, ip={}",
                    i, f.module_idx, f.ip
                ),
            };
            self.print_line(&info);
        }
        match err {
            VmError::UnhandledEffect(name) => {
                self.print_line(&format!("UnhandledEffect: {}", name))
            }
            VmError::UnhandledError => self.print_line("UnhandledError"),
            VmError::RuntimeError(msg) => self.print_line(&format!("RuntimeError: {}", msg)),
            _ => self.print_line("Error"),
        }
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
}

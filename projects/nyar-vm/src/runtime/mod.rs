use crate::vm::core::NyarVM;
use crate::vm::value::Value;
use nyar_types::QualifiedName;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NyarBuiltin {
    /// Print to standard output
    Print,
    /// Print to standard output with a newline
    Println,
    /// Exit the process with a status code
    Exit,
    /// Get the current system time
    GetTime,
    /// Sleep for a duration in milliseconds
    Sleep,
    /// Add two integers (native implementation)
    NativeAdd,
}

impl NyarBuiltin {
    /// Get the canonical name for this builtin
    pub fn name(&self) -> &'static str {
        match self {
            Self::Print => "std::io::print",
            Self::Println => "std::io::println",
            Self::Exit => "std::process::exit",
            Self::GetTime => "std::time::now",
            Self::Sleep => "std::thread::sleep",
            Self::NativeAdd => "std::ops::add",
        }
    }

    /// Try to map a name to a builtin
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "std::io::print" => Some(Self::Print),
            "std::io::println" => Some(Self::Println),
            "std::process::exit" => Some(Self::Exit),
            "std::time::now" => Some(Self::GetTime),
            "std::thread::sleep" => Some(Self::Sleep),
            "std::ops::add" => Some(Self::NativeAdd),
            _ => None,
        }
    }

    /// Helper to emit an IKunTree node for this builtin
    pub fn emit(&self, args: Vec<chomsky_extract::IKunTree>) -> chomsky_extract::IKunTree {
        chomsky_extract::IKunTree::CrossLangCall(
            "nyar".to_string(),
            self.name().to_string(),
            args,
        )
    }

    /// Emit bytecode for this builtin
    pub fn emit_bytecode(
        &self,
        code: &mut Vec<u8>,
        arg_count: u8,
        add_constant: &mut dyn FnMut(crate::bytecode::format::Constant) -> u16,
    ) {
        let name = self.name();
        let name_idx = add_constant(crate::bytecode::format::Constant::String(name.to_string()));
        code.extend_from_slice(
            &crate::bytecode::instruction::Instruction::FFICall(name_idx, arg_count).encode(),
        );
    }
}

impl NyarVM {
    pub fn register_builtins(&mut self) {
        // Builtins are registered using their canonical names
        self.register_java_builtins();
        
        self.ffi.register(NyarBuiltin::NativeAdd.name().to_string(), Box::new(crate::vm::ffi::NativeAdd));
        self.ffi.register(NyarBuiltin::GetTime.name().to_string(), Box::new(crate::vm::ffi::NativeGetTime));
        self.ffi.register(NyarBuiltin::Sleep.name().to_string(), Box::new(crate::vm::ffi::NativeSleep));
        self.ffi.register(NyarBuiltin::Print.name().to_string(), Box::new(crate::vm::ffi::NativePrint));
        self.ffi.register(NyarBuiltin::Println.name().to_string(), Box::new(crate::vm::ffi::NativePrintln));
        self.ffi.register(NyarBuiltin::Exit.name().to_string(), Box::new(crate::vm::ffi::NativeExit));
    }

    fn register_java_builtins(&mut self) {
        let out = Value::dyn_object(&self.gc);

        // System.out.println -> standard mapping
        let system = Value::dyn_object(&self.gc);
        if let Some(system_mut) = system.try_as_dyn_object_mut() {
            system_mut.entries.insert("out".to_string(), out);
        }

        self.builtins.insert(QualifiedName::new(vec!["System".to_string()]), system);
    }
}

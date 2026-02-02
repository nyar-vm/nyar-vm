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
    /// Try to map a standard name to a builtin
    pub fn from_standard_name(name: &str) -> Option<Self> {
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

    /// Get the standard name for this builtin
    pub fn standard_name(&self) -> &'static str {
        match self {
            Self::Print => "std::io::print",
            Self::Println => "std::io::println",
            Self::Exit => "std::process::exit",
            Self::GetTime => "std::time::now",
            Self::Sleep => "std::thread::sleep",
            Self::NativeAdd => "std::ops::add",
        }
    }

    /// Convert back to a cross-language call representation
    pub fn to_cross_lang_call(&self) -> (&'static str, &'static str) {
        ("nyar", self.standard_name())
    }

    /// Helper to emit an IKunTree node for this builtin
    pub fn emit(&self, args: Vec<chomsky_extract::IKunTree>) -> chomsky_extract::IKunTree {
        chomsky_extract::IKunTree::CrossLangCall(
            "nyar".to_string(),
            self.standard_name().to_string(),
            args,
        )
    }
}

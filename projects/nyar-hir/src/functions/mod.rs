use crate::{Identifier, IndexedIterator, NyarType, NyarValue, Symbol};
use indexmap::IndexMap;
use nyar_error::{FileSpan, NyarError};
use std::slice::Iter;

pub mod externals;
pub mod keywords;
pub mod resolver;

#[derive(Default)]
pub struct FunctionRegister {
    native: IndexMap<String, FunctionType>,
    external: IndexMap<String, ExternalType>,
}

/// `@ffi("module", "field")`
pub struct ExternalType {
    pub module: Symbol,
    pub field: Symbol,
    pub input: Vec<NyarType>,
    pub output: Vec<NyarType>,
}

/// `function`
pub struct FunctionType {
    pub symbol: Symbol,
    pub export: bool,
    pub entry: bool,
    pub input: IndexMap<String, ParameterType>,
    pub output: Vec<NyarType>,
    pub body: FunctionBody,
    pub span: FileSpan,
}

pub struct ParameterType {
    pub name: Symbol,
    pub type_hint: NyarType,
    pub span: FileSpan,
}

impl FunctionType {
    pub fn new(path: Symbol) -> Self {
        Self {
            symbol: path,
            export: false,
            entry: false,
            input: IndexMap::default(),
            output: vec![],
            body: FunctionBody::default(),
            span: Default::default(),
        }
    }
    pub fn name(&self) -> String {
        self.symbol.to_string()
    }
    pub fn with_public(self) -> Self {
        Self { export: true, ..self }
    }
    pub fn with_inputs<I>(mut self, inputs: I) -> Self
    where
        I: IntoIterator<Item = ParameterType>,
    {
        for i in inputs {
            self.input.insert(i.name.to_string(), i);
        }
        self
    }
    pub fn with_outputs<I>(mut self, outputs: I) -> Self
    where
        I: IntoIterator<Item = NyarType>,
    {
        self.output = outputs.into_iter().collect();
        self
    }
    pub fn with_operations<I>(mut self, operations: I) -> Self
    where
        I: IntoIterator<Item = Operation>,
    {
        self.body.codes = operations.into_iter().collect();
        self
    }
}

#[derive(Default)]
pub struct FunctionBody {
    codes: Vec<Operation>,
}

impl<'i> IntoIterator for &'i FunctionBody {
    type Item = &'i Operation;
    type IntoIter = Iter<'i, Operation>;

    fn into_iter(self) -> Self::IntoIter {
        self.codes.iter()
    }
}

#[derive(Debug)]
pub enum Operation {
    Sequence {
        items: Vec<Operation>,
    },
    CallFunction {
        name: Symbol,
        input: Vec<Operation>,
    },
    GetVariable {
        kind: VariableKind,
        variable: Symbol,
    },
    SetVariable {
        kind: VariableKind,
        variable: Symbol,
    },
    TeeVariable {
        variable: Symbol,
    },
    Loop {
        r#continue: Symbol,
        r#break: Symbol,
        body: Vec<Operation>,
    },
    Goto {
        label: Symbol,
    },
    Drop,
    Return,
    Unreachable,

    /// `if cond { } { }`
    Conditional {
        condition: Vec<Operation>,
        then: Vec<Operation>,
        r#else: Vec<Operation>,
    },
    Constant {
        value: NyarValue,
    },
    NativeSum {
        native: NyarType,
        terms: Vec<Operation>,
    },
    Convert {
        from: NyarType,
        into: NyarType,
        code: Vec<Operation>,
    },
    Transmute {
        from: NyarType,
        into: NyarType,
        code: Vec<Operation>,
    },

    NativeEqual {
        native: NyarType,
        terms: Vec<Operation>,
    },
    NativeEqualZero {
        native: NyarType,
        term: Box<Operation>,
    },
}

impl Operation {
    pub fn r#loop(label: &str, body: Vec<Operation>) -> Self {
        Self::Loop {
            r#continue: Symbol::new(&format!("{label}@continue")),
            r#break: Symbol::new(&format!("{label}@break")),
            body,
        }
    }
    pub fn r#continue(label: &str) -> Self {
        Self::Goto { label: Symbol::new(&format!("{label}@continue")) }
    }
    pub fn r#break(label: &str) -> Self {
        Self::Goto { label: Symbol::new(&format!("{label}@break")) }
    }
}

#[derive(Debug)]
pub enum VariableKind {
    Global,
    Local,
    Table,
}

impl FunctionRegister {
    pub fn get_id(&self, name: &str) -> Result<usize, NyarError> {
        match self.native.get_full(name) {
            Some((index, _, _)) => return Ok(index),
            None => {}
        }
        match self.external.get_full(name) {
            Some((index, _, _)) => return Ok(self.native.len() + index),
            None => {}
        }
        Err(NyarError::custom(format!("missing function {name}")))
    }
    pub fn add_native(&mut self, item: FunctionType) {
        self.native.insert(item.symbol.to_string(), item);
    }
    pub fn get_natives(&self) -> IndexedIterator<FunctionType> {
        IndexedIterator::new(&self.native).with_index(self.external.len())
    }
    pub fn add_external(&mut self, item: ExternalType) {
        self.external.insert(item.name(), item);
    }
    pub fn get_externals(&self) -> IndexedIterator<ExternalType> {
        IndexedIterator::new(&self.external)
    }
}

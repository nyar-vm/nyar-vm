use crate::{NyarType, NyarValue, Symbol};
use indexmap::IndexMap;
use nyar_error::FileSpan;
use std::slice::Iter;

pub mod operations;

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

#[derive(Debug)]
pub enum VariableKind {
    Global,
    Local,
    Table,
}

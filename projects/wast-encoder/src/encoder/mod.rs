use std::{
    fmt::{Debug, Formatter, Write},
    ops::AddAssign,
    sync::Arc,
};

use nyar_error::NyarError;

use crate::{
    dag::DependenciesTrace,
    wasi_types::{ComponentDefine, LowerFunction},
    DependentGraph, ExternalFunction, WasiInstance, WasiParameter, WasiType,
};

mod for_instance;

pub struct CanonicalWasi {
    pub name: Arc<str>,
    pub graph: DependentGraph,
    pub imports: Vec<CanonicalImport>,
    pub type_signatures: bool,
    pub indent_text: &'static str,
}

pub(crate) struct WastEncoder<'a, W> {
    pub source: &'a CanonicalWasi,
    pub writer: W,
    pub indent: usize,
}

impl CanonicalWasi {
    pub fn draw_mermaid(&self) -> String {
        let mut out = String::new();
        out.push_str("flowchart LR\n");
        for import in &self.imports {
            match import {
                CanonicalImport::Instance(v) => {}
                CanonicalImport::Type(wasi) => match wasi {
                    WasiType::Integer8 { .. } => {}
                    WasiType::Integer16 { .. } => {}
                    WasiType::Integer32 { .. } => {}
                    WasiType::Integer64 { .. } => {}
                    WasiType::Option { .. } => {}
                    WasiType::Result { .. } => {}
                    WasiType::Resource(_) => {}
                    WasiType::Variant(v) => {
                        out.push_str(&format!("    subgraph \"{}\"\n", v.symbol));
                        // for item in v.variants.keys() {
                        //     println!("    {:#}::{}[\"{}\"]:::variant-item", v.symbol, item, v.wasi_name);
                        // }
                    }
                    WasiType::TypeHandler { .. } => {}
                    WasiType::Array { .. } => {}
                    WasiType::TypeAlias { .. } => {}
                    WasiType::External(_) => {}
                },
                CanonicalImport::MockMemory => {}
            }
        }

        for import in &self.imports {
            match import {
                CanonicalImport::Instance(v) => {
                    out.push_str(&format!("    subgraph \"{}\"\n", v.module));
                    for wasi in v.resources.values() {
                        out.push_str(&format!("        {:#}[\"{}\"]:::resource\n", wasi.symbol, wasi.wasi_name));
                    }
                    for wasi in v.functions.values() {
                        out.push_str(&format!("        {:#}[\"{}\"]:::function\n", wasi.symbol, wasi.wasi_name));
                    }
                    for wasi in v.functions.values() {
                        let mut types = vec![];
                        wasi.collect_wasi_types(&self.graph, &mut types);
                        for ty in types {
                            match ty.language_id() {
                                None => {}
                                Some(s) => {
                                    out.push_str(&format!("        {:#} -.-> {:#}\n", s, wasi.symbol));
                                }
                            }
                        }
                    }
                    out.push_str("    end\n");
                }
                CanonicalImport::Type(WasiType::Variant(v)) => {
                    let mut types = vec![];
                    v.collect_wasi_types(&self.graph, &mut types);
                    for ty in types {
                        match ty.language_id() {
                            Some(lhs) => {
                                out.push_str(&format!("    {:#} -.-> {:#}\n", lhs, v.symbol));
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }
}

pub enum CanonicalImport {
    MockMemory,
    Instance(WasiInstance),
    Type(WasiType),
}

impl Debug for CanonicalImport {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MockMemory => f.write_str("MockMemory"),
            Self::Instance(v) => Debug::fmt(v, f),
            Self::Type(v) => Debug::fmt(v, f),
        }
    }
}

impl Default for CanonicalWasi {
    fn default() -> Self {
        Self { name: Arc::from("root"), graph: Default::default(), imports: vec![], type_signatures: true, indent_text: "    " }
    }
}

impl AddAssign<WasiInstance> for CanonicalWasi {
    fn add_assign(&mut self, rhs: WasiInstance) {
        self.imports.push(CanonicalImport::Instance(rhs));
    }
}

impl LowerFunction for CanonicalImport {
    fn lower_function<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            CanonicalImport::MockMemory => {}
            CanonicalImport::Instance(v) => {
                for x in v.functions.values() {
                    w.newline()?;
                    x.lower_function(w)?;
                }
            }
            CanonicalImport::Type(_) => {}
        }
        Ok(())
    }
    fn lower_function_import<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        match self {
            CanonicalImport::MockMemory => {}
            CanonicalImport::Instance(v) => {
                for x in v.functions.values() {
                    w.newline()?;
                    x.lower_function_import(w)?;
                }
            }
            CanonicalImport::Type(_) => {}
        }
        Ok(())
    }
}

impl CanonicalWasi {
    pub fn new(graph: DependentGraph) -> Result<Self, NyarError> {
        let dag = match graph.resolve_imports() {
            Ok(o) => o,
            Err(e) => Err(NyarError::custom("graph error"))?,
        };
        let mut this = CanonicalWasi::default();
        this.graph = graph;
        this.imports.push(CanonicalImport::MockMemory);
        this.imports.extend(dag);
        Ok(this)
    }
    pub fn add_instance(&mut self, instance: WasiInstance) {
        self.imports.push(CanonicalImport::Instance(instance));
    }
    pub fn encode(&self) -> String {
        let mut output = String::with_capacity(1024);
        let mut encoder = WastEncoder::new(&self, &mut output);
        encoder.encode().unwrap();
        output
    }
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub fn new(source: &'a CanonicalWasi, writer: W) -> Self {
        Self { source, writer, indent: 0 }
    }
    pub fn with_indent_text(self, text: &'static str) -> Self {
        Self { ..self }
    }
}

impl<'a, W: Write> Write for WastEncoder<'a, W> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.writer.write_str(s)
    }
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub fn encode(&mut self) -> std::fmt::Result {
        write!(self.writer, "(component ${}", self.source.name)?;
        self.indent();
        for import in &self.source.imports {
            self.newline()?;
            import.component_define(self)?;
        }
        for import in &self.source.imports {
            import.lower_function(self)?;
        }
        {
            self.newline()?;
            write!(self, "(core module $Main")?;
            self.indent();

            for import in &self.source.imports {
                self.newline()?;
                import.lower_function_import(self)?;
            }

            self.dedent(1);
        }

        self.dedent(1);
        Ok(())
    }
    pub fn indent(&mut self) {
        self.indent += 1;
    }
    pub fn dedent(&mut self, end: usize) {
        self.indent -= 1;
        self.newline().ok();
        for _ in 0..end {
            self.write_char(')').ok();
        }
    }
    pub fn newline(&mut self) -> std::fmt::Result {
        self.write_str("\n")?;
        let range = (0..self.indent).into_iter();
        for _ in range {
            let indent = self.source.indent_text.as_ref();
            self.writer.write_str(indent)?;
        }
        Ok(())
    }
}

pub fn encode_id(id: &str) -> String {
    let mut alloc = String::with_capacity(id.len() + 1);
    alloc.push('$');
    make_kebab(id, &mut alloc);
    alloc
}

pub fn encode_kebab(id: &str) -> String {
    let mut alloc = String::with_capacity(id.len() + 2);
    alloc.push('"');
    make_kebab(id, &mut alloc);
    alloc.push('"');
    alloc
}

fn make_kebab(input: &str, buffer: &mut String) {
    for c in input.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '-' | ':' | '@' | '/' => buffer.push(c),
            _ => buffer.push('-'),
        }
    }
}

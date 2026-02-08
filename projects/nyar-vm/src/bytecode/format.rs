use byteorder::{LittleEndian, ReadBytesExt};
use nyar_types::{QualifiedName, SourceLocation};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum Constant {
    Int(i64),
    Float(f64),
    String(String),
    QualifiedName(QualifiedName),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Handler {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub locals: u16,
    pub upvalues: u16,
    pub max_stack: u16,
    pub code: Vec<u8>,
    pub handlers: Vec<Handler>,
    #[serde(default)]
    pub lines: Vec<(u32, u32)>, // offset, line
    #[serde(skip)]
    pub decoded: std::sync::OnceLock<std::sync::Arc<Vec<(crate::bytecode::instruction::Instruction, u32)>>>,
    #[serde(skip)]
    pub hotness: std::sync::atomic::AtomicU32,
}

impl Clone for Chunk {
    fn clone(&self) -> Self {
        let decoded = std::sync::OnceLock::new();
        if let Some(d) = self.decoded.get() {
            let _ = decoded.set(d.clone());
        }
        Self {
            locals: self.locals,
            upvalues: self.upvalues,
            max_stack: self.max_stack,
            code: self.code.clone(),
            handlers: self.handlers.clone(),
            lines: self.lines.clone(),
            decoded,
            hotness: std::sync::atomic::AtomicU32::new(
                self.hotness.load(std::sync::atomic::Ordering::Relaxed),
            ),
        }
    }
}

impl PartialEq for Chunk {
    fn eq(&self, other: &Self) -> bool {
        self.locals == other.locals
            && self.upvalues == other.upvalues
            && self.max_stack == other.max_stack
            && self.code == other.code
            && self.handlers == other.handlers
            && self.lines == other.lines
            && self.decoded.get() == other.decoded.get()
            && self.hotness.load(std::sync::atomic::Ordering::Relaxed)
                == other.hotness.load(std::sync::atomic::Ordering::Relaxed)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassInfo {
    pub name: QualifiedName,
    pub fields: Vec<String>,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraitInfo {
    pub name: QualifiedName,
    pub methods: Vec<String>,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImplInfo {
    pub class_idx: u16,
    pub trait_idx: u16,
    pub methods: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportInfo {
    pub provider: String,
    pub symbol: QualifiedName,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportInfo {
    pub symbol: QualifiedName,
    pub chunk_idx: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NyarcModule {
    pub version: u16,
    pub flags: u32,
    pub timestamp: u64,
    pub constants: Vec<Constant>,
    pub effects: Vec<QualifiedName>,
    pub chunks: Vec<Chunk>,
    #[serde(default)]
    pub classes: Vec<ClassInfo>,
    #[serde(default)]
    pub traits: Vec<TraitInfo>,
    #[serde(default)]
    pub impls: Vec<ImplInfo>,
    #[serde(default)]
    pub imports: Vec<ImportInfo>,
    #[serde(default)]
    pub exports: Vec<ExportInfo>,
}

pub type NyarModule = NyarcModule;

pub use nyar_types::FormatError;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "clap", derive(clap::ValueEnum))]
pub enum EmitTarget {
    /// Nyar Binary module (.nyar)
    Nyar,
    /// Nyar TOML module (.nyar.toml)
    NyarToml,
    /// Nyar JSON module (.nyar.json)
    NyarJson,
    /// WASM component (.wasm)
    Wasm,
    /// UIR as JSON
    Json,
    /// Tokens
    Tokens,
    /// UIR EGraph debug output
    Uir,
}

impl EmitTarget {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Nyar => "nyar",
            Self::NyarToml => "nyar.toml",
            Self::NyarJson => "nyar.json",
            Self::Wasm => "wasm",
            Self::Json => "json",
            Self::Tokens => "tokens",
            Self::Uir => "uir",
        }
    }
}

pub fn write_string(buf: &mut Vec<u8>, s: &str) {
    let l = s.len() as u32;
    buf.extend_from_slice(&l.to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
}

pub fn write_qualified_name(buf: &mut Vec<u8>, qn: &QualifiedName) {
    let l = qn.parts.len() as u32;
    buf.extend_from_slice(&l.to_le_bytes());
    for p in &qn.parts {
        write_string(buf, p);
    }
}

pub fn read_string(cur: &mut Cursor<&[u8]>) -> Result<String, FormatError> {
    let l = cur
        .read_u32::<LittleEndian>()
        .map_err(|_| FormatError::Truncated)? as usize;
    let mut buf = vec![0u8; l];
    cur.read_exact(&mut buf)
        .map_err(|_| FormatError::Truncated)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

pub fn read_qualified_name(cur: &mut Cursor<&[u8]>) -> Result<QualifiedName, FormatError> {
    let count = cur
        .read_u32::<LittleEndian>()
        .map_err(|_| FormatError::Truncated)? as usize;
    let mut parts = Vec::with_capacity(count);
    for _ in 0..count {
        parts.push(read_string(cur)?);
    }
    Ok(QualifiedName::new(parts))
}

impl Default for Chunk {
    fn default() -> Self {
        Self {
            locals: 0,
            upvalues: 0,
            max_stack: 0,
            code: vec![],
            handlers: vec![],
            lines: vec![],
            decoded: std::sync::OnceLock::new(),
            hotness: std::sync::atomic::AtomicU32::new(0),
        }
    }
}

impl Default for NyarcModule {
    fn default() -> Self {
        Self {
            version: 1,
            flags: 0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            constants: vec![],
            effects: vec![],
            chunks: vec![],
            classes: vec![],
            traits: vec![],
            impls: vec![],
            imports: vec![],
            exports: vec![],
        }
    }
}

impl NyarcModule {
    pub fn parse(b: &[u8]) -> Result<Self, FormatError> {
        if b.len() < 8 {
            return Err(FormatError::InvalidHeader);
        }
        if &b[0..8] != b"NYAR\x01\x00\x00\x00" {
            return Err(FormatError::InvalidHeader);
        }
        let mut cur = Cursor::new(&b[8..]);
        let version = cur
            .read_u16::<LittleEndian>()
            .map_err(|_| FormatError::Truncated)?;
        let flags = cur
            .read_u32::<LittleEndian>()
            .map_err(|_| FormatError::Truncated)?;
        let timestamp = cur
            .read_u64::<LittleEndian>()
            .map_err(|_| FormatError::Truncated)?;
        let const_count = cur
            .read_u32::<LittleEndian>()
            .map_err(|_| FormatError::Truncated)? as usize;
        let mut constants = Vec::with_capacity(const_count);
        for _ in 0..const_count {
            let kind = cur.read_u8().map_err(|_| FormatError::Truncated)?;
            match kind {
                0 => {
                    let v = cur
                        .read_u64::<LittleEndian>()
                        .map_err(|_| FormatError::Truncated)? as i64;
                    constants.push(Constant::Int(v));
                }
                1 => {
                    let raw = cur
                        .read_u64::<LittleEndian>()
                        .map_err(|_| FormatError::Truncated)?;
                    constants.push(Constant::Float(f64::from_bits(raw)));
                }
                2 => {
                    let s = read_string(&mut cur)?;
                    constants.push(Constant::String(s));
                }
                3 => {
                    let qn = read_qualified_name(&mut cur)?;
                    constants.push(Constant::QualifiedName(qn));
                }
                _ => return Err(FormatError::Truncated),
            }
        }
        let eff_count = cur
            .read_u32::<LittleEndian>()
            .map_err(|_| FormatError::Truncated)? as usize;
        let mut effects = Vec::with_capacity(eff_count);
        for _ in 0..eff_count {
            effects.push(read_qualified_name(&mut cur)?);
        }
        let chunk_count = cur
            .read_u32::<LittleEndian>()
            .map_err(|_| FormatError::Truncated)? as usize;
        let mut chunks = Vec::with_capacity(chunk_count);
        for _ in 0..chunk_count {
            let locals = cur
                .read_u16::<LittleEndian>()
                .map_err(|_| FormatError::Truncated)?;
            let upvalues = cur
                .read_u16::<LittleEndian>()
                .map_err(|_| FormatError::Truncated)?;
            let max_stack = cur
                .read_u16::<LittleEndian>()
                .map_err(|_| FormatError::Truncated)?;
            let code_size = cur
                .read_u32::<LittleEndian>()
                .map_err(|_| FormatError::Truncated)? as usize;
            let mut code = vec![0u8; code_size];
            cur.read_exact(&mut code)
                .map_err(|_| FormatError::Truncated)?;

            let line_count = cur
                .read_u32::<LittleEndian>()
                .map_err(|_| FormatError::Truncated)? as usize;
            let mut lines = Vec::with_capacity(line_count);
            for _ in 0..line_count {
                let off = cur
                    .read_u32::<LittleEndian>()
                    .map_err(|_| FormatError::Truncated)?;
                let line = cur
                    .read_u32::<LittleEndian>()
                    .map_err(|_| FormatError::Truncated)?;
                lines.push((off, line));
            }

            chunks.push(Chunk {
                locals,
                upvalues,
                max_stack,
                code,
                handlers: vec![],
                lines,
                decoded: std::sync::OnceLock::new(),
                hotness: std::sync::atomic::AtomicU32::new(0),
            });
        }

        let mut classes = Vec::new();
        if cur.position() < cur.get_ref().len() as u64 {
            let class_count = cur
                .read_u32::<LittleEndian>()
                .map_err(|_| FormatError::Truncated)? as usize;
            for _ in 0..class_count {
                let name = read_qualified_name(&mut cur)?;
                let field_count = cur
                    .read_u16::<LittleEndian>()
                    .map_err(|_| FormatError::Truncated)?;
                let mut fields = Vec::with_capacity(field_count as usize);
                for _ in 0..field_count {
                    fields.push(read_string(&mut cur)?);
                }
                classes.push(ClassInfo { name, fields, location: SourceLocation::default() });
            }
        }

        let mut traits = Vec::new();
        if cur.position() < cur.get_ref().len() as u64 {
            let trait_count = cur
                .read_u32::<LittleEndian>()
                .map_err(|_| FormatError::Truncated)? as usize;
            for _ in 0..trait_count {
                let name = read_qualified_name(&mut cur)?;
                let method_count = cur
                    .read_u16::<LittleEndian>()
                    .map_err(|_| FormatError::Truncated)?;
                let mut methods = Vec::with_capacity(method_count as usize);
                for _ in 0..method_count {
                    methods.push(read_string(&mut cur)?);
                }
                traits.push(TraitInfo { name, methods, location: SourceLocation::default() });
            }
        }

        let mut impls = Vec::new();
        if cur.position() < cur.get_ref().len() as u64 {
            let impl_count = cur
                .read_u32::<LittleEndian>()
                .map_err(|_| FormatError::Truncated)? as usize;
            for _ in 0..impl_count {
                let class_idx = cur
                    .read_u16::<LittleEndian>()
                    .map_err(|_| FormatError::Truncated)?;
                let trait_idx = cur
                    .read_u16::<LittleEndian>()
                    .map_err(|_| FormatError::Truncated)?;
                let method_count = cur
                    .read_u16::<LittleEndian>()
                    .map_err(|_| FormatError::Truncated)?;
                let mut methods = Vec::with_capacity(method_count as usize);
                for _ in 0..method_count {
                    let chunk_idx = cur
                        .read_u16::<LittleEndian>()
                        .map_err(|_| FormatError::Truncated)?;
                    methods.push(chunk_idx);
                }
                impls.push(ImplInfo {
                    class_idx,
                    trait_idx,
                    methods,
                });
            }
        }

        let mut imports = Vec::new();
        if cur.position() < cur.get_ref().len() as u64 {
            let import_count = cur
                .read_u32::<LittleEndian>()
                .map_err(|_| FormatError::Truncated)? as usize;
            for _ in 0..import_count {
                let provider = read_string(&mut cur)?;
                let symbol = read_qualified_name(&mut cur)?;
                imports.push(ImportInfo { provider, symbol });
            }
        }

        let mut exports = Vec::new();
        if cur.position() < cur.get_ref().len() as u64 {
            let export_count = cur
                .read_u32::<LittleEndian>()
                .map_err(|_| FormatError::Truncated)? as usize;
            for _ in 0..export_count {
                let symbol = read_qualified_name(&mut cur)?;
                let chunk_idx = cur
                    .read_u16::<LittleEndian>()
                    .map_err(|_| FormatError::Truncated)?;
                exports.push(ExportInfo { symbol, chunk_idx });
            }
        }

        Ok(Self {
            version,
            flags,
            timestamp,
            constants,
            effects,
            chunks,
            classes,
            traits,
            impls,
            imports,
            exports,
        })
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(b"NYAR\x01\x00\x00\x00");
        buf.extend_from_slice(&self.version.to_le_bytes());
        buf.extend_from_slice(&self.flags.to_le_bytes());
        buf.extend_from_slice(&self.timestamp.to_le_bytes());
        buf.extend_from_slice(&(self.constants.len() as u32).to_le_bytes());
        for c in &self.constants {
            match c {
                Constant::Int(v) => {
                    buf.push(0);
                    buf.extend_from_slice(&(*v as u64).to_le_bytes());
                }
                Constant::Float(v) => {
                    buf.push(1);
                    buf.extend_from_slice(&v.to_bits().to_le_bytes());
                }
                Constant::String(s) => {
                    buf.push(2);
                    write_string(&mut buf, s);
                }
                Constant::QualifiedName(qn) => {
                    buf.push(3);
                    write_qualified_name(&mut buf, qn);
                }
            }
        }
        buf.extend_from_slice(&(self.effects.len() as u32).to_le_bytes());
        for e in &self.effects {
            write_qualified_name(&mut buf, e);
        }
        buf.extend_from_slice(&(self.chunks.len() as u32).to_le_bytes());
        for ch in &self.chunks {
            buf.extend_from_slice(&ch.locals.to_le_bytes());
            buf.extend_from_slice(&ch.upvalues.to_le_bytes());
            buf.extend_from_slice(&ch.max_stack.to_le_bytes());
            buf.extend_from_slice(&(ch.code.len() as u32).to_le_bytes());
            buf.extend_from_slice(&ch.code);
            buf.extend_from_slice(&(ch.lines.len() as u32).to_le_bytes());
            for (off, line) in &ch.lines {
                buf.extend_from_slice(&off.to_le_bytes());
                buf.extend_from_slice(&line.to_le_bytes());
            }
        }

        buf.extend_from_slice(&(self.classes.len() as u32).to_le_bytes());
        for c in &self.classes {
            write_qualified_name(&mut buf, &c.name);
            buf.extend_from_slice(&(c.fields.len() as u16).to_le_bytes());
            for f in &c.fields {
                write_string(&mut buf, f);
            }
        }

        buf.extend_from_slice(&(self.traits.len() as u32).to_le_bytes());
        for t in &self.traits {
            write_qualified_name(&mut buf, &t.name);
            buf.extend_from_slice(&(t.methods.len() as u16).to_le_bytes());
            for m in &t.methods {
                write_string(&mut buf, m);
            }
        }

        buf.extend_from_slice(&(self.impls.len() as u32).to_le_bytes());
        for i in &self.impls {
            buf.extend_from_slice(&i.class_idx.to_le_bytes());
            buf.extend_from_slice(&i.trait_idx.to_le_bytes());
            buf.extend_from_slice(&(i.methods.len() as u16).to_le_bytes());
            for m in &i.methods {
                buf.extend_from_slice(&m.to_le_bytes());
            }
        }

        buf.extend_from_slice(&(self.imports.len() as u32).to_le_bytes());
        for i in &self.imports {
            write_string(&mut buf, &i.provider);
            write_qualified_name(&mut buf, &i.symbol);
        }

        buf.extend_from_slice(&(self.exports.len() as u32).to_le_bytes());
        for e in &self.exports {
            write_qualified_name(&mut buf, &e.symbol);
            buf.extend_from_slice(&e.chunk_idx.to_le_bytes());
        }

        buf
    }
    pub fn parse_toml_str(s: &str) -> Result<Self, FormatError> {
        toml::from_str::<NyarcModule>(s).map_err(|e| FormatError::Text(e.to_string()))
    }
}

pub fn minimal_module_with_chunk(code: Vec<u8>, constants: Vec<Constant>) -> NyarcModule {
    NyarcModule {
        constants,
        chunks: vec![Chunk {
            max_stack: 8,
            code,
            ..Default::default()
        }],
        ..Default::default()
    }
}

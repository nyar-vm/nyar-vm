use byteorder::{LittleEndian, ReadBytesExt};
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum Constant {
    Int(i64),
    Float(f64),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Handler {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chunk {
    pub locals: u16,
    pub upvalues: u16,
    pub max_stack: u16,
    pub code: Vec<u8>,
    pub handlers: Vec<Handler>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassInfo {
    pub name: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraitInfo {
    pub name: String,
    pub methods: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImplInfo {
    pub class_idx: u16,
    pub trait_idx: u16,
    pub methods: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NyarcModule {
    pub version: u16,
    pub flags: u32,
    pub timestamp: u64,
    pub constants: Vec<Constant>,
    pub effects: Vec<String>,
    pub chunks: Vec<Chunk>,
    #[serde(default)]
    pub classes: Vec<ClassInfo>,
    #[serde(default)]
    pub traits: Vec<TraitInfo>,
    #[serde(default)]
    pub impls: Vec<ImplInfo>,
}

pub type NyarModule = NyarcModule;

pub use nyar_error::FormatError;

pub fn write_string(buf: &mut Vec<u8>, s: &str) {
    let l = s.len() as u32;
    buf.extend_from_slice(&l.to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
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
                    let l = cur
                        .read_u32::<LittleEndian>()
                        .map_err(|_| FormatError::Truncated)? as usize;
                    let mut buf = vec![0u8; l];
                    cur.read_exact(&mut buf)
                        .map_err(|_| FormatError::Truncated)?;
                    let s = String::from_utf8_lossy(&buf).into_owned();
                    constants.push(Constant::String(s));
                }
                _ => return Err(FormatError::Truncated),
            }
        }
        let eff_count = cur
            .read_u32::<LittleEndian>()
            .map_err(|_| FormatError::Truncated)? as usize;
        let mut effects = Vec::with_capacity(eff_count);
        for _ in 0..eff_count {
            let l = cur
                .read_u32::<LittleEndian>()
                .map_err(|_| FormatError::Truncated)? as usize;
            let mut buf = vec![0u8; l];
            cur.read_exact(&mut buf)
                .map_err(|_| FormatError::Truncated)?;
            effects.push(String::from_utf8_lossy(&buf).into_owned());
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
            chunks.push(Chunk {
                locals,
                upvalues,
                max_stack,
                code,
                handlers: vec![],
            });
        }
        
        let mut classes = Vec::new();
        if cur.position() < cur.get_ref().len() as u64 {
             let class_count = cur.read_u32::<LittleEndian>().map_err(|_| FormatError::Truncated)? as usize;
             for _ in 0..class_count {
                 let name_len = cur.read_u32::<LittleEndian>().map_err(|_| FormatError::Truncated)? as usize;
                 let mut buf = vec![0u8; name_len];
                 cur.read_exact(&mut buf).map_err(|_| FormatError::Truncated)?;
                 let name = String::from_utf8_lossy(&buf).into_owned();
                 let field_count = cur.read_u16::<LittleEndian>().map_err(|_| FormatError::Truncated)?;
                 let mut fields = Vec::with_capacity(field_count as usize);
                 for _ in 0..field_count {
                     let flen = cur.read_u32::<LittleEndian>().map_err(|_| FormatError::Truncated)? as usize;
                     let mut fbuf = vec![0u8; flen];
                     cur.read_exact(&mut fbuf).map_err(|_| FormatError::Truncated)?;
                     fields.push(String::from_utf8_lossy(&fbuf).into_owned());
                 }
                 classes.push(ClassInfo { name, fields });
             }
        }
        
        let mut traits = Vec::new();
        if cur.position() < cur.get_ref().len() as u64 {
             let trait_count = cur.read_u32::<LittleEndian>().map_err(|_| FormatError::Truncated)? as usize;
             for _ in 0..trait_count {
                 let name_len = cur.read_u32::<LittleEndian>().map_err(|_| FormatError::Truncated)? as usize;
                 let mut buf = vec![0u8; name_len];
                 cur.read_exact(&mut buf).map_err(|_| FormatError::Truncated)?;
                 let name = String::from_utf8_lossy(&buf).into_owned();
                 let method_count = cur.read_u16::<LittleEndian>().map_err(|_| FormatError::Truncated)?;
                 let mut methods = Vec::with_capacity(method_count as usize);
                 for _ in 0..method_count {
                     let mlen = cur.read_u32::<LittleEndian>().map_err(|_| FormatError::Truncated)? as usize;
                     let mut mbuf = vec![0u8; mlen];
                     cur.read_exact(&mut mbuf).map_err(|_| FormatError::Truncated)?;
                     methods.push(String::from_utf8_lossy(&mbuf).into_owned());
                 }
                 traits.push(TraitInfo { name, methods });
             }
        }

        let mut impls = Vec::new();
        if cur.position() < cur.get_ref().len() as u64 {
             let impl_count = cur.read_u32::<LittleEndian>().map_err(|_| FormatError::Truncated)? as usize;
             for _ in 0..impl_count {
                 let class_idx = cur.read_u16::<LittleEndian>().map_err(|_| FormatError::Truncated)?;
                 let trait_idx = cur.read_u16::<LittleEndian>().map_err(|_| FormatError::Truncated)?;
                 let method_count = cur.read_u16::<LittleEndian>().map_err(|_| FormatError::Truncated)?;
                 let mut methods = Vec::with_capacity(method_count as usize);
                 for _ in 0..method_count {
                     let chunk_idx = cur.read_u16::<LittleEndian>().map_err(|_| FormatError::Truncated)?;
                     methods.push(chunk_idx);
                 }
                 impls.push(ImplInfo { class_idx, trait_idx, methods });
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
            }
        }
        buf.extend_from_slice(&(self.effects.len() as u32).to_le_bytes());
        for e in &self.effects {
            write_string(&mut buf, e);
        }
        buf.extend_from_slice(&(self.chunks.len() as u32).to_le_bytes());
        for ch in &self.chunks {
            buf.extend_from_slice(&ch.locals.to_le_bytes());
            buf.extend_from_slice(&ch.upvalues.to_le_bytes());
            buf.extend_from_slice(&ch.max_stack.to_le_bytes());
            buf.extend_from_slice(&(ch.code.len() as u32).to_le_bytes());
            buf.extend_from_slice(&ch.code);
        }
        
        if !self.classes.is_empty() || !self.traits.is_empty() || !self.impls.is_empty() {
             buf.extend_from_slice(&(self.classes.len() as u32).to_le_bytes());
             for c in &self.classes {
                 write_string(&mut buf, &c.name);
                 buf.extend_from_slice(&(c.fields.len() as u16).to_le_bytes());
                 for f in &c.fields {
                     write_string(&mut buf, f);
                 }
             }
        }
        
        if !self.traits.is_empty() || !self.impls.is_empty() {
             buf.extend_from_slice(&(self.traits.len() as u32).to_le_bytes());
             for t in &self.traits {
                 write_string(&mut buf, &t.name);
                 buf.extend_from_slice(&(t.methods.len() as u16).to_le_bytes());
                 for m in &t.methods {
                     write_string(&mut buf, m);
                 }
             }
        }
        
        if !self.impls.is_empty() {
             buf.extend_from_slice(&(self.impls.len() as u32).to_le_bytes());
             for i in &self.impls {
                 buf.extend_from_slice(&i.class_idx.to_le_bytes());
                 buf.extend_from_slice(&i.trait_idx.to_le_bytes());
                 buf.extend_from_slice(&(i.methods.len() as u16).to_le_bytes());
                 for m in &i.methods {
                     buf.extend_from_slice(&m.to_le_bytes());
                 }
             }
        }
        
        buf
    }
    pub fn parse_toml_str(s: &str) -> Result<Self, FormatError> {
        toml::from_str::<NyarcModule>(s).map_err(|e| FormatError::Text(e.to_string()))
    }
}

pub fn minimal_module_with_chunk(code: Vec<u8>, constants: Vec<Constant>) -> NyarcModule {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    NyarcModule {
        version: 1,
        flags: 0,
        timestamp: ts,
        constants,
        effects: vec![],
        chunks: vec![Chunk {
            locals: 0,
            upvalues: 0,
            max_stack: 8,
            code,
            handlers: vec![],
        }],
        classes: vec![],
        traits: vec![],
        impls: vec![],
    }
}

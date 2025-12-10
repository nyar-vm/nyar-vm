use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Int(i64),
    Float(f64),
    String(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Handler {}

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub locals: u16,
    pub upvalues: u16,
    pub max_stack: u16,
    pub code: Vec<u8>,
    pub handlers: Vec<Handler>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NyarcModule {
    pub version: u16,
    pub flags: u32,
    pub timestamp: u64,
    pub constants: Vec<Constant>,
    pub effects: Vec<String>,
    pub chunks: Vec<Chunk>,
}

#[derive(Debug)]
pub enum FormatError {
    InvalidHeader,
    Truncated,
}

fn read_u16(b: &[u8], i: &mut usize) -> Option<u16> {
    if *i + 1 >= b.len() {
        None
    } else {
        let v = u16::from_le_bytes([b[*i], b[*i + 1]]);
        *i += 2;
        Some(v)
    }
}
fn read_u32(b: &[u8], i: &mut usize) -> Option<u32> {
    if *i + 3 >= b.len() {
        None
    } else {
        let v = u32::from_le_bytes([b[*i], b[*i + 1], b[*i + 2], b[*i + 3]]);
        *i += 4;
        Some(v)
    }
}
fn read_u64(b: &[u8], i: &mut usize) -> Option<u64> {
    if *i + 7 >= b.len() {
        None
    } else {
        let v = u64::from_le_bytes([
            b[*i],
            b[*i + 1],
            b[*i + 2],
            b[*i + 3],
            b[*i + 4],
            b[*i + 5],
            b[*i + 6],
            b[*i + 7],
        ]);
        *i += 8;
        Some(v)
    }
}

fn read_len<'a>(b: &'a [u8], i: &mut usize, n: usize) -> Option<&'a [u8]> {
    if *i + n > b.len() {
        None
    } else {
        let s = &b[*i..*i + n];
        *i += n;
        Some(s)
    }
}

pub fn write_string(buf: &mut Vec<u8>, s: &str) {
    let l = s.len() as u32;
    buf.extend_from_slice(&l.to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
}
fn read_string(b: &[u8], i: &mut usize) -> Option<String> {
    let l = read_u32(b, i)? as usize;
    let s = read_len(b, i, l)?;
    Some(String::from_utf8_lossy(s).into_owned())
}

impl NyarcModule {
    pub fn parse(b: &[u8]) -> Result<Self, FormatError> {
        let mut i = 0usize;
        if b.len() < 8 {
            return Err(FormatError::InvalidHeader);
        }
        if &b[0..8] != b"NYAR\x01\x00\x00\x00" {
            return Err(FormatError::InvalidHeader);
        }
        i = 8;
        let version = read_u16(b, &mut i).ok_or(FormatError::Truncated)?;
        let flags = read_u32(b, &mut i).ok_or(FormatError::Truncated)?;
        let timestamp = read_u64(b, &mut i).ok_or(FormatError::Truncated)?;
        let const_count = read_u32(b, &mut i).ok_or(FormatError::Truncated)? as usize;
        let mut constants = Vec::with_capacity(const_count);
        for _ in 0..const_count {
            let kind = b.get(i).copied().ok_or(FormatError::Truncated)?;
            i += 1;
            match kind {
                0 => {
                    let v = read_u64(b, &mut i).ok_or(FormatError::Truncated)? as i64;
                    constants.push(Constant::Int(v));
                }
                1 => {
                    let raw = read_u64(b, &mut i).ok_or(FormatError::Truncated)?;
                    constants.push(Constant::Float(f64::from_bits(raw)));
                }
                2 => {
                    let s = read_string(b, &mut i).ok_or(FormatError::Truncated)?;
                    constants.push(Constant::String(s));
                }
                _ => return Err(FormatError::Truncated),
            }
        }
        let eff_count = read_u32(b, &mut i).ok_or(FormatError::Truncated)? as usize;
        let mut effects = Vec::with_capacity(eff_count);
        for _ in 0..eff_count {
            let s = read_string(b, &mut i).ok_or(FormatError::Truncated)?;
            effects.push(s);
        }
        let chunk_count = read_u32(b, &mut i).ok_or(FormatError::Truncated)? as usize;
        let mut chunks = Vec::with_capacity(chunk_count);
        for _ in 0..chunk_count {
            let locals = read_u16(b, &mut i).ok_or(FormatError::Truncated)?;
            let upvalues = read_u16(b, &mut i).ok_or(FormatError::Truncated)?;
            let max_stack = read_u16(b, &mut i).ok_or(FormatError::Truncated)?;
            let code_size = read_u32(b, &mut i).ok_or(FormatError::Truncated)? as usize;
            let code = read_len(b, &mut i, code_size)
                .ok_or(FormatError::Truncated)?
                .to_vec();
            chunks.push(Chunk {
                locals,
                upvalues,
                max_stack,
                code,
                handlers: vec![],
            });
        }
        Ok(Self {
            version,
            flags,
            timestamp,
            constants,
            effects,
            chunks,
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
        buf
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
    }
}

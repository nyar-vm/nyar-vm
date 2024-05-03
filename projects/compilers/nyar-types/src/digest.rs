//! 跨阶段可复现的内容摘要。

use sha2::{Digest, Sha256};

/// 将若干 UTF-8 片段用 `\0` 连接后做 SHA-256，返回小写十六进制。
pub fn combined_content_hash(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            hasher.update([0u8]);
        }
        hasher.update(part.as_bytes());
    }
    hex_encode(hasher.finalize().as_slice())
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

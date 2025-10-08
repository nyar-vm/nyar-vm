// Rust-GC 示例语言

export interface RustProgram {
    type: 'Program';
    items: RustStatement[];
}

export interface RustStatement {
    type: 'Function';
}

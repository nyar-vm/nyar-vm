//! 辅助 `micro` 导出片段（memo / 内联 handler 等）。

/// 收集并在 hydrate/mount 函数之前刷出的辅助导出。
pub(crate) struct AuxExports {
    /// 已注册的辅助 `micro` 源码块。
    pub items: Vec<String>,
}

impl AuxExports {
    pub(crate) fn new(_route: &str) -> Self {
        Self { items: Vec::new() }
    }

    pub(crate) fn push(&mut self, item: impl Into<String>) {
        self.items.push(item.into());
    }

    pub(crate) fn flush(&self, out: &mut String) {
        for item in &self.items {
            out.push_str(item);
            if !item.ends_with('\n') {
                out.push('\n');
            }
        }
    }
}

//! 宿主控制台输出通道：默认写 stdout，测试可在当前线程捕获完整行。
//!
//! 捕获是线程局部的，测试并行运行时互不干扰；未安装捕获时行为与直接 `println!` 相同。

use std::cell::RefCell;

thread_local! {
    static CAPTURE: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// 输出一行（调用方自带换行语义，本函数只在无捕获时写 stdout 并追加换行）。
pub fn emit_line(line: &str) {
    CAPTURE.with(|capture| {
        let mut slot = capture.borrow_mut();
        match slot.as_mut() {
            Some(lines) => lines.push(line.to_string()),
            None => println!("{line}"),
        }
    });
}

/// 在当前线程捕获 [`emit_line`] 的输出，返回 `f` 的结果与按调用顺序记录的行。
///
/// 仅用于测试：安装后本线程所有输出都进入缓冲区，结束后恢复为直接写 stdout。
#[doc(hidden)]
pub fn capture<T>(f: impl FnOnce() -> T) -> (T, Vec<String>) {
    CAPTURE.with(|capture| *capture.borrow_mut() = Some(Vec::new()));
    let result = f();
    let lines = CAPTURE.with(|capture| capture.borrow_mut().take().unwrap_or_default());
    (result, lines)
}

//! 命令边界上的**纯文本**失败句（无 span、无 diagnostic code）。
//!
//! 库内传播用领域错误 + `?`；见 [`readme`](readme.md)。结构化事实只用 `boost_diagnostic`。

/// 最终写给用户/N-API 的短句（不是中间链节点）。
pub type UserMessage = String;

/// 构造用户可见短句（仅命令入口；库内优先返回 `PlannerError` 等）。
#[macro_export]
macro_rules! report {
    ($($fmt:tt)*) => {
        format!($($fmt)*)
    };
}

/// 同 `report!`，函数形式。
pub fn report_msg(message: impl Into<UserMessage>) -> UserMessage {
    message.into()
}

/// 命令 `main` 出口：把领域错误打成一行 stderr 文本。
pub fn display_failure(error: impl std::fmt::Display) -> UserMessage {
    error.to_string()
}

/// 表达式在父节点中的位置。
///
/// `Callee` 只用于 `Call` / `GenericApply` 的被调用者本身：那里的 `receiver.member`
/// 已经是成员调用的选择名，不得再按值位置成员访问改写一次。其余位置一律 `Value`。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExprPosition {
    Callee,
    Value,
}

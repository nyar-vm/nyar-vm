use super::*;


/// `switch {...}`
///
/// ```v
/// @head
/// switch {
///     @check.1
///     case 1:
///         @begin.1
///     @check.2
///     case 2:
///         @begin.2
/// }
/// @tail
/// ```
///
///
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwitchStatement {
    pub label_head: u32,
    pub label_tail: u32,
    pub branches: Vec<()>,
    pub default: Vec<()>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SwitchBranch {
    pub label_check: u32,
    pub label_begin: u32,
}

// if a {}
// if case A() := b {}
// let a = b;
// let b = a.s;
impl SwitchStatement {

}

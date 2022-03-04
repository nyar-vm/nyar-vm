use std::ops::Range;
use super::*;


pub struct ControlFlowCounter {
    c_loop: usize,
    c_for: usize,
    c_if: usize,
}

/// `loop { ... }`
///
/// ```v
/// @head
/// loop {
///     @begin
///     <body>
///     <continue = goto@begin>
///     <break    = goto@tail>
///     <return   = goto@return>
///     <yield    = goto@reurn>
///     @end
/// }
/// @tail
///
/// @return
/// ret()
/// @yield_break
/// unreachable();
/// ```
///
///
/// ```v
///
///
/// ```
#[derive(Clone, Debug)]
pub struct LoopSemantic {
    pub label_head: String,
    pub label_begin: String,
    pub label_end: String,
    pub label_tail: String,
    pub body_span: Range<u32>,
}

pub struct JumpSemantic {
    pub label: String,
    pub condition: Option<()>,
}

pub struct JumpInstruction {
    pub label: usize,
}
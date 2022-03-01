use std::ops::Range;
use super::*;


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
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoopStatement {
    pub label_head: u32,
    pub label_begin: u32,
    pub label_end: u32,
    pub label_tail: u32,
    pub body_span: Range<u32>,
}

pub struct GotoStatement {
    pub label: u32,
}
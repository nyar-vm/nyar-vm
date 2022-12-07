use super::*;

mod loop_each;
mod loop_until;
mod loop_while;
mod normal;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoopRepeat {
    pub label: Arc<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoopEach {
    pub label: Arc<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoopWhileBody {
    pub label: Arc<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoopUntilBody {
    pub label: Arc<str>,
}

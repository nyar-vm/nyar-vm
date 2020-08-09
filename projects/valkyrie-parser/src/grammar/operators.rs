use std::lazy::SyncLazy;
use valkyrie_pest::{Assoc::*, Operator, PrecClimber, Rule, Rule::*};

pub static PREC_CLIMBER: SyncLazy<PrecClimber<Rule>> = SyncLazy::new(|| {
    //TODO: use macro
    PrecClimber::new(vec![
        Operator::new(Set, Left),
        Operator::new(Plus, Left) | Operator::new(Minus, Left),
        Operator::new(Multiply, Left) | Operator::new(CenterDot, Left),
        Operator::new(Power, Right),
        Operator::new(Dot, Left),
    ])
});

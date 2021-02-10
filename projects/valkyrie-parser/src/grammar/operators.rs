use crate::Rule;
use pest::prec_climber::{
    Assoc::{Left, Right},
    Operator, PrecClimber,
};
use std::lazy::SyncLazy;

pub static PREC_CLIMBER: SyncLazy<PrecClimber<Rule>> = SyncLazy::new(|| {
    //TODO: use macro
    use crate::Rule::*;
    PrecClimber::new(vec![
        Operator::new(Set, Left),
        Operator::new(Plus, Left) | Operator::new(Minus, Left),
        Operator::new(Multiply, Left) | Operator::new(CenterDot, Left),
        Operator::new(Power, Right),
        Operator::new(Dot, Left),
    ])
});

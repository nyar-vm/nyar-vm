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
        Operator::new(CONCAT, Left) | Operator::new(REMOVE, Left),
        Operator::new(ADD, Left) | Operator::new(SUB, Left),
        Operator::new(Multiply, Left) | Operator::new(CenterDot, Left),
        Operator::new(Power, Right),
        Operator::new(Dot, Left),
    ])
});

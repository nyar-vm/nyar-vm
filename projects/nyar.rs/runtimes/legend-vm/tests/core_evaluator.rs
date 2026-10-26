//! `algebra::CoreEvaluator` integration tests.

use legend_vm::{LegacyValue, algebra::CoreEvaluator};

#[test]
fn arithmetic_and_loops() {
    let mut core = CoreEvaluator::new(None);
    core.set_var("x", core.int_const(0));
    let result = core.while_loop(
        |core| core.lt(&core.var("x"), &core.int_const(3)),
        |core| {
            let next = core.add(&core.var("x"), &core.int_const(1));
            core.set_var("x", next.clone());
            next
        },
    );
    assert_eq!(result, LegacyValue::Int(3));
}

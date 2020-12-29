use ::next_gen::prelude::*;

#[generator(yield(u8))]
fn range(start: u8, end: u8) {
    let mut current = start;
    while current < end {
        yield_!(current);
        current += 1;
    }
}

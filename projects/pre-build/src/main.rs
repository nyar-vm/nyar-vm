#[macro_use]
extern crate quote;
extern crate pest_generator;
extern crate proc_macro;

mod gen;

use gen::gen_valkyrie;

fn main() {
    gen_valkyrie()
}

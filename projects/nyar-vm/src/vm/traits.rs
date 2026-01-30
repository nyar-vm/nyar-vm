use crate::vm::value::WitnessTable;

pub fn get_witness_table(module_idx: usize, methods: Vec<u16>) -> WitnessTable {
    WitnessTable { module_idx, methods }
}

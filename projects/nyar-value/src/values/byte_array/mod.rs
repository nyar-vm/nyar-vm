use nyar_macro::sync_value_type;
use shredder::Scan;

#[derive(Clone, Debug)]
pub struct NyarBlob {
    inner: Vec<u8>,
}

sync_value_type!(NyarBlob);

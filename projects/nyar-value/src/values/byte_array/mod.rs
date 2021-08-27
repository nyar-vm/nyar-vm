use nyar_macro::sync_value_type;

#[derive(Clone, Debug)]
pub struct NyarBlob {
    inner: Vec<u8>,
}

sync_value_type!(NyarBlob);

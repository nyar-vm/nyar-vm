use crate::{
    helpers::{Id, IntoWasm},
    WasmSymbol,
};
use nyar_error::FileSpan;
use wast::{
    core::{Data, DataKind, DataVal, Expression, Instruction},
    token::{Index, NameAnnotation, Span},
};

pub struct DataItem {
    pub symbol: WasmSymbol,
    pub data: Vec<u8>,
    pub span: FileSpan,
}

impl DataItem {
    pub fn utf8(name: WasmSymbol, data: String) -> Self {
        Self { symbol: name, data: data.into_bytes(), span: FileSpan::default() }
    }
}

impl<'a, 'i> IntoWasm<'a, Data<'i>> for DataItem
where
    'a: 'i,
{
    fn as_wast(&'a self) -> Data<'i> {
        Data {
            span: Span::from_offset(0),
            id: Id::type_id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            kind: DataKind::Active {
                memory: Index::Id(Id::new("memory")),
                offset: Expression { instrs: Box::new([Instruction::I32Const(0)]) },
            },
            data: vec![DataVal::String(self.data.as_slice())],
        }
    }
}

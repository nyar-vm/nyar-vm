use crate::{helpers::WasmName, WasmSymbol};
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

    pub(crate) fn as_wast<'a, 'i>(&'a self, start: &mut usize) -> Data<'i>
    where
        'a: 'i,
    {
        let offset = Expression { instrs: Box::new([Instruction::I32Const(*start as i32)]) };
        *start += self.data.len();
        Data {
            span: Span::from_offset(0),
            id: WasmName::type_id(self.symbol.as_ref()),
            name: Some(NameAnnotation { name: self.symbol.as_ref() }),
            kind: DataKind::Active { memory: Index::Id(WasmName::new("memory")), offset },
            data: vec![DataVal::String(self.data.as_slice())],
        }
    }
}

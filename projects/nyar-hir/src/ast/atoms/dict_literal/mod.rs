use crate::{ASTNode};

#[derive(Debug, Clone)]
pub struct KVPair {
    pub k: ASTNode,
    pub v: ASTNode,
}

impl KVPair {
    pub fn key_name(&self) -> Option<String> {
       todo!()
    }
    // pub fn key_id(&self) -> Option<usize> {
    //     match &self.k.kind {
    //         ASTKind::NumberLiteral(s) => {
    //             s.
    //         },
    //         _ => None
    //     }
    // }
}

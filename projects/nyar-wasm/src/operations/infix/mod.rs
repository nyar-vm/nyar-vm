use super::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InfixCall {
    pub infix: InfixOperator,
    pub lhs: Vec<WasiInstruction>,
    pub rhs: Vec<WasiInstruction>,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InfixOperator {
    // %infix=
    Equal,
}

impl Emit for InfixCall {
    fn emit<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        for x in self.lhs.iter() {
            x.emit(w)?
        }
        let lt = match w.stack.pop() {
            Some(s) => s,
            None => {
                panic!()
            }
        };
        for x in self.rhs.iter() {
            x.emit(w)?
        }
        let rt = match w.stack.pop() {
            Some(s) => s,
            None => {
                panic!()
            }
        };
        todo!()
    }
}

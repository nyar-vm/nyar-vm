use super::*;
impl Emit for LoopUntilBody {
    fn emit<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        self.make_loop().emit(w)
    }
}
impl LoopUntilBody {
    pub fn make_loop(&self) -> LoopRepeat {
        LoopRepeat { label: self.label.clone() }
    }
}

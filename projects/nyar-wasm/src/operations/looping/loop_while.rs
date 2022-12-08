use super::*;
impl Emit for LoopWhileBody {
    fn emit<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        self.make_loop().emit(w)
    }
}
impl LoopWhileBody {
    pub fn make_loop(&self) -> LoopRepeat {
        LoopRepeat { label: self.label.clone(), body: vec![] }
    }
}

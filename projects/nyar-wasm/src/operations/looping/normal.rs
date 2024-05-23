use super::*;

// (loop $label%continue
//     (block $label%break
//
//     )
// )
impl Emit for LoopRepeat {
    fn emit<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.write_str("(loop $")?;
        w.write_str(self.label.as_ref())?;
        w.indent();
        w.write_str("(block $")?;
        w.write_str(self.label.as_ref())?;
        for x in self.body.iter() {
            x.emit(w)?
        }
        w.write_str(")")?;
        w.dedent(1);
        w.write_str(")")?;
        w.dedent(1);
        Ok(())
    }
}

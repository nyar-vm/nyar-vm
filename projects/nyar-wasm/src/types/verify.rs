use super::*;

impl Verify for I32Attr {
    fn verify(&self, _: &Context) -> pliron::error::Result<()> {
        Ok(())
    }
}

impl Verify for I64Attr {
    fn verify(&self, _: &Context) -> pliron::error::Result<()> {
        Ok(())
    }
}

impl Verify for F32Attr {
    fn verify(&self, _: &Context) -> pliron::error::Result<()> {
        Ok(())
    }
}

impl Verify for F64Attr {
    fn verify(&self, _: &Context) -> pliron::error::Result<()> {
        Ok(())
    }
}

impl Verify for V128Attr {
    fn verify(&self, _: &Context) -> pliron::error::Result<()> {
        Ok(())
    }
}


impl Verify for FuncRefAttr {
    fn verify(&self, _: &Context) -> pliron::error::Result<()> {
        Ok(())
    }
}


impl Verify for ExternRefAttr {
    fn verify(&self, _: &Context) -> pliron::error::Result<()> {
        Ok(())
    }
}

impl Verify for ValueAttr {
    fn verify(&self, _: &Context) -> pliron::error::Result<()> {
        Ok(())
    }
}

impl Verify for RefAttr {
    fn verify(&self, _: &Context) -> pliron::error::Result<()> {
        Ok(())
    }
}

impl Verify for FuncAttr {
    fn verify(&self, _: &Context) -> pliron::error::Result<()> {
        Ok(())
    }
}

impl Verify for LimitsAttr {
    fn verify(&self, _: &Context) -> Result<()> {
        Ok(())
    }
}

impl Verify for MemAttr {
    fn verify(&self, _: &Context) -> Result<()> {
        Ok(())
    }
}

impl Verify for TableAttr {
    fn verify(&self, _: &Context) -> Result<()> {
        Ok(())
    }
}

impl Verify for GlobalAttr {
    fn verify(&self, _: &Context) -> Result<()> {
        Ok(())
    }
}

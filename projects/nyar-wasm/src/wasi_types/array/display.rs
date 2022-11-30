use super::*;

impl Display for WasiArrayType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.length {
            Some(length) => {
                write!(f, "Array<{}, {}>", self.r#type, length)
            }
            None => {
                write!(f, "Array<{}>", self.r#type)
            }
        }
    }
}

impl TypeDefinition for WasiArrayType {
    ///
    /// ```wat
    /// (type (array i8))
    /// (type (array i16))
    /// (type (array i32))
    /// (type (array i64))
    /// (type (array f32))
    /// (type (array f64))
    /// (type (array anyref))
    /// (type (array (ref struct)))
    /// (type (array (ref 0)))
    /// (type (array (ref null 1)))
    /// (type (array (mut i8)))
    /// (type (array (mut i16)))
    /// (type (array (mut i32)))
    /// (type (array (mut i64)))
    /// (type (array (mut i32)))
    /// (type (array (mut i64)))
    /// (type (array (mut anyref)))
    /// (type (array (mut (ref struct))))
    /// (type (array (mut (ref 0))))
    /// (type (array (mut (ref null i31))))
    /// ```
    fn lower_type_define<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        write!(w, "(type {} (array ", self.symbol.wasi_id())?;
        if self.mutable {
            write!(w, "(mut ")?;
        }
        self.r#type.lower_type_inner(w)?;
        w.write_str("))")?;
        if self.mutable {
            w.write_str(")")?
        }
        Ok(())
    }
}

impl TypeReference for WasiArrayType {
    fn upper_type<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.write_str("(list ")?;
        self.r#type.upper_type(w)?;
        w.write_str(")")
    }

    fn lower_type<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.write_str("arrayref")
    }

    fn lower_type_inner<W: Write>(&self, w: &mut WastEncoder<W>) -> std::fmt::Result {
        w.write_str("(array ")?;
        self.r#type.lower_type_inner(w)?;
        w.write_str(")")
    }
}

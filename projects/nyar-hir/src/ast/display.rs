use super::*;

impl Default for ASTNode {
    fn default() -> Self {
        Self { kind: ASTKind::Nothing, span: Default::default() }
    }
}

impl From<ASTKind> for ASTNode {
    fn from(kind: ASTKind) -> Self {
        Self { kind, span: Default::default() }
    }
}

impl Debug for ASTNode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ASTKind::Nothing => f.write_str("<<unreachable Nothing>>"),
            ASTKind::Sequence(v) => {
                f.write_str("<<unreachable Sequence>>")?;
                f.debug_list().entries(v.iter()).finish()
            }
            ASTKind::Boolean(v) => Display::fmt(v, f),
            ASTKind::Number(v) => Display::fmt(v, f),
            ASTKind::String(v) => Display::fmt(v, f),
            ASTKind::Symbol(v) => Display::fmt(v, f),
            _ => Debug::fmt(&self.kind, f),
        }
    }
}

impl Display for ASTKind {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            ASTKind::Nothing => f.write_str("<<unreachable Nothing>>"),
            ASTKind::Program(v) => {
                f.write_str("Program")?;
                f.debug_list().entries(v.iter()).finish()
            }
            ASTKind::Sequence(v) => f.write_str("<<unreachable Sequence>>"),
            ASTKind::LetBind(v) => {
                todo!()
            }
            ASTKind::LambdaFunction(v) => {
                todo!()
            }
            ASTKind::InfixExpression(v) => {
                todo!()
            }
            ASTKind::TupleExpression(v) => {
                todo!()
            }
            ASTKind::ListExpression(v) => {
                todo!()
            }
            ASTKind::DictExpression(v) => {
                todo!()
            }
            ASTKind::Boolean(v) => write!(f, "{}", v),
            ASTKind::Number(v) => write!(f, "{}", v),
            ASTKind::String(v) => write!(f, "{}", v),
            ASTKind::StringTemplate(v) => {
                todo!()
            }
            ASTKind::XMLTemplate(v) => {
                todo!()
            }
            ASTKind::Symbol(v) => write!(f, "{}", v),
            ASTKind::IfStatement(_) => {
                todo!()
            }
            ASTKind::LoopStatement(_) => {
                todo!()
            }
        }
    }
}

impl Debug for Prefix {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Prefix::Negative => f.write_str("Negative::negative"),
            Prefix::Positive => f.write_str("Positive::positive"),
            Prefix::Not => f.write_str("Not::not"),
        }
    }
}

impl Debug for Infix {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Infix::Addition => f.write_str("Addition::add"),
            Infix::Subtraction => f.write_str("Subtraction::subtract"),
            Infix::Multiplication => f.write_str("Multiplication::multiply"),
            Infix::Division => f.write_str("Division::divide"),
        }
    }
}

impl Debug for Postfix {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Postfix::Increment => f.write_str("Increment::increment"),
            Postfix::Decrement => f.write_str("Decrement::decrement"),
        }
    }
}

impl Debug for Operator {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Operator::Prefix(o) => Debug::fmt(o, f),
            Operator::Infix(o) => Debug::fmt(o, f),
            Operator::Postfix(o) => Debug::fmt(o, f),
        }
    }
}

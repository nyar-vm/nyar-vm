use super::*;

pub enum ValkyrieKeyword {
    Type,
    Class,
    Union,
    Trait,
    Implement,
    Namespace,
    NamespaceMajor,
    For,
    ForIn,
    Return,
    Yield,
    New,
    Quote,
    Define,
    LetBind,
}

impl FromStr for ValkyrieKeyword {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "namespace" => Ok(ValkyrieKeyword::Namespace),
            "namespace!" => Ok(ValkyrieKeyword::NamespaceMajor),
            "type" => Ok(ValkyrieKeyword::Type),
            "class" | "struct" => Ok(ValkyrieKeyword::Class),
            "union" | "enum" | "tagged" | "variant" => Ok(ValkyrieKeyword::Union),
            "trait" => Ok(ValkyrieKeyword::Trait),
            "for" => Ok(ValkyrieKeyword::For),
            "in" => Ok(ValkyrieKeyword::ForIn),
            "return" => Ok(ValkyrieKeyword::Return),
            "yield" => Ok(ValkyrieKeyword::Yield),
            "new" => Ok(ValkyrieKeyword::New),
            "quote" | "¶" => Ok(ValkyrieKeyword::Quote),
            "impl" | "extend" | "extends" => Ok(ValkyrieKeyword::Implement),
            "define" | "def" => Ok(ValkyrieKeyword::Define),
            "let" | "var" | "val" => Ok(ValkyrieKeyword::LetBind),
            _ => Err(s.to_string()),
        }
    }
}

impl ValkyrieKeyword {
    pub fn name(&self) -> &str {
        match self {
            ValkyrieKeyword::Type => "type",
            ValkyrieKeyword::Class => "class",
            ValkyrieKeyword::Union => "union",
            ValkyrieKeyword::Trait => "trait",
            ValkyrieKeyword::Namespace => "namespace",
            ValkyrieKeyword::NamespaceMajor => "major namespace",
            ValkyrieKeyword::For => "for",
            ValkyrieKeyword::ForIn => "in",
            ValkyrieKeyword::Return => "return",
            ValkyrieKeyword::Yield => "yield",
            ValkyrieKeyword::New => "new",
            ValkyrieKeyword::Quote => "quote",
            ValkyrieKeyword::Implement => "extends",
            ValkyrieKeyword::Define => "def",
            ValkyrieKeyword::LetBind => "let",
        }
    }
    pub fn document(&self) -> String {
        "A keyword is a reserved word that cannot be used as an identifier.".to_string()
    }
}

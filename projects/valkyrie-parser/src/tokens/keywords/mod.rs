use super::*;

pub enum ValkyrieKeyword {
    Class,
    Union,
    Trait,
    Namespace,
    NamespaceMajor,
}

impl FromStr for ValkyrieKeyword {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "namespace" => Ok(ValkyrieKeyword::Namespace),
            "namespace!" => Ok(ValkyrieKeyword::NamespaceMajor),
            "class" | "struct" => Ok(ValkyrieKeyword::Class),
            "union" | "enum" => Ok(ValkyrieKeyword::Union),
            "trait" => Ok(ValkyrieKeyword::Trait),
            _ => Err(s.to_string()),
        }
    }
}

impl ValkyrieKeyword {
    pub fn name(&self) -> &str {
        match self {
            ValkyrieKeyword::Class => "class",
            ValkyrieKeyword::Union => "union",
            ValkyrieKeyword::Trait => "trait",
            ValkyrieKeyword::Namespace => "namespace",
            ValkyrieKeyword::NamespaceMajor => "major namespace",
        }
    }
}

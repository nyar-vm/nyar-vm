use std::ops::Range;

use serde::{Deserialize, Serialize};

use valkyrie_errors::{BigInt, FileID, FileSpan};

use crate::{ValkyrieASTKind, ValkyrieASTNode, ValkyrieIdentifierNode};

mod atomic;
mod binary;
pub mod identifier;
pub mod integer;
mod unary;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BinaryExpression {}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UnaryExpression {}

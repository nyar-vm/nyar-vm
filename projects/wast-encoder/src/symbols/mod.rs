use std::{
    fmt::{Display, Formatter},
    str::FromStr,
    sync::Arc,
};

use convert_case::Case;
use nyar_error::SyntaxError;

pub mod identifiers;

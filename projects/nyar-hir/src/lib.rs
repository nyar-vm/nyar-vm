#![feature(trivial_bounds)]
#![feature(allocator_api)]
#![feature(never_type)]
#![feature(unboxed_closures)]
#![feature(iter_from_generator)]
#![feature(generators)]
#![feature(lazy_cell)]
#![feature(extend_one)]

extern crate salsa_2022 as salsa;

pub mod projects;

mod files;
mod packages;
pub mod testing;

mod structures;

pub use self::packages::ids::{ValkyrieID, ValkyrieUniverse};
pub use nyar_error::{
    Failure, FileCache, MissingError, NyarError as ValkyrieError, Result as ValkyrieResult, RuntimeError, SourceID, Success,
    SyntaxError,
};
mod jars;
pub use crate::jars::{Jar, NyarData};
mod diagnostics;
mod parser;

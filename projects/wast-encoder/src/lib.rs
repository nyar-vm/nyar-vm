use std::{fmt::Debug, ops::AddAssign, sync::Arc};

use crate::{dag::DependenciesTrace, encoder::CanonicalImport};
pub use crate::{
    dag::DependentGraph,
    encoder::{encode_id, encode_kebab, CanonicalWasi, WastEncoder},
    enumerations::{WasiEnumeration, WasiEnumerationItem},
    functions::{ExternalFunction, WasiParameter},
    instances::WasiInstance,
    resources::WasiResource,
    symbols::{
        identifiers::Identifier,
        wasi_publisher::{WasiModule, WasiPublisher},
    },
    variants::{VariantItem, VariantType},
    wasi_types::WasiType,
};

mod dag;
mod encoder;
mod enumerations;
mod functions;
mod instances;
mod records;
mod resources;
mod symbols;
mod variants;
mod wasi_types;

#[cfg(feature = "native")]
pub mod native;

#[cfg(feature = "native")]
pub use native::NativeBackend;
pub use nyar_aot::NyarAot;

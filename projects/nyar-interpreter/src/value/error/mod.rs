mod log_level;

pub use log_level::ErrorLevels;

// pub type Result<T> = std::result::Result<T, NyarError>;
//
// #[derive(Debug, Clone)]
// pub enum ErrorKind {}
//
// #[derive(Debug, Clone)]
// pub struct NyarError {
//     kind: Box<ErrorKind>,
// }
//
// impl NyarError {
//     pub fn msg(_: impl Into<String>) -> NyarError {
//         unimplemented!()
//     }
// }

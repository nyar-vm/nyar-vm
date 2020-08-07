use super::*;

macro_rules! native_error {
    ($native:ty => $error:ident) => {
        impl From<$native> for SDLError {
            fn from(e: $native) -> Self {
                Self { kind: Box::new(SDLErrorKind::$error(e)) }
            }
        }
    };
    {$($native:ty => $error:ident,)*} => {
        $(native_error!($native => $error);)*
    };
    {$($native:ty => $error:ident), *} => {
        native_error!($($native => $error,)*);
    };
}

native_error! {
    std::io::Error  => IOError,
    std::fmt::Error => FormatError,
}
use super::*;

macro_rules! native_error {
    ($native:ty => $error:ident) => {
        impl From<$native> for NyarError {
            fn from(e: $native) -> Self {
                Self {
                    kind: box NyarErrorKind::$error(e),
                    span: Default::default(),
                }
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
    std::num::ParseFloatError => ParseDecimalError,
}

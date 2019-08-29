use smartstring::{LazyCompact, SmartString};

#[derive(Debug)]
pub struct StringView {
    inner: SmartString<LazyCompact>,
}

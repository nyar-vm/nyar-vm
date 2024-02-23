use crate::WasiCanonical;
use semver::Version;
use std::{fmt::Write, sync::Arc};

pub struct WastEncoder<'a, W> {
    source: &'a WasiCanonical,
    writer: W,
    indent: usize,
}

impl<'a, W: Write> WastEncoder<'a, W> {
    pub fn new(source: &'a WasiCanonical, writer: W) -> Self {
        Self { source, writer, indent: 0 }
    }
}

impl<'a, W: Write> Write for WastEncoder<'a, W> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.writer.write_str(s)
    }
}

// (component $Root
//     (import "wasi:io/error@0.2.0" (instance $wasi:io/error@0.2.0
//         (export "error" (type (sub resource)))
//     ))
//     (alias export $wasi:io/error@0.2.0 "error" (type $io-error))
//
//     (type $stream-error (variant
//         (case "last-operation-failed" (own $io-error))
//         (case "closed")
//     ))
//     (type $stream-result (result (error $stream-error)))
//
//     (import "wasi:io/streams@0.2.0" (instance $wasi:io/streams@0.2.0
//         (export $output-stream "output-stream" (type (sub resource)))
//         (alias outer $Root $io-error (type $io-error))
//
//         (alias outer $Root $stream-error (type $stream-error0))
//         (export $stream-error "stream-error" (type (eq $stream-error0)))
//         (type $stream-result (result (error $stream-error)))
//
//         (export "[method]output-stream.blocking-write-and-flush"
//             (func (param "self" (borrow $output-stream)) (param "contents" (list u8)) (result $stream-result))
//         )
//     ))
//     (alias export $wasi:io/streams@0.2.0 "output-stream" (type $output-stream))
//     (alias export $wasi:io/streams@0.2.0 "[method]output-stream.blocking-write-and-flush" (func $output-stream.blocking-write-and-flush))
// )
impl<'a, W: Write> WastEncoder<'a, W> {
    pub(crate) fn write_id(&mut self, id: &str) -> std::fmt::Result {
        write!(self.writer, "${}", id)
    }
    pub(crate) fn encode(&mut self) -> std::fmt::Result {
        self.write_str("(component ")?;
        self.write_id(self.source.component.as_ref())?;

        Ok(())
    }
}

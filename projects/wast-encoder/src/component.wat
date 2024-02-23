(component $root
    (import "wasi:io/error@0.2.0" (instance $wasi:io/error@0.2.0
        (export "error" (type (sub resource)))
        (export "runtime-error" (type (sub resource)))
        (export "stream-error" (type (sub resource)))
        (export "[method]output-stream.blocking-write-and-flush"
            ;; std::io::OutputStream::blocking-write-and-flush() -> ()
            (func (param "self" (type ?) (result (type ?))
        )
    ))
    (alias export $wasi:io/error@0.2.0 "error" (type $std::io::IoError<usize~usize>))
    (alias export $wasi:io/error@0.2.0 "runtime-error" (type $std::io::RuntimeError))
    (alias export $wasi:io/error@0.2.0 "stream-error" (type $std::io::StreamError))

)
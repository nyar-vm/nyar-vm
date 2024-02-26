- input

```vk
namespace std::io;

#ffi("wasi:io/error@0.2.0", "error")
resource class IoError { }

variant StreamError {
    LastOperationFailed {
        error: IoError
    },
    Closed
}

#ffi("wasi:io/streams", "input-stream")
resource class InputStream {
    #ffi("wasi:io/streams", "[method]output-stream.blocking-write-and-flush")
    blocking_write_and_flush(self, contents: List<u8>) -> Result<(), StreamError>;
}

#ffi("wasi:cli/stdin", "get-stdin")
function standard_input() -> InputStream { }

#ffi("wasi:cli/stdout", "get-stdout")
function standard_output() -> OutputStream { }

#ffi("wasi:cli/stderr", "get-stderr")
function standard_error() -> OutputStream { }
```

- output

```wat
(component $root
    (import "wasi:io/error@0.2.0" (instance $wasi:io/error@0.2.0
        (export $std::io::IoError "error" (type (sub resource)))
    ))
    (alias export $wasi:io/error@0.2.0 "error" (type $std::io::IoError))
    (type $std::io::StreamError (variant
        (case "last-operation-failed" (own $std::io::IoError))
        (case "closed")
    ))
    (import "wasi:io/streams@0.2.0" (instance $wasi:io/streams@0.2.0
        (export $std::io::InputStream "input-stream" (type (sub resource)))
        (export $std::io::OutputStream "output-stream" (type (sub resource)))
        (alias outer $root $std::io::StreamError (type $std::io::StreamError?))(export $std::io::StreamError "stream-error" (type (eq $std::io::StreamError?)))
        (export "[method]output-stream.blocking-write-and-flush" (func
            (param "self" (borrow $std::io::OutputStream)) 
            (param "contents" (list u8)) 
            (result (result (error $std::io::StreamError)))
        ))
    ))
    (alias export $wasi:io/streams@0.2.0 "input-stream" (type $std::io::InputStream))
    (alias export $wasi:io/streams@0.2.0 "output-stream" (type $std::io::OutputStream))
    (alias export $wasi:io/streams@0.2.0 "[method]output-stream.blocking-write-and-flush" (func $std::io::OutputStream::blocking_write_and_flush))
    (import "wasi:cli/stderr@0.2.0" (instance $wasi:cli/stderr@0.2.0
        (export "get-stderr" (func
            (result (own $std::io::OutputStream))
        ))
    ))
    (alias export $wasi:cli/stderr@0.2.0 "get-stderr" (func $std::io::standard_error))
    (import "wasi:cli/stdin@0.2.0" (instance $wasi:cli/stdin@0.2.0
        (export "get-stdin" (func
            (result (own $std::io::InputStream))
        ))
    ))
    (alias export $wasi:cli/stdin@0.2.0 "get-stdin" (func $std::io::standard_input))
    (import "wasi:cli/stdout@0.2.0" (instance $wasi:cli/stdout@0.2.0
        (export "get-stdout" (func
            (result (own $std::io::OutputStream))
        ))
    ))
    (alias export $wasi:cli/stdout@0.2.0 "get-stdout" (func $std::io::standard_output))
)
```




(component $root
    (import "wasi:io/error@0.2.0" (instance $wasi:io/error@0.2.0
        (export $std::io::IoError "error" (type (sub resource)))
    ))
    (alias export $wasi:io/error@0.2.0 "error" (type $std::io::IoError))
    ;; variant std∷io∷StreamError
    (type $std::io::StreamError (variant
        ;; LastOperationFailed
        (case "last-operation-failed" (own $std::io::IoError))
        ;; Closed
        (case "closed")
    ))
    (import "wasi:io/streams@0.2.0" (instance $wasi:io/streams@0.2.0
        (export $std::io::InputStream "input-stream" (type (sub resource)))
        (export $std::io::OutputStream "output-stream" (type (sub resource)))
        (alias outer $root $std::io::StreamError (type $std::io::StreamError?))(export $std::io::StreamError "stream-error" (type (eq $std::io::StreamError?)))
        (export "[method]output-stream.blocking-write-and-flush" (func
            (param "self" (borrow $std::io::OutputStream)) 
            (result (result (error $std::io::StreamError)))
        ))
    ))
    (alias export $wasi:io/streams@0.2.0 "input-stream" (type $std::io::InputStream))
    (alias export $wasi:io/streams@0.2.0 "output-stream" (type $std::io::OutputStream))
    (alias export $wasi:io/streams@0.2.0 "[method]output-stream.blocking-write-and-flush" (func $std::io::OutputStream::blocking_write_and_flush))
    (import "wasi:cli/stdio@0.2.0" (instance $wasi:cli/stdio@0.2.0
        
        
        (export "get-stderr" (func
            (result $std::io::OutputStream)
        ))
        (export "get-stdin" (func
            (result $std::io::InputStream)
        ))
        (export "get-stdout" (func
            (result $std::io::OutputStream)
        ))
    ))
    (alias export $wasi:cli/stdio@0.2.0 "get-stderr" (func $std::io::standard_error))
    (alias export $wasi:cli/stdio@0.2.0 "get-stdin" (func $std::io::standard_input))
    (alias export $wasi:cli/stdio@0.2.0 "get-stdout" (func $std::io::standard_output))
)
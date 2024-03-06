(component $root
    (core module $MockMemory
        (func $realloc (export "realloc") (param i32 i32 i32 i32) (result i32)
            (i32.const 0)
        )
        (data (i32.const 8) "Hello World!")
        (memory $memory (export "memory") 1)
    )
    (core instance $memory (instantiate $MockMemory))
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
        (alias outer $root $std::io::StreamError (type $std::io::StreamError?)) (export $std::io::StreamError "stream-error" (type (eq $std::io::StreamError?)))
        (export "[method]output-stream.blocking-write-and-flush" (func
            (param "self" (borrow $std::io::OutputStream))
            (param "contents" (list u8))
            (result (result (error $std::io::StreamError)))
        ))
    ))
    (alias export $wasi:io/streams@0.2.0 "input-stream" (type $std::io::InputStream))
    (alias export $wasi:io/streams@0.2.0 "output-stream" (type $std::io::OutputStream))
    (alias export $wasi:io/streams@0.2.0 "[method]output-stream.blocking-write-and-flush" (func $std::io::OutputStream::write))
    (import "wasi:cli/stdout@0.2.0" (instance $wasi:cli/stdout@0.2.0
        (export "get-stdout" (func
            (result (own $std::io::OutputStream))
        ))
    ))
    (alias export $wasi:cli/stdout@0.2.0 "get-stdout" (func $std::io::standard_output))
    (core func $std::io::OutputStream::write (canon lower
        (func $wasi:io/streams@0.2.0 "[method]output-stream.blocking-write-and-flush")
        (memory $memory "memory") (realloc (func $memory "realloc"))
    ))
    (core func $std::io::standard_output (canon lower
        (func $wasi:cli/stdout@0.2.0 "get-stdout")
    ))
    (core module $Main
        (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush" (func $std::io::OutputStream::write (param i32 i32 i32 i32)))
        (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $std::io::standard_output (result i32)))
        (func $main (export "main")
            (call $std::io::standard_output)
            i32.const 8
            i32.const 12
            i32.const 0
            (call $std::io::OutputStream::write)
        )
        (start $main)
    )
    (core instance $main (instantiate $Main
        (with "wasi:io/streams@0.2.0" (instance
            (export "[method]output-stream.blocking-write-and-flush" (func $std::io::OutputStream::write))
        ))
        (with "wasi:cli/stdout@0.2.0" (instance
            (export "get-stdout" (func $std::io::standard_output))
        ))
    ))
)
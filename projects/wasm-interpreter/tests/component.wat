(component $root
    (core module $MockMemory
        (func $realloc (export "realloc") (param i32 i32 i32 i32) (result i32)
            (i32.const 0)
        )
        (memory $memory (export "memory") 15)
    )
    (core instance $memory (instantiate $MockMemory))
    (import "wasi:debugger/print" (instance $wasi:debugger/print
        (export "print-i8" (func
            (param "i" s8)
        ))
    ))
    (alias export $wasi:debugger/print "print-i8" (func $print_i8))
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
        (export "[method]output-stream.write" (func
            (param "self" (borrow $std::io::OutputStream))
            (param "contents" (list u8))
            (result (result (error $std::io::StreamError)))
        ))
    ))
    (alias export $wasi:io/streams@0.2.0 "input-stream" (type $std::io::InputStream))
    (alias export $wasi:io/streams@0.2.0 "output-stream" (type $std::io::OutputStream))
    (alias export $wasi:io/streams@0.2.0 "[method]output-stream.write" (func $std::io::OutputStream::write))
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
    (core func $print_i8 (canon lower
        (func $wasi:debugger/print "print-i8")
        (memory $memory "memory")(realloc (func $memory "realloc"))
        string-encoding=utf8
    ))
    (core func $std::io::OutputStream::write (canon lower
        (func $wasi:io/streams@0.2.0 "[method]output-stream.write")
        (memory $memory "memory")(realloc (func $memory "realloc"))
        string-encoding=utf8
    ))
    (core func $std::io::standard_error (canon lower
        (func $wasi:cli/stderr@0.2.0 "get-stderr")
        (memory $memory "memory")(realloc (func $memory "realloc"))
        string-encoding=utf8
    ))
    (core func $std::io::standard_input (canon lower
        (func $wasi:cli/stdin@0.2.0 "get-stdin")
        (memory $memory "memory")(realloc (func $memory "realloc"))
        string-encoding=utf8
    ))
    (core func $std::io::standard_output (canon lower
        (func $wasi:cli/stdout@0.2.0 "get-stdout")
        (memory $memory "memory")(realloc (func $memory "realloc"))
        string-encoding=utf8
    ))
    (core module $Main
        
        
        (import "wasi:debugger/print" "print-i8" (func $print_i8 (param $i i32)))
        
        
        
        (import "wasi:io/streams@0.2.0" "[method]output-stream.write" (func $std::io::OutputStream::write (param $self i32) (param $contents (array i32)) (result result)))
        
        (import "wasi:cli/stderr@0.2.0" "get-stderr" (func $std::io::standard_error (result i32)))
        
        (import "wasi:cli/stdin@0.2.0" "get-stdin" (func $std::io::standard_input (result i32)))
        
        (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $std::io::standard_output (result i32)))
    )
    (core instance $main (instantiate $Main
        (with "wasi:debugger/print" (instance
            (export "print-i8" (func $print_i8))
        ))
        (with "wasi:io/error@0.2.0" (instance
        ))
        (with "wasi:io/streams@0.2.0" (instance
            (export "[method]output-stream.write" (func $std::io::OutputStream::write))
        ))
        (with "wasi:cli/stderr@0.2.0" (instance
            (export "get-stderr" (func $std::io::standard_error))
        ))
        (with "wasi:cli/stdin@0.2.0" (instance
            (export "get-stdin" (func $std::io::standard_input))
        ))
        (with "wasi:cli/stdout@0.2.0" (instance
            (export "get-stdout" (func $std::io::standard_output))
        ))
    ))
)
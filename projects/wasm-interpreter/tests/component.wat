(component $root
    (core module $MockMemory
        (func $realloc (export "realloc") (param i32 i32 i32 i32) (result i32)
            (i32.const 0)
        )
        (memory $memory (export "memory") 15)
    )
    (core instance $memory (instantiate $MockMemory))
    (import "wasi:io/streams@0.2.0" (instance $wasi:io/streams@0.2.0
        (export $std::io::InputStream "input-stream" (type (sub resource)))
        (export $std::io::OutputStream "output-stream" (type (sub resource)))
    ))
    (alias export $wasi:io/streams@0.2.0 "input-stream" (type $std::io::InputStream))
    (alias export $wasi:io/streams@0.2.0 "output-stream" (type $std::io::OutputStream))
    (import "wasi:io/error@0.2.0" (instance $wasi:io/error@0.2.0
        (export $std::io::IoError "error" (type (sub resource)))
    ))
    (alias export $wasi:io/error@0.2.0 "error" (type $std::io::IoError))
    ;; record Point
    (type $Point (record
        (field "x" float32)
        (field "y" float32)
    ))
    (import "wasi:cli/stderr@0.2.0" (instance $wasi:cli/stderr@0.2.0(alias outer $root $std::io::OutputStream (type $std::io::OutputStream))
        (export "get-stderr" (func
            (result (own $std::io::OutputStream))
        ))
    ))
    (alias export $wasi:cli/stderr@0.2.0 "get-stderr" (func $std::io::standard_error))
    (import "wasi:cli/stdin@0.2.0" (instance $wasi:cli/stdin@0.2.0(alias outer $root $std::io::InputStream (type $std::io::InputStream))
        (export "get-stdin" (func
            (result (own $std::io::InputStream))
        ))
    ))
    (alias export $wasi:cli/stdin@0.2.0 "get-stdin" (func $std::io::standard_input))
    (import "wasi:cli/stdout@0.2.0" (instance $wasi:cli/stdout@0.2.0(alias outer $root $std::io::OutputStream (type $std::io::OutputStream))
        (export "get-stdout" (func
            (result (own $std::io::OutputStream))
        ))
    ))
    (alias export $wasi:cli/stdout@0.2.0 "get-stdout" (func $std::io::standard_output))
    ;; variant std∷io∷StreamError
    (type $std::io::StreamError (variant
        ;; LastOperationFailed
        (case "last-operation-failed" $std::io::IoError)
        ;; Closed
        (case "closed")
    ))
    (import "unstable:debugger/print" (instance $unstable:debugger/print
        (alias outer $root $Point (type $Point?)) (export $Point "point" (type (eq $Point?)))
        (export "print-i32" (func
            (param "value" s32)
        ))
        (export "print-u32" (func
            (param "value" s32)
        ))
        (export "print-point" (func
            (param "value" $Point)
        ))
    ))
    (alias export $unstable:debugger/print "print-i32" (func $print_i32))
    (alias export $unstable:debugger/print "print-u32" (func $print_u32))
    (alias export $unstable:debugger/print "print-point" (func $test::print_point))
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
    (core func $print_i32 (canon lower
        (func $unstable:debugger/print "print-i32")
        (memory $memory "memory")(realloc (func $memory "realloc"))
        string-encoding=utf8
    ))
    (core func $print_u32 (canon lower
        (func $unstable:debugger/print "print-u32")
        (memory $memory "memory")(realloc (func $memory "realloc"))
        string-encoding=utf8
    ))
    (core func $test::print_point (canon lower
        (func $unstable:debugger/print "print-point")
        (memory $memory "memory")(realloc (func $memory "realloc"))
        string-encoding=utf8
    ))
    (core module $Main
        
        
        
        
        
        (import "wasi:cli/stderr@0.2.0" "get-stderr" (func $std::io::standard_error (result i32)))
        
        (import "wasi:cli/stdin@0.2.0" "get-stdin" (func $std::io::standard_input (result i32)))
        
        (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $std::io::standard_output (result i32)))
        
        
        (import "unstable:debugger/print" "print-i32" (func $print_i32 (param $value i32)))
        (import "unstable:debugger/print" "print-u32" (func $print_u32 (param $value i32)))
        (import "unstable:debugger/print" "print-point" (func $test::print_point (param $value i32)))
    )
    (core instance $main (instantiate $Main
        (with "wasi:io/streams@0.2.0" (instance
        ))
        (with "wasi:io/error@0.2.0" (instance
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
        (with "unstable:debugger/print" (instance
            (export "print-i32" (func $print_i32))
            (export "print-u32" (func $print_u32))
            (export "print-point" (func $test::print_point))
        ))
    ))
)
(component $Root
    (import "wasi:io/error@0.2.0" (instance $wasi:io/error@0.2.0
        (export "error" (type (sub resource)))
    ))
    (alias export $wasi:io/error@0.2.0 "error" (type $io-error))

    (import "wasi:io/streams@0.2.0" (instance $wasi:io/streams@0.2.0
        (export $output-stream "output-stream" (type (sub resource)))

        (alias outer $Root $io-error (type $io-error))
        (type $stream-error (variant
            (case "last-operation-failed" (own $io-error))
            (case "closed")
        ))
        (export $stream-error0 "stream-error" (type (eq $stream-error)))
        (type $stream-result (result (error $stream-error0)))

        (export "[method]output-stream.blocking-write-and-flush"
            (func (param "self" (borrow $output-stream)) (param "contents" (list u8)) (result $stream-result))
        )
    ))
    (alias export $wasi:io/streams@0.2.0 "output-stream" (type $output-stream))

    (import "wasi:cli/stdout@0.2.0" (instance $wasi:cli/stdout@0.2.0
        (alias outer $Root $output-stream (type $output-stream))
        (export "get-stdout" (func (result (own $output-stream))))
    ))

;;    (core module $HelloWorld
;;        (import "wasi:cli/stdout@0.2.0" "get-stdout" (func $wasi:cli/stdout@0.2.0/get-stdout (result i32)))
;;        (import "wasi:io/streams@0.2.0" "[method]output-stream.blocking-write-and-flush" (func $wasi:cli/stdout@0.2.0/output-stream.blocking-write-and-flush@method (result i32)))
;;        (memory $memory 255)
;;    )
;;    (core instance $hello-world (instantiate $HelloWorld
;;        (with "wasi:io/error@0.2.0" (instance $wasi:io/error@0.2.0))
;;        (with "wasi:io/streams@0.2.0" (instance $wasi:io/streams@0.2.0))
;;        (with "wasi:cli/stdout@0.2.0" (instance $wasi:cli/stdout@0.2.0))
;;    )
)
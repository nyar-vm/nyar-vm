(component $Root
    (import "wasi:io/error@0.2.0" (instance $wasi:io/error@0.2.0
        (export "error" (type (sub resource)))
    ))
    (alias export $wasi:io/error@0.2.0 "error" (type $io-error))

    (type $stream-error (variant
        (case "last-operation-failed" (own $io-error))
        (case "closed")
    ))
    (type $stream-result (result (error $stream-error)))

    (import "wasi:io/streams@0.2.0" (instance $wasi:io/streams@0.2.0
        (export $output-stream "output-stream" (type (sub resource)))
        (alias outer $Root $io-error (type $io-error))
        (alias outer $Root $stream-error (type $stream-error?)) (export $stream-error "stream-error" (type (eq $stream-error?)))

        (export "[method]output-stream.blocking-write-and-flush"
            (func (param "self" (borrow $output-stream)) (param "contents" (list u8)) (result (result (error $stream-error))))
        )
    ))
    (alias export $wasi:io/streams@0.2.0 "output-stream" (type $output-stream))
    (alias export $wasi:io/streams@0.2.0 "[method]output-stream.blocking-write-and-flush" (func $output-stream.blocking-write-and-flush))
)
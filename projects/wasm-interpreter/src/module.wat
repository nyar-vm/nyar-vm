(component
    (import "wasi:random/random@0.2.0" (instance $wasi_random
        (export "get-random-u64" (func (result u64)))

    ))
    (core func $get_random_u64 (canon lower
        (func $wasi_random "get-random-u64")
    ))
;;    (core instance $ffi
;;        (export "get-random-u64" (func $get_random_u64))
;;    )
;;    (alias core export $ffi "get-random-u64" (core func))
;;
;;    (core instance (instantiate 0
;;        (with "wasi:random/random@0.2.0" (instance 0))
;;    ))
    (core module $Library
;;        (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (param i32 i32 i32 i32) (result i32)))
        (import "wasi:random/random@0.2.0" "get-random-u64" (func $get_random_u64 (result i64)))
        (func $one (result i32) (i32.const 1))
        (func $two (export "a-two") (result i32) (i32.const 2))
        (func $three (result i32) (i32.const 3))
    )
    (core instance $library (instantiate $Library))
    (func $a_two (export "org:package/a-two@0.1.0") (result u32) (canon lift (core func $library "a-two")))

)

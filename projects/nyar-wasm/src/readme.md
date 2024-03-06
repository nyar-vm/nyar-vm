Another benefit is that if all gc types are used, there is no need to bring in a memory allocator, which helps reduce
the size and warm up faster.

rustc's `cabi_export_realloc` takes about 27000 lines of instructions, libc is even larger.

Other smaller allocators sacrifice either speed or security.

```wat
(component
    ;; Define a memory allocator
    (core module $MockMemory ;; Replaced by an actual allocator module, such as libc
        (func $realloc (export "realloc") (param i32 i32 i32 i32) (result i32)
            (i32.const 0)
        )
        (memory $memory (export "memory") 255)
    )
    (core instance $mock_memory (instantiate $MockMemory))
    ;; import wasi function
    (import "wasi:random/random@0.2.0" (instance $wasi:random/random@0.2.0
        (export "get-random-bytes" (func (param "length" u64) (result (list u8))))
    ))
    ;; wasi function to wasm function
    (core func $wasi:random/random@0.2.0/get-random-bytes (canon lower
        (func $wasi:random/random@0.2.0 "get-random-bytes")
        (memory $mock_memory "memory")
        (realloc (func $mock_memory "realloc"))
    ))
    ;; import wasm function
    (core module $TestRandom
        (type (func (param i64 i32)))
        (import "wasi:random/random@0.2.0" "get-random-bytes" (func $wasi:random/random@0.2.0/get-random-bytes (type 0)))
    )
    ;; instantiate wasm module with wasi instance
    (core instance $test_random (instantiate $TestRandom
        (with "wasi:random/random@0.2.0" (instance (export "get-random-bytes" (func $wasi:random/random@0.2.0/get-random-bytes))))
    ))
)
```

If using the gc type, this can be simplified to:

```wat
(component
    ;; import wasi function
    (import "wasi:random/random@0.2.0" (instance $wasi:random/random@0.2.0
        (export "get-random-bytes" (func (param "length" u64) (result (list u8))))
    ))
    ;; wasi function to wasm function
    (core func $wasi:random/random@0.2.0/get-random-bytes (canon lower
        (func $wasi:random/random@0.2.0 "get-random-bytes")
        reference-type
    ))
    ;; import wasm function
    (core module $TestRandom
        (type (func (param i64) (result (array u8))))
        (import "wasi:random/random@0.2.0" "get-random-bytes" (func $wasi:random/random@0.2.0/get-random-bytes (type 0)))
    )
    ;; instantiate wasm module with wasi instance
    (core instance $test_random (instantiate $TestRandom
        (with "wasi:random/random@0.2.0" (instance (export "get-random-bytes" (func $wasi:random/random@0.2.0/get-random-bytes))))
    ))
)
```

Obtaining a field of gc type requires only one instruction and does not require pointer algebra (at least three
instructions), further reducing the binary size.

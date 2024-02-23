(component
  (core module $MockMemory
    (func $realloc (export "realloc") (param i32 i32 i32 i32) (result i32)
        (i32.const 0)
    )
    (memory $memory (export "memory") 255)
  )
  (core instance $mock_memory (instantiate $MockMemory))



  (type
    (instance
      (type (;0;) (list u8))
      (type (;1;) (func (param "length" u64) (result 0)))
      (export (;0;) "get-random-bytes" (func (type 1)))
    )
  )
  (import "wasi:random/random@0.2.0" (instance $wasi:random/random@0.2.0 (;0;) (type 0)))





  (core func $wasi:random/random@0.2.0:get-random-bytes (canon lower
    (func $wasi:random/random@0.2.0 "get-random-bytes")
    (memory $mock_memory "memory")
    (realloc (func $mock_memory "realloc"))
    string-encoding=utf8
  ))



  (core module $TestRandom
    (type (func (param i64 i32)))
    (import "wasi:random/random@0.2.0" "get-random-bytes" (func $wasi:random/random@0.2.0:get-random-bytes (;0;) (type 0)))

  )


    (core instance $test_random (instantiate $TestRandom
        (with "wasi:random/random@0.2.0" (instance (export "get-random-bytes" (func $wasi:random/random@0.2.0:get-random-bytes))))
    ))
)
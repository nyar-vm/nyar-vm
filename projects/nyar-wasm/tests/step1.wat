(module
    (import "env" "print_i32" (func $print_i32 (param i32)))
    (import "env" "print_i64" (func $print_i64 (param i64)))

    (func (export "_start") (result i32)
        (call $print_i32 (i32.const 42))
        ;; no problem, return 0
        (i32.const 0)
    )
)
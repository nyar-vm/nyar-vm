(module
  (type (;0;) (func (param i32 i32) (result i32)))
  (type (;1;) (func (param i32 i32 i32) (result i32)))
  (type (;2;) (func (param f32) (result i32)))
  (type (;3;) (func))
  (import "wasi_snapshot_preview1" "random_get" (func (;0;) (type 0)))
  (func (;1;) (type 1) (param i32 i32 i32) (result i32)
    local.get 0
    local.get 1
    i32.add
    i32.const 3
    i32.add
    i32.const 1
    i32.const 2
    local.get 0
    select
    i32.add
    i32.const 1
    i32.const 2
    i32.const 3
    i32.const 4
    local.get 2
    select
    local.get 1
    select
    local.get 0
    select
    i32.add
  )
  (func (;2;) (type 2) (param f32) (result i32)
    local.get 0
    f32.const 0x1.91eb86p+1 (;=3.14;)
    f32.add
    i32.trunc_f32_s
  )
  (func (;3;) (type 3)
    i32.const 1
    i32.const 1
    call 0
    drop
  )
  (memory (;0;) 0 0)
  (export "sum_all" (func 1))
  (export "add_ba" (func 2))
  (export "static" (memory 0))
  (start 3)
  (@producers
    (language "valkyrie" "2024")
    (language "player" "berserker")
    (processed-by "nyar-wasm" "0.0.1")
  )
)
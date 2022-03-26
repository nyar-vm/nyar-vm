(module
  (type (;0;) (func (param i32 i32) (result i32)))
  (type (;1;) (func (param f32) (result i32)))
  (type (;2;) (func))
  (func (;0;) (type 0) (param i32 i32) (result i32)
    i32.const 0
  )
  (func (;1;) (type 1) (param f32) (result i32)
    local.get 0
    f32.const 0x1.91eb86p+1 (;=3.14;)
    f32.add
    i32.trunc_f32_s
  )
  (func (;2;) (type 2))
  (export "add_ab" (func 0))
  (export "add_ba" (func 1))
  (export "_start" (func 2))
  (@producers
    (language "valkyrie" "2024")
    (language "player" "berserker")
    (processed-by "nyar-wasm" "0.0.0")
  )
)
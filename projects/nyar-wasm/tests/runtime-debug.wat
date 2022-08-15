(module $#module0<> (@name "")
  (type $Stable (;0;) (struct (field f32) (field f32) (field f32) (field f32) (field f32)))
  (type $a (;1;) (struct (field f32) (field f32)))
  (type (;2;) (func (param i32 i32 i32 i32) (result i32)))
  (type (;3;) (func (param i32 i32) (result i32)))
  (type (;4;) (func (param i32 i32 i32) (result i32)))
  (type (;5;) (func (param f32) (result i32)))
  (type (;6;) (func))
  (import "wasi_snapshot_preview1" "fd_write" (func $file_descriptor_write (;0;) (type 2)))
  (import "wasi_snapshot_preview1" "random_get" (func $random_get (;1;) (type 3)))
  (func $sum_all (;2;) (type 4) (param $a i32) (param $b i32) (param $c i32) (result i32)
    local.get $a
    local.get $b
    i32.add
    f32.const 0x0p+0 (;=0;)
    call $add_ba
    i32.add
    local.get $a
    if (result i32) ;; label = @1
      i32.const 1
    else
      i32.const 2
      i32.const 3
      drop
    end
    i32.add
    local.get $a
    if (result i32) ;; label = @1
      i32.const 1
    else
      local.get $b
      if (result i32) ;; label = @2
        i32.const 2
      else
        local.get $c
        if (result i32) ;; label = @3
          i32.const 3
        else
          i32.const 4
        end
      end
    end
    i32.add
    return
  )
  (func $add_ba (;3;) (type 5) (param $b f32) (result i32)
    global.get $math.pi
    local.get $b
    f32.add
    i32.trunc_f32_s
  )
  (func $__main (;4;) (type 6)
    i32.const 1
    i32.const 1
    call $random_get
  )
  (memory $static (;0;) 0 0)
  (global $math.pi (;0;) (mut f32) f32.const 0x1.91eb86p+1 (;=3.14;))
  (export "sum_all" (func $sum_all))
  (export "add_ba" (func $add_ba))
  (export "static" (memory $static))
  (start $__main)
  (data (;0;) (i32.const 0) "")
  (@producers
    (language "valkyrie" "2024")
    (language "player" "berserker")
    (processed-by "nyar-wasm" "0.0.1")
  )
)
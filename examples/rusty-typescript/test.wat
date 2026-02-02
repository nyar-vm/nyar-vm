(module
  (func $f0 (param $p0 i64) (param $p1 i64) (param $p2 i64) (param $p3 i64) (param $p4 i64) (param $p5 i64) (result i32)
    local.get 0
    local.set 0
    local.get 1
    local.set 1
    local.get 2
    local.set 2
    local.get 3
    local.set 3
    local.get 4
    local.set 5
    local.get 5
    local.set 6
    local.get 1
    i64.load offset=0 align=0
    local.set 4
    local.get 4
    local.set 7
    call 0
    i32.const 0
    local.get 4
    local.get 1
    i64.store offset=0 align=0
    return
    end))

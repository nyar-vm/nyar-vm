## Tests

```bash
wee test
```

```v
@external("wasi_snapshot_preview1") 
class Wasi {
    random_get(a: i32, b: i32): i32
}
```

```v
namespace wasi.io.poll

structure Error {
    _handler: Resource
    to_debug_string(self);
}

struct Pollable {
    _handler: Resource
    ready(self): bool;
    block()
}
function poll()

```

```v
structure Point {
    x: f64 = 0
    y: f64 = 0
}

extends Point {
    construct(x: f64, y: f64) {
        self.x = x;
        self.y = y;
    }
    sum(self): f64 {
        self.x + self.y
    }
}
Point(1.618, 0.618).sum()
```

```vk
class Point {
    x: 1
    y: 2
}

new Point {
    y: 1
}

function `__anonymous_new__.1`() {
    let output = Point();
    output.x = 1;
    output.y = 1;
    output
}
```

```wat
(func $__anonymous_new__.1 (result (ref $Point))
    (local $output (ref $Point))
    (local.set $output (call struct.new $Point))
    (struct.set $Point $output 0 (i32.const 1))
    (struct.set $Point $output 4 (i32.const 1))
)
```
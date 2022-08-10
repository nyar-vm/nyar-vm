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
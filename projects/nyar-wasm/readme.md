NyarVM WebAssembly Backend
=====






```valkyrie
function add(a, b) -> ret {
    a + b
}

function add(a, b, ret) {
    ret(a + b)
}

let data = "hello world"
```



```valkyrie
/// - file_descriptor: 文件描述符, 比如 `stdout` 的文件描述符为 1
/// - memory: 数据在哪个内存, 这一项总是传 0.
/// - length: 字符串的长度
/// - nwritten: 写入的字节数
#ffi("wasi_snapshot_preview1", "fd_write")
function fd_write(file_descriptor: i32, memory: i32, length: i32, nwritten: i32): i32 {

}


class UTF8Text {
    private buffer: array<u8>
}

structure UTF8View {
    offset: u32,
    length: u32,
}

let data = "hello world";

#main
function run() {
    fd_write(1, 0, 1, 20)
    @asm("drop")
}
```


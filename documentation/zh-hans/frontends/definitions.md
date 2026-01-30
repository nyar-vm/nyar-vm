# 定义

Valkyrie 提供了多种定义语法，用于声明命名空间、变量、函数、类型和其他程序实体。

## 命名空间定义

Valkyrie 使用 `namespace` 或 `namespace!` 关键字声明模块所属的命名空间。

```valkyrie
# 显式声明命名空间
namespace! package.collection.option;

# 或者
namespace package.text;
```

## 变量定义

### 基本变量定义

```valkyrie
# 不可变变量
let name = "Alice"
let age = 30

# 可变变量
let mut counter = 0

# 显式类型注解
let score: i32 = 95

# 延迟初始化
let result: i32
if condition {
    result = 42
} else {
    result = 0
}
```

## 函数定义 (micro)

Valkyrie 使用 `micro` 关键字定义函数。

### 基本函数定义

```valkyrie
# 无参数函数
micro greet() {
    print("Hello, World!")
}

# 带参数函数
micro add(a: i32, b: i32) -> i32 {
    a + b
}
```

## 类型定义

Valkyrie 区分结构化数据（`class`）和代数数据类型（`unity`）。

### 类定义 (class)

```valkyrie
class Point {
    x: f64
    y: f64
}
```

### 联合类型定义 (unity)

`unity` 用于定义和 Rust `enum` 类似的代数数据类型。

```valkyrie
unity Option⟨V⟩ {
    Some {
        value: V
    }
    None
}
```

## 实现定义 (imply)

Valkyrie 使用 `imply` 关键字为类型实现方法或 Trait。

```valkyrie
imply Option⟨V⟩⸬Some {
    constructor(value: V) {
        this.value = value
    }
}

imply Unicode {
    # 实现方法
}
```

# 命名参数调用
let user = create_user(name: "Alice", active: false)
let result = sum(1, 2, 3, 4, 5)

# 引用参数
micro modify_array(arr: &mut [i32]) {
    for i in 0..<arr.len() {
        arr[i] *= 2
    }
}

# 泛型参数
micro identity<T>(value: T) -> T {
    value
}

micro map<T, U>(items: [T], transform: micro(T) -> U) -> [U] {
    let mut result = []
    for item in items {
        result.push(transform(item))
    }
    result
}
```

### 高阶函数

```valkyrie
# 函数作为参数
micro apply_operation(x: i32, y: i32, op: micro(i32, i32) -> i32) -> i32 {
    op(x, y)
}

# 返回函数
micro make_adder(n: i32) -> micro(i32) -> i32 {
    micro(x: i32) -> i32 {
        x + n
    }
}

# 闭包
let add_five = make_adder(5)
let result = add_five(10)  # 15

# 匿名函数
let numbers = [1, 2, 3, 4, 5]
let doubled = numbers.map(micro(x) { x * 2 })
let filtered = numbers.filter(micro(x) { x % 2 == 0 })
```

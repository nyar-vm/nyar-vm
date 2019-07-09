# Valkyrie 语言特性指南

## 概述

Valkyrie 是基于 Nyar 编译工具平台构建的示范性编程语言。本指南介绍 Valkyrie 语言的核心特性和使用方法，帮助开发者理解如何在 Nyar 平台上设计和实现现代编程语言。

## Nyar 平台与 Valkyrie 语言的关系

### Nyar 编译工具平台

Nyar 是一个通用的编译器构建平台，提供：
- 增量编译基础设施（基于 Salsa）
- 多层中间表示（AST → HIR → MIR → LIR）
- 可插拔的代码生成后端
- 统一的错误处理框架（基于 miette）
- 语言服务器协议支持

### Valkyrie 示范语言

Valkyrie 是在 Nyar 平台上实现的具体编程语言，展示了：
- 如何利用 Nyar 的基础设施
- 现代语言特性的设计与实现
- 编译器前端到后端的完整流程

## Valkyrie 语言特性

### 基础语法

#### 变量声明
```valkyrie
// 不可变绑定
let x = 42
let name = "Alice"

// 可变变量
let mut counter = 0
counter = counter + 1

// 类型注解
let age: Int = 25
let pi: Float = 3.14159
```

#### 函数定义
```valkyrie
// 简单函数
fn greet(name: String) -> String {
    "Hello, " + name + "!"
}

// 带默认参数的函数
fn power(base: Int, exponent: Int = 2) -> Int {
    if exponent == 0 {
        1
    } else {
        base * power(base, exponent - 1)
    }
}

// 高阶函数
fn apply_twice<T>(f: T -> T, x: T) -> T {
    f(f(x))
}
```

### 数据类型

#### 基础类型
```valkyrie
// 数值类型
let integer: Int = 42
let floating: Float = 3.14
let boolean: Bool = true

// 字符串
let text: String = "Hello, World!"
let multiline: String = """
    This is a
    multiline string
"""
```

#### 复合类型
```valkyrie
// 数组
let numbers: Array<Int> = [1, 2, 3, 4, 5]
let mixed: Array<String> = ["apple", "banana", "cherry"]

// 元组
let point: (Int, Int) = (10, 20)
let person: (String, Int, Bool) = ("Alice", 30, true)

// 记录类型
class Point {
    x: Int,
    y: Int
}

let origin = Point { x: 0, y: 0 }
```

#### 代数数据类型
```valkyrie
// 枚举类型
union Color {
    Red,
    Green,
    Blue,
    RGB(Int, Int, Int)
}

// 可选类型
union Option<T> {
    Some(T),
    None
}

// 结果类型
union Result<T, E> {
    Ok(T),
    Err(E)
}
```

### 模式匹配

```valkyrie
// 基础模式匹配
fn describe_color(color: Color) -> String {
    match color {
        Red => "红色",
        Green => "绿色",
        Blue => "蓝色",
        RGB(r, g, b) => "RGB(" + r + ", " + g + ", " + b + ")"
    }
}

// 守卫条件
fn classify_number(n: Int) -> String {
    match n {
        x if x < 0 => "负数",
        0 => "零",
        x if x > 0 && x <= 10 => "小正数",
        _ => "大正数"
    }
}

// 解构匹配
fn process_point(point: Point) -> String {
    match point {
        Point { x: 0, y: 0 } => "原点",
        Point { x: 0, y } => "Y轴上的点: " + y,
        Point { x, y: 0 } => "X轴上的点: " + x,
        Point { x, y } => "点(" + x + ", " + y + ")"
    }
}
```

### 控制流

#### 条件表达式
```valkyrie
// if 表达式
let result = if condition {
    "true branch"
} else {
    "false branch"
}

// 多分支条件
let grade = if score >= 90 {
    "A"
} else if score >= 80 {
    "B"
} else if score >= 70 {
    "C"
} else {
    "F"
}
```

#### 循环
```valkyrie
// for 循环
for i in 0..10 {
    println(i)
}

// 遍历数组
for item in ["apple", "banana", "cherry"] {
    println(item)
}

// while 循环
var count = 0
while count < 5 {
    println(count)
    count = count + 1
}

// loop 表达式
let result = loop {
    let input = read_input()
    if input == "quit" {
        break "goodbye"
    }
    process_input(input)
}
```

### 错误处理

```valkyrie
// 使用 Result 类型
fn divide(a: Float, b: Float) -> Result<Float, String> {
    if b == 0.0 {
        Err("除零错误")
    } else {
        Ok(a / b)
    }
}

// 错误传播
fn calculate() -> Result<Float, String> {
    let x = divide(10.0, 2.0)?  // 自动传播错误
    let y = divide(x, 3.0)?
    Ok(y * 2.0)
}

// 错误处理
fn main() {
    match calculate() {
        Ok(result) => println("结果: " + result),
        Err(error) => println("错误: " + error)
    }
}
```

### 泛型和特征

#### 泛型函数
```valkyrie
// 泛型函数
fn identity<T>(x: T) -> T {
    x
}

// 约束泛型
fn max<T: Ord>(a: T, b: T) -> T {
    if a > b { a } else { b }
}
```

#### 特征定义
```valkyrie
// 特征定义
trait Display {
    fn display(self) -> String
}

// 为类型实现特征
impl Display for Point {
    fn display(self) -> String {
        "(" + self.x + ", " + self.y + ")"
    }
}

// 使用特征
fn print_displayable<T: Display>(item: T) {
    println(item.display())
}
```

### 模块系统

```valkyrie
// 模块定义
mod math {
    pub fn add(a: Int, b: Int) -> Int {
        a + b
    }
    
    fn internal_helper() -> Int {
        42
    }
}

// 使用模块
use math::add

fn main() {
    let result = add(2, 3)
    println(result)
}
```

### 内存管理

```valkyrie
// 自动内存管理
class Node {
    value: Int,
    next: Option<Box<Node>>
}

// 创建链表
fn create_list() -> Option<Box<Node>> {
    Some(Box::new(Node {
        value: 1,
        next: Some(Box::new(Node {
            value: 2,
            next: None
        }))
    }))
}

// 引用计数
fn share_data() {
    let data = Rc::new(vec![1, 2, 3, 4, 5])
    let shared1 = data.clone()
    let shared2 = data.clone()
    // 自动管理引用计数
}
```

## 标准库概览

### 集合类型
```valkyrie
// 动态数组
let mut vec = Vec::new()
vec.push(1)
vec.push(2)
vec.push(3)

// 哈希映射
let mut map = HashMap::new()
map.insert("key1", "value1")
map.insert("key2", "value2")

// 集合
let mut set = HashSet::new()
set.insert("apple")
set.insert("banana")
```

### 迭代器
```valkyrie
// 函数式编程风格
let numbers = [1, 2, 3, 4, 5]
let doubled: Vec<Int> = numbers
    .iter()
    .map(|x| x * 2)
    .filter(|x| x > 5)
    .collect()

// 链式操作
let result = (0..100)
    .filter(|x| x % 2 == 0)
    .map(|x| x * x)
    .take(10)
    .sum()
```

### 字符串处理
```valkyrie
// 字符串操作
let text = "Hello, World!"
let uppercase = text.to_uppercase()
let words: Vec<String> = text.split(", ").collect()

// 格式化字符串
let name = "Alice"
let age = 30
let message = format!("My name is {} and I am {} years old", name, age)
```

## 与 Nyar 平台的集成

### 编译过程

1. **词法分析**: Valkyrie 源码 → Token 流
2. **语法分析**: Token 流 → AST
3. **语义分析**: AST → HIR（高级中间表示）
4. **优化**: HIR → MIR（中级中间表示）
5. **代码生成**: MIR → LIR → 目标代码

### 工具链支持

```bash
# 编译 Valkyrie 程序
nyar compile main.vk --output main.wasm

# 运行解释模式（开发时）
nyar run main.vk --mode interpret

# 语言服务器
valkyrie-lsp

# 格式化代码
nyar format main.vk

# 类型检查
nyar check main.vk
```

### 调试支持

```valkyrie
// 调试宏
fn debug_example() {
    let x = 42
    debug!("x 的值是: {}", x)  // 仅在调试模式下输出
    
    assert!(x > 0, "x 必须是正数")
    
    // 条件编译
    #[cfg(debug)]
    println("这是调试信息")
}
```

## 最佳实践

### 代码组织
```valkyrie
// 使用模块组织代码
mod models {
    pub struct User {
        pub name: String,
        pub email: String
    }
}

mod services {
    use super::models::User
    
    pub fn create_user(name: String, email: String) -> User {
        User { name, email }
    }
}
```

### 错误处理策略
```valkyrie
// 定义应用特定的错误类型
union AppError {
    ValidationError(String),
    DatabaseError(String),
    NetworkError(String)
}

// 统一的错误处理
fn handle_request() -> Result<Response, AppError> {
    let user = validate_input()?;
    let data = fetch_from_database(user.id)?;
    let response = process_data(data)?;
    Ok(response)
}
```

### 性能优化
```valkyrie
// 使用迭代器避免中间分配
fn efficient_processing(data: &[Int]) -> Int {
    data.iter()
        .filter(|&&x| x > 0)
        .map(|&x| x * x)
        .sum()
}

// 避免不必要的克隆
fn process_string(s: &String) -> usize {
    s.len()  // 使用引用而不是克隆
}
```

## 总结

Valkyrie 语言展示了如何在 Nyar 编译工具平台上构建现代编程语言。通过利用 Nyar 提供的基础设施，Valkyrie 实现了：

- **表达力强**: 支持函数式和面向对象编程范式
- **类型安全**: 静态类型系统防止运行时错误
- **性能优秀**: 零成本抽象和优化编译
- **开发友好**: 优秀的错误信息和工具链支持

Valkyrie 不仅是一个实用的编程语言，更是 Nyar 平台能力的最佳展示，为其他语言设计者提供了宝贵的参考和灵感。
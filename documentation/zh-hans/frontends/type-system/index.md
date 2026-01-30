# 类型系统

Valkyrie 提供了一个强大而灵活的类型系统，支持静态类型检查、类型推导和高级类型特性。

## 基本类型

### 原始类型

```valkyrie
# 整数类型
let a: i32 = 42
let b: u64 = 100
let c: isize = -1

# 浮点类型
let x: f32 = 3.14
let y: f64 = 2.718281828

# 布尔类型
let flag: bool = true

# 字符和字符串
let ch: char = 'A'
let text: string = "Hello, World!"
```

### 复合类型

```valkyrie
# 数组类型
let numbers: [i32; 5] = [1, 2, 3, 4, 5]
let dynamic: [string] = ["a", "b", "c"]

# 元组类型
let point: (f64, f64) = (3.0, 4.0)
let mixed: (string, i32, bool) = ("test", 42, true)

# 可选类型
let maybe_value: i32? = 42
let empty: string? = None
```

## 复合类型定义

### 记录类型

```valkyrie
# 基本记录类型
type Point = {
    x: f64,
    y: f64,
}

# 泛型记录类型
type Container⟨T⟩ = {
    value: T,
    metadata: string,
}

# 嵌套记录类型
type Person = {
    name: string,
    age: i32,
    address: {
        street: string,
        city: string,
    },
}
```

### 联合类型

```valkyrie
# 基本联合类型
unity Result⟨T, E⟩ {
    Fine(T),
    Fail(E)
}

# 复杂联合类型
unity Expression {
    Literal(i32),
    Variable(string),
    Binary {
        left: Expression,
        operator: string,
        right: Expression,
    }
}
```

## 特征和实现

### 特征定义

```valkyrie
# 基本特征
trait Display {
    micro fmt(self) -> string
}

# 泛型特征
trait Iterator<T> {
    micro next(mut self) -> Option<T>
    micro collect<C: FromIterator<T>>(self) -> C
}

# 关联类型特征
trait IntoIterator {
    type Item
    type IntoIter: Iterator<Self::Item>
    
    micro into_iter(self) -> Self::IntoIter
}
```

### 特征实现

```valkyrie
# 为类型实现特征
imply Point: Display {
    micro fmt(self) -> String {
        @format("Point({}, {})", self.x, self.y)
    }
}

# 泛型实现
imply<T: Display> Container<T>: Display {
    micro fmt(self) -> String {
        @format("Container({})", self.value.fmt())
    }
}

# 条件实现
imply<T> Container<T>: Clone where T: Clone {
    micro clone(self) -> Self {
        Container {
            value: self.value.clone(),
            metadata: self.metadata.clone(),
        }
    }
}
```

## 泛型和约束

### 泛型函数

```valkyrie
# 基本泛型函数
micro identity<T>(value: T) -> T {
    value
}

# 带约束的泛型函数
micro print_display<T: Display>(value: T) {
    print(value.fmt())
}

# 多重约束
micro complex_function<T>(value: T) -> String 
where
    T: Display + Clone + Send
{
    let cloned = value.clone()
    cloned.fmt()
}
```

### 关联类型

```valkyrie
# 使用关联类型的特征
trait Collect<T> {
    type Output
    
    micro collect(self, items: [T]) -> Self::Output
}

# 实现关联类型
imply Vector<i32>: Collect<i32> {
    type Output = Vector<i32>
    
    micro collect(mut self, items: [i32]) -> Vector<i32> {
        for item in items {
            self.push(item)
        }
        self
    }
}
```

## 内存管理

### 垃圾回收

```valkyrie
# Valkyrie 使用垃圾回收，无需手动管理内存
micro process_data(data: String) -> String {
    let processed = data.to_uppercase()
}

# 自动内存管理
micro safe_string_processing(input: String) -> String {
    let result = input.trim().to_uppercase()
    result  // 垃圾回收器自动管理内存
}

# 解析器示例
type Parser = {
    input: String,
    position: i32,
}

impl Parser {
    micro new(input: String) -> Self {
        Parser { input, position: 0 }
    }
    
    micro current(self) -> Option<String> {
        if self.position < self.input.len() {
            Some(self.input[self.position..])
        } else {
            None
        }
    }
}
```

## 类型推导

### 自动类型推导

```valkyrie
# 编译器可以推导出类型
let numbers = [1, 2, 3, 4, 5]  // 推导为 [i32; 5]
let text = "Hello"              // 推导为 &str
let result = Some(42)          // 推导为 Option<i32>

# 部分类型注解
let container: Container<_> = Container {
    value: 42,  // 推导为 i32
    metadata: "test".to_string(),
}

# 函数返回类型推导
micro create_vec() {
    let mut v = Vec::new()  // 类型待定
    v.push(1)               // 现在推导为 Vector<i32>
    v
}
```

### 类型提示

```valkyrie
# 显式类型转换
let x: f64 = 42 as f64
let y = 42.0 as i32

# 类型提示
let numbers: Vector<i32> = (0..10).collect()
let result = parse::<i32>("42")

# 涡轮鱼语法
let parsed = "42".parse::<i32>()
let collected = iterator.collect::<Vector<_>>()
```

## 高级类型特性

### 类型别名

```valkyrie
# 简单类型别名
type UserId = u64
type UserName = String
type Result<T> = Result<T, Error>

# 泛型类型别名
type HashMap<K, V> = std::collections::HashMap<K, V>
type BoxedFuture<T> = Box<dyn Future<Output = T> + Send>

# 复杂类型别名
type EventHandler<T> = Box<dyn Micro(T) -> Result<(), Error> + Send + Sync>
```

### 新类型模式

```valkyrie
# 新类型包装
type Meters = { value: f64 }
type Seconds = { value: f64 }

# 为新类型实现操作
impl Meters {
    micro new(value: f64) -> Self {
        Meters { value }
    }
    
    micro value(self) -> f64 {
        self.value
    }
}

# 类型安全的计算
micro calculate_speed(distance: Meters, time: Seconds) -> f64 {
    distance.value() / time.value()
}
```

### 状态类型

```valkyrie
# 状态类型用于编译时状态跟踪
type State<S> = {
    data: String,
}

type Open = {}
type Closed = {}

impl State<Closed> {
    micro new(data: String) -> Self {
        State { data }
    }
    
    micro open(self) -> State<Open> {
        State { data: self.data }
    }
}

impl State<Open> {
    micro read(self) -> String {
        self.data
    }
    
    micro close(self) -> State<Closed> {
        State { data: self.data }
    }
}
```

## 类型安全和错误处理

### 编译时类型检查

```valkyrie
# 类型不匹配会在编译时被捕获
micro type_safe_example() {
    let x: i32 = 42
    let y: f64 = 3.14
    
    // 编译错误：类型不匹配
    // let result = x + y
    
    // 正确的做法
    let result = (x as f64) + y
}

# 垃圾回收确保内存安全
micro memory_safe_example() {
    let mut data = vec![1, 2, 3]
    let first_element = data[0]
    
    // GC语言中可以安全地修改数据
    data.push(4)
    
    print("First element: {}", first_element)
    print("Updated data: {:?}", data)
}
```

### 空指针安全

```valkyrie
# Option 类型确保空指针安全
micro safe_division(x: f64, y: f64) -> Option<f64> {
    if y != 0.0 {
        Some(x / y)
    } else {
        None
    }
}

# 使用 Option
micro use_safe_division() {
    match safe_division(10.0, 2.0) {
        Some(result) => print("Result: {}", result),
        None => print("Division by zero!"),
    }
}

# Result 类型用于错误处理
micro parse_number(s: String) -> Result<i32, ParseError> {
    s.parse()
}
```

## 性能优化

### 零成本抽象

```valkyrie
# 迭代器是零成本抽象
micro sum_squares(numbers: [i32]) -> i32 {
    numbers
        .iter()
        .map { $x * $x }
        .filter { $x > 10 }
        .sum()
}

# 编译后等价于手写循环
micro sum_squares_manual(numbers: [i32]) -> i32 {
    let mut sum = 0
    for number in numbers {
        let square = number * number
        if square > 10 {
            sum += square
        }
    }
    sum
}
```

### 内联和特化

```valkyrie
# 内联函数
@.inline
micro fast_add(a: i32, b: i32) -> i32 {
    a + b
}

# 特化实现
@.specialize
micro generic_function<T: Display>(value: T) -> String {
    value.fmt()
}

# 为特定类型提供优化实现
@.specialize
micro generic_function(value: i32) -> String {
    // 针对 i32 的优化实现
    @format("{}", value)
}
```

## 相关文档

- [高阶类型](./hkt.md) - 高阶类型和类型构造器
- [类型级编程](./type-level.md) - 类型级计算和证明
- [依赖类型](./dependent-types.md) - 依赖类型系统
- [线性类型](./linear-types.md) - 线性类型和资源管理

## 总结

Valkyrie 的类型系统提供了：

1. **静态类型安全**: 编译时捕获类型错误
2. **零成本抽象**: 高级抽象不影响运行时性能
3. **内存安全**: 垃圾回收器自动管理内存
4. **表达能力**: 支持复杂的类型关系和约束
5. **人体工程学**: 类型推导减少样板代码

这些特性使得 Valkyrie 能够编写既安全又高效的代码，同时保持良好的开发体验。
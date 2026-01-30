
## 面向对象编程

Valkyrie 支持基于类的面向对象编程，提供类定义、构造函数、方法和继承等特性。

### 特殊类类型

- [神经网络类型 (Neural)](./neural.md) - 用于机器学习的特殊类类型
- [界面组件类型 (Widget)](./widget.md) - 用于 UI 开发的特殊类类型

### 字段定义

```valkyrie
# 基本字段定义
name: string
age: i32
is_active: bool = true  # 默认值

# 访问控制
public username: string
private password: string
protected internal_id: i64

# 只读字段
readonly created_at: DateTime
```

### 类定义 (class)

使用 `class` 关键字定义结构化数据类型。

```valkyrie
class Person {
    name: string
    age: i32
    
    micro constructor(self, name: string, age: i32) {
        self.name = name
        self.age = age
    }

    micro greet(self) {
        print("Hello, I'm ${self.name}")
    }
}
```

### 方法定义

```valkyrie
imply Person {
    # 实例方法
    micro say_hello(self) {
        print("Hello, I'm ${self.name}")
    }

    # 可变方法
    micro set_age(mut self, new_age: i32) {
        self.age = new_age
    }

    # 静态方法
    micro static create_anonymous() -> Person {
        Person { name: "Anonymous", age: 0 }
    }

    # 带返回值的方法
    micro get_info(self) -> string {
        "${self.name} is ${self.age} years old"
    }
}
```

### 继承 (Inheritance)

类可以通过在类名后加括号来继承一个或多个类。

```valkyrie
class Student(Person) {
    student_id: string
}
```




## 标志类型 (flags)

### 基本标志类型

```valkyrie
# 简单标志
flags FilePermissions {
    READ = 1,
    WRITE = 2,
    EXECUTE = 4
}

# 使用标志
let perms = FilePermissions::READ | FilePermissions::WRITE
if perms.contains(FilePermissions::READ) {
    print("可读")
}

# 复杂标志
flags WindowStyle {
    RESIZABLE = 0x01,
    MINIMIZABLE = 0x02,
    MAXIMIZABLE = 0x04,
    CLOSABLE = 0x08,
    TITLEBAR = 0x10,
    BORDER = 0x20,
    
    # 组合标志
    DEFAULT = RESIZABLE | MINIMIZABLE | MAXIMIZABLE | CLOSABLE | TITLEBAR | BORDER,
    DIALOG = CLOSABLE | TITLEBAR | BORDER
}
```

### 标志操作

```valkyrie
flags Permissions {
    READ = 1,
    write = 2,
    execute = 4,
    
    # 方法
    micro has_read(self) -> bool {
        self.contains(Permissions::read)
    }
    
    micro add_write(mut self) {
        self |= Permissions::write
    }
    
    micro remove_execute(mut self) {
        self &= !Permissions::execute
    }
}
```

## 特征定义 (trait)

### 基本特征

```valkyrie
# 简单特征
trait Display {
    micro to_string(self) -> string
}

# 带默认实现的特征
trait Debug {
    micro debug(self) -> string
    
    # 默认实现
    micro print_debug(self) {
        print(self.debug())
    }
}

# 泛型特征
trait Iterator<T> {
    micro next(mut self) -> T?
    
    # 默认方法
    micro collect(mut self) -> [T] {
        let mut result = []
        while let item = self.next()? {
            result.push(item)
        }
        result
    }
    
    micro map<U>(self, f: micro(T) -> U) -> MapIterator<T, U> {
        MapIterator::new(self, f)
    }
}
```

### 特征实现

```valkyrie
# 为类型实现特征
imply Person: Display {
    micro to_string(self) -> string {
        "${self.name} (${self.age} years old)"
    }
}

imply Person: Debug {
    micro debug(self) -> string {
        "Person { name: \"${self.name}\", age: ${self.age} }"
    }
}

# 条件实现
imply<T> T?: Display where T: Display {
    micro to_string(self) -> string {
        match self {
            value? => "Some(${value.to_string()})",
            _ => "None"
        }
    }
}
```

### 特征约束

```valkyrie
# 函数中的特征约束
micro print_items<T>(items: [T]) where T: Display {
    for item in items {
        print(item.to_string())
    }
}

# 多重约束
micro process<T>(value: T) -> string 
where 
    T: Display + Debug + Clone 
{
    let cloned = value.clone()
    "Display: ${value.to_string()}, Debug: ${cloned.debug()}"
}

# 关联类型
trait Collect<T> {
    type Output
    
    micro collect(self) -> Self::Output
}
```

## 类型别名

```valkyrie
# 简单类型别名
type UserId = i64
type UserName = string
type Coordinates = (f64, f64)

# 泛型类型别名
type Result<T> = Result<T, string>
type HashMap<K, V> = std::collections::HashMap<K, V>

# 函数类型别名
type Handler = micro(Request) -> Response
type Predicate<T> = micro(T) -> bool
```

## 常量定义

```valkyrie
# 基本常量
const PI: f64 = 3.14159265359
const MAX_USERS: i32 = 1000
const APP_NAME: string = "MyApp"

# 复杂常量
const DEFAULT_CONFIG: Config = Config {
    timeout: 30,
    retries: 3,
    debug: false
}

# 计算常量
const BUFFER_SIZE: usize = 1024 * 1024  # 1MB
const HALF_PI: f64 = PI / 2.0
```

## 模块定义

```valkyrie
# 模块声明
mod utils {
    public micro helper_function() {
        # 实现
    }
    
    public class UtilityClass {
        # 实现
    }
}

# 使用模块
using utils::helper_function
using utils::UtilityClass

# 重导出
public using utils::*
```

## 泛型定义

```valkyrie
# 泛型函数
micro swap<T>(mut a: T, mut b: T) {
    let temp = a
    a = b
    b = temp
}

# 泛型类
class Container<T> {
    value: T
    
    new(value: T) {
        Self { value }
    }
    
    get(self) -> T {
        self.value
    }
    
    set(mut self, new_value: T) {
        self.value = new_value
    }
}

# 约束泛型
class SortedList<T> where T: Ord {
    items: [T]
    
    insert(mut self, item: T) {
        # 保持排序插入
        let pos = self.items.binary_search(item).default { $e }
        self.items.insert(pos, item)
    }
}
```


## 属性和装饰器

```valkyrie
# 属性装饰器
@.derive(Debug, Clone, PartialEq)
class Point {
    x: f64
    y: f64
}

@.test
micro test_addition() {
    @assert_equal(2 + 2, 4)
}

@.deprecated("Use new_function instead")
micro old_function() {
    # 已废弃的函数
}

@.inline
micro fast_calculation(x: i32) -> i32 {
    x * x + 2 * x + 1
}
```



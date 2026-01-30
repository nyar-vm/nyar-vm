# Trait 系统 (Trait System)

## 概述

Valkyrie 的 Trait 系统提供了强大的抽象机制，支持接口定义、默认实现和多重继承。Trait 系统是 Valkyrie 面向对象编程的核心组成部分。

## 基本 Trait 定义

Valkyrie 使用 `trait` 关键字定义接口。

### 简单 Trait

```valkyrie
trait Display {
    micro fmt(self) -> string
}

trait Clone {
    micro clone(self) -> Self
}

trait Debug {
    micro debug_fmt(self) -> string {
        # 默认实现
        @format("{}@{:p}", self.type_name(), &self)
    }
}
```

### 带关联类型的 Trait

```valkyrie
trait Iterator {
    type Item
    
    micro next(mut self) -> Self::Item?
    
    micro collect⟨C: FromIterator⟨Self::Item⟩⟩(self) -> C {
        C::from_iter(self)
    }
}

trait FromIterator⟨T⟩ {
    micro from_iter⟨I: Iterator⟨Item = T⟩⟩(iter: I) -> Self
}
```

### 带约束的 Trait

```valkyrie
trait PartialEq⟨Rhs = Self⟩ {
    micro eq(self, other: &Rhs) -> bool
    
    micro ne(self, other: &Rhs) -> bool {
        !self.eq(other)
    }
}

trait Ord: PartialEq + PartialOrd {
    micro cmp(self, other: &Self) -> Ordering
}
```

## Trait 实现 (imply)

Valkyrie 使用 `imply` 关键字为特定类型实现 Trait。

### 基本实现

```valkyrie
class Point {
    x: f64,
    y: f64,
}

imply Point: Display {
    micro fmt(self, f: mut Formatter) -> Result {
        f.write(@format("({}, {})", self.x, self.y))
    }
}

imply Point: Clone {
    micro clone(self) -> Self {
        Point { x: self.x, y: self.y }
    }
}
```

### 泛型实现

泛型使用数学角括号 `⟨ ⟩`。

```valkyrie
imply⟨T: Display⟩ [T]: Display {
    micro fmt(self) -> string {
        let items = self.iter()
            .map { $item.fmt() }
            .collect::⟨[string]⟩()
            .join(", ")
        @format("[{}]", items)
    }
}

imply⟨T: Clone⟩ [T]: Clone {
    micro clone(self) -> Self {
        self.iter().map { $item.clone() }.collect()
    }
}
```

### 条件实现

```valkyrie
imply⟨T: PartialEq⟩ [T]: PartialEq {
    micro eq(self, other: Self) -> bool {
        self.len() == other.len() && 
        self.iter().zip(other.iter()).all { $a.eq($b) }
    }
}
```

## 匿名 Trait

Valkyrie 支持匿名 trait，可以在函数参数中直接定义：

```valkyrie
# 匿名 trait 作为参数
micro process_drawable(drawable: ftrait {
    micro draw(self)
    micro get_bounds(self) -> Rectangle
}) {
    let bounds = drawable.get_bounds()
    print("Drawing object with bounds: {}", bounds)
    drawable.draw()
}

# 使用匿名 trait
let circle = class {
    radius: f64,
    
    micro draw(self) {
        print("Drawing circle with radius {}", self.radius)
    }
    
    micro get_bounds(self) -> Rectangle {
        Rectangle::new(-self.radius, -self.radius, 
                      self.radius * 2, self.radius * 2)
    }
}

process_drawable(circle { radius: 5.0 })
```

### 匿名 Trait 继承

```valkyrie
# 继承现有 trait 的匿名 trait
micro handle_serializable(obj: ftrait(Display, Clone) {
    micro serialize(self) -> string
}) {
    print("Object: {}", obj.fmt())
    let cloned = obj.clone()
    let serialized = obj.serialize()
    print("Serialized: {}", serialized)
}
```

## Trait 对象

### 动态分发

```valkyrie
trait Animal {
    micro make_sound(self)
    micro name(self) -> string
}

class Dog {
    name: string,
}

imply Dog: Animal {
    micro make_sound(self) {
        print("Woof!")
    }
    
    micro name(self) -> string {
        self.name.clone()
    }
}

class Cat {
    name: string,
}

imply Cat: Animal {
    micro make_sound(self) {
        print("Meow!")
    }
    
    micro name(self) -> string {
        self.name.clone()
    }
}

# 使用 trait 对象
let animals: [Animal] = [
    Dog { name: "Buddy" },
    Cat { name: "Whiskers" },
]

for animal in animals {
    print("{} says:", animal.name())
    animal.make_sound()
}
```

# Trait 对象安全

```valkyrie
# 对象安全的 trait
trait Draw {
    micro draw(self)  # 接收 self，对象安全
}

# 非对象安全的 trait
trait Clone {
    micro clone(self) -> Self  # 返回 Self，非对象安全
}

# 使用 where 子句限制
trait Container {
    type Item
    
    micro get(self, index: usize) -> Self::Item?
    
    # 只有当 Self::Item 实现了 Display 时才能调用
    micro display_item(self, index: usize) 
    where Self::Item: Display {
        if let item = self.get(index)? {
            print("{}", item.fmt())
        }
    }
}
```

## 高级特性

### 关联常量

```valkyrie
trait MathConstants {
    const PI: f64 = 3.14159265359
    const E: f64 = 2.71828182846
    
    micro circle_area(radius: f64) -> f64 {
        Self::PI * radius * radius
    }
}

class Calculator {}

imply Calculator: MathConstants {}

let area = Calculator::circle_area(5.0)
```

### 高阶 Trait 边界

```valkyrie
# 高阶 trait 边界
micro map_closure⟨F, T, U⟩(items: [T], f: F) -> [U]
where
    F: for⟨'a⟩ Fn(T) -> U,
{
    items.iter().map(f).collect()
}

# 使用示例
let numbers = [1, 2, 3, 4, 5]
let doubled = map_closure(numbers, micro(x) { x * 2 })
```

### Trait 别名

```valkyrie
# 定义 trait 别名
trait Printable = Display + Debug + Clone

# 使用 trait 别名
micro print_info⟨T: Printable⟩(item: T) {
    print("Display: {}", item.fmt())
    print("Debug: {}", item.debug_fmt())
    let cloned = item.clone()
    print("Cloned: {}", cloned.fmt())
}
```

## 派生宏

Valkyrie 提供了自动派生常用 trait 的宏：

```valkyrie
@.derive(Debug, Clone, PartialEq, Eq, Hash)
class User {
    id: u64,
    name: string,
    email: string,
}

@.derive(Display)
class Point {
    x: f64,
    y: f64,
}

# 自定义派生行为
@.derive(Debug, Clone)
@.derive_display(format = "User({})", field = "name")
class SimpleUser {
    name: string,
    internal_id: u64,  # 不会在 Display 中显示
}
```

## 最佳实践

### 1. Trait 设计原则

```valkyrie
# 好的设计：单一职责
trait Readable {
    micro read(mut self, buffer: mut [u8]) -> Result⟨usize, Error⟩
}

trait Writable {
    micro write(self, data: [u8]) -> Result⟨usize, Error⟩
}

# 组合使用
trait ReadWrite: Readable + Writable {}
```

### 2. 使用关联类型 vs 泛型参数

```valkyrie
# 使用关联类型：每个类型只有一个实现
trait Iterator {
    type Item
    micro next(mut self) -> Self::Item?
}

# 使用泛型参数：可以有多个实现
trait From⟨T⟩ {
    micro from(value: T) -> Self
}

# string 可以从多种类型转换
imply string: From⟨string⟩ { ... }
imply string: From⟨char⟩ { ... }
imply string: From⟨[char]⟩ { ... }
```

### 3. 错误处理

```valkyrie
trait TryFrom⟨T⟩ {
    type Error
    
    micro try_from(value: T) -> Result⟨Self, Self::Error⟩
}

trait TryInto⟨T⟩ {
    type Error
    
    micro try_into(self) -> Result⟨T, Self::Error⟩
}

# 自动实现
imply⟨T, U⟩ T: TryInto⟨U⟩ 
where U: TryFrom⟨T⟩ {
    type Error = U::Error
    
    micro try_into(self) -> Result⟨U, Self::Error⟩ {
        U::try_from(self)
    }
}
```

## 总结

Valkyrie 的 trait 系统提供了：

1. **灵活的抽象**：通过 trait 定义行为接口
2. **代码复用**：通过默认实现和泛型
3. **类型安全**：编译时检查 trait 边界
4. **动态分发**：通过 trait 对象支持运行时多态
5. **匿名 trait**：支持临时的行为定义
6. **组合能力**：通过 trait 边界组合多个能力

正确使用 trait 系统可以编写出既灵活又类型安全的代码。
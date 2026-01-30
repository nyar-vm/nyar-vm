# 属性系统 (Property System)

## 概述

Valkyrie 的属性系统提供了一种优雅的方式来封装字段访问，允许开发者定义 getter 和 setter 方法，同时保持简洁的访问语法。属性系统遵循统一访问原则，使得字段访问和方法调用在语法上保持一致。

## 基本属性定义

### Getter 属性

```valkyrie
class Rectangle {
    width: f64,
    height: f64,
    
    # 计算属性 - 面积
    get area(self) -> f64 {
        self.width * self.height
    }
    
    # 计算属性 - 周长
    get perimeter(self) -> f64 {
        2.0 * (self.width + self.height)
    }
}

# 使用方式
let rect = Rectangle { width: 10.0, height: 5.0 }
let area = rect.area        # 调用 getter，返回 50.0
let perimeter = rect.perimeter  # 调用 getter，返回 30.0
```

### Setter 属性

```valkyrie
class Temperature {
    celsius: f64,
    
    # Getter - 获取华氏温度
    get fahrenheit(self) -> f64 {
        self.celsius * 9.0 / 5.0 + 32.0
    }
    
    # Setter - 设置华氏温度
    set fahrenheit(mut self, value: f64) {
        self.celsius = (value - 32.0) * 5.0 / 9.0
    }
    
    # Getter - 获取开尔文温度
    get kelvin(self) -> f64 {
        self.celsius + 273.15
    }
    
    # Setter - 设置开尔文温度
    set kelvin(mut self, value: f64) {
        self.celsius = value - 273.15
    }
}

# 使用方式
let mut temp = Temperature { celsius: 25.0 }
print("摄氏度: ${temp.celsius}")     # 25.0
print("华氏度: ${temp.fahrenheit}")   # 77.0
print("开尔文: ${temp.kelvin}")      # 298.15

# 通过 setter 修改温度
temp.fahrenheit = 86.0  # 设置华氏温度
print("摄氏度: ${temp.celsius}")     # 30.0

temp.kelvin = 300.0     # 设置开尔文温度
print("摄氏度: ${temp.celsius}")     # 26.85
```

## 只读和只写属性

### 只读属性

```valkyrie
class Person {
    first_name: string,
    last_name: string,
    birth_year: u32,
    
    # 只读属性 - 全名
    get full_name(self) -> string {
        "${self.first_name} ${self.last_name}"
    }
    
    # 只读属性 - 年龄（基于当前年份）
    get age(self) -> u32 {
        2024 - self.birth_year  # 简化示例
    }
}

let person = Person {
    first_name: "张",
    last_name: "三",
    birth_year: 1990
}

print(person.full_name)  # "张 三"
print(person.age)        # 34
# person.full_name = "李四"  # 编译错误：没有 setter
```

### 只写属性

```valkyrie
class Logger {
    messages: [string],
    
    # 只写属性 - 添加日志消息
    set message(mut self, msg: string) {
        let timestamp = get_current_timestamp()
        self.messages.push("[${timestamp}] ${msg}")
    }
    
    # 获取所有消息的方法
    micro get_messages(self) -> &[string] {
        &self.messages
    }
}

let mut logger = Logger { messages: [] }
logger.message = "系统启动"  # 使用 setter
logger.message = "用户登录"  # 使用 setter
# let msg = logger.message   # 编译错误：没有 getter
```

## 属性验证

```valkyrie
class BankAccount {
    balance: f64,
    min_balance: f64,
    
    # 带验证的 setter
    set balance(mut self, value: f64) {
        if value < self.min_balance {
            panic("余额不能低于最低限额: ${self.min_balance}")
        }
        self.balance = value
    }
    
    get balance(self) -> f64 {
        self.balance
    }
}

let mut account = BankAccount {
    balance: 1000.0,
    min_balance: 100.0
}

account.balance = 500.0   # 正常设置
# account.balance = 50.0  # 运行时 panic
```

## 懒加载属性

```valkyrie
class DataProcessor {
    raw_data: [string],
    processed_data: [ProcessedItem]?,
    
    # 懒加载的计算属性
    get processed(mut self) -> &[ProcessedItem] {
        if self.processed_data.is_none() {
            let processed = self.raw_data
                .iter()
                .map { $item -> self.process_item($item) }
                .collect()
            self.processed_data = Some(processed)
        }
        
        if let Some(ref data) = self.processed_data {
            data
        } else {
            unreachable!()
        }
    }
    
    micro process_item(self, item: &string) -> ProcessedItem {
        # 复杂的处理逻辑
        ProcessedItem::from(item)
    }
}
```

## 属性链式调用

```valkyrie
class Builder {
    name: string?,
    age: u32?,
    email: string?,
    
    # 链式 setter
    set name(mut self, value: string) -> Self {
        self.name = Some(value)
        self
    }
    
    set age(mut self, value: u32) -> Self {
        self.age = Some(value)
        self
    }
    
    set email(mut self, value: string) -> Self {
        self.email = Some(value)
        self
    }
    
    micro build(self) -> Person {
        Person {
            name: self.name.unwrap_or("Unknown"),
            age: self.age.unwrap_or(0),
            email: self.email.unwrap_or("unknown@example.com")
        }
    }
}

# 链式调用
let person = Builder::new()
    .name("Alice")
    .age(30)
    .email("alice@example.com")
    .build()
```

## 静态属性

```valkyrie
class MathConstants {
    # 静态只读属性
    static get pi() -> f64 {
        3.14159265359
    }
    
    static get e() -> f64 {
        2.71828182846
    }
    
    # 静态可变属性
    static mut counter: u32 = 0
    
    static get next_id() -> u32 {
        Self::counter += 1
        Self::counter
    }
}

# 使用静态属性
let pi_value = MathConstants::pi
let id1 = MathConstants::next_id  # 1
let id2 = MathConstants::next_id  # 2
```

## 属性重写

```valkyrie
class Shape {
    # 虚拟属性
    virtual get area(self) -> f64 {
        0.0
    }
}

class Circle: Shape {
    radius: f64,
    
    # 重写父类属性
    override get area(self) -> f64 {
        3.14159 * self.radius * self.radius
    }
}

class Square: Shape {
    side: f64,
    
    # 重写父类属性
    override get area(self) -> f64 {
        self.side * self.side
    }
}
```

## 最佳实践

### 1. 使用属性进行数据封装

```valkyrie
# 好的实践：使用属性封装内部状态
class Counter {
    value: u32,
    
    get count(self) -> u32 {
        self.value
    }
    
    set count(mut self, new_value: u32) {
        if new_value > 1000 {
            panic("计数器值不能超过 1000")
        }
        self.value = new_value
    }
    
    micro increment(mut self) {
        self.count = self.count + 1
    }
}
```

### 2. 避免副作用过大的 Getter

```valkyrie
# 避免：getter 中有复杂的副作用
class BadExample {
    get data(mut self) -> [string] {
        # 不好：每次访问都重新计算
        expensive_computation()
    }
}

# 推荐：使用懒加载或缓存
class GoodExample {
    cached_data: [string]?,
    
    get data(mut self) -> &[string] {
        if self.cached_data.is_none() {
            self.cached_data = Some(expensive_computation())
        }
        self.cached_data.as_ref().unwrap()
    }
}
```

### 3. 保持属性语义的一致性

```valkyrie
class Rectangle {
    width: f64,
    height: f64,
    
    # 好的实践：getter 和 setter 操作相同的概念
    get area(self) -> f64 {
        self.width * self.height
    }
    
    # 如果提供 area setter，应该合理地更新 width 和 height
    set area(mut self, new_area: f64) {
        let ratio = (new_area / self.area).sqrt()
        self.width *= ratio
        self.height *= ratio
    }
}
```

## 总结

Valkyrie 的属性系统提供了：

1. **统一访问原则**：字段和计算属性使用相同的访问语法
2. **数据封装**：通过 getter/setter 控制数据访问
3. **计算属性**：支持动态计算的属性值
4. **验证机制**：在 setter 中添加数据验证逻辑
5. **懒加载**：支持延迟计算和缓存
6. **链式调用**：支持流畅的 API 设计

正确使用属性系统可以提高代码的封装性、可维护性和用户体验。
# 控制流

Valkyrie 提供了丰富的控制流语句，用于控制程序的执行流程。

## 条件语句

### if 语句

```valkyrie
# 基本 if 语句
if condition {
    # 执行代码
}

# if-else 语句
if x > 0 {
    print("正数")
}
else {
    print("非正数")
}

# if-else if-else 链
if score >= 90 {
    grade = "A"
}
else if score >= 80 {
    grade = "B"
}
else if score >= 70 {
    grade = "C"
}
else {
    grade = "F"
}

# if 表达式（返回值）
let result = if x > 0 { "positive" } else { "non-positive" }

# 多行 if 表达式
let message = if user.is_admin {
    "管理员用户"
}
else if user.is_premium {
    "高级用户"
}
else {
    "普通用户"
}
```

### 条件表达式

```valkyrie
# 三元运算符风格
let max = if a > b { a } else { b }

# 链式条件
let status = if online { "在线" } else if busy { "忙碌" } else { "离线" }
```

## 循环语句

### loop 语句（无限循环）

```valkyrie
# 基本无限循环
loop {
    # 无限执行的代码
    if should_break {
        break
    }
}

# 带标签的循环
'outer: loop {
    'inner: loop {
        if condition1 {
            break 'outer  # 跳出外层循环
        }
        if condition2 {
            break 'inner  # 跳出内层循环
        }
    }
}

# 循环返回值
let result = loop {
    let input = get_input()
    if input.is_valid() {
        break input.value()  # 返回值
    }
}
```

### while 语句

```valkyrie
# 基本 while 循环
while condition {
    # 当条件为真时执行
    update_condition()
}

# 复杂条件
while x > 0 && y < 100 {
    x -= 1
    y += 2
}

# while let 模式匹配
while let Some { value: item } = iterator.next() {
    process(item)
}

# 带标签的 while 循环
'search: while has_more_data() {
    let data = get_next_data()
    if data.is_target() {
        break 'search
    }
}
```

### until 语句

```valkyrie
# until 循环（当条件为假时执行）
until condition {
    # 当条件为假时执行
    update_condition()
}

# 等价于 while !condition
until x <= 0 {
    x -= 1
}

# until let 模式匹配
until let None = optional_value {
    process(optional_value.unwrap())
    optional_value = get_next_optional()
}
```

### for 语句

```valkyrie
# 范围循环
for i in 0..<10 {
    print(i)  # 输出 0 到 9
}

# 包含结束值的范围
for i in 0..=10 {
    print(i)  # 输出 0 到 10
}

# 数组迭代
let numbers = [1, 2, 3, 4, 5]
for num in numbers {
    print(num)
}

# 带索引的迭代
for (index, value) in numbers.enumerate() {
    print(f"索引 {index}: 值 {value}")
}

# 字符串迭代
for char in "hello".chars() {
    print(char)
}

# 对象属性迭代
for (key, value) in object.entries() {
    print(f"{key}: {value}")
}

# 带条件的 for 循环
for item in collection where item.is_valid() {
    process(item)
}

# 嵌套循环
for i in 0..<3 {
    for j in 0..<3 {
        print(f"({i}, {j})")
    }
}
```

## 模式匹配

### match 语句

Valkyrie 的 `match` 语句提供了强大的结构化模式匹配能力。

```valkyrie
match value {
    case 1: print("一")
    case 2: print("二")
    case _: print("其他")
}
```

### 模式解构

```valkyrie
unity Option⟨T⟩ {
    Some { value: T }
    None
}

let result = Some { value: 42 }

match result {
    case Some { value }: print("值: ${value}")
    case None: print("空值")
}
```

### 解构赋值

```valkyrie
# 数组解构
let [first, second, ..rest] = array  # 解构数组到kvs
let [a, _, c] = [1, 2, 3]  # 忽略第二个元素

# 元组解构
let (x, y, z) = (1, 2, 3)
let (name, ..) = ("Alice", 25, "Engineer")  # 只取第一个

# 对象解构
let { name, age } = person
let { x: new_x, y: new_y } = point  # 重命名
let { name, ..rest } = user  # 解构dict到kvs
```

## 异常处理

### catch 语句（异常处理器）

```valkyrie
# 基本异常处理
catch {
    risky_operation()
} handle error {
    print(f"发生错误: {error}")
}

# 多种异常类型处理
catch {
    complex_operation()
} handle NetworkError(msg) {
    print(f"网络错误: {msg}")
} handle ValidationError(field) {
    print(f"验证错误: {field}")
} handle error {
    print(f"未知错误: {error}")
}

# 带资源管理的异常处理
using resource = acquire_resource() {
    catch {
        file_operation()
    } handle IOError(msg) {
        print("IO错误: ${msg}")
    }
}  # resource会自动清理

# 异常处理表达式
let result = catch {
    parse_number(input)
} handle ParseError(_) {
    0  # 默认值
}

# 嵌套异常处理
catch {
    catch {
        inner_operation()
    } handle InnerError(e) {
        handle_inner_error(e)
    }
    outer_operation()
} handle OuterError(e) {
    handle_outer_error(e)
}
```

### 异常传播

```valkyrie
# 使用 ? 操作符传播异常
micro process_file(path: string) -> Result⟨string, IOError⟩ {
    let content = read_file(path)?  # 如果失败则提前返回错误
    let processed = transform(content)?
    Fine { value: processed }
}

# 手动抛出异常
micro validate_age(age: i32) -> Result⟨unit, ValidationError⟩ {
    if age < 0 {
        throw ValidationError("年龄不能为负数")
    }
    if age > 150 {
        throw ValidationError("年龄不能超过150")
    }
    Fine { value: () }
}
```

## 控制流关键字

### break 和 continue

```valkyrie
# break 跳出循环
for i in 0..<10 {
    if i == 5 {
        break  # 跳出循环
    }
    print(i)
}

# continue 跳过当前迭代
for i in 0..<10 {
    if i % 2 == 0 {
        continue  # 跳过偶数
    }
    print(i)  # 只打印奇数
}

# 带标签的 break 和 continue
'outer: for i in 0..<3 {
'inner: for j in 0..<3 {
        if i == 1 && j == 1 {
            break 'outer  # 跳出外层循环
        }
        if j == 2 {
            continue 'outer  # 继续外层循环的下一次迭代
        }
        print(f"({i}, {j})")
    }
}

# break 返回值
let found = loop {
    let item = get_next_item()
    if item.is_target() {
        break Some { value: item }  # 返回找到的项
    }
    if no_more_items() {
        break None  # 返回空值
    }
}
```

### return 语句

```valkyrie
# 函数返回
micro calculate(x: i32, y: i32) -> i32 {
    if x < 0 || y < 0 {
        return -1  # 提前返回
    }
    x + y  # 隐式返回
}

# 空返回
micro log_message(msg: string) {
    if msg.is_empty() {
        return  # 提前返回，无返回值
    }
    print(msg)
}
```

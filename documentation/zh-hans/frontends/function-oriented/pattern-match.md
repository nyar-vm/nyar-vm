# 模式匹配 (Match)

Valkyrie 提供了强大的模式匹配功能，支持多种匹配模式和语法形式。

## 基本 Match 语法

### 标准 Match 语句

```valkyrie
# 基本模式匹配
match value {
    case 1: "one"
    case 2: "two"
    case 3: "three"
    else: "other"
}

# 范围匹配
match score {
    case 90..=100: "A"
    case 80..=89: "B"
    case 70..=79: "C"
    case 60..=69: "D"
    else: "F"
}

# 多值匹配
match day {
    case "Saturday" | "Sunday": "Weekend"
    case "Monday"..="Friday": "Weekday"
    else: "Invalid day"
}
```

### 表达式 Match 语法

```valkyrie
# 表达式形式的 match
let result = match value {
    case 1: "one"
    case 2: "two"
    case 3: "three"
    else: "other"
}

# 链式调用
let processed = match input.transform() {
    case Fine(value): value * 2
    case Fail(error): 0
}
```

## 解构匹配

### 元组解构

```valkyrie
match point {
    case (0, 0): "Origin"
    case (x, 0): "On X-axis at ${x}"
    case (0, y): "On Y-axis at ${y}"
    case (x, y): "Point at (${x}, ${y})"
}

# 嵌套元组
match nested {
    case ((a, b), c): "Nested: ${a}, ${b}, ${c}"
    case (x, (y, z)): "Other nested: ${x}, ${y}, ${z}"
    else: "No match"
}
```

### 数组解构

```valkyrie
match array {
    case []: "Empty array"
    case [x]: "Single element: ${x}"
    case [first, second]: "Two elements: ${first}, ${second}"
    case [head, ..tail]: "Head: ${head}, Tail length: ${tail.len()}"
    case [.., last]: "Last element: ${last}"
    case [first, .., last]: "First: ${first}, Last: ${last}"
}

# 固定长度匹配
match coordinates {
    case [x, y]: "2D point: (${x}, ${y})"
    case [x, y, z]: "3D point: (${x}, ${y}, ${z})"
    else: "Unsupported dimension"
}
```

### 对象解构

```valkyrie
match person {
    case { name: "Alice", age }: "Alice is ${age} years old"
    case { name, age: 18..=65 }: "${name} is working age"
    case { name, age, city: "Beijing" }: "${name} from Beijing, age ${age}"
}
```

## Guard 条件

可以使用 `if` 子句为模式匹配添加额外的过滤条件：

```valkyrie
match point {
    case (x, y) if x == y: "On diagonal"
    case (x, y) if x > y: "Below diagonal"
    case (x, y): "Above diagonal"
}
```

## 类型匹配

模式匹配也可以用于检查和转换类型：

```valkyrie
match shape {
    case s: Circle: "Circle with radius ${s.radius}"
    case s: Rectangle: "Rectangle ${s.width}x${s.height}"
    case _: "Unknown shape"
}
```

# 生成器

Valkyrie 的生成器是一种特殊的函数，可以通过 `yield` 关键字产生一系列值。生成器提供了一种惰性计算的方式，只在需要时计算下一个值，非常适合处理大量数据或无限序列。

## 基本生成器语法

### 简单生成器

```valkyrie
# 基本生成器函数
micro count_up(max: i32) {
    let mut i = 0
    while i < max {
        yield i
        i += 1
    }
}

# 使用生成器
let counter = count_up(5)
for value in counter {
    print(value)  # 输出: 0, 1, 2, 3, 4
}
```

### 无限生成器

```valkyrie
# 斐波那契数列生成器
micro fibonacci() {
    let mut a = 0
    let mut b = 1
    loop {
        yield a
        let temp = a + b
        a = b
        b = temp
    }
}

# 获取前10个斐波那契数
let fib = fibonacci()
for i in 0..<10 {
    print(fib.next())  # 0, 1, 1, 2, 3, 5, 8, 13, 21, 34
}
```

### 带返回值的生成器

```valkyrie
# 生成器可以有最终返回值
micro process_items(items: [string]) -> i32 {
    let mut count = 0
    for item in items {
        if item.is_valid() {
            yield item.process()
            count += 1
        }
    }
    count  # 最终返回处理的项目数量
}

# 使用
let processor = process_items(["item1", "item2", "item3"])
for result in processor {
    print("Processed: ${result}")
}
let total_count = processor.return_value()  # 获取最终返回值
```

## 生成器状态管理

### 生成器生命周期

```valkyrie
# 生成器状态枚举
unity GeneratorState {
    Created,     # 已创建但未开始
    Running,     # 正在执行
    Suspended,   # 已暂停（yield）
    Completed,   # 已完成
    Fail { error: any } # 发生错误
}

# 检查生成器状态
micro example_generator() {
    print("开始执行")
    yield "第一个值"
    print("继续执行")
    yield "第二个值"
    print("执行完成")
}

let gen = example_generator()
print(gen.state())  # Created

let first = gen.next()
print(gen.state())  # Suspended
print(first)        # "第一个值"

let second = gen.next()
print(gen.state())  # Suspended
print(second)       # "第二个值"

gen.next()          # 完成执行
print(gen.state())  # Completed
```

### 生成器控制

```valkyrie
# 手动控制生成器执行
micro controlled_generator() {
    let mut value = 0
    loop {
        let input = yield value
        if input != null {
            value = input  # 接收外部输入
        } else {
            value += 1     # 默认递增
        }
    }
}

let gen = controlled_generator()
print(gen.next())        # 0
print(gen.send(10))      # 10 (发送值给生成器)
print(gen.next())        # 11
print(gen.send(100))     # 100
```

## 生成器管道

### 管道处理

```valkyrie
# 生成器管道处理
micro pipeline_stage1(input: Iterator<i32>) {
    for value in input {
        yield value * 2  # 第一阶段：乘以2
    }
}

micro pipeline_stage2(input: Iterator<i32>) {
    for value in input {
        if value % 4 == 0 {
            yield value  # 第二阶段：过滤4的倍数
        }
    }
}

micro pipeline_stage3(input: Iterator<i32>) {
    for value in input {
        yield "Result: ${ value }"  # 第三阶段：格式化
    }
}

# 构建管道
let numbers = [1, 2, 3, 4, 5, 6, 7, 8]
let stage1 = pipeline_stage1(numbers.iter())
let stage2 = pipeline_stage2(stage1)
let stage3 = pipeline_stage3(stage2)

for result in stage3 {
    print(result)  # "Result: 4", "Result: 8", "Result: 12", "Result: 16"
}
```

### 组合生成器

```valkyrie
# 组合多个生成器
micro combine_generators(gen1: Generator<i32>, gen2: Generator<i32>) {
    # 交替产生两个生成器的值
    loop {
        let val1 = gen1.next()
        let val2 = gen2.next()
        
        if val1.is_some() {
            yield val1.unwrap()
        }
        if val2.is_some() {
            yield val2.unwrap()
        }
        
        if val1.is_none() && val2.is_none() {
            break
        }
    }
}

let gen1 = count_up(3)  # 0, 1, 2
let gen2 = count_up(2)  # 0, 1
let combined = combine_generators(gen1, gen2)

for value in combined {
    print(value)  # 0, 0, 1, 1, 2
}
```

## 高级生成器模式

### 惰性计算

```valkyrie
# 惰性计算素数
micro prime_generator() {
    let mut candidates = 2..
    let mut primes = []
    
    for candidate in candidates {
        let is_prime = primes.all(|p| candidate % p != 0)
        if is_prime {
            primes.push(candidate)
            yield candidate
        }
    }
}

# 获取前10个素数
let primes = prime_generator()
for i in 0..<10 {
    print(primes.next())  # 2, 3, 5, 7, 11, 13, 17, 19, 23, 29
}
```

### 文件处理生成器

```valkyrie
# 逐行读取文件
micro read_lines(filename: String) {
    let file = open_file(filename)
    try {
        while !file.eof() {
            let line = file.read_line()
            if !line.is_empty() {
                yield line.trim()
            }
        }
    } finally {
        file.close()
    }
}

# 使用
for line in read_lines("data.txt") {
    print("Line: ${ line }")
}
```

### 数据转换生成器

```valkyrie
# 数据转换管道
micro transform_data(data: Iterator<String>) {
    for item in data {
        # 解析JSON
        let parsed = json_parse(item)
        if parsed.is_ok() {
            let obj = parsed.unwrap()
            
            # 验证数据
            if obj.has_field("id") && obj.has_field("name") {
                # 转换格式
                let transformed = {
                    id: obj.id,
                    name: obj.name.to_uppercase(),
                    timestamp: current_time()
                }
                yield transformed
            }
        }
    }
}
```

## 错误处理

### 生成器异常处理

```valkyrie
# 生成器中的异常处理
micro error_prone_generator() {
    try {
        yield "开始处理"
        
        let risky_operation = perform_risky_task()
        yield "风险操作完成"
        
        if risky_operation.is_error() {
            raise "操作失败"
        }
        
        yield "处理成功"
    }
    .catch {
        case _:
            yield "发生错误: ${ error }"
            raise error  # 重新抛出异常
    }
}

# 使用带错误处理的生成器
let gen = error_prone_generator()
try {
    for status in gen {
        print(status)
    }
}
.catch {
    case _:
        print("生成器异常: ${ error }")
}
```

## 最佳实践

### 1. 生成器设计原则

```valkyrie
# 保持生成器简单和专注
micro good_generator(data: [String]) {
    for item in data {
        if item.is_valid() {
            yield item.process()  # 只做一件事
        }
    }
}

# 避免在生成器中进行复杂的状态管理
# 不好的例子：
micro bad_generator() {
    let mut complex_state = ComplexState::new()
    # ... 复杂的状态逻辑
}
```

### 2. 资源管理

```valkyrie
# 确保资源正确释放
micro file_processor(filename: String) {
    using file = open_file(filename) {
        while !file.eof() {
            let line = file.read_line()
            yield process_line(line)
        }
    }  # 文件自动关闭
}
```

### 3. 性能考虑

```valkyrie
# 避免频繁的小yield
# 不好的例子：
micro inefficient_generator(data: [i32]) {
    for item in data {
        yield item  # 每个元素都yield
    }
}

# 好的例子：
micro efficient_generator(data: [i32]) {
    let mut batch = []
    for item in data {
        batch.push(item)
        if batch.len() >= 100 {
            yield batch  # 批量yield
            batch = []
        }
    }
    if !batch.is_empty() {
        yield batch  # 处理剩余项目
    }
}
```

### 4. 测试生成器

```valkyrie
# 生成器测试策略
micro test_generator() {
    let gen = count_up(3)
    
    # 测试生成的值
    @assert_equal(gen.next(), Some(0))
    @assert_equal(gen.next(), Some(1))
    @assert_equal(gen.next(), Some(2))
    @assert_equal(gen.next(), None)
    
    # 测试状态
    @assert_equal(gen.state(), GeneratorState::Completed)
}

# 生成器集成测试
micro test_pipeline() {
    let input = [1, 2, 3, 4]
    let pipeline = pipeline_stage1(input.iter())
    let results = pipeline.collect()
    
    @assert_equal(results, [2, 4, 6, 8])
}
```

### 5. 返回值限制

```valkyrie
# 生成器返回值不能是匿名类
# 错误示例：
micro bad_generator() -> class { x: i32 } {  # 编译错误
    yield 1
    class { x: 42 }  # 匿名类作为返回值会导致类型推断困难
}

# 正确示例：
class Result {
    x: i32
}

micro good_generator() -> Result {
    yield 1
    Result { x: 42 }  # 使用具名类型
}

# 或者使用类型别名
type GeneratorResult = class { x: i32 }

micro another_good_generator() -> GeneratorResult {
    yield 1
    GeneratorResult { x: 42 }
}
```
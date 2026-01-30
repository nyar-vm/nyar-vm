# 协程

Valkyrie 提供了强大的协程支持，通过 `yield` 关键字实现协作式多任务处理。协程允许函数在执行过程中暂停和恢复，非常适合处理异步操作和状态机。协程与生成器的主要区别在于协程更注重控制流的暂停和恢复，而不仅仅是产生值序列。

## 协程状态管理

### 协程生命周期

```valkyrie
# 协程状态枚举
union CoroutineState {
    Created,     # 已创建但未开始
    Running,     # 正在执行
    Suspended,   # 已暂停（yield）
    Completed,   # 已完成
    Fail { error: Any } # 发生错误
}

# 检查协程状态
micro example_coroutine() {
    print("开始执行")
    yield "第一个值"
    print("继续执行")
    yield "第二个值"
    print("执行完成")
}

let coro = example_coroutine()
print(coro.state())  # Created

let first = coro.next()
print(coro.state())  # Suspended
print(first)         # "第一个值"

let second = coro.next()
print(coro.state())  # Suspended
print(second)        # "第二个值"

coro.next()          # 完成执行
print(coro.state())  # Completed
```

### 协程控制

```valkyrie
# 手动控制协程执行
micro controlled_coroutine() {
    let mut state = "idle"
    loop {
        let command = yield state
        match command {
            case "start":
                state = "running"
            case "pause":
                state = "paused"
            case "stop":
                state = "stopped"
                break
            case _:
                state = "unknown_command"
        }
    }
}

let coro = controlled_coroutine()
print(coro.next())           # "idle"
print(coro.send("start"))    # "running"
print(coro.send("pause"))    # "paused"
print(coro.send("stop"))     # "stopped"
```

## 异步协程

### 异步操作

```valkyrie
# 异步协程
async micro fetch_data(url: string) -> string {
    print("开始请求: ${ url }")
    let response = http_get(url).await?
    yield "请求已发送"  # 可以在异步函数中使用 yield
    
    if response.status == 200 {
        yield "请求成功"
        response.body
    } else {
        raise "请求失败: ${ response.status }"
    }
}

# 使用异步协程
async micro main() {
    let fetcher = fetch_data("https://api.example.com/data")
    
    # 处理中间状态
    for status in fetcher {
        print("状态: ${ status }")
    }
    
    # 获取最终结果
    try {
        let data = fetcher.await?
        print("数据: ${ data }")
    }
    .catch {
        case _:
            print("错误: ${ error }")
    }
}
```

### 并发协程

```valkyrie
# 并发执行多个协程
async micro concurrent_processing(items: [string]) {
    let promises = items.map(async micro(item) {
        let result = process_item(item)
        yield "处理完成: ${ item }"
        result
    })
    
    # 等待所有 Promise 完成
    let results = Promise::all(promises).await?
    yield "所有任务完成"
    results
}

# 使用
async micro run_concurrent() {
    let processor = concurrent_processing(["item1", "item2", "item3"])
    
    for update in processor {
        print(update)
    }
    
    let final_results = processor.await?
    print("最终结果: ${ final_results }")
}
```

## 高级协程模式

### 状态机协程

```valkyrie
# 状态机实现
union State {
    Idle,
    Processing,
    Waiting,
    Complete
}

micro state_machine() {
    let mut state = State::Idle
    let mut data = null
    
    loop {
        state.match {
            case State::Idle: {
                yield "等待输入"
                data = yield_receive()  # 等待外部输入
                state = State::Processing
            }
            case State::Processing: {
                yield "处理中..."
                let result = process_data(data)
                if result.is_ok() {
                    state = State::Complete
                } else {
                    state = State::Waiting
                }
            }
            case State::Waiting: {
                yield "等待重试"
                sleep(1000)  # 等待1秒
                state = State::Processing
            }
            case State::Complete: {
                yield "处理完成"
                break
            }
        }
    }
}
```

### 协程池

```valkyrie
# 协程池管理
class CoroutinePool {
    coroutines: [Coroutine],
    max_size: i32,
    active_count: i32
    
    micro new(max_size: i32) -> Self {
        CoroutinePool {
            coroutines: [],
            max_size: max_size,
            active_count: 0
        }
    }
    
    micro spawn(task: micro() -> Any) -> bool {
        if self.active_count < self.max_size {
            let coro = Coroutine::new(task)
            self.coroutines.push(coro)
            self.active_count += 1
            true
        } else {
            false  # 池已满
        }
    }
    
    micro run_all() {
        while self.active_count > 0 {
            for coro in self.coroutines {
                if coro.state() == CoroutineState::Suspended {
                    let result = coro.resume()
                    yield "协程进度: ${ result }"
                    
                    if coro.state() == CoroutineState::Completed {
                        self.active_count -= 1
                    }
                }
            }
        }
        yield "所有协程完成"
    }
}
```

## 错误处理

### 协程异常处理

```valkyrie
# 协程中的异常处理
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

# 使用带错误处理的协程
let gen = error_prone_generator()
try {
    for status in gen {
        print(status)
    }
}
.catch {
    case _:
        print("协程异常: ${ error }")
}
```

## 最佳实践

### 1. 协程设计原则

```valkyrie
# 保持协程简单和专注
micro good_generator(data: [String]) {
    for item in data {
        if item.is_valid() {
            yield item.process()  # 只做一件事
        }
    }
}

# 避免在协程中进行复杂的状态管理
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
    let file = open_file(filename)
    try {
        while !file.eof() {
            let line = file.read_line()
            yield process_line(line)
        }
    }
    # 使用using确保文件关闭
    # using file = open_file(filename) { ... }
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

### 4. 测试协程

```valkyrie
# 协程测试策略
micro test_generator() {
    let gen = count_up(3)
    
    # 测试生成的值
    @assert_equal(gen.next(), 0)
    @assert_equal(gen.next(), 1)
    @assert_equal(gen.next(), 2)
    @assert_equal(gen.next(), null)
    
    # 测试状态
    @assert_equal(gen.state(), CoroutineState::Completed)
}

# 异步协程测试
async micro test_async_generator() {
    let gen = async_data_processor()
    
    let first_result = gen.next().await?
    assert!(first_result != null)
    
    let final_result = gen.collect_all().await?
    @assert_equal(final_result.len(), 5)
}
```

# 反应式编程

反应式编程是一种基于数据流和变化传播的编程范式。在 Valkyrie 中，反应式编程通过 Observable、Signal 和 Reactive 等抽象提供了强大的数据流处理能力。

## 核心概念

### Observable

Observable 是反应式编程的基础，表示一个可观察的数据流：

```valkyrie
# Observable 特征定义
trait Observable⟨T⟩ {
    micro subscribe⟨F⟩(self, observer: F) -> Subscription where F: micro(T) -> unit
    micro map⟨U, F⟩(self, f: F) -> Observable⟨U⟩ where F: micro(T) -> U
    micro filter⟨F⟩(self, predicate: F) -> Observable⟨T⟩ where F: micro(T) -> bool
    micro merge(self, other: Observable⟨T⟩) -> Observable⟨T⟩
    micro take(self, count: usize) -> Observable⟨T⟩
}

# 创建 Observable
let numbers = Observable.from([1, 2, 3, 4, 5])
let mouse_clicks = Observable.from_events("click")
let timer = Observable.interval(1000)  # 每秒触发一次
```

### Signal

Signal 是具有当前值的反应式状态：

```valkyrie
# Signal 定义
class Signal⟨T⟩ {
    private value: T
    private observers: [micro(T) -> unit]
    
    micro Signal(initial: T) -> Signal⟨T⟩
    micro get(self) -> T
    micro set(mut self, new_value: T)
    micro update⟨F⟩(mut self, updater: F) where F: micro(T) -> T
    micro subscribe⟨F⟩(self, observer: F) -> Subscription where F: micro(T) -> unit
}

# 使用 Signal
let counter = Signal(0)
let doubled = counter.map { $x * 2 }

# 订阅变化
counter.subscribe {
    print("计数器值: ${$value}")
}

# 更新值
counter.set(5)  # 输出: 计数器值: 5
counter.update { $x + 1 }  # 输出: 计数器值: 6
```

## 基本操作符

### 转换操作符

```valkyrie
# map - 转换每个值
let numbers = Observable.from([1, 2, 3, 4, 5])
let squares = numbers.map { $x * $x }

# flat_map - 扁平化映射
let words = Observable.from(["hello", "world"])
let characters = words.flat_map {
    Observable.from($word.chars())
}

# scan - 累积操作
let numbers = Observable.from([1, 2, 3, 4, 5])
let running_sum = numbers.scan(0) { $acc, $x -> $acc + $x }
# 输出: 1, 3, 6, 10, 15
```

### 过滤操作符

```valkyrie
# filter - 过滤值
let numbers = Observable.from([1, 2, 3, 4, 5, 6])
let evens = numbers.filter { $x % 2 == 0 }

# take - 取前 N 个值
let first_three = numbers.take(3)

# skip - 跳过前 N 个值
let after_two = numbers.skip(2)

# distinct - 去重
let unique = Observable.from([1, 1, 2, 2, 3, 3]).distinct()
```

### 组合操作符

```valkyrie
# merge - 合并多个流
let stream1 = Observable.from([1, 3, 5])
let stream2 = Observable.from([2, 4, 6])
let merged = stream1.merge(stream2)

# zip - 配对组合
let names = Observable.from(["Alice", "Bob", "Charlie"])
let ages = Observable.from([25, 30, 35])
let people = names.zip(ages).map { ($name, $age) ->
    Person { name: $name, age: $age }
}

# combine_latest - 最新值组合
let temperature = Signal(20.0)
let humidity = Signal(60.0)
let comfort_index = temperature.combine_latest(humidity).map { ($temp, $hum) ->
    calculate_comfort($temp, $hum)
}
```

## 实际应用示例

### 用户界面反应式更新

```valkyrie
# 反应式 UI 组件
class CounterComponent {
    private count: Signal⟨i32⟩
    private increment_clicks: Observable⟨unit⟩
    private decrement_clicks: Observable⟨unit⟩
    
    micro CounterComponent() -> CounterComponent {
        let count = Signal(0)
        let increment_clicks = Observable.from_events("increment")
        let decrement_clicks = Observable.from_events("decrement")
        
        # 响应点击事件
        increment_clicks.subscribe {
            count.update { $x + 1 }
        }
        
        decrement_clicks.subscribe {
            count.update { $x - 1 }
        }
        
        CounterComponent {
            count,
            increment_clicks,
            decrement_clicks
        }
    }
    
    micro render(self) -> Widget {
        let count_text = self.count.map { "计数: ${$value}" }
        
        VStack {
            Text(count_text)
            HStack {
                Button("增加").on_click(self.increment_clicks)
                Button("减少").on_click(self.decrement_clicks)
            }
        }
    }
}
```

### 数据流处理

```valkyrie
# 实时数据处理管道
class DataProcessor {
    micro process_sensor_data(sensor_stream: Observable⟨SensorReading⟩) -> Observable⟨ProcessedData⟩ {
        sensor_stream
            .filter { $reading.is_valid() }  # 过滤无效数据
            .map { $reading.normalize() }    # 标准化数据
            .buffer(Duration.seconds(5))          # 5秒缓冲窗口
            .map { $batch -> self.analyze_batch($batch) } # 批量分析
            .filter { $result.confidence > 0.8 } # 过滤低置信度结果
    }
    
    private micro analyze_batch(batch: [SensorReading]) -> ProcessedData {
        let average = batch.iter().map { $r.value }.sum() / batch.len()
        let variance = calculate_variance(batch)
        
        ProcessedData {
            timestamp: now(),
            average,
            variance,
            confidence: calculate_confidence(variance)
        }
    }
}

# 使用数据处理器
let processor = DataProcessor()
let sensor_stream = Observable.from_websocket("ws://sensor.example.com")
let processed_stream = processor.process_sensor_data(sensor_stream)

processed_stream.subscribe { $data ->
    print("处理结果: 平均值=${$data.average}, 置信度=${$data.confidence}")
    
    if $data.confidence > 0.95 {
        alert_system.notify("高置信度数据: ${$data}")
    }
}
```

### 异步操作组合

```valkyrie
# 反应式 HTTP 客户端
class ReactiveHttpClient {
    micro get⟨T⟩(url: string) -> Observable⟨Result⟨T, HttpError⟩⟩ {
        Observable.create { $observer ->
            async {
                try {
                    let response = http_get(url).await?
                    let data = response.json⟨T⟩().await?
                    $observer.next(Fine { value: data })
                    $observer.complete()
                }
                .catch {
                    case $e:
                        $observer.error($e)
                }
            }
        }
    }
    
    micro retry⟨T⟩(observable: Observable⟨Result⟨T, HttpError⟩⟩, max_retries: usize) -> Observable⟨Result⟨T, HttpError⟩⟩ {
        observable.catch_error { $error ->
            if max_retries > 0 {
                print("重试请求，剩余次数: ${max_retries}")
                Observable.timer(Duration.seconds(1))
                    .flat_map { self.retry(observable, max_retries - 1) }
            } else {
                Observable.error($error)
            }
        }
    }
}

# 使用示例
let client = ReactiveHttpClient()
let user_data = client.get⟨User⟩("https://api.example.com/user/123")
    .retry(3)  # 最多重试3次
    .timeout(Duration.seconds(10))  # 10秒超时

user_data.subscribe { $result ->
    match $result {
        case Fine { value: $user }:
            print("用户信息: ${$user.name}")
        case Fail { error: $error }:
            print("获取用户信息失败: ${$error}")
    }
}
```

## 错误处理

### 错误恢复策略

```valkyrie
# 错误处理操作符
trait ObservableErrorHandling⟨T⟩ {
    # 捕获错误并提供默认值
    micro catch_error⟨F⟩(self, handler: F) -> Observable⟨T⟩ where F: micro(Any) -> Observable⟨T⟩
    
    # 重试操作
    micro retry(self, count: usize) -> Observable⟨T⟩
    
    # 超时处理
    micro timeout(self, duration: Duration) -> Observable⟨T⟩
}

# 实际使用
let unreliable_stream = fetch_data_stream()
    .catch_error { $error ->
        print("发生错误: ${$error}，使用缓存数据")
        Observable.from(cached_data)
    }
    .retry(3)
    .timeout(Duration.seconds(30))

unreliable_stream.subscribe { $data ->
    process_data($data)
}
```

### 错误传播控制

```valkyrie
# 部分错误处理
let mixed_stream = Observable.from([1, 2, 3, 4, 5])
    .map { $x ->
        if $x == 3 {
            raise ValueError { error: "无效值: 3" }
        }
        $x * 2
    }
    .on_error_resume_next { $error ->
        print("跳过错误: ${$error}")
        Observable.empty()  # 跳过错误项
    }

mixed_stream.subscribe { $value ->
    print("处理值: ${$value}")
}  # 输出: 2, 4, 8, 10 (跳过了3)
```

## 资源管理

### 订阅生命周期

```valkyrie
# Subscription 管理
class Subscription {
    private is_disposed: bool
    private cleanup: micro() -> unit
    
    micro dispose(mut self) {
        if !self.is_disposed {
            self.cleanup()
            self.is_disposed = true
        }
    }
    
    micro is_disposed(self) -> bool {
        self.is_disposed
    }
}

# CompositeSubscription 用于管理多个订阅
class CompositeSubscription {
    private subscriptions: [Subscription]
    
    micro add(mut self, subscription: Subscription) {
        self.subscriptions.push(subscription)
    }
    
    micro dispose_all(mut self) {
        for subscription in self.subscriptions {
            subscription.dispose()
        }
        self.subscriptions.clear()
    }
}

# 使用示例
let composite = CompositeSubscription()

let sub1 = timer_stream.subscribe { print("定时器触发") }
let sub2 = click_stream.subscribe { print("点击事件") }

composite.add(sub1)
composite.add(sub2)

# 在组件销毁时清理所有订阅
composite.dispose_all()
```

### 背压处理

```valkyrie
# 背压策略
struct BackpressureStrategy {
    micro Buffer(capacity: usize)     # 缓冲策略
    micro Drop                        # 丢弃策略
    micro Latest                      # 保留最新策略
    micro Error                       # 错误策略
}

# 应用背压控制
let fast_producer = Observable.interval(Duration.milliseconds(1))  # 每毫秒产生数据
let slow_consumer = fast_producer
    .observe_on(Scheduler.computation())  # 在计算线程池处理
    .buffer(100)  # 缓冲100个元素
    .sample(Duration.seconds(1))  # 每秒采样一次

slow_consumer.subscribe { $batch ->
    print("处理批次，大小: ${$batch.len()}")
    # 慢速处理逻辑
    Thread.sleep(Duration.milliseconds(100))
}
```

## 调度器

### 线程调度

```valkyrie
# 调度器类型
struct Scheduler {
    micro CurrentThread    # 当前线程
    micro Computation      # 计算线程池
    micro IO               # I/O 线程池
    micro NewThread        # 新线程
    micro Trampoline       # 蹦床调度器
}

# 指定调度器
let data_stream = Observable.from_file("large_file.txt")
    .subscribe_on(Scheduler.IO())        # 在 I/O 线程读取文件
    .observe_on(Scheduler.Computation()) # 在计算线程处理数据
    .map { $line -> expensive_computation($line) }
    .observe_on(Scheduler.CurrentThread()) # 在主线程更新 UI

data_stream.subscribe { $result ->
    update_ui($result)  # UI 更新必须在主线程
}
```

## 测试支持

### 测试调度器

```valkyrie
# 测试用的虚拟时间调度器
class TestScheduler {
    private virtual_time: Duration
    private scheduled_actions: [(Duration, micro() -> unit)]
    
    micro advance_time_by(mut self, duration: Duration) {
        let target_time = self.virtual_time + duration
        
        while let (time, action)? = self.scheduled_actions.first() {
            if time <= target_time {
                self.virtual_time = time
                action()
                self.scheduled_actions.remove(0)
            } else {
                break
            }
        }
        
        self.virtual_time = target_time
    }
}

# 测试示例
#[test]
micro test_timer_observable() {
    let scheduler = TestScheduler()
    let timer = Observable.timer(Duration.seconds(5), scheduler)
    let mut received_values = []
    
    timer.subscribe { $value ->
        received_values.push($value)
    }
    
    # 推进虚拟时间
    scheduler.advance_time_by(Duration.seconds(3))
    assert_eq!(received_values.len(), 0)  # 还没到时间
    
    scheduler.advance_time_by(Duration.seconds(3))
    assert_eq!(received_values.len(), 1)  # 定时器触发
}
```

## 最佳实践

### 1. 避免内存泄漏

```valkyrie
# 正确的订阅管理
class Component {
    private subscriptions: CompositeSubscription
    
    micro Component() -> Component {
        let subscriptions = CompositeSubscription()
        
        # 订阅数据流
        let sub = data_stream.subscribe { $data ->
            self.handle_data($data)
        }
        
        subscriptions.add(sub)
        
        Component { subscriptions }
    }
    
    micro destroy(mut self) {
        # 组件销毁时清理订阅
        self.subscriptions.dispose_all()
    }
}
```

### 2. 合理使用操作符

```valkyrie
# 优化操作符链
let optimized_stream = source_stream
    .filter { $x.is_valid() }     # 尽早过滤
    .take(1000)                   # 限制数量
    .map { $x.transform() }       # 转换数据
    .distinct()                   # 去重
    .buffer(Duration.seconds(1)) # 批处理

# 避免过长的操作符链
let intermediate = source_stream
    .filter { $x.is_valid() }
    .map { $x.normalize() }

let final_stream = intermediate
    .group_by { $x.category }
    .flat_map { $group -> $group.buffer(10) }
```

### 3. 错误边界

```valkyrie
# 设置错误边界防止整个流崩溃
let resilient_stream = risky_stream
    .map { $item ->
        try {
            process_item($item)
        }
        .catch {
            case ProcessingError { error: $e }:
                log_error("处理失败: ${$e}")
                default_value()  # 提供默认值
            case _:
                raise  # 重新抛出严重错误
        }
    }
    .filter { $result? }
```

通过这些反应式编程模式，Valkyrie 提供了强大而灵活的数据流处理能力，使开发者能够构建响应式、可维护的应用程序。
# 效果系统

Valkyrie 的效果系统提供了强大的副作用管理和控制流机制，包括异常处理、协程、生成器、反应式编程、面向切面编程和依赖注入等功能。

## 效果系统组件

- **[异常处理](./error-handler.md)** - 灵活的错误处理和异常传播机制
- **[协程](./coroutine.md)** - 协作式多任务处理和异步编程
- **[生成器](./generator.md)** - 惰性计算和值序列生成
- **[反应式编程](./reactive.md)** - 数据流和变化传播的编程范式
- **[面向切面编程](./aop.md)** - 横切关注点的分离和管理
- **[依赖注入](./ioc.md)** - 控制反转和依赖管理

---

# 异常处理系统

Valkyrie 中任何对象都可以作为异常被抛出和捕获。这提供了一种灵活的错误处理机制，允许程序以结构化的方式处理各种异常情况。

## 基本异常处理

### 抛出异常

```valkyrie
# 任何对象都可以作为异常抛出
raise "Something went wrong"
raise 404
raise { code: 500, message: "Internal Server Error" }

# 抛出自定义对象
class NetworkError {
    message: string
    code: i32
}

raise NetworkError {
    message: "Connection timeout",
    code: -1
}
```

### 捕获异常

```valkyrie
# 基本异常捕获
try {
    risky_operation()
}
.catch {
    case Fail(error): print("Caught error: ${error}")
}

# 类型特定的异常捕获
try {
    network_request()
}
.catch {
    case Fail(error: NetworkError):
            print("Network error: ${error.message}")
            retry_connection()
    case Fail(error: string): print("String error: ${error}")
    else: print("Unknown error")
}
```

## 异常类型和模式

### 字符串异常

```valkyrie
# 简单的字符串异常
micro validate_age(age: i32) {
    if age < 0 {
        raise "Age cannot be negative"
    }
    if age > 150 {
        raise "Age seems unrealistic"
    }
}

try {
    validate_age(-5)
}
.catch {
    case message: string: print("Validation error: ${message}")
}
```

### 数值异常

```valkyrie
# 使用数值作为错误码
micro http_request(url: string) {
    if !is_valid_url(url) {
        raise 400  # Bad Request
    }
    if !is_authorized() {
        raise 401  # Unauthorized
    }
    if !resource_exists(url) {
        raise 404  # Not Found
    }
    # 正常处理
}

try {
    http_request("invalid-url")
}
.catch {
    case code: i32 {
        match code {
        400 => print("Bad request")
        401 => print("Unauthorized")
        404 => print("Not found")
        _ => print("HTTP error: ${code}")
    }
}
```

### 对象异常

```valkyrie
# 结构化异常对象
class DatabaseError {
    query: string
    error_code: i32
    message: string
    timestamp: DateTime
}

class ValidationError {
    field: string
    value: string
    constraint: string
}

micro save_user(user: User) {
    # 验证用户数据
    if user.email.is_empty() {
        raise ValidationError {
            field: "email",
            value: user.email,
            constraint: "Email cannot be empty"
        }
    }
    
    # 数据库操作
    try {
        database.insert(user)
    }
    .catch {
        case db_error: raise DatabaseError {
            query: "INSERT INTO users...",
            error_code: db_error.code,
            message: db_error.message,
            timestamp: DateTime.now()
        }
    }
}

# 处理不同类型的异常
try {
    save_user(invalid_user)
}
.catch {
    case ValidationError: print("Validation failed for field '${error.field}': ${error.constraint}")
        case DatabaseError:
        print("Database error at ${error.timestamp}: ${error.message}")
        log_error(error)
}
```

## 异常传播

### 自动传播

```valkyrie
# 异常会自动向上传播
micro level3() {
    raise "Error from level 3"
}

micro level2() {
    level3()  # 异常会传播到这里
}

micro level1() {
    level2()  # 异常继续传播
}

try {
    level1()
}
.catch {
    case _: print("Caught at top level: ${error}")
}
```

### 异常转换

```valkyrie
# 捕获并转换异常
micro parse_config(content: string) -> Config {
    try {
        json.parse(content)
    }
    .catch {
        case parse_error: raise ConfigError {
            message: "Failed to parse configuration",
            cause: parse_error,
            content_preview: content.substring(0, 100)
        }
    }
}

# 异常链
class ConfigError {
    message: string
    cause: Any
    content_preview: string
}
```

## 资源管理

### 自动清理

```valkyrie
# 使用 using 确保资源清理
micro process_file(filename: string) {
    using file = File.open(filename) {
        let content = file.read_all()
        process_content(content)
    }  # file会自动关闭
}

# 使用 defer 延迟执行
micro database_transaction() {
    let transaction = db.begin_transaction()
    defer transaction.rollback()  # 默认回滚
    
    try {
        # 执行数据库操作
        db.insert(data1)
        db.update(data2)
        db.delete(data3)
        
        transaction.commit()
        defer.cancel()  # 取消回滚
    }
    .catch {
        case error {
            # 异常时自动回滚
            raise error
        }
    }
}
```

### 资源包装

```valkyrie
# 自动资源管理的包装器
class ManagedResource⟨T⟩ {
    resource: T
    cleanup: () -> ()
    
    micro new(resource: T, cleanup: () -> ()) -> Self {
        Self { resource, cleanup }
    }
    
    micro use⟨R⟩(block: (T) -> R) -> R {
        let result = block(self.resource)
        self.cleanup()
        result
    }
}

# 使用示例
let managed_file = ManagedResource.new(
    File.open("data.txt"),
    { => file.close() }
)

managed_file.use({ $file =>
    let content = file.read_all()
    process_content(content)
})  # 文件自动关闭
```

## 异常处理模式

### 重试模式

```valkyrie
micro retry⟨T⟩(max_attempts: i32, operation: () -> T) -> T {
    let mut attempts = 0
    loop {
        try {
            return operation()
        }
        .catch {
            case error {
                attempts += 1
            if attempts >= max_attempts {
                raise RetryExhausted {
                    attempts: attempts,
                    last_error: error
                }
            }
            sleep(Duration.seconds(attempts))  # 指数退避
        }
    }
}

# 使用重试
try {
    let result = retry(3, { =>
        unreliable_network_call()
    })
    print("Success: ${result}")
}
.catch {
    case RetryExhausted: print("Failed after ${error.attempts} attempts: ${error.last_error}")
}
```

### 断路器模式

```valkyrie
class CircuitBreaker {
    failure_count: i32
    failure_threshold: i32
    state: CircuitState
    last_failure_time: DateTime
    
    micro call⟨T⟩(operation: () -> T) -> T {
        match self.state {
            CircuitState.Closed => {
                try {
                    let result = operation()
                    self.reset()
                    result
                }
                .catch {
                    case error {
                        self.record_failure()
                        raise error
                    }
                }
            }
            CircuitState.Open => {
                if self.should_attempt_reset() {
                    self.state = CircuitState.HalfOpen
                    self.call(operation)
                } else {
                    raise CircuitBreakerOpen {
                        message: "Circuit breaker is open"
                    }
                }
            }
            CircuitState.HalfOpen => {
                try {
                    let result = operation()
                    self.reset()
                    result
                }
                .catch {
                    case error {
                        self.state = CircuitState.Open
                        raise error
                    }
                }
            }
        }
    }
}
```

### 异常聚合

```valkyrie
# 收集多个异常
class AggregateException {
    exceptions: [Any]
    
    micro add(exception: Any) {
        self.exceptions.push(exception)
    }
    
    micro has_errors() -> bool {
        !self.exceptions.is_empty()
    }
}

micro process_batch(items: [Item]) {
    let errors = AggregateException { exceptions: [] }
    
    for item in items {
        try {
            process_item(item)
        }
        .catch {
            case _: errors.add(error)
        }
    }
    
    if errors.has_errors() {
        raise errors
    }
}
```

## 最佳实践

### 1. 异常类型设计

```valkyrie
# 使用有意义的异常类型
class UserNotFoundError {
    user_id: string
    search_criteria: Map⟨string, string⟩
}

class PermissionDeniedError {
    user: string
    resource: string
    required_permission: string
}

# 而不是通用的字符串
# raise "User not found"  # 不推荐
```

### 2. 异常信息

```valkyrie
# 提供足够的上下文信息
class FileProcessingError {
    filename: string
    line_number: i32
    column: i32
    error_type: string
    suggestion: string
}

micro parse_csv(filename: string) {
    try {
        # 解析逻辑
    }
    .catch {
        case _: raise FileProcessingError {
            filename: filename,
            line_number: current_line,
            column: current_column,
            error_type: "Invalid CSV format",
            suggestion: "Check for missing quotes or commas"
        }
    }
}
```

### 3. 异常处理策略

```valkyrie
# 在适当的层级处理异常
micro application_main() {
    try {
        run_application()
    }
    .catch {
        case ConfigurationError:
            print("Configuration error: ${error.message}")
            print("Please check your configuration file")
            exit(1)
        case NetworkError:
            print("Network error: ${error.message}")
            print("Please check your internet connection")
            exit(2)
        else:
            print("Unexpected error: ${error}")
            log_error(error)
            exit(99)
    }
}

# 不要过度捕获异常
micro bad_example() {
    try {
        some_operation()
    }
    .catch {
        case error {
            # 什么都不做，隐藏了错误
        }
    }
}
```

### 4. 测试异常处理

```valkyrie
#[test]
micro test_validation_error() {
    let invalid_user = User { email: "" }
    
    try {
        save_user(invalid_user)
        assert(false, "Expected ValidationError")
    }
    .catch {
        case ValidationError:
            @assert_equal(error.field, "email")
            @assert_equal(error.constraint, "Email cannot be empty")
    }
}

#[test]
micro test_retry_exhausted() {
    let mut call_count = 0
    
    try {
        retry(3, { =>
            call_count += 1
            raise "Always fails"
        })
        assert(false, "Expected RetryExhausted")
    }
    .catch {
        case RetryExhausted:
            @assert_equal(error.attempts, 3)
            @assert_equal(call_count, 3)
    }
}
```
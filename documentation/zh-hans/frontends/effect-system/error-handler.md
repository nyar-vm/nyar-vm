# 错误处理

Valkyrie 使用 `try` 语句和 `catch` 机制来处理错误。`try` 是一个独立的语句，可以与类型系统结合使用。

## Try 语句

### 基本 Try 语法

```valkyrie
# try 是独立语句，返回 Result 类型
let result = try Result⟨string⟩ {
    read_file("config.txt")?
}

# 带错误类型的 try
let data = try Result⟨Data, ParseError⟩ {
    let content = read_file("data.json")?
    parse_json(content)?
}

# 简化形式
let value = try {
    risky_operation()?
}
```

### Try 与可选类型

```valkyrie
# 处理可能失败的操作
let maybe_value = try i32? {
    let input = get_user_input()?
    parse_number(input)?
}

# 链式操作
let result = try User? {
    let id = extract_user_id(request)?
    let user = find_user_by_id(id)?
    validate_user(user)?
}
```

## Catch 处理

### 非主干控制流 Catch

```valkyrie
# 使用 .catch 处理错误
let config = try Result⟨Config⟩ {
    read_config_file()?
}
.catch {
    case Fail(FileNotFound(path)): 
        create_default_config(path)
    case Fail(ParseError(msg)):
        log_error(msg)
        Config::default()
    case Fail(error):
        print("Unexpected error: ${error}")
        Config::empty()
}

# 命名 catch
let data = try Result⟨Data⟩ {
    fetch_remote_data()?
}
catch network_error {
    case Fail(TimeoutError): retry_with_backoff()
    case Fail(ConnectionError(msg)): use_cached_data()
    case Fail(error):
        log_error(error)
        Data::empty()
}
```

### Match 风格的 Catch

```valkyrie
# catch 和 match 是对偶的，能力一模一样
let user_data = try Result⟨UserData⟩ {
    let raw = fetch_user_data(user_id)?
    validate_and_parse(raw)?
}
.catch {
    case Fail(ValidationError { field, message }):
        show_field_error(field, message)
        UserData::guest()
    case Fail(NetworkError { code, .. }) if code >= 500:
        # 服务器错误，稍后重试
        schedule_retry()
        UserData::cached(user_id)
    case Fail(NetworkError { code, .. }) if code >= 400:
        # 客户端错误
        UserData::error(code)
    else: UserData::unknown_error()
}
```

## 错误传播

### 问号操作符

```valkyrie
# ? 操作符用于错误传播
micro process_file(path: string) -> Result⟨string, FileError⟩ {
    let content = read_file(path)?  # 如果失败，直接返回错误
    let processed = transform_content(content)?
    validate_result(processed)?
}

# 在 try 块中使用
let final_result = try Result⟨ProcessedData⟩ {
    let raw = fetch_data()?
    let cleaned = clean_data(raw)?
    let validated = validate_data(cleaned)?
    process_final(validated)?
}
```

### 错误转换

```valkyrie
# 自动错误转换
micro read_and_parse(path: string) -> Result⟨Config, AppError⟩ {
    try Result⟨Config, AppError⟩ {
        let content = read_file(path)?  # FileError -> AppError
        let config = parse_json(content)?  # ParseError -> AppError
        validate_config(config)?  # ValidationError -> AppError
    }
}

# 手动错误转换
let result = try Result⟨Data⟩ {
    fetch_data().map_err { $e -> AppError::Network($e) }?
}
```

## 自定义错误类型

```valkyrie
# 定义错误类型
unity AppError {
    Network(NetworkError),
    Parse(ParseError),
    Validation { field: string, message: string },
    IO(IOError)
}

# 实现错误转换
imply AppError: From⟨NetworkError⟩ {
    micro from(err: NetworkError) -> AppError {
        AppError::Network(err)
    }
}

# 使用自定义错误
micro load_user_config(user_id: string) -> Result⟨UserConfig, AppError⟩ {
    try Result⟨UserConfig, AppError⟩ {
        let path = get_config_path(user_id)?
        let content = read_file(path)?
        let config = parse_config(content)?
        validate_user_config(config)?
    }
}
```

## 错误恢复模式

### 回退策略

```valkyrie
# 多级回退
let avatar = try Image? {
    load_from_cdn(user_id)?
}
.catch {
    case null: try Image? {
        load_from_cache(user_id)?
    }
    .catch {
        case null: default_avatar()
        else: null
    }
    else: default_avatar()
}

# 重试机制
let data = try Result⟨Data⟩ {
    fetch_with_retry(url, max_retries = 3)?
}
.catch {
    case Fail(RetryExhausted(attempts)):
        log_error("Failed after ${attempts} attempts")
        use_fallback_data()
    case Fail(error):
        log_error("Unexpected error: ${error}")
        Data::empty()
}
```

### 部分恢复

```valkyrie
# 处理部分失败
let results = try Result⟨[ProcessedItem]⟩ {
    items.map { $item ->
        try ProcessedItem? {
            process_item($item)?
        }
        .catch {
            case ProcessingError(msg):
                log_warning("Skipping item: ${msg}")
                None  # 跳过失败的项目
            else: None
        }
    }).filter_map { $x }.collect()
}
```

## 最佳实践

### 1. 错误类型设计

```valkyrie
# 自定义验证错误
union ValidationError {
    InvalidFormat { field: string, expected: string },
    ValueOutOfRange { field: string, min: i32, max: i32 },
    RequiredFieldMissing { field: string }
}

# 验证效果定义
effect Validation {
    micro validate_user(user: User) -> Result⟨(), [ValidationError]⟩
}

# 实现验证逻辑
micro check_user(user: User) -> Result⟨(), [ValidationError]⟩ {
    let mut errors = []
    
    if user.name.is_empty() {
        errors.push(ValidationError::RequiredFieldMissing { field: "name" })
    }
    
    if user.age < 0 || user.age > 150 {
        errors.push(ValidationError::ValueOutOfRange { 
            field: "age", 
            min: 0, 
            max: 150 
        })
    }
    
    if errors.is_empty() {
        Fine(())
    } else {
        Fail(errors)
    }
}

# 上下文信息
class ContextualError {
    operation: string,
    context: Map⟨string, string⟩,
    source: Error
}
```

### 2. 错误处理策略

```valkyrie
# 就近处理
micro validate_user_input(input: UserInput) -> Result<ValidatedInput, ValidationError> {
    try Result<ValidatedInput> {
        let email = validate_email(input.email)?
        let age = validate_age(input.age)?
        let name = validate_name(input.name)?
        class: ValidatedInput { email, age, name }
    }
}

# 统一错误处理
micro main() {
    let result = try Result<()> {
        run_application()?
    }
    .catch {
        case ConfigError(msg):
            print("Configuration error: ${msg}")
            exit(1)
        case NetworkError(msg):
            print("Network error: ${msg}")
            exit(2)
        case error:
            print("Unexpected error: ${error}")
            exit(99)
    }
}
```

### 3. 资源管理

```valkyrie
# 使用 RAII 模式
class FileHandle {
    path: String,
    handle: File
}

impl Drop for FileHandle {
    micro drop() {
        self.handle.close()
    }
}

# 安全的资源使用
micro process_file_safely(path: String) -> Result<String, FileError> {
    try Result<String> {
        let file = FileHandle::open(path)?
        let content = file.read_all()?
        process_content(content)?
    }  # file 自动关闭
}
```

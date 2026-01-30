# 宏系统 (Macro System)

## 概述

Valkyrie 提供了强大的宏系统，支持编译时代码生成和元编程。宏系统分为两个主要部分：

- **Macro (`@`)**: 编译时函数调用，不捕捉后续参数
- **Annotation (`@.`)**: 编译时注解，会捕捉并作用于后续的 `class`、`micro` 等声明

## Macro vs Annotation

### Macro (`@`)

Macro 使用 `@` 前缀，是编译时的函数调用，不会捕捉后续的代码元素：

```valkyrie
# 编译时常量计算
let FIBONACCI_10: i32 = @const_eval(fibonacci(10))
let LOOKUP_TABLE: [i32; 256] = @const_eval(generate_lookup_table())

# 环境变量获取
let database_url: string = @env("DATABASE_URL")

# 字符串格式化
let message: string = @format("Hello, {}!", name)

# 向量创建
let numbers = [1, 2, 3, 4, 5]
let zeros = [0; 10]

# SQL 查询
let query = @sql(
    "SELECT id, name FROM users WHERE active = $1",
    true
)
```

### Annotation (`@.`)

Annotation 使用 `@.` 前缀，会捕捉并作用于后续的声明：

```valkyrie
# 测试注解
@.test
micro test_addition() {
    @.assert_eq(2 + 2, 4)
}

# 序列化注解
@.derive(Serialize, Deserialize)
class User {
    name: string
    email: string
}

# 性能测试注解
@.benchmark
micro fibonacci_benchmark() {
    fibonacci(30)
}

# 条件编译注解
@.cfg(feature = "debug")
micro debug_function() {
    print("Debug mode enabled")
}
```

## 常用 Macro

### 编译时计算

```valkyrie
# 编译时常量计算
let PI_SQUARED: f64 = @const_eval(3.14159 * 3.14159)

# 编译时文件读取
let config_content: string = @compile_time_read_file("config.toml")

# 编译时环境配置
@compile_time_env {
    memory_limit: "256MB",
    execution_timeout: "30s",
}
```


### 代码生成

```valkyrie
# 模板定义
@template {
    name: "crud_operations",
    params: [Entity: Type, Key: Type],
    body: {
        micro create(entity: Entity) -> Result⟨Key, Any⟩ {
            # 创建实体的通用逻辑
        }
        
        micro read(key: Key) -> Result⟨Entity, Any⟩ {
            # 读取实体的通用逻辑
        }
        
        micro update(key: Key, entity: Entity) -> Result⟨unit, Any⟩ {
            # 更新实体的通用逻辑
        }
        
        micro delete(key: Key) -> Result⟨unit, Any⟩ {
            # 删除实体的通用逻辑
        }
    }
}

# 模板实例化
@generate_code {
    crud_operations⟨User, UserId⟩
    crud_operations⟨Product, ProductId⟩
}
```

### 宏展开控制

```valkyrie
# 宏展开策略控制
@macro_expansion(strategy: "eager", max_depth: 100)
macro recursive_macro {
    # 递归宏定义
}
```

## 常用 Annotation

### 测试相关

```valkyrie
@.test
micro test_user_creation() {
    let user = User("Alice", "alice@example.com")
    @.assert_true(user.is_valid())
    @.assert_eq(user.name, "Alice")
}

@.test
@.should_panic
micro test_invalid_email() {
    User("Bob", "invalid-email")
}
```

### 派生注解

```valkyrie
@.derive(Debug, Clone, PartialEq)
class Point {
    x: f64,
    y: f64,
}

@.derive(Serialize, Deserialize)
class Config {
    database_url: string
    port: u16
}
```

### 条件编译

```valkyrie
@.cfg(target_os = "windows")
micro windows_specific_function() {
    # Windows 特定实现
}

@.cfg(feature = "async")
class AsyncHandler {
    # 异步处理器实现
}
```

## 自定义宏

### 声明式宏

```valkyrie
macro vec_of {
    ($elem:expr; $n:expr) => {
        {
            let mut v = []
            for _ in 0..$n {
                v.push($elem)
            }
            v
        }
    }
    (#(#x:expr),+ #(,)?) => {
        @vec(#(#x),+)
    }
}

### 过程宏

```valkyrie
@macro
micro debug_print(args: TokenStream) -> TokenStream {
    if @cfg(debug_assertions) {
        @quote {
            print(#args)
        }
    } else {
        @quote {}
    }
}
```

## 最佳实践

1. **明确区分用途**：
   - 使用 `@` 进行编译时计算和代码生成
   - 使用 `@.` 为声明添加元数据和行为

2. **性能考虑**：
   - 编译时计算可以提高运行时性能
   - 避免过度使用宏导致编译时间过长

3. **可读性**：
   - 为复杂宏添加文档注释
   - 使用有意义的宏名称

4. **调试**：
   - 使用 `@macro_expansion` 控制宏展开
   - 利用编译器的宏展开输出进行调试

## 总结

Valkyrie 的宏系统提供了强大的元编程能力：

- **Macro (`@`)**: 编译时函数，用于计算、生成和转换
- **Annotation (`@.`)**: 声明注解，用于添加元数据和行为

正确使用这两种机制可以大大提高代码的表达力和性能。
# 快速开始

欢迎使用 Valkyrie 语言！Valkyrie 是一个现代化的函数式编程语言，专为构建高性能、类型安全的应用程序而设计，具有强大的代数效应系统。

## 什么是 Valkyrie？

Valkyrie 是一个函数式编程语言，它提供：

- 🎯 **代数效应系统**：优雅地处理副作用，如异步操作、错误处理、状态管理
- 🚀 **多后端支持**：编译到 WebAssembly 和 JavaScript，适应不同部署场景
- 🔒 **类型安全**：强大的类型系统，编译时保证程序正确性
- ⚡ **高性能**：通过多阶段编译优化，生成高效的目标代码
- 🔧 **现代语法**：简洁直观的语法，易于学习和使用

## 5分钟快速体验

### 1. 安装 Valkyrie 编译器

```bash
# 使用 Cargo 安装
cargo install valkyrie-cli

# 或者从源码构建
git clone https://github.com/valkyrie-lang/valkyrie-vm.git
cd valkyrie-vm
cargo build --release
```

### 2. 创建你的第一个 Valkyrie 程序

创建 `hello.vk`：

```valkyrie
// 定义一个简单的问候函数
fn greet(name: String) -> String {
    "Hello, " + name + "!"
}

// 使用代数效应处理异步操作
effect Http {
    fn get(url: String) -> String
}

// 异步获取用户信息
fn fetch_user_info(user_id: i32) -> String {
    let url = "https://api.example.com/users/" + user_id.to_string();
    perform Http::get(url)
}

// 主函数展示效应处理
fn main() {
    let greeting = greet("Nyar");
    println(greeting);
    
    // 处理HTTP效应
    handle {
        let user_info = fetch_user_info(123);
        println(user_info);
    } with Http {
        get(url) -> resume => {
            // 模拟HTTP请求
            let response = "User data for: " + url;
            resume(response)
        }
    }
}
```

### 3. 编译和运行

```bash
# 编译到 WebAssembly (生产环境)
valkyrie compile hello.vk --target wasm --output hello.wasm

# 编译到 JavaScript (开发环境)
valkyrie compile hello.vk --target js --output hello.js

# 直接运行 (解释执行)
valkyrie run hello.vk

# 查看编译过程的中间表示
valkyrie compile hello.vk --emit-hir --emit-mir --emit-lir
```

### 4. 在不同环境中使用

#### 在 Web 浏览器中使用 (WebAssembly)

```html
<!DOCTYPE html>
<html>
<head>
    <title>Valkyrie WebAssembly Demo</title>
</head>
<body>
    <script type="module">
        import init, { run_valkyrie_program } from './hello.js';
        
        async function main() {
            await init();
            
            // 运行编译后的 Valkyrie 程序
            const result = run_valkyrie_program();
            console.log('Valkyrie program output:', result);
        }
        
        main();
    </script>
</body>
</html>
```

#### 在 Node.js 中使用 (JavaScript)

```javascript
// 导入编译后的 JavaScript 代码
const { greet, fetch_user_info, main } = require('./hello.js');

// 直接调用函数
console.log(greet('World'));

// 运行主程序
main();
```

## 核心概念

### 类型系统

Valkyrie 提供了强大的函数式类型系统：

```valkyrie
// 基础类型
type UserId = Int
type Email = String

// 可选类型 (Maybe monad)
type OptionalField = Maybe<String>

// 列表类型
type Tags = List<String>

// 记录类型
type User = {
    id: UserId,
    name: String,
    email: Email,
}

// 联合类型 (Sum types)
type Status = Active | Inactive | Pending

// 泛型类型
type Result<T, E> = Ok(T) | Err(E)
```

### 代数效应系统

Valkyrie 使用代数效应来优雅地处理副作用：

```valkyrie
// 定义效应
effect State<T> {
    fn get() -> T
    fn put(value: T) -> ()
}

effect IO {
    fn read_file(path: String) -> String
    fn write_file(path: String, content: String) -> ()
}

effect Error<E> {
    fn throw(error: E) -> Never
}

// 使用效应
fn process_file(input_path: String, output_path: String) -> () {
    let content = perform IO::read_file(input_path);
    let processed = content.to_uppercase();
    perform IO::write_file(output_path, processed)
}

// 处理效应
fn main() {
    handle {
        process_file("input.txt", "output.txt")
    } with IO {
        read_file(path) -> resume => {
            // 实际的文件读取逻辑
            let content = std::fs::read_to_string(path)?;
            resume(content)
        },
        write_file(path, content) -> resume => {
            // 实际的文件写入逻辑
            std::fs::write(path, content)?;
            resume(())
        }
    }
}
```

### 函数式编程特性

```valkyrie
// 高阶函数
fn map<A, B>(f: A -> B, list: List<A>) -> List<B> {
    match list {
        [] => [],
        [head, ...tail] => [f(head), ...map(f, tail)]
    }
}

// 柯里化函数
fn add(x: Int) -> (Int -> Int) {
    fn(y: Int) -> Int { x + y }
}

// 模式匹配
fn process_result<T, E>(result: Result<T, E>) -> String {
    match result {
        Ok(value) => "Success: " + value.to_string(),
        Err(error) => "Error: " + error.to_string()
    }
}

// 递归函数
fn factorial(n: Int) -> Int {
    match n {
        0 => 1,
        n if n > 0 => n * factorial(n - 1),
        _ => perform Error::throw("Invalid input")
    }
}

// 尾递归优化
fn fibonacci_tail(n: Int, acc1: Int = 0, acc2: Int = 1) -> Int {
    match n {
        0 => acc1,
        1 => acc2,
        _ => fibonacci_tail(n - 1, acc2, acc1 + acc2)
    }
}
```

## 下一步

现在你已经了解了 VOS 的基础概念，可以继续深入学习：

### 📚 学习指南
- **[开发指南](/development/)**：了解如何开发和扩展 VOS 项目
- **[维护指南](/maintenance/)**：学习项目架构和维护最佳实践

### 🔧 技术参考
- **[语言规范](/language/)**：完整的 VOS 语言参考
- **[中间件系统](/guide/middleware/)**：了解请求处理管道
- **[认证系统](/guide/auth)**：配置身份验证
- **[授权系统](/guide/acl)**：设置访问控制
- **[配置管理](/guide/config)**：服务配置最佳实践

### 🎯 实践示例
- **[示例项目](/examples/)**：查看完整的项目示例
- **[电商 API](/examples/ecommerce)**：复杂业务场景示例
- **[用户服务](/examples/user-service)**：认证授权示例

## 社区和支持

- **GitHub**: [vos-lang/vos-rs](https://github.com/vos-lang/vos-rs)
- **文档**: 你正在阅读的这份文档
- **问题反馈**: [GitHub Issues](https://github.com/vos-lang/vos-rs/issues)

恭喜！你已经成功创建了第一个 Valkyrie 程序。现在你可以：

- 📖 查看 [核心概念](./core-concepts.md) 了解 Valkyrie 的设计理念和代数效应系统
- 🛠️ 学习 [最佳实践](./best-practices.md) 编写高质量的 Valkyrie 代码
- 🔧 探索 [配置选项](./config.md) 自定义编译器行为
- 🚀 深入 [开发指南](./development.md) 了解高级特性和工具链开发

开始你的 Valkyrie 之旅吧！🚀
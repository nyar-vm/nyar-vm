# 模块系统

Valkyrie 采用基于命名空间（namespace）和导入（using）的模块系统，提供灵活的代码组织和依赖管理方式。

## 命名空间声明 (namespace)

Valkyrie 使用 `namespace!` 关键字在文件顶部声明命名空间。

```valkyrie
namespace! math.geometry;

class Point {
    x: f64
    y: f64
}

imply Point {
    micro distance(self, other: Point) -> f64 {
        let dx = self.x - other.x
        let dy = self.y - other.y
        (dx * dx + dy * dy).sqrt()
    }
}
```

## 导入系统 (using)

使用 `using` 关键字导入其他命名空间的成员。

```valkyrie
using math.geometry;
using math.geometry.Point;
```

## 可见性控制

### 访问修饰符

```valkyrie
namespace database {
    # 公开结构体
    pub class Connection {
        # 私有字段
        host: string,
        port: u16,
        # 公开字段
        pub timeout: Duration,
    }
    
    # 包内可见
    class InternalConfig {
        secret_key: string,
    }
    
    # 私有函数
    micro validate_connection(conn: &Connection) -> bool {
        # 内部验证逻辑
        true
    }
    
    # 公开函数
    pub micro connect(host: string, port: u16) -> Result⟨Connection, Error⟩ {
        let conn = Connection(host, port, timeout: Duration::seconds(30))
        if validate_connection(&conn) {
            Fine(conn)
        } else {
            Fail(Error::InvalidConnection)
        }
    }
}
```

### 重新导出

```valkyrie
namespace api {
    # 重新导出其他模块的类型
    pub using database.{Connection, Error}
    pub using auth.{User, Session}
    
    # 提供统一的 API 接口
    pub micro create_authenticated_connection(credentials: Credentials) -> Result⟨(Connection, Session), Error⟩ {
        let session = auth::login(credentials)?
        let connection = database::connect("localhost", 5432)?
        Fine(connection, session)
    }
}
```

## 文件路径无关的模块

### 逻辑模块组织

Valkyrie 的模块系统不依赖文件路径，而是基于逻辑命名空间：

```valkyrie
# 文件: src/geometry.val
namespace math.geometry {
    pub class Point { x: f64, y: f64 }
}

# 文件: src/algebra.val  
namespace math.algebra {
    pub class Matrix { data: [[f64]] }
}

# 文件: src/utils.val
namespace math.geometry {  # 扩展已存在的命名空间
    pub micro origin() -> Point {
        Point { x: 0.0, y: 0.0 }
    }
}
```

### 模块声明文件

```valkyrie
# 文件: math.module.val
# 声明模块的公开接口
module math {
    pub namespace geometry {
        pub class Point
        pub micro distance(Point, Point) -> f64
        pub micro origin() -> Point
    }
    
    pub namespace algebra {
        pub class Matrix
        pub micro multiply(Matrix, Matrix) -> Matrix
    }
}
```

## 条件编译和特性

### 特性门控

```valkyrie
namespace network {
    # 基础网络功能
    pub class TcpStream { /* ... */ }
    
    # 异步功能（需要 async 特性）
    @.cfg(feature = "async")
    pub namespace async {
        pub class AsyncTcpStream { /* ... */ }
        
        pub micro connect_async(addr: SocketAddr) -> Future<Result<AsyncTcpStream, Error>> {
            # 异步连接实现
        }
    }
    
    # TLS 支持（需要 tls 特性）
    @.cfg(feature = "tls")
    pub namespace tls {
        pub class TlsStream { /* ... */ }
        
        pub micro wrap_tls(stream: TcpStream, config: TlsConfig) -> Result<TlsStream, TlsError> {
            # TLS 包装实现
        }
    }
}
```

### 平台特定代码

```valkyrie
namespace platform {
    # 通用接口
    pub trait FileSystem {
        micro read_file(path: String) -> Result<String, IoError>
        micro write_file(path: String, content: String) -> Result<(), IoError>
    }
    
    # Windows 实现
    @.cfg(target_os = "windows")
    pub namespace windows {
        pub class WindowsFileSystem
        
        imply WindowsFileSystem: FileSystem {
            micro read_file(&self, path: string) -> string {
                # Windows 特有的实现
                "Windows content".to_string()
            }
        }
    }
    
    # Unix 实现
    @.cfg(any(target_os = "linux", target_os = "macos"))
    pub namespace unix {
        pub class UnixFileSystem
        
        imply UnixFileSystem: FileSystem {
            micro read_file(&self, path: string) -> string {
                # Unix 特有的实现
                "Unix content".to_string()
            }
        }
    }
}
```

## 依赖管理

### 外部依赖

```valkyrie
# 项目配置文件: valkyrie.toml
[dependencies]
serde = "1.0"
tokio = { version = "1.0", features = ["full"] }
log = "0.4"

[dev-dependencies]
tokio-test = "0.4"

# 在代码中使用外部依赖
using serde.{Serialize, Deserialize}
using tokio.runtime.Runtime
using log.{info, warn, error}

@.derive(Serialize, Deserialize)
class Config {
    host: String,
    port: u16,
}

micro main() {
    let rt = Runtime::new().unwrap()
    rt.block_on(async_main())
}

async micro async_main() {
    info("Starting application")
    # 异步逻辑
}
```

### 工作空间

```valkyrie
# 工作空间配置: Workspace.toml
[workspace]
members = [
    "core",
    "api",
    "cli",
    "web"
]

# 共享依赖
[workspace.dependencies]
serde = "1.0"
tokio = "1.0"

# 在子项目中引用工作空间依赖
# core/valkyrie.toml
[dependencies]
serde = { workspace = true }
tokio = { workspace = true }

# 内部依赖
api = { path = "../api" }
```

## 模块初始化

### 静态初始化

```valkyrie
namespace config {
    # 静态配置
    pub static DATABASE_URL: String = @env("DATABASE_URL")
    pub static MAX_CONNECTIONS: i32 = 100
    
    # 延迟初始化
    pub static LOGGER: Lazy<Logger> = Lazy::new({
        Logger::new()
            .with_level(LogLevel::Info)
            .with_output(Output::Stdout)
    })
}
```

### 动态初始化

```valkyrie
namespace database {
    static mut CONNECTION_POOL: Option<ConnectionPool> = None
    
    pub micro initialize(config: DatabaseConfig) -> Result<(), Error> {
        unsafe {
            if CONNECTION_POOL.is_some() {
                return Fail(Error::AlreadyInitialized)
            }
            
            let pool = ConnectionPool::new(config)?
            CONNECTION_POOL = Some(pool)
            Fine { value: () }
        }
    }
    
    pub micro get_connection() -> Result<Connection, Error> {
        unsafe {
            CONNECTION_POOL
                .as_ref()
                .ok_or(Error::NotInitialized)?
                .get_connection()
        }
    }
}
```

## 测试模块

### 单元测试

```valkyrie
namespace math.geometry {
    pub micro distance(p1: Point, p2: Point) -> f64 {
        let dx = p1.x - p2.x
        let dy = p1.y - p2.y
        (dx * dx + dy * dy).sqrt()
    }
    
    # 测试模块
    @.cfg(test)
    namespace tests {
        using super.*
        
        @.test
        micro test_distance_same_point() {
            let p = Point { x: 1.0, y: 2.0 }
            let dist = distance(p, p)
            @assert_equal(dist, 0.0)
        }
        
        @.test
        micro test_distance_different_points() {
            let p1 = Point { x: 0.0, y: 0.0 }
            let p2 = Point { x: 3.0, y: 4.0 }
            let dist = distance(p1, p2)
            @assert_equal(dist, 5.0)
        }
    }
}
```

### 集成测试

```valkyrie
# 文件: tests/integration.val
using myapp.api.*
using myapp.database.*

@.test
micro test_full_workflow() {
    # 设置测试环境
    let config = TestConfig::default()
    initialize_test_database(config)
    
    # 执行测试
    let user = create_user("alice", "alice@example.com")
    @.assert_true(user.is_ok())
    
    let found_user = find_user_by_email("alice@example.com")
    @.assert_true(found_user.is_some())
    
    # 清理
    cleanup_test_database()
}
```

## 最佳实践

### 模块设计原则

1. **单一职责**: 每个模块应该有明确的职责
2. **低耦合**: 模块间依赖应该最小化
3. **高内聚**: 相关功能应该组织在同一模块中
4. **接口稳定**: 公开接口应该保持稳定

### 命名约定

```valkyrie
# 好的命名空间组织
namespace myapp {
    namespace core {        # 核心功能
        namespace types     # 基础类型
        namespace traits    # 特征定义
        namespace utils     # 工具函数
    }
    
    namespace services {    # 业务服务
        namespace user      # 用户服务
        namespace auth      # 认证服务
        namespace payment   # 支付服务
    }
    
    namespace adapters {    # 适配器层
        namespace database  # 数据库适配器
        namespace http      # HTTP 适配器
        namespace cache     # 缓存适配器
    }
}
```

### 版本兼容性

```valkyrie
namespace api {
    # 版本化 API
    namespace v1 {
        pub class User {
            id: i64,
            name: String,
        }
        
        pub micro get_user(id: i64) -> Option<User> {
            # v1 实现
        }
    }
    
    namespace v2 {
        pub class User {
            id: i64,
            name: String,
            email: String,  # 新增字段
        }
        
        pub micro get_user(id: i64) -> Option<User> {
            # v2 实现
        }
        
        # 向后兼容
        pub micro get_user_v1(id: i64) -> Option<v1::User> {
            get_user(id).map({ v1::User {
                id: u.id,
                name: u.name,
            })
        }
    }
    
    # 当前版本别名
    pub using v2.*
}
```
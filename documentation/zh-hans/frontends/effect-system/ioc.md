# 控制反转 (Inversion of Control)

控制反转（IoC）是一种设计原则，通过将对象的创建和依赖关系的管理从对象本身转移到外部容器或框架中，实现了松耦合的设计。在 Valkyrie 中，IoC 通过 Effect 系统实现，提供了强大的依赖注入和服务定位能力。

## 基本概念

### 依赖注入 (Dependency Injection)
依赖注入是 IoC 的一种实现方式，通过外部注入依赖对象，而不是在对象内部创建依赖。

### 服务定位 (Service Locator)
服务定位器是一个中央注册表，用于查找和获取服务实例。

### 生命周期管理
容器负责管理对象的生命周期，包括创建、初始化、销毁等。

## Effect 系统中的 IoC

### 定义依赖注入 Effect

```valkyrie
# 定义依赖注入 Effect
eff DependencyInjection {
    resolve⟨T⟩(service_type: Type⟨T⟩) -> T
    resolve_named⟨T⟩(service_type: Type⟨T⟩, name: string) -> T
    register⟨T⟩(service_type: Type⟨T⟩, instance: T): unit
    register_factory⟨T⟩(service_type: Type⟨T⟩, factory: { -> T }): unit
    register_singleton⟨T⟩(service_type: Type⟨T⟩, factory: { -> T }): unit
}

# 定义服务生命周期 Effect
eff ServiceLifecycle {
    create⟨T⟩(service_type: Type⟨T⟩) -> T
    initialize⟨T⟩(instance: T): T
    dispose⟨T⟩(instance: T): unit
}
```

### 实现 IoC 容器

```valkyrie
# IoC 容器实现
class IoCContainer {
    private mut services: {Type: Any} = {}
    private mut factories: {Type: { -> Any }} = {}
    private mut singletons: {Type: Any} = {}
    private mut named_services: {string: {Type: Any}} = {}
    
    handle DependencyInjection {
        resolve⟨T⟩(service_type) -> T {
            # 首先检查单例
            match self.singletons.get(service_type) {
                case instance: return instance as T
                case null: {}
            }
            
            # 检查工厂方法
            match self.factories.get(service_type) {
                case factory: return factory() as T
                case null: {}
            }
            
            # 检查已注册的实例
            match self.services.get(service_type) {
                case instance: return instance as T
                case null: {}
            }
            
            # 尝试自动装配
            self.auto_wire(service_type)
        }
        
        resolve_named⟨T⟩(service_type, name) -> T {
            match self.named_services.get(name) {
                case named_map:
                    match named_map.get(service_type) {
                        case instance: return instance as T
                        case null: {}
                    }
                case null: {}
            }
            
            raise ServiceNotFoundError { service_type, name }
        }
        
        register⟨T⟩(service_type, instance) {
            self.services[service_type] = instance
        }
        
        register_factory⟨T⟩(service_type, factory) {
            self.factories[service_type] = factory
        }
        
        register_singleton⟨T⟩(service_type, factory) {
            let instance = factory()
            self.singletons[service_type] = instance
        }
    }
    
    handle ServiceLifecycle {
        create⟨T⟩(service_type) -> T {
            # 使用反射或编译时信息创建实例
            let constructor = service_type.get_constructor()
            let dependencies = constructor.get_parameters().map({ 
                perform DependencyInjection.resolve($param.type)
            })
            constructor.invoke(dependencies)
        }
        
        initialize⟨T⟩(instance) -> T {
            # 执行初始化逻辑
            if instance implements Initializable {
                instance.initialize()
            }
            instance
        }
        
        dispose⟨T⟩(instance) {
            # 执行清理逻辑
            if instance implements Disposable {
                instance.dispose()
            }
        }
    }
    
    private micro auto_wire⟨T⟩(self, service_type: Type⟨T⟩) -> T {
        let instance = perform ServiceLifecycle.create(service_type)
        let initialized = perform ServiceLifecycle.initialize(instance)
        self.services[service_type] = initialized
        initialized
    }
}
```

### 使用依赖注入注解

```valkyrie
# 定义服务接口
trait UserRepository {
    micro find_by_id(self, id: string) -> User?
    micro save(self, user: User) -> Unit
    micro delete(self, id: string) -> Unit
}

trait EmailService {
    micro send_email(self, to: string, subject: string, body: string) -> Unit
}

# 实现服务
class DatabaseUserRepository {
    private connection: DatabaseConnection
    
    # 构造函数注入
    new(@.inject connection: DatabaseConnection) {
        self.connection = connection
    }
    
    impl UserRepository {
        micro find_by_id(self, id) -> User? {
            self.connection.query("SELECT * FROM users WHERE id = ?", [id])
                .map({ User::from_row($row) })
        }
        
        micro save(self, user) {
            self.connection.execute(
                "INSERT INTO users (id, name, email) VALUES (?, ?, ?)",
                [user.id, user.name, user.email]
            )
        }
        
        micro delete(self, id) {
            self.connection.execute("DELETE FROM users WHERE id = ?", [id])
        }
    }
}

class SmtpEmailService {
    private config: EmailConfig
    
    new(@.inject config: EmailConfig) {
        self.config = config
    }
    
    impl EmailService {
        micro send_email(self, to, subject, body) {
            # SMTP 发送逻辑
            let smtp_client = SmtpClient::new(self.config)
            smtp_client.send(to, subject, body)
        }
    }
}
```

### 服务类使用依赖注入

```valkyrie
# 用户服务类
class UserService {
    private user_repository: UserRepository
    private email_service: EmailService
    
    # 构造函数注入
    new(
        @.inject user_repository: UserRepository,
        @.inject email_service: EmailService
    ) {
        self.user_repository = user_repository
        self.email_service = email_service
    }
    
    @.transactional
    micro create_user(self, name: string, email: string) -> User {
        let user = User {
            id: generate_id(),
            name,
            email,
            created_at: now()
        }
        
        self.user_repository.save(user)
        
        # 发送欢迎邮件
        self.email_service.send_email(
            user.email,
            "Welcome!",
            f"Welcome {user.name}!"
        )
        
        user
    }
    
    micro get_user(self, id: string) -> User? {
        self.user_repository.find_by_id(id)
    }
    
    @.authorized("admin")
    micro delete_user(self, id: string) -> Unit {
        if let user = self.user_repository.find_by_id(id) {
            self.user_repository.delete(id)
            
            # 发送账户删除通知
            self.email_service.send_email(
                user.email,
                "Account Deleted",
                "Your account has been deleted."
            )
        }
    }
}
```

### 配置和启动

```valkyrie
# 应用程序配置
class ApplicationConfig {
    micro configure_services(self, container: IoCContainer) {
        # 注册配置
        container.register(EmailConfig, EmailConfig {
            smtp_host: "smtp.example.com",
            smtp_port: 587,
            username: "app@example.com",
            password: "password"
        })
        
        # 注册数据库连接
        container.register_singleton(DatabaseConnection, {
            DatabaseConnection::new("postgresql://localhost/myapp")
        })
        
        # 注册服务实现
        container.register_factory(UserRepository, {
            let connection = perform DependencyInjection.resolve(DatabaseConnection)
            DatabaseUserRepository::new(connection)
        })
        
        container.register_factory(EmailService, {
            let config = perform DependencyInjection.resolve(EmailConfig)
            SmtpEmailService::new(config)
        })
        
        # 注册应用服务
        container.register_factory(UserService, {
            let user_repo = perform DependencyInjection.resolve(UserRepository)
            let email_service = perform DependencyInjection.resolve(EmailService)
            UserService::new(user_repo, email_service)
        })
    }
}

# 应用程序启动
class Application {
    private container: IoCContainer
    
    new() {
        self.container = IoCContainer {}
        let config = ApplicationConfig {}
        config.configure_services(self.container)
    }
    
    micro run(self) {
        with self.container {
            let user_service = perform DependencyInjection.resolve(UserService)
            
            # 使用服务
            let user = user_service.create_user("Alice", "alice@example.com")
            print(f"Created user: {user.id}")
            
            let found_user = user_service.get_user(user.id)
            print(f"Found user: {found_user}")
        }
    }
}
```

### 作用域和生命周期

```valkyrie
# 定义服务作用域
union ServiceScope {
    Singleton,
    Scoped,
    Transient,
}

# 作用域管理器
class ScopeManager {
    private mut scoped_services: {string: {Type: Any}} = {}
    private mut current_scope: string? = null
    
    micro begin_scope(mut self, scope_id: string) {
        self.current_scope = scope_id
        self.scoped_services[scope_id] = {}
    }
    
    micro end_scope(mut self, scope_id: string) {
        if let services = self.scoped_services.remove(scope_id) {
            # 清理作用域内的服务
            for (_, service) in services {
                perform ServiceLifecycle.dispose(service)
            }
        }
        
        if self.current_scope == scope_id {
            self.current_scope = null
        }
    }
    
    micro get_scoped_service⟨T⟩(self, service_type: Type⟨T⟩) -> T? {
        if let scope_id = self.current_scope {
            if let services = self.scoped_services.get(scope_id) {
                services.get(service_type).map { $service as T }
            } else {
                null
            }
        } else {
            null
        }
    }
    
    micro set_scoped_service⟨T⟩(mut self, service_type: Type⟨T⟩, instance: T) {
        if let scope_id = self.current_scope {
            self.scoped_services.get_mut(scope_id)[service_type] = instance
        }
    }
}
```

### 条件注册

```valkyrie
# 条件注册
class ConditionalRegistration {
    micro register_if⟨T⟩(
        self,
        container: IoCContainer,
        service_type: Type⟨T⟩,
        factory: { -> T },
        condition: { -> bool }
    ) {
        if condition() {
            container.register_factory(service_type, factory)
        }
    }
    
    micro register_profile⟨T⟩(
        self,
        container: IoCContainer,
        service_type: Type⟨T⟩,
        implementations: {string: { -> T }},
        active_profile: string
    ) {
        if let factory = implementations.get(active_profile) {
            container.register_factory(service_type, factory)
        }
    }
}

# 使用示例
let conditional = ConditionalRegistration {}
let container = IoCContainer {}

# 根据环境注册不同实现
conditional.register_profile(
    container,
    EmailService,
    {
        "development": { MockEmailService {} },
        "production": { SmtpEmailService::new(email_config) },
        "testing": { InMemoryEmailService {} }
    },
    get_active_profile()
)
```

### 装饰器模式

```valkyrie
# 服务装饰器
class ServiceDecorator⟨T⟩ {
    private inner: T
    
    new(inner: T) {
        self.inner = inner
    }
    
    micro get_inner(self) -> T {
        self.inner
    }
}

# 缓存装饰器
class CachedUserRepository {
    private inner: UserRepository
    private mut cache: {string: User} = {}
    
    new(@.inject inner: UserRepository) {
        self.inner = inner
    }
    
    impl UserRepository {
        micro find_by_id(self, id) -> User? {
            if let cached = self.cache.get(id) {
                return cached
            }
            
            let user = self.inner.find_by_id(id)
            if let u = user {
                self.cache[id] = u
            }
            user
        }
        
        micro save(self, user) {
            self.inner.save(user)
            self.cache[user.id] = user
        }
        
        micro delete(self, id) {
            self.inner.delete(id)
            self.cache.remove(id)
        }
    }
}

# 注册装饰器
container.register_factory(UserRepository, {
    let base_repo = DatabaseUserRepository::new(
        perform DependencyInjection.resolve(DatabaseConnection)
    )
    CachedUserRepository::new(base_repo)
})
```

## 最佳实践

### 1. 接口隔离
定义小而专注的接口，避免大而全的接口。

### 2. 单一职责
每个服务应该只有一个职责，避免服务过于复杂。

### 3. 生命周期管理
合理选择服务的生命周期，避免内存泄漏和资源浪费。

### 4. 循环依赖检测
在容器中实现循环依赖检测，避免无限递归。

```valkyrie
# 完整示例：Web 应用程序
class WebApplication {
    private container: IoCContainer
    private scope_manager: ScopeManager
    
    new() {
        self.container = IoCContainer {}
        self.scope_manager = ScopeManager {}
        self.configure_services()
    }
    
    private micro configure_services(self) {
        # 基础设施服务
        self.container.register_singleton(DatabaseConnection, {
            DatabaseConnection::new(get_connection_string())
        })
        
        # 仓储层
        self.container.register_factory(UserRepository, {
            let conn = perform DependencyInjection.resolve(DatabaseConnection)
            let base_repo = DatabaseUserRepository::new(conn)
            CachedUserRepository::new(base_repo)
        })
        
        # 应用服务层
        self.container.register_scoped(UserService, {
            let user_repo = perform DependencyInjection.resolve(UserRepository)
            let email_service = perform DependencyInjection.resolve(EmailService)
            UserService::new(user_repo, email_service)
        })
        
        # 控制器层
        self.container.register_scoped(UserController, {
            let user_service = perform DependencyInjection.resolve(UserService)
            UserController::new(user_service)
        })
    }
    
    micro handle_request(self, request: HttpRequest) -> HttpResponse {
        let request_id = generate_request_id()
        
        self.scope_manager.begin_scope(request_id)
        
        try {
            with self.container, self.scope_manager {
                let controller = perform DependencyInjection.resolve(UserController)
                controller.handle(request)
            }
        } finally {
            self.scope_manager.end_scope(request_id)
        }
    }
}
```

通过 Effect 系统实现的 IoC 提供了类型安全、灵活配置、易于测试的依赖注入能力，使得应用程序的架构更加清晰和可维护。
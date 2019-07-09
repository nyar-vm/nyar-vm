# 开发指南

本指南面向**外部开发者和贡献者**，详细介绍如何参与Valkyrie语言工具链的开发，包括编译器前端、后端、插件开发以及相关工具的使用。

> **目标读者**: 外部贡献者、插件开发者、工具链扩展开发者、社区开发者
> **内容重点**: 开发环境搭建、贡献流程、API使用、插件开发、工具扩展

## 开发环境设置

### 必需工具

```bash
# Rust工具链 (编译器核心)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add clippy rustfmt
rustup target add wasm32-unknown-unknown  # WebAssembly目标

# Node.js (JavaScript后端和工具)
npm install -g pnpm
npm install -g wasm-pack  # WebAssembly打包工具

# 开发工具
cargo install cargo-watch    # 文件监控
cargo install cargo-expand   # 宏展开
cargo install wasm-bindgen-cli  # Wasm绑定生成
```

### 项目设置

```bash
# 克隆项目
git clone https://github.com/your-org/nyar-vm.git
cd nyar-vm

# 构建编译器核心
cargo build --workspace

# 运行测试套件
cargo test --workspace

# 构建WebAssembly后端
cd projects/nyar-wasm
wasm-pack build --target web

# 启动文档开发服务器
cd projects/nyar-document
npm install
npm run dev
```

## Nyar语言工具链开发

### 编译器架构概览

Nyar编译器采用多阶段编译架构：

```
源代码 → AST → HIR → MIR → LIR → 目标代码
        ↑     ↑     ↑     ↑      ↑
     解析器  语义分析 控制流 线性化  后端
```

### 语言特性开发流程

当需要添加新的Nyar语言特性时，按以下步骤进行：

#### 1. 设计阶段

**创建RFC文档**:
```markdown
# RFC: 新特性名称

## 摘要
简要描述新特性的目的和价值。

## 动机
为什么需要这个特性？解决什么问题？

## 详细设计
### 语法设计
### 语义定义
### 类型系统影响
### 代数效应集成

## 实现计划
### AST节点设计
### HIR降糖策略
### MIR控制流表示
### LIR线性化方案
### 后端代码生成

## 向后兼容性
## 替代方案
## 未解决的问题
```

#### 2. AST节点定义 (valkyrie-parser)

在`valkyrie-parser/src/ast.rs`中添加新的AST节点：

```rust
// 示例：添加async/await语法支持
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    // ... 现有表达式
    
    // 新增异步表达式
    Async(Box<Expression>),
    Await(Box<Expression>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    // ... 现有语句
    
    // 新增异步函数定义
    AsyncFunction {
        name: Identifier,
        params: Vec<Parameter>,
        return_type: Option<Type>,
        body: Block,
    },
}

impl Expression {
    // 添加类型检查方法
    pub fn is_async(&self) -> bool {
        matches!(self, Expression::Async(_))
    }
    
    // 添加访问者模式支持
    pub fn accept<V: ExpressionVisitor>(&self, visitor: &mut V) -> V::Result {
        match self {
            Expression::Async(expr) => visitor.visit_async(expr),
            Expression::Await(expr) => visitor.visit_await(expr),
            _ => visitor.visit_default(self),
        }
    }
}
```

#### 3. HIR降糖处理 (nyar-hir)

在`nyar-hir/src/lower.rs`中添加AST到HIR的降糖逻辑：

```rust
impl HirLowering {
    fn lower_async_expr(&mut self, expr: &ast::Expression) -> hir::Expression {
        match expr {
            ast::Expression::Async(inner) => {
                // 将async表达式降糖为效应处理
                let effect_id = self.register_effect("Async");
                let inner_hir = self.lower_expression(inner);
                
                hir::Expression::Perform {
                    effect: effect_id,
                    args: vec![inner_hir],
                    continuation: self.fresh_continuation(),
                }
            }
            
            ast::Expression::Await(inner) => {
                // 将await表达式降糖为效应处理
                let effect_id = self.register_effect("Await");
                let inner_hir = self.lower_expression(inner);
                
                hir::Expression::Handle {
                    expr: Box::new(inner_hir),
                    handlers: vec![AwaitHandler::new(effect_id)],
                }
            }
            
            _ => self.lower_expression_default(expr),
        }
    }
}
```

#### 4. MIR控制流生成 (nyar-mir)

在`nyar-mir/src/build.rs`中添加HIR到MIR的控制流生成：

```rust
impl MirBuilder {
    fn build_async_expr(&mut self, expr: &hir::Expression) -> LocalVariable {
        match expr {
            hir::Expression::Perform { effect, args, continuation } => {
                // 创建异步操作的基本块
                let async_block = self.new_basic_block();
                let resume_block = self.new_basic_block();
                
                // 生成异步调用
                let result = self.new_local();
                self.push_statement(Statement::Assign {
                    place: result,
                    rvalue: RValue::Call {
                        func: *effect,
                        args: args.clone(),
                    },
                });
                
                // 生成控制流转移
                self.terminate_block(Terminator::Resume {
                    value: result,
                    target: resume_block,
                });
                
                result
            }
            
            _ => self.build_expression_default(expr),
        }
    }
}
```

#### 5. LIR线性化和后端代码生成

**LIR线性化** (`nyar-lir/src/lower.rs`):
```rust
impl LirLowering {
    fn lower_async_call(&mut self, call: &mir::Statement) -> Vec<lir::Instruction> {
        match call {
            mir::Statement::Assign { place, rvalue: mir::RValue::Call { func, args } } => {
                let mut instructions = Vec::new();
                
                // 准备参数
                for arg in args {
                    instructions.push(lir::Instruction::LoadLocal(*arg));
                }
                
                // 异步函数调用
                instructions.push(lir::Instruction::Call {
                    func_id: *func,
                    arg_count: args.len(),
                });
                
                // 存储结果
                instructions.push(lir::Instruction::StoreLocal(*place));
                
                instructions
            }
            _ => self.lower_statement_default(call),
        }
    }
}
```

**WebAssembly后端** (`nyar-wasm/src/codegen.rs`):
```rust
impl WasmCodegen {
    fn generate_async_call(&mut self, instr: &lir::Instruction) {
        match instr {
            lir::Instruction::Call { func_id, arg_count } => {
                // 生成Wasm调用指令
                self.emit(wasm::Instruction::Call(*func_id));
                
                // 处理异步返回值
                if self.is_async_function(*func_id) {
                    self.emit(wasm::Instruction::CallIndirect {
                        type_index: self.async_continuation_type(),
                    });
                }
            }
            _ => self.generate_instruction_default(instr),
        }
    }
}
```

**JavaScript后端** (`nyar-js/src/codegen.rs`):
```rust
impl JsCodegen {
    fn generate_async_call(&mut self, instr: &lir::Instruction) -> String {
        match instr {
            lir::Instruction::Call { func_id, arg_count } => {
                let func_name = self.get_function_name(*func_id);
                
                if self.is_async_function(*func_id) {
                    format!("await {}({})", func_name, self.get_args(*arg_count))
                } else {
                    format!("{}({})", func_name, self.get_args(*arg_count))
                }
            }
            _ => self.generate_instruction_default(instr),
        }
    }
}
```

#### 6. 集成测试

为新特性添加端到端的集成测试：

```rust
#[cfg(test)]
mod async_feature_tests {
    use super::*;
    
    #[test]
    fn test_async_function_compilation() {
        let source = r#"
            async fn fetch_data() -> String {
                let result = await http_get("https://api.example.com");
                result
            }
        "#;
        
        // 测试完整的编译流程
        let ast = parse_source(source).unwrap();
        let hir = lower_to_hir(ast).unwrap();
        let mir = build_mir(hir).unwrap();
        let lir = lower_to_lir(mir).unwrap();
        
        // 验证生成的LIR包含正确的异步指令
        assert!(lir.instructions.iter().any(|instr| {
            matches!(instr, lir::Instruction::Call { .. })
        }));
    }
    
    #[test]
    fn test_wasm_backend_async_generation() {
        let lir = create_async_lir_function();
        let wasm_module = WasmCodegen::new().compile(lir).unwrap();
        
        // 验证生成的Wasm模块包含异步支持
        assert!(wasm_module.has_async_imports());
        assert!(wasm_module.exports_async_functions());
    }
    
    #[test]
    fn test_js_backend_async_generation() {
        let lir = create_async_lir_function();
        let js_code = JsCodegen::new().compile(lir).unwrap();
        
        // 验证生成的JavaScript代码包含async/await
        assert!(js_code.contains("async function"));
        assert!(js_code.contains("await"));
    }
}
```

## 工具链开发

### Valkyrie前端开发

Valkyrie是Nyar语言的前端编译器，负责词法分析、语法分析和AST构建。

#### 架构设计

```rust
// valkyrie-parser/src/lib.rs
pub struct Parser {
    lexer: Lexer,
    current_token: Token,
    peek_token: Token,
}

impl Parser {
    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut statements = Vec::new();
        
        while !self.is_at_end() {
            statements.push(self.parse_statement()?);
        }
        
        Ok(Program { statements })
    }
    
    fn parse_statement(&mut self) -> Result<Statement, ParseError> {
        match &self.current_token {
            Token::Let => self.parse_let_statement(),
            Token::Fn => self.parse_function_definition(),
            Token::Effect => self.parse_effect_definition(),
            _ => self.parse_expression_statement(),
        }
    }
}
```

#### 错误恢复策略

```rust
impl Parser {
    fn recover_from_error(&mut self, expected: &[TokenType]) -> ParseError {
        // 同步到下一个语句边界
        while !self.is_at_end() && !self.is_statement_start() {
            self.advance();
        }
        
        ParseError::UnexpectedToken {
            expected: expected.to_vec(),
            found: self.current_token.clone(),
            suggestions: self.suggest_corrections(),
        }
    }
    
    fn suggest_corrections(&self) -> Vec<String> {
        // 基于编辑距离的错误建议
        let mut suggestions = Vec::new();
        
        if let Token::Identifier(name) = &self.current_token {
            suggestions.extend(self.find_similar_keywords(name));
            suggestions.extend(self.find_similar_identifiers(name));
        }
        
        suggestions
    }
}
```

### WebAssembly后端开发

WebAssembly后端将LIR编译为高效的WebAssembly代码。

#### WasmGC集成

```rust
// nyar-wasm/src/codegen.rs
pub struct WasmCodegen {
    module: wasm_encoder::Module,
    type_section: wasm_encoder::TypeSection,
    function_section: wasm_encoder::FunctionSection,
    code_section: wasm_encoder::CodeSection,
}

impl WasmCodegen {
    pub fn compile_function(&mut self, func: &lir::Function) -> Result<u32, CodegenError> {
        let mut function_body = wasm_encoder::Function::new([]);
        
        for instruction in &func.instructions {
            self.compile_instruction(instruction, &mut function_body)?;
        }
        
        let func_idx = self.function_section.len();
        self.function_section.function(func.type_idx);
        self.code_section.function(&function_body);
        
        Ok(func_idx)
    }
    
    fn compile_instruction(
        &mut self, 
        instr: &lir::Instruction, 
        func: &mut wasm_encoder::Function
    ) -> Result<(), CodegenError> {
        match instr {
            lir::Instruction::StructNew { type_idx, fields } => {
                // 加载字段值
                for field in fields {
                    self.compile_operand(field, func)?;
                }
                // 创建结构体
                func.instruction(&wasm_encoder::Instruction::StructNew(*type_idx));
            }
            
            lir::Instruction::StructGet { obj, field_idx } => {
                self.compile_operand(obj, func)?;
                func.instruction(&wasm_encoder::Instruction::StructGet {
                    struct_type_index: obj.type_idx(),
                    field_index: *field_idx,
                });
            }
            
            lir::Instruction::Call { func_id, args } => {
                // 加载参数
                for arg in args {
                    self.compile_operand(arg, func)?;
                }
                func.instruction(&wasm_encoder::Instruction::Call(*func_id));
            }
            
            _ => return Err(CodegenError::UnsupportedInstruction(instr.clone())),
        }
        
        Ok(())
    }
}
```

### JavaScript后端开发

JavaScript后端专注于快速开发和Web生态集成。

#### 代码生成策略

```rust
// nyar-js/src/codegen.rs
pub struct JsCodegen {
    output: String,
    indent_level: usize,
    temp_counter: usize,
    runtime_imports: HashSet<String>,
}

impl JsCodegen {
    pub fn compile_function(&mut self, func: &lir::Function) -> Result<String, CodegenError> {
        let mut js_func = format!("function {}(", func.name);
        
        // 生成参数列表
        for (i, param) in func.params.iter().enumerate() {
            if i > 0 { js_func.push_str(", "); }
            js_func.push_str(&format!("_{}", param.index));
        }
        js_func.push_str(") {\n");
        
        self.indent_level += 1;
        
        // 生成函数体
        for instruction in &func.instructions {
            js_func.push_str(&self.compile_instruction(instruction)?);
        }
        
        self.indent_level -= 1;
        js_func.push_str("}\n");
        
        Ok(js_func)
    }
    
    fn compile_instruction(&mut self, instr: &lir::Instruction) -> Result<String, CodegenError> {
        let indent = "  ".repeat(self.indent_level);
        
        match instr {
            lir::Instruction::StructNew { fields } => {
                let mut obj = format!("{}const obj_{} = {{\n", indent, self.next_temp());
                
                for (i, field) in fields.iter().enumerate() {
                    obj.push_str(&format!("{}  field_{}: {},\n", 
                        indent, i, self.compile_operand(field)?));
                }
                
                obj.push_str(&format!("{}}};\n", indent));
                Ok(obj)
            }
            
            lir::Instruction::Call { func_id, args } => {
                let func_name = self.get_function_name(*func_id);
                let mut call = format!("{}const result_{} = {}(", 
                    indent, self.next_temp(), func_name);
                
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 { call.push_str(", "); }
                    call.push_str(&self.compile_operand(arg)?);
                }
                
                call.push_str(");\n");
                Ok(call)
            }
            
            _ => Err(CodegenError::UnsupportedInstruction(instr.clone())),
        }
    }
}
```

### React组件生成

可以生成React hooks和组件：

```typescript
// 生成的React hooks
export function useUser(id: string) {
  return useQuery(['user', id], () => client.getUser({ id }));
}

export function useCreateUser() {
  const queryClient = useQueryClient();
  
  return useMutation(
    (request: CreateUserRequest) => client.createUser(request),
    {
      onSuccess: () => {
        queryClient.invalidateQueries(['users']);
      }
    }
  );
}

// 使用示例
function UserProfile({ userId }: { userId: string }) {
  const { data: user, isLoading, error } = useUser(userId);
  const createUser = useCreateUser();
  
  if (isLoading) return <div>Loading...</div>;
  if (error) return <div>Error: {error.message}</div>;
  
  return (
    <div>
      <h1>{user.username}</h1>
      <p>{user.email}</p>
    </div>
  );
}
```

## 后端开发

### Rust服务端生成

#### 服务trait生成

```rust
// 生成的服务trait

pub trait UserService {
    async fn create_user(&self, request: CreateUserRequest) -> Result<User, UserServiceError>;
    async fn get_user(&self, request: GetUserRequest) -> Result<Option<User>, UserServiceError>;
    async fn update_user(&self, request: UpdateUserRequest) -> Result<User, UserServiceError>;
    async fn delete_user(&self, request: DeleteUserRequest) -> Result<(), UserServiceError>;
}

// 错误类型定义
#[derive(Debug, thiserror::Error)]
pub enum UserServiceError {
    #[error("User not found: {id}")]
    UserNotFound { id: String },
    
    #[error("Validation error: {message}")]
    ValidationError { message: String },
    
    #[error("Database error: {0}")]
    DatabaseError(#[from] sqlx::Error),
}
```

#### 实现示例

```rust
// 用户实现
pub struct UserServiceImpl {
    db: PgPool,
}


impl UserService for UserServiceImpl {
    async fn create_user(&self, request: CreateUserRequest) -> Result<User, UserServiceError> {
        // 验证输入
        request.validate()?;
        
        // 检查用户是否已存在
        if self.user_exists(&request.email).await? {
            return Err(UserServiceError::ValidationError {
                message: "Email already exists".to_string(),
            });
        }
        
        // 创建用户
        let user = sqlx::query_as!(
            User,
            "INSERT INTO users (username, email, password_hash) VALUES ($1, $2, $3) RETURNING *",
            request.username,
            request.email,
            hash_password(&request.password)?
        )
        .fetch_one(&self.db)
        .await?;
        
        Ok(user)
    }
    
    async fn get_user(&self, request: GetUserRequest) -> Result<Option<User>, UserServiceError> {
        let user = sqlx::query_as!(
            User,
            "SELECT * FROM users WHERE id = $1",
            request.id
        )
        .fetch_optional(&self.db)
        .await?;
        
        Ok(user)
    }
}
```

#### HTTP服务器集成

```rust
// Axum集成示例
use axum::{extract::State, http::StatusCode, response::Json, routing::post, Router};

pub fn create_router(service: Arc<dyn UserService>) -> Router {
    Router::new()
        .route("/users", post(create_user_handler))
        .route("/users/:id", get(get_user_handler))
        .with_state(service)
}

async fn create_user_handler(
    State(service): State<Arc<dyn UserService>>,
    Json(request): Json<CreateUserRequest>,
) -> Result<Json<User>, (StatusCode, String)> {
    match service.create_user(request).await {
        Ok(user) => Ok(Json(user)),
        Err(UserServiceError::ValidationError { message }) => {
            Err((StatusCode::BAD_REQUEST, message))
        }
        Err(err) => Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string())),
    }
}
```

### 其他语言后端

#### Go服务端生成

```go
// 生成的Go接口
type UserService interface {
    CreateUser(ctx context.Context, req *CreateUserRequest) (*User, error)
    GetUser(ctx context.Context, req *GetUserRequest) (*User, error)
    UpdateUser(ctx context.Context, req *UpdateUserRequest) (*User, error)
    DeleteUser(ctx context.Context, req *DeleteUserRequest) error
}

// HTTP处理器生成
func NewUserServiceHandler(service UserService) http.Handler {
    mux := http.NewServeMux()
    
    mux.HandleFunc("/users", func(w http.ResponseWriter, r *http.Request) {
        switch r.Method {
        case http.MethodPost:
            handleCreateUser(service, w, r)
        default:
            http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
        }
    })
    
    return mux
}
```

## 插件开发

### 代码生成插件

#### 插件接口定义

```rust
// vos-core/src/plugin.rs
pub trait CodeGenerator {
    fn name(&self) -> &str;
    fn supported_languages(&self) -> Vec<&str>;
    fn generate(&self, schema: &Schema, config: &GeneratorConfig) -> Result<GeneratedCode, GeneratorError>;
}

#[derive(Debug)]
pub struct GeneratedCode {
    pub files: Vec<GeneratedFile>,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug)]
pub struct GeneratedFile {
    pub path: PathBuf,
    pub content: String,
    pub language: String,
}
```

#### 插件实现示例

```rust
// 自定义Python生成器插件
pub struct PythonGenerator {
    config: PythonConfig,
}

impl CodeGenerator for PythonGenerator {
    fn name(&self) -> &str {
        "python"
    }
    
    fn supported_languages(&self) -> Vec<&str> {
        vec!["python"]
    }
    
    fn generate(&self, schema: &Schema, config: &GeneratorConfig) -> Result<GeneratedCode, GeneratorError> {
        let mut files = Vec::new();
        
        // 生成数据类
        for class_def in &schema.classes {
            let content = self.generate_dataclass(class_def)?;
            files.push(GeneratedFile {
                path: PathBuf::from(format!("{}.py", class_def.name.to_snake_case())),
                content,
                language: "python".to_string(),
            });
        }
        
        // 生成客户端
        for service_def in &schema.services {
            let content = self.generate_client(service_def)?;
            files.push(GeneratedFile {
                path: PathBuf::from(format!("{}_client.py", service_def.name.to_snake_case())),
                content,
                language: "python".to_string(),
            });
        }
        
        Ok(GeneratedCode {
            files,
            dependencies: vec![
                Dependency::new("pydantic", "^1.10.0"),
                Dependency::new("httpx", "^0.24.0"),
            ],
        })
    }
}

impl PythonGenerator {
    fn generate_dataclass(&self, class_def: &ClassDefinition) -> Result<String, GeneratorError> {
        let mut output = String::new();
        
        output.push_str("from pydantic import BaseModel\n\n");
        output.push_str(&format!("class {}(BaseModel):\n", class_def.name));
        
        for field in &class_def.fields {
            let field_type = self.map_type(&field.type_def)?;
            output.push_str(&format!("    {}: {}\n", field.name, field_type));
        }
        
        Ok(output)
    }
}
```

### 验证插件

```rust
// 自定义验证器插件
pub trait Validator {
    fn name(&self) -> &str;
    fn validate(&self, value: &VosValue, type_def: &TypeDefinition) -> Result<(), ValidationError>;
}

pub struct EmailValidator;

impl Validator for EmailValidator {
    fn name(&self) -> &str {
        "email"
    }
    
    fn validate(&self, value: &VosValue, _type_def: &TypeDefinition) -> Result<(), ValidationError> {
        if let VosValue::String(s) = value {
            if email_address::EmailAddress::is_valid(s) {
                Ok(())
            } else {
                Err(ValidationError::InvalidEmail(s.clone()))
            }
        } else {
            Err(ValidationError::TypeMismatch {
                expected: "String".to_string(),
                actual: value.type_name().to_string(),
            })
        }
    }
}
```

### 插件注册和使用

```rust
// 插件注册
pub struct PluginRegistry {
    generators: HashMap<String, Box<dyn CodeGenerator>>,
    validators: HashMap<String, Box<dyn Validator>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            generators: HashMap::new(),
            validators: HashMap::new(),
        };
        
        // 注册内置插件
        registry.register_generator(Box::new(RustGenerator::new()));
        registry.register_generator(Box::new(TypeScriptGenerator::new()));
        registry.register_validator(Box::new(EmailValidator));
        
        registry
    }
    
    pub fn register_generator(&mut self, generator: Box<dyn CodeGenerator>) {
        self.generators.insert(generator.name().to_string(), generator);
    }
    
    pub fn generate(&self, language: &str, schema: &Schema, config: &GeneratorConfig) -> Result<GeneratedCode, GeneratorError> {
        let generator = self.generators.get(language)
            .ok_or_else(|| GeneratorError::UnsupportedLanguage(language.to_string()))?;
        
        generator.generate(schema, config)
    }
}
```

## 调试和测试

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    
    #[test]
    fn test_parse_service_definition() {
        let input = r#"
            agent UserService {
                function get_user(id: UserId): User?;
                function create_user(req: CreateUserRequest): User;
            }
        "#;
        
        let mut parser = VosParser::new(input.to_string());
        let result = parser.parse_service_definition().unwrap();
        
        assert_eq!(result.name, "UserService");
        assert_eq!(result.functions.len(), 2);
    }
    
    #[test]
    fn test_generate_rust_service() {
        let service = ServiceDefinition {
            name: "UserService".to_string(),
            functions: vec![
                // ... 函数定义
            ],
        };
        
        let generator = RustGenerator::new();
        let result = generator.generate_service(&service).unwrap();
        
        assert!(result.contains("trait UserService"));
        assert!(result.contains("async fn get_user"));
    }
}
```

### 集成测试

```rust
// tests/integration_test.rs
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_end_to_end_generation() {
    let temp_dir = TempDir::new().unwrap();
    
    // 创建测试VOS文件
    let vos_content = r#"
        namespace test::service;
        
        class User {
            id: u64;
            name: String;
        }
        
        agent UserService {
            function get_user(id: u64): User?;
        }
    "#;
    
    std::fs::write(temp_dir.path().join("test.vos"), vos_content).unwrap();
    
    // 运行代码生成
    let output = Command::new("cargo")
        .args(["run", "--bin", "vos", "--", "generate", "--lang", "rust"])
        .current_dir(temp_dir.path())
        .output()
        .unwrap();
    
    assert!(output.status.success());
    
    // 验证生成的文件
    let generated_content = std::fs::read_to_string(
        temp_dir.path().join("generated/rust/user_service.rs")
    ).unwrap();
    
    assert!(generated_content.contains("trait UserService"));
    assert!(generated_content.contains("struct User"));
}
```

### 性能测试

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_parser(c: &mut Criterion) {
    let large_vos_file = include_str!("../test_data/large_schema.vos");
    
    c.bench_function("parse_large_schema", |b| {
        b.iter(|| {
            let mut parser = VosParser::new(black_box(large_vos_file.to_string()));
            parser.parse_program().unwrap()
        })
    });
}

fn benchmark_codegen(c: &mut Criterion) {
    let schema = load_test_schema();
    let generator = RustGenerator::new();
    
    c.bench_function("generate_rust_code", |b| {
        b.iter(|| {
            generator.generate(black_box(&schema), &GeneratorConfig::default()).unwrap()
        })
    });
}

criterion_group!(benches, benchmark_parser, benchmark_codegen);
criterion_main!(benches);
```

## 最佳实践

### 代码组织

1. **模块化设计**: 每个功能模块都应该有清晰的职责边界
2. **错误处理**: 使用`thiserror`和`miette`提供友好的错误信息
3. **测试覆盖**: 确保核心功能有充分的测试覆盖
4. **文档**: 所有公共API都应该有详细的文档

### 性能优化

1. **避免不必要的克隆**: 使用借用和`Cow`类型
2. **缓存解析结果**: 避免重复解析相同的文件
3. **并行处理**: 在可能的情况下使用并行处理
4. **内存管理**: 合理使用`Box`和`Arc`

### 安全考虑

1. **输入验证**: 严格验证所有外部输入
2. **依赖管理**: 定期更新依赖，修复安全漏洞
3. **代码审查**: 所有代码都应该经过审查
4. **模糊测试**: 使用模糊测试发现潜在问题

## 发布和部署

### 版本发布

```bash
# 更新版本号
cargo set-version 0.2.0

# 更新CHANGELOG
echo "## [0.2.0] - $(date +%Y-%m-%d)" >> CHANGELOG.md

# 运行完整测试
cargo test --workspace --all-features

# 发布到crates.io
cargo publish -p vos-core
cargo publish -p vos-ast
cargo publish -p vos-cli
```

### CI/CD配置

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - run: cargo test --workspace --all-features
      - run: cargo clippy -- -D warnings
      - run: cargo fmt --check
  
  release:
    if: startsWith(github.ref, 'refs/tags/')
    needs: test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - run: cargo publish --token ${{ secrets.CRATES_TOKEN }}
```

这个开发指南涵盖了VOS项目开发的各个方面，从语言特性开发到前后端代码生成，再到插件开发和测试。遵循这些指南可以确保代码质量和项目的可维护性。
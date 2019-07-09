# 执行模型的二元性

## 概述

Nyar 承认并拥抱软件开发的不同阶段和不同场景对执行环境有着截然不同的需求。因此，它提供了两种互补的执行模型，它们共享同一个前端（语言、解析器、语义分析器），但在后端采取了不同的策略。

## 设计理念

### 场景驱动的架构选择

不同的开发场景需要不同的执行特性：
- **开发阶段**：需要快速反馈、强大的调试能力、完整的运行时特性
- **生产部署**：需要高性能、小体积、快速启动、与现有生态的互操作性

### 共享前端，分化后端

两种执行模型共享相同的语言设计和前端编译流水线（AST → HIR → MIR），在 MIR 层面分化为不同的执行策略。

## 动态解释执行模型

### 设计目标

专为**开发、调试和交互式环境**（如 REPL）设计，提供最高保真度的语言特性支持。

### 核心组件

#### nyar-interpreter

```rust
/// Nyar 解释器的核心结构
pub struct NyarInterpreter {
    /// 内建的垃圾收集器
    gc: GarbageCollector,
    /// 对象模型和运行时
    runtime: Runtime,
    /// WebAssembly FFI 引擎
    wasm_engine: WasmEngine,
    /// 当前执行上下文
    context: ExecutionContext,
}

impl NyarInterpreter {
    /// 执行 MIR 程序
    pub fn execute(&mut self, program: &mir::Program) -> Result<Value, RuntimeError> {
        self.execute_function(&program.main_function)
    }
    
    /// 执行单个函数
    fn execute_function(&mut self, function: &mir::Function) -> Result<Value, RuntimeError> {
        let mut frame = StackFrame::new(function);
        
        loop {
            match &function.basic_blocks[frame.current_block].terminator {
                mir::Terminator::Return(value) => {
                    return Ok(self.evaluate_operand(value, &frame));
                }
                mir::Terminator::Call { func, args, destination, .. } => {
                    let result = self.handle_call(func, args, &frame)?;
                    frame.set_local(*destination, result);
                }
                mir::Terminator::Effect { effect, args, handlers } => {
                    self.handle_effect(effect, args, handlers, &mut frame)?;
                }
                // ... 其他终结符
            }
        }
    }
}
```

#### 内建垃圾收集器

```rust
/// 标记-清除垃圾收集器
pub struct GarbageCollector {
    heap: Heap,
    roots: Vec<ObjectRef>,
    allocation_threshold: usize,
}

impl GarbageCollector {
    /// 分配新对象
    pub fn allocate<T>(&mut self, value: T) -> ObjectRef
    where
        T: GcObject,
    {
        let obj_ref = self.heap.allocate(value);
        
        // 检查是否需要触发 GC
        if self.heap.allocated_bytes() > self.allocation_threshold {
            self.collect();
        }
        
        obj_ref
    }
    
    /// 执行垃圾收集
    fn collect(&mut self) {
        // 标记阶段：从根对象开始标记所有可达对象
        self.mark_phase();
        
        // 清除阶段：释放未标记的对象
        self.sweep_phase();
    }
    
    fn mark_phase(&mut self) {
        let mut worklist = self.roots.clone();
        
        while let Some(obj_ref) = worklist.pop() {
            if let Some(obj) = self.heap.get_mut(obj_ref) {
                if !obj.is_marked() {
                    obj.mark();
                    // 将对象引用的其他对象加入工作列表
                    worklist.extend(obj.references());
                }
            }
        }
    }
}
```

#### WebAssembly FFI 引擎

```rust
/// WebAssembly 外部函数接口引擎
pub struct WasmEngine {
    /// 已加载的 Wasm 模块
    modules: HashMap<String, WasmModule>,
    /// Wasm 运行时实例
    runtime: wasmtime::Engine,
}

impl WasmEngine {
    /// 加载外部 Wasm 模块
    pub fn load_module(&mut self, name: String, bytes: &[u8]) -> Result<(), WasmError> {
        let module = WasmModule::from_bytes(&self.runtime, bytes)?;
        self.modules.insert(name, module);
        Ok(())
    }
    
    /// 调用外部 Wasm 函数
    pub fn call_extern_function(
        &mut self,
        module_name: &str,
        function_name: &str,
        args: &[Value],
    ) -> Result<Value, WasmError> {
        let module = self.modules.get(module_name)
            .ok_or_else(|| WasmError::ModuleNotFound(module_name.to_string()))?;
        
        let function = module.get_function(function_name)
            .ok_or_else(|| WasmError::FunctionNotFound(function_name.to_string()))?;
        
        // 转换 Nyar 值为 Wasm 值
        let wasm_args = args.iter().map(|v| self.nyar_to_wasm_value(v)).collect::<Result<Vec<_>, _>>()?;
        
        // 调用 Wasm 函数
        let wasm_result = function.call(&wasm_args)?;
        
        // 转换 Wasm 值为 Nyar 值
        self.wasm_to_nyar_value(wasm_result)
    }
}
```

### 代数效应的完整支持

```rust
/// 代数效应的运行时表示
#[derive(Debug, Clone)]
pub struct Effect {
    pub name: String,
    pub args: Vec<Value>,
}

/// 效应处理器
pub struct EffectHandler {
    pub effect_name: String,
    pub handler_function: mir::FunctionId,
}

/// 延续对象
pub struct Continuation {
    /// 捕获的栈帧
    pub frames: Vec<StackFrame>,
    /// 恢复点
    pub resume_point: ResumePoint,
}

impl NyarInterpreter {
    /// 处理代数效应
    fn handle_effect(
        &mut self,
        effect: &Effect,
        args: &[mir::Operand],
        handlers: &[EffectHandler],
        frame: &mut StackFrame,
    ) -> Result<(), RuntimeError> {
        // 查找匹配的处理器
        for handler in handlers {
            if handler.effect_name == effect.name {
                // 创建延续对象
                let continuation = self.capture_continuation(frame);
                
                // 调用处理器函数
                let handler_args = vec![
                    Value::Effect(effect.clone()),
                    Value::Continuation(continuation),
                ];
                
                return self.call_handler_function(handler.handler_function, handler_args);
            }
        }
        
        // 如果没有找到处理器，向上传播效应
        self.propagate_effect(effect)
    }
    
    /// 恢复延续
    pub fn resume_continuation(
        &mut self,
        continuation: Continuation,
        value: Value,
    ) -> Result<Value, RuntimeError> {
        // 恢复栈帧
        self.context.stack_frames = continuation.frames;
        
        // 在恢复点继续执行
        match continuation.resume_point {
            ResumePoint::AfterCall { destination } => {
                self.context.current_frame_mut().set_local(destination, value);
                self.continue_execution()
            }
            ResumePoint::AfterEffect { .. } => {
                // 处理效应恢复逻辑
                self.continue_execution_after_effect(value)
            }
        }
    }
}
```

### 调试和开发工具支持

```rust
/// 调试器接口
pub struct Debugger {
    interpreter: NyarInterpreter,
    breakpoints: HashSet<mir::Location>,
    watch_expressions: Vec<String>,
}

impl Debugger {
    /// 设置断点
    pub fn set_breakpoint(&mut self, location: mir::Location) {
        self.breakpoints.insert(location);
    }
    
    /// 单步执行
    pub fn step(&mut self) -> Result<DebugState, RuntimeError> {
        let current_location = self.interpreter.current_location();
        
        // 执行一条指令
        self.interpreter.step_once()?;
        
        // 检查是否命中断点
        if self.breakpoints.contains(&current_location) {
            Ok(DebugState::Breakpoint(current_location))
        } else {
            Ok(DebugState::Running)
        }
    }
    
    /// 检查变量值
    pub fn inspect_variable(&self, name: &str) -> Option<Value> {
        self.interpreter.current_frame().get_local(name)
    }
}
```

## 静态编译执行模型

### 设计目标

专为**生产部署和高性能场景**设计，生成轻量、高效、可移植的 WebAssembly 模块。

### 核心特性

#### 利用 WasmGC 原生能力

```rust
/// LIR 到 WasmGC 的代码生成器
pub struct WasmGCCodeGen {
    module: wasm_encoder::Module,
    type_section: wasm_encoder::TypeSection,
    function_section: wasm_encoder::FunctionSection,
}

impl WasmGCCodeGen {
    /// 生成 WasmGC 类型定义
    fn generate_gc_types(&mut self, program: &lir::Program) {
        for type_def in &program.type_definitions {
            match type_def {
                lir::TypeDef::Struct { name, fields } => {
                    // 生成 WasmGC 结构体类型
                    let struct_type = wasm_encoder::StructType::new(
                        fields.iter().map(|f| self.lir_type_to_wasm_type(&f.ty)).collect()
                    );
                    self.type_section.subtype(wasm_encoder::SubType::new(
                        vec![],
                        wasm_encoder::CompositeType::Struct(struct_type),
                        false,
                    ));
                }
                lir::TypeDef::Array { element_type } => {
                    // 生成 WasmGC 数组类型
                    let array_type = wasm_encoder::ArrayType::new(
                        self.lir_type_to_wasm_type(element_type),
                        true, // mutable
                    );
                    self.type_section.subtype(wasm_encoder::SubType::new(
                        vec![],
                        wasm_encoder::CompositeType::Array(array_type),
                        false,
                    ));
                }
            }
        }
    }
    
    /// 生成函数代码
    fn generate_function(&mut self, function: &lir::Function) {
        let mut func_body = wasm_encoder::Function::new(vec![]);
        
        for instruction in &function.instructions {
            match instruction {
                lir::Instruction::StructNew { type_id, fields } => {
                    // 生成 WasmGC struct.new 指令
                    for field in fields {
                        self.generate_operand(&mut func_body, field);
                    }
                    func_body.instruction(&wasm_encoder::Instruction::StructNew(*type_id));
                }
                lir::Instruction::StructGet { type_id, field_index, object } => {
                    // 生成 WasmGC struct.get 指令
                    self.generate_operand(&mut func_body, object);
                    func_body.instruction(&wasm_encoder::Instruction::StructGet {
                        struct_type_index: *type_id,
                        field_index: *field_index,
                    });
                }
                lir::Instruction::ArrayNew { type_id, length, init_value } => {
                    // 生成 WasmGC array.new 指令
                    self.generate_operand(&mut func_body, init_value);
                    self.generate_operand(&mut func_body, length);
                    func_body.instruction(&wasm_encoder::Instruction::ArrayNew(*type_id));
                }
                // ... 其他指令
            }
        }
        
        self.function_section.function(func_body);
    }
}
```

#### 零成本抽象的实现

```rust
/// 内建函数优化器
pub struct IntrinsicOptimizer {
    intrinsics: HashMap<String, IntrinsicHandler>,
}

/// 内建函数处理器
type IntrinsicHandler = fn(&mut WasmGCCodeGen, &[lir::Operand]) -> Result<(), CodeGenError>;

impl IntrinsicOptimizer {
    pub fn new() -> Self {
        let mut intrinsics = HashMap::new();
        
        // i32 加法直接映射到 Wasm 指令
        intrinsics.insert("i32.add".to_string(), |codegen, args| {
            codegen.generate_operand(args[0]);
            codegen.generate_operand(args[1]);
            codegen.emit_instruction(wasm_encoder::Instruction::I32Add);
            Ok(())
        });
        
        // 字符串连接使用优化的实现
        intrinsics.insert("string.concat".to_string(), |codegen, args| {
            // 调用优化的字符串连接函数
            codegen.generate_call("__nyar_string_concat", args);
            Ok(())
        });
        
        Self { intrinsics }
    }
    
    /// 尝试优化函数调用为内建指令
    pub fn try_optimize_call(
        &self,
        codegen: &mut WasmGCCodeGen,
        function_name: &str,
        args: &[lir::Operand],
    ) -> Result<bool, CodeGenError> {
        if let Some(handler) = self.intrinsics.get(function_name) {
            handler(codegen, args)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
```

#### 代数效应的 CPS 变换

```rust
/// 延续传递风格 (CPS) 变换器
pub struct CpsTransformer {
    continuation_type_id: u32,
}

impl CpsTransformer {
    /// 将带有效应的函数转换为 CPS 形式
    pub fn transform_function(&mut self, function: &mir::Function) -> lir::Function {
        let mut lir_function = lir::Function::new(function.name.clone());
        
        // 为每个可能抛出效应的函数添加延续参数
        if self.function_may_throw_effects(function) {
            lir_function.parameters.push(lir::Parameter {
                name: "__continuation".to_string(),
                ty: lir::Type::Ref(self.continuation_type_id),
            });
        }
        
        // 转换函数体
        for block in &function.basic_blocks {
            let lir_block = self.transform_basic_block(block);
            lir_function.basic_blocks.push(lir_block);
        }
        
        lir_function
    }
    
    /// 转换基本块
    fn transform_basic_block(&mut self, block: &mir::BasicBlock) -> lir::BasicBlock {
        let mut lir_block = lir::BasicBlock::new();
        
        // 转换语句
        for statement in &block.statements {
            match statement {
                mir::Statement::Call { func, args, destination } => {
                    if self.function_may_throw_effects_by_name(func) {
                        // 创建延续对象
                        let continuation = self.create_continuation(&lir_block);
                        
                        // 调用函数时传递延续
                        let mut call_args = args.clone();
                        call_args.push(mir::Operand::Local(continuation));
                        
                        lir_block.instructions.push(lir::Instruction::Call {
                            function: func.clone(),
                            args: call_args,
                            destination: *destination,
                        });
                    } else {
                        // 普通函数调用，无需 CPS 变换
                        lir_block.instructions.push(lir::Instruction::Call {
                            function: func.clone(),
                            args: args.clone(),
                            destination: *destination,
                        });
                    }
                }
                // ... 其他语句
            }
        }
        
        lir_block
    }
}
```

### 与现有生态的互操作

```rust
/// Wasm 模块导入/导出管理器
pub struct WasmInterop {
    imports: Vec<WasmImport>,
    exports: Vec<WasmExport>,
}

#[derive(Debug, Clone)]
pub struct WasmImport {
    pub module: String,
    pub name: String,
    pub ty: WasmType,
}

#[derive(Debug, Clone)]
pub struct WasmExport {
    pub name: String,
    pub internal_name: String,
    pub ty: WasmType,
}

impl WasmInterop {
    /// 处理 extern 块
    pub fn process_extern_block(&mut self, extern_block: &hir::ExternBlock) {
        match extern_block.abi.as_str() {
            "wasm" => {
                for item in &extern_block.items {
                    match item {
                        hir::ExternItem::Function(func) => {
                            self.imports.push(WasmImport {
                                module: "env".to_string(),
                                name: func.name.clone(),
                                ty: self.hir_function_to_wasm_type(func),
                            });
                        }
                    }
                }
            }
            "js" => {
                // 处理 JavaScript 互操作
                self.process_js_interop(extern_block);
            }
            _ => {
                // 未知 ABI，报告错误
            }
        }
    }
    
    /// 生成导入段
    pub fn generate_import_section(&self) -> wasm_encoder::ImportSection {
        let mut import_section = wasm_encoder::ImportSection::new();
        
        for import in &self.imports {
            import_section.import(
                &import.module,
                &import.name,
                wasm_encoder::EntityType::Function(import.ty.clone()),
            );
        }
        
        import_section
    }
}
```

## 两种模型的协调

### 共享的中间表示

两种执行模型都基于相同的 MIR，确保语义一致性：

```rust
/// 执行模型选择器
pub enum ExecutionTarget {
    /// 解释执行
    Interpreter,
    /// 编译为 WebAssembly
    WebAssembly,
    /// 编译为 JavaScript
    JavaScript,
}

/// 统一的编译接口
pub struct Compiler {
    database: NyarDatabase,
}

impl Compiler {
    /// 编译到指定目标
    pub fn compile(
        &self,
        source: &str,
        target: ExecutionTarget,
    ) -> Result<CompilationResult, CompilerError> {
        // 共享的前端流水线
        let ast = self.database.parse_source(source)?;
        let hir = self.database.lower_to_hir(ast)?;
        let mir = self.database.lower_to_mir(hir)?;
        
        // 根据目标选择后端
        match target {
            ExecutionTarget::Interpreter => {
                Ok(CompilationResult::Mir(mir))
            }
            ExecutionTarget::WebAssembly => {
                let lir = self.database.lower_to_lir(mir)?;
                let wasm_bytes = self.database.generate_wasm(lir)?;
                Ok(CompilationResult::Wasm(wasm_bytes))
            }
            ExecutionTarget::JavaScript => {
                let lir = self.database.lower_to_lir(mir)?;
                let js_code = self.database.generate_js(lir)?;
                Ok(CompilationResult::JavaScript(js_code))
            }
        }
    }
}
```

### 开发到生产的无缝切换

```rust
/// 项目配置
#[derive(Debug, Clone)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
    pub development: DevelopmentConfig,
    pub production: ProductionConfig,
}

#[derive(Debug, Clone)]
pub struct DevelopmentConfig {
    /// 使用解释器进行快速开发
    pub use_interpreter: bool,
    /// 启用调试信息
    pub debug_info: bool,
    /// 启用热重载
    pub hot_reload: bool,
}

#[derive(Debug, Clone)]
pub struct ProductionConfig {
    /// 目标平台
    pub target: ExecutionTarget,
    /// 优化级别
    pub optimization_level: OptimizationLevel,
    /// 是否启用 WasmGC
    pub enable_wasm_gc: bool,
}
```

## 性能对比

### 开发阶段性能特征

- **启动时间**: 极快（无需编译）
- **内存使用**: 较高（完整运行时）
- **执行速度**: 中等（解释执行）
- **调试能力**: 极强（完整反射和调试信息）

### 生产阶段性能特征

- **启动时间**: 快（预编译）
- **内存使用**: 低（利用平台 GC）
- **执行速度**: 高（原生编译）
- **体积**: 小（无运行时捆绑）

## 总结

执行模型的二元性是 Nyar 架构的核心创新之一。通过在开发和生产阶段提供不同的执行策略，Nyar 既保证了开发者在开发阶段的高效体验，又确保了生产环境的高性能表现。这种设计避免了传统语言在开发体验和运行时性能之间的权衡，为现代软件开发提供了理想的解决方案。
# Salsa 增量计算框架

## 概述

Salsa 是 Nyar 编译器的核心增量计算引擎，它将整个编译过程分解为细粒度的、可缓存的计算任务。当开发者修改代码时，只有真正受到影响的最小计算子集会被重新执行，实现亚秒级的编译响应。

## 设计理念

### 心流不被打断 (Uninterrupted Flow State)

现代软件开发是一种创造性的心流过程。漫长的编译等待是这种心流的最大破坏者。Salsa 的目标是让"编译"这个动作从开发者的意识中消失，从而保护其宝贵的心流状态。

### 细粒度缓存

Salsa 将编译过程建模为一个有向无环图 (DAG)，其中每个节点代表一个计算任务，边代表依赖关系。当输入发生变化时，只有依赖于该输入的节点需要重新计算。

## 核心概念

### Query (查询)

查询是 Salsa 中的基本计算单元。每个查询都是一个纯函数，给定相同的输入总是产生相同的输出。

```rust
#[salsa::query_group(CompilerDatabase)]
pub trait CompilerQueries {
    /// 解析源文件为 AST
    fn parse_file(&self, file_id: FileId) -> Arc<ast::Module>;
    
    /// 从 AST 构建 HIR
    fn lower_to_hir(&self, file_id: FileId) -> Arc<hir::Program>;
    
    /// 类型检查
    fn type_check(&self, program: Arc<hir::Program>) -> TypeCheckResult;
}
```

### Database (数据库)

数据库是所有查询的执行环境，负责管理缓存、依赖追踪和增量更新。

```rust
#[salsa::database(CompilerDatabase)]
pub struct NyarDatabase {
    storage: salsa::Storage<Self>,
}

impl salsa::Database for NyarDatabase {}
```

### Input (输入)

输入是计算图的根节点，代表外部数据源（如源文件内容）。

```rust
#[salsa::input]
pub struct SourceFile {
    #[return_ref]
    pub content: String,
    pub path: PathBuf,
}
```

## 工作流程

1. **初始化**: 创建数据库实例，注册所有查询函数
2. **设置输入**: 将源文件内容作为输入添加到数据库
3. **执行查询**: 调用高级查询（如类型检查），Salsa 自动处理依赖关系
4. **缓存结果**: 所有中间结果都被缓存
5. **增量更新**: 当源文件变化时，只重新计算受影响的查询

## 在 Nyar 中的应用

### 编译流水线集成

```rust
// 完整的编译流水线作为查询链
fn compile_to_wasm(db: &dyn CompilerQueries, file_id: FileId) -> Result<Vec<u8>, CompileError> {
    let ast = db.parse_file(file_id);           // 可缓存
    let hir = db.lower_to_hir(file_id);         // 可缓存
    let mir = db.lower_to_mir(hir);             // 可缓存
    let lir = db.lower_to_lir(mir);             // 可缓存
    let wasm = db.generate_wasm(lir);           // 可缓存
    Ok(wasm)
}
```

### 语言服务器支持

Salsa 的增量特性使得实现高性能的语言服务器变得简单：

```rust
// 当用户修改文件时
fn on_file_changed(&mut self, file_id: FileId, new_content: String) {
    // 更新输入
    self.db.set_file_content(file_id, new_content);
    
    // 所有依赖的查询会自动失效
    // 下次访问时会重新计算
}

// 提供诊断信息
fn get_diagnostics(&self, file_id: FileId) -> Vec<Diagnostic> {
    // 只有必要的部分会重新计算
    self.db.type_check_file(file_id).diagnostics
}
```

## 性能优化

### 并行计算

Salsa 支持并行执行独立的查询，充分利用多核处理器：

```rust
#[salsa::query_group(ParallelQueries)]
pub trait ParallelQueries {
    #[salsa::invoke(parallel_type_check)]
    fn type_check_module(&self, module_id: ModuleId) -> TypeCheckResult;
}
```

### 内存管理

Salsa 提供了多种缓存策略来平衡内存使用和计算性能：

- `#[salsa::memoized]`: 完全缓存结果
- `#[salsa::volatile]`: 每次都重新计算
- `#[salsa::transparent]`: 透明查询，不缓存

## 调试和监控

### 查询依赖图可视化

```rust
// 生成依赖图用于调试
fn debug_dependencies(db: &NyarDatabase) {
    let debug_info = db.debug();
    println!("Query dependencies: {:#?}", debug_info.dependencies());
}
```

### 性能分析

```rust
// 启用性能分析
let mut db = NyarDatabase::default();
db.set_debug_query_table();

// 执行编译
let result = compile_to_wasm(&db, file_id);

// 查看性能统计
for (query, stats) in db.query_stats() {
    println!("{}: {} calls, {}ms total", query, stats.calls, stats.duration_ms);
}
```

## 最佳实践

1. **保持查询纯净**: 查询函数应该是纯函数，不应有副作用
2. **合理的粒度**: 查询不应过于细粒度（增加开销）也不应过于粗粒度（减少缓存效果）
3. **避免循环依赖**: 设计查询时要避免循环依赖
4. **使用适当的缓存策略**: 根据查询的特性选择合适的缓存策略

## 与其他组件的集成

### 错误处理集成

```rust
#[salsa::query_group(ErrorQueries)]
pub trait ErrorQueries {
    fn collect_errors(&self, file_id: FileId) -> Vec<miette::Report>;
}

// 错误也可以被缓存
fn collect_errors(db: &dyn ErrorQueries, file_id: FileId) -> Vec<miette::Report> {
    let mut errors = Vec::new();
    
    // 收集各阶段的错误
    if let Err(parse_errors) = db.parse_file(file_id) {
        errors.extend(parse_errors);
    }
    
    if let Err(type_errors) = db.type_check(file_id) {
        errors.extend(type_errors);
    }
    
    errors
}
```

## 总结

Salsa 增量计算框架是 Nyar 编译器实现高性能、响应式开发体验的核心技术。通过将编译过程建模为细粒度的查询图，Salsa 确保了只有真正需要的计算会被执行，从而实现了亚秒级的编译响应时间，保护了开发者的心流状态。
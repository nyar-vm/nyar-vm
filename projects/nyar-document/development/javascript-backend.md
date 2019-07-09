# JavaScript 后端

JavaScript 后端是 Nyar 虚拟机平台的重要组成部分，负责将 Nyar LIR (Low-level Intermediate Representation) 编译为高效的 JavaScript 代码。通过这个后端，任何编译到 Nyar 平台的语言都能无缝运行在 JavaScript 生态系统中。

## 概述

JavaScript 后端的核心职责是将经过 Nyar 平台多层优化的 LIR 代码转换为可在 JavaScript 引擎中高效执行的代码。这种设计让语言实现者无需关心 JavaScript 的复杂性，而是专注于语言特性的设计，由 Nyar 平台处理所有底层的编译和优化工作。

## 从 Nyar 平台获得的核心优势

### 🚀 **高级优化能力**
- **死代码消除**: 自动移除未使用的函数和变量
- **常量折叠**: 编译时计算常量表达式
- **内联优化**: 智能的函数内联决策
- **循环优化**: 循环展开和强度削减
- **控制流优化**: 分支预测和跳转优化

### 🌐 **JavaScript 生态集成**
- **无缝互操作**: 与现有 JavaScript 库完美集成
- **模块系统**: 支持 ES6 模块和 CommonJS
- **异步支持**: 原生支持 Promise 和 async/await
- **类型映射**: 智能的类型到 JavaScript 的映射

### 🎯 **多运行时支持**
- **浏览器优化**: 针对 V8、SpiderMonkey、JavaScriptCore 的优化
- **Node.js 支持**: 服务端 JavaScript 的完整支持
- **Deno 兼容**: 现代 JavaScript 运行时支持
- **WebWorker**: 多线程 JavaScript 执行

### 🛠️ **开发者友好**
- **源码映射**: 精确的调试信息保留
- **可读输出**: 生成人类可读的 JavaScript 代码
- **性能分析**: 内置的性能监控和分析
- **错误追踪**: 完整的错误堆栈信息

## 编译流程架构

```
Nyar LIR
    ↓
JavaScript 代码生成器
    ↓
运行时库注入
    ↓
优化和压缩
    ↓
源码映射生成
    ↓
JavaScript 输出
```

## 核心组件设计

### LIR 到 JavaScript 转换器

**职责**: 将 Nyar LIR 指令转换为等价的 JavaScript 代码。

**转换策略**:
- **指令映射**: 每个 LIR 指令对应特定的 JavaScript 模式
- **寄存器分配**: 虚拟寄存器到 JavaScript 变量的映射
- **内存管理**: 堆分配和垃圾回收的 JavaScript 实现
- **控制流**: 跳转和分支的 JavaScript 表示

```rust
pub struct JSCodeGenerator {
    output: String,
    register_map: HashMap<Register, String>,
    label_map: HashMap<Label, String>,
    runtime_functions: HashSet<String>,
    source_map: SourceMapBuilder,
}

impl JSCodeGenerator {
    pub fn generate(&mut self, lir: &LIRModule) -> Result<JSOutput, CodeGenError> {
        // 生成函数声明
        for function in &lir.functions {
            self.generate_function(function)?;
        }
        
        // 注入运行时库
        self.inject_runtime();
        
        // 生成模块导出
        self.generate_exports(&lir.exports);
        
        Ok(JSOutput {
            code: self.output.clone(),
            source_map: self.source_map.build(),
            runtime_deps: self.runtime_functions.clone(),
        })
    }
    
    fn generate_instruction(&mut self, instr: &LIRInstruction) -> Result<(), CodeGenError> {
        match instr {
            LIRInstruction::Load { dest, src } => {
                let dest_var = self.get_register_name(*dest);
                let src_val = self.format_operand(src);
                writeln!(self.output, "  {} = {};", dest_var, src_val)?;
            },
            LIRInstruction::Store { addr, value } => {
                let addr_expr = self.format_operand(addr);
                let value_expr = self.format_operand(value);
                writeln!(self.output, "  nyar_store({}, {});", addr_expr, value_expr)?;
                self.runtime_functions.insert("nyar_store".to_string());
            },
            LIRInstruction::Call { dest, func, args } => {
                self.generate_call(*dest, func, args)?;
            },
            LIRInstruction::Jump { target } => {
                let label = self.get_label_name(*target);
                writeln!(self.output, "  goto {};", label)?;
            },
            // ... 其他指令
        }
        Ok(())
    }
}
```

### 运行时库 (Runtime Library)

**职责**: 提供 Nyar 平台特性在 JavaScript 中的实现。

**核心功能**:
- **内存管理**: 堆分配、垃圾回收辅助
- **类型系统**: 动态类型检查和转换
- **异常处理**: 结构化异常处理
- **代数效应**: 效应处理器的 JavaScript 实现

```javascript
// Nyar JavaScript 运行时库
class NyarRuntime {
    constructor() {
        this.heap = new ArrayBuffer(1024 * 1024); // 1MB 初始堆
        this.heapView = new DataView(this.heap);
        this.freeList = new Set();
        this.gcThreshold = 0.8;
        this.effectHandlers = new Map();
    }
    
    // 内存分配
    allocate(size, alignment = 8) {
        const alignedSize = Math.ceil(size / alignment) * alignment;
        
        // 尝试从空闲列表分配
        for (const block of this.freeList) {
            if (block.size >= alignedSize) {
                this.freeList.delete(block);
                if (block.size > alignedSize + alignment) {
                    // 分割块
                    const remainder = {
                        offset: block.offset + alignedSize,
                        size: block.size - alignedSize
                    };
                    this.freeList.add(remainder);
                }
                return block.offset;
            }
        }
        
        // 触发垃圾回收
        if (this.shouldGC()) {
            this.collectGarbage();
        }
        
        // 扩展堆
        return this.expandHeap(alignedSize);
    }
    
    // 类型检查
    checkType(value, expectedType) {
        const actualType = this.getType(value);
        if (!this.isCompatible(actualType, expectedType)) {
            throw new TypeError(
                `Expected ${expectedType}, got ${actualType}`
            );
        }
        return value;
    }
    
    // 代数效应处理
    perform(effect, operation, args) {
        const handler = this.effectHandlers.get(effect);
        if (!handler) {
            throw new Error(`No handler for effect: ${effect}`);
        }
        
        const operationHandler = handler[operation];
        if (!operationHandler) {
            throw new Error(
                `No handler for operation ${operation} in effect ${effect}`
            );
        }
        
        return operationHandler.apply(this, args);
    }
    
    // 异常处理
    try(block, handlers) {
        try {
            return block();
        } catch (error) {
            for (const [errorType, handler] of handlers) {
                if (error instanceof errorType) {
                    return handler(error);
                }
            }
            throw error; // 重新抛出未处理的异常
        }
    }
}

// 全局运行时实例
const nyar = new NyarRuntime();

// 运行时辅助函数
function nyar_store(addr, value) {
    const offset = typeof addr === 'number' ? addr : addr.offset;
    const size = nyar.getTypeSize(nyar.getType(value));
    
    switch (size) {
        case 1: nyar.heapView.setUint8(offset, value); break;
        case 2: nyar.heapView.setUint16(offset, value, true); break;
        case 4: nyar.heapView.setUint32(offset, value, true); break;
        case 8: nyar.heapView.setBigUint64(offset, BigInt(value), true); break;
        default:
            // 复杂类型的存储
            nyar.storeComplex(offset, value);
    }
}

function nyar_load(addr, type) {
    const offset = typeof addr === 'number' ? addr : addr.offset;
    const size = nyar.getTypeSize(type);
    
    switch (size) {
        case 1: return nyar.heapView.getUint8(offset);
        case 2: return nyar.heapView.getUint16(offset, true);
        case 4: return nyar.heapView.getUint32(offset, true);
        case 8: return Number(nyar.heapView.getBigUint64(offset, true));
        default:
            return nyar.loadComplex(offset, type);
    }
}

function nyar_call(func, args) {
    // 函数调用的统一入口
    if (typeof func === 'function') {
        return func.apply(null, args);
    } else if (func.type === 'closure') {
        return func.call(args);
    } else {
        throw new Error(`Invalid function type: ${typeof func}`);
    }
}
```

## LIR 指令到 JavaScript 的映射

### 基本指令映射

| LIR 指令 | JavaScript 等价 | 说明 |
|----------|----------------|------|
| `load %r1, %r2` | `r1 = r2;` | 寄存器赋值 |
| `store %addr, %val` | `nyar_store(addr, val);` | 内存存储 |
| `add %r1, %r2, %r3` | `r1 = r2 + r3;` | 整数加法 |
| `call %ret, @func, [%args]` | `ret = func(args);` | 函数调用 |
| `jump @label` | `goto label;` | 无条件跳转 |
| `branch %cond, @true, @false` | `if (cond) goto true; else goto false;` | 条件跳转 |

### 复杂特性映射

**闭包 (Closures)**:
```javascript
// LIR: closure %func, @code, [%env]
// JavaScript:
const func = {
    type: 'closure',
    code: function(args) { /* 生成的代码 */ },
    environment: [env_vars],
    call: function(args) {
        return this.code.apply(this.environment, args);
    }
};
```

**模式匹配 (Pattern Matching)**:
```javascript
// LIR: match %value, [patterns]
// JavaScript:
function match_value(value) {
    switch (nyar.getTag(value)) {
        case 0: // Some(x)
            const x = nyar.getField(value, 0);
            return process_some(x);
        case 1: // None
            return process_none();
        default:
            throw new Error('Non-exhaustive pattern match');
    }
}
```

**代数效应 (Algebraic Effects)**:
```javascript
// LIR: perform %result, @effect, @operation, [%args]
// JavaScript:
const result = nyar.perform('Http', 'get', [url]);
```

## 优化策略

### 编译时优化

**内联优化**:
```javascript
// 优化前
function add(a, b) { return a + b; }
const result = add(x, y);

// 优化后
const result = x + y;
```

**常量折叠**:
```javascript
// 优化前
const result = 2 + 3 * 4;

// 优化后
const result = 14;
```

**死代码消除**:
```javascript
// 优化前
function unused() { return 42; }
const x = 10;
if (false) {
    console.log('never executed');
}

// 优化后
const x = 10;
```

### 运行时优化

**类型特化**:
```javascript
// 通用版本
function add_generic(a, b) {
    nyar.checkType(a, 'number');
    nyar.checkType(b, 'number');
    return a + b;
}

// 特化版本（当类型已知时）
function add_int(a, b) {
    return (a | 0) + (b | 0); // 快速整数加法
}
```

**内联缓存**:
```javascript
class InlineCache {
    constructor() {
        this.cache = new Map();
    }
    
    call(func, args) {
        const key = this.getCacheKey(func, args);
        const cached = this.cache.get(key);
        
        if (cached) {
            return cached.result;
        }
        
        const result = func.apply(null, args);
        this.cache.set(key, { result, timestamp: Date.now() });
        return result;
    }
}
```

## 与 JavaScript 生态的集成

### 模块系统集成

**ES6 模块**:
```javascript
// 生成的 JavaScript 模块
export function fibonacci(n) {
    // 从 Nyar LIR 生成的代码
    if (n <= 1) return n;
    return fibonacci(n - 1) + fibonacci(n - 2);
}

export const PI = 3.14159265359;

export default {
    fibonacci,
    PI
};
```

**CommonJS 兼容**:
```javascript
// CommonJS 格式输出
function fibonacci(n) {
    // 生成的代码
}

module.exports = {
    fibonacci,
    PI: 3.14159265359
};
```

### 异步编程支持

**Promise 集成**:
```javascript
// LIR 异步函数转换为 JavaScript Promise
async function fetch_user(id) {
    try {
        const response = await nyar.perform('Http', 'get', [`/api/users/${id}`]);
        return JSON.parse(response);
    } catch (error) {
        throw new Error(`Failed to fetch user ${id}: ${error.message}`);
    }
}
```

**Generator 支持**:
```javascript
// 协程到 Generator 的转换
function* coroutine_example() {
    const a = yield fetch_data(1);
    const b = yield fetch_data(2);
    return a + b;
}
```

### 类型系统映射

**基本类型映射**:
- `Int32` → `number` (32位整数)
- `Float64` → `number` (双精度浮点)
- `String` → `string`
- `Bool` → `boolean`
- `Unit` → `undefined`

**复合类型映射**:
```javascript
// 记录类型 (Records)
const person = {
    name: "Alice",
    age: 30,
    email: "alice@example.com"
};

// 联合类型 (Unions)
const result = {
    tag: 0, // Ok variant
    value: 42
};

// 数组类型
const numbers = new Int32Array([1, 2, 3, 4, 5]);
```

## 调试和源码映射

### Source Map 生成

```rust
pub struct SourceMapBuilder {
    mappings: Vec<Mapping>,
    sources: Vec<String>,
    names: Vec<String>,
}

impl SourceMapBuilder {
    pub fn add_mapping(&mut self, 
        generated_line: u32,
        generated_column: u32,
        source_line: u32,
        source_column: u32,
        source_file: &str,
        name: Option<&str>
    ) {
        self.mappings.push(Mapping {
            generated_line,
            generated_column,
            source_line,
            source_column,
            source_file: source_file.to_string(),
            name: name.map(|s| s.to_string()),
        });
    }
    
    pub fn build(&self) -> SourceMap {
        SourceMap {
            version: 3,
            sources: self.sources.clone(),
            names: self.names.clone(),
            mappings: self.encode_mappings(),
        }
    }
}
```

### 调试信息保留

```javascript
// 生成的 JavaScript 代码包含调试信息
function fibonacci(n) {
    // @source: fibonacci.vk:5:1
    if (n <= 1) {
        // @source: fibonacci.vk:6:5
        return n;
    }
    // @source: fibonacci.vk:8:5
    return fibonacci(n - 1) + fibonacci(n - 2);
}

// 错误处理包含源码位置
function throwError(message, sourceLocation) {
    const error = new Error(message);
    error.sourceLocation = sourceLocation;
    error.stack = `${message}\n    at ${sourceLocation.file}:${sourceLocation.line}:${sourceLocation.column}`;
    throw error;
}
```

## 性能优化和基准测试

### 性能监控

```javascript
class PerformanceMonitor {
    constructor() {
        this.metrics = new Map();
        this.startTimes = new Map();
    }
    
    startTimer(name) {
        this.startTimes.set(name, performance.now());
    }
    
    endTimer(name) {
        const startTime = this.startTimes.get(name);
        if (startTime) {
            const duration = performance.now() - startTime;
            this.recordMetric(name, duration);
            this.startTimes.delete(name);
        }
    }
    
    recordMetric(name, value) {
        if (!this.metrics.has(name)) {
            this.metrics.set(name, []);
        }
        this.metrics.get(name).push(value);
    }
    
    getStats(name) {
        const values = this.metrics.get(name) || [];
        if (values.length === 0) return null;
        
        const sum = values.reduce((a, b) => a + b, 0);
        const avg = sum / values.length;
        const min = Math.min(...values);
        const max = Math.max(...values);
        
        return { avg, min, max, count: values.length };
    }
}

// 全局性能监控器
const perfMonitor = new PerformanceMonitor();

// 在生成的代码中插入性能监控
function instrumented_function() {
    perfMonitor.startTimer('function_execution');
    try {
        // 实际函数代码
        return actual_computation();
    } finally {
        perfMonitor.endTimer('function_execution');
    }
}
```

### 基准测试框架

```javascript
class BenchmarkSuite {
    constructor() {
        this.benchmarks = new Map();
    }
    
    add(name, fn, setup = null, teardown = null) {
        this.benchmarks.set(name, { fn, setup, teardown });
    }
    
    async run(iterations = 1000) {
        const results = new Map();
        
        for (const [name, benchmark] of this.benchmarks) {
            console.log(`Running benchmark: ${name}`);
            
            const times = [];
            
            for (let i = 0; i < iterations; i++) {
                if (benchmark.setup) {
                    await benchmark.setup();
                }
                
                const start = performance.now();
                await benchmark.fn();
                const end = performance.now();
                
                times.push(end - start);
                
                if (benchmark.teardown) {
                    await benchmark.teardown();
                }
            }
            
            const avg = times.reduce((a, b) => a + b) / times.length;
            const min = Math.min(...times);
            const max = Math.max(...times);
            
            results.set(name, { avg, min, max, times });
        }
        
        return results;
    }
}
```

## 部署和分发

### 构建配置

```json
{
  "name": "nyar-js-backend",
  "version": "0.1.0",
  "description": "JavaScript backend for Nyar VM",
  "main": "dist/index.js",
  "module": "dist/index.esm.js",
  "types": "dist/index.d.ts",
  "files": [
    "dist/",
    "runtime/",
    "README.md"
  ],
  "scripts": {
    "build": "rollup -c",
    "test": "jest",
    "benchmark": "node benchmarks/run.js",
    "optimize": "terser dist/index.js -o dist/index.min.js"
  },
  "dependencies": {
    "source-map": "^0.7.4"
  },
  "devDependencies": {
    "rollup": "^3.0.0",
    "terser": "^5.0.0",
    "jest": "^29.0.0"
  }
}
```

### 运行时优化

```javascript
// 生产环境优化
if (process.env.NODE_ENV === 'production') {
    // 禁用调试信息
    nyar.debugMode = false;
    
    // 启用激进优化
    nyar.enableOptimizations([
        'inline-small-functions',
        'eliminate-dead-code',
        'constant-folding',
        'loop-unrolling'
    ]);
    
    // 预编译常用函数
    nyar.precompile([
        'math.add', 'math.multiply',
        'string.concat', 'array.map'
    ]);
}
```

## 未来发展方向

### WebAssembly 集成

**混合编译**:
- 性能关键代码编译为 WebAssembly
- 其他代码保持 JavaScript 以便调试
- 自动选择最优编译目标

### 现代 JavaScript 特性

**计划支持**:
- **BigInt**: 任意精度整数运算
- **WeakRef**: 更好的内存管理
- **FinalizationRegistry**: 资源清理
- **Top-level await**: 模块级异步支持

### 性能提升

**优化方向**:
- **JIT 编译**: 运行时代码优化
- **类型反馈**: 基于运行时类型信息的优化
- **内联缓存**: 多态调用优化
- **逃逸分析**: 栈分配优化

## 总结

JavaScript 后端通过与 Nyar 虚拟机平台的深度集成，为语言实现者提供了以下关键优势：

1. **高效编译**: 从 LIR 到优化 JavaScript 的直接转换
2. **生态集成**: 与 JavaScript 生态系统的无缝互操作
3. **现代特性**: 支持代数效应、模式匹配等高级语言特性
4. **性能优化**: 多层次的编译时和运行时优化
5. **调试支持**: 完整的源码映射和调试信息
6. **跨平台**: 在浏览器、Node.js、Deno 等环境中运行

这种设计让语言设计者能够专注于语言特性的创新，而将复杂的 JavaScript 编译和优化工作交给经过验证的 Nyar 平台处理，从而快速获得高质量的 JavaScript 支持。
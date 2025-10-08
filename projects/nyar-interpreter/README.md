# Nyar Interpreter - 字节码解释器

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![项目状态](https://img.shields.io/badge/状态-活跃开发中-orange.svg)](https://github.com/nyar-lang/nyar-vm/tree/main/projects/nyar-interpreter)

## 项目概述

Nyar Interpreter 是Nyar虚拟机框架的字节码解释器，提供高效的字节码执行引擎和运行时环境支持。

## 项目状态

- **开发阶段**: 🔧 活跃开发中
- **版本**: 0.1.0
- **稳定性**: 实验性
- **测试覆盖率**: 进行中

## 核心特性

### 字节码执行
- 高效的字节码解释器
- 寄存器式虚拟机设计
- 即时编译支持

### 运行时环境
- 内存管理支持
- 垃圾回收集成
- 异常处理机制

### 调试支持
- 字节码调试器
- 执行跟踪功能
- 性能分析工具

## 技术栈

- **语言**: TypeScript
- **构建工具**: Vite
- **测试框架**: Vitest
- **代码质量**: ESLint

## 快速开始

### 安装

```bash
cd projects/nyar-interpreter
npm install
```

### 构建

```bash
npm run build
```

### 测试

```bash
npm test
```

### 使用示例

```typescript
import { BytecodeInterpreter, BytecodeLoader } from '@nyar/interpreter';

// 创建解释器实例
const interpreter = new BytecodeInterpreter();

// 加载字节码
const loader = new BytecodeLoader();
const bytecode = loader.loadFromFile('program.nybc');

// 配置运行时选项
const runtimeOptions = {
  memoryLimit: 1024 * 1024, // 1MB内存限制
  stackSize: 64 * 1024,     // 64KB栈大小
  enableGC: true,          // 启用垃圾回收
  debugMode: false          // 调试模式
};

// 执行字节码
const result = interpreter.execute(bytecode, runtimeOptions);

// 检查执行结果
if (result.success) {
  console.log('程序执行成功:', result.value);
} else {
  console.error('执行错误:', result.error);
}
```

## 核心API

### 字节码解释器

```typescript
class BytecodeInterpreter {
  // 执行字节码
  execute(bytecode: Bytecode, options?: RuntimeOptions): ExecutionResult;
  
  // 运行时控制
  pause(): void;
  resume(): void;
  stop(): void;
  
  // 状态查询
  getExecutionState(): ExecutionState;
  getMemoryUsage(): MemoryUsage;
  getPerformanceStats(): PerformanceStats;
  
  // 调试支持
  setBreakpoint(address: number): void;
  removeBreakpoint(address: number): void;
  step(): ExecutionResult;
}
```

### 字节码加载器

```typescript
class BytecodeLoader {
  // 从文件加载
  loadFromFile(filepath: string): Bytecode;
  
  // 从缓冲区加载
  loadFromBuffer(buffer: ArrayBuffer): Bytecode;
  
  // 从字符串加载
  loadFromString(str: string): Bytecode;
  
  // 验证字节码
  validate(bytecode: Bytecode): ValidationResult;
  
  // 序列化字节码
  serialize(bytecode: Bytecode): ArrayBuffer;
}
```

### 运行时环境

```typescript
class RuntimeEnvironment {
  // 内存管理
  allocate(size: number): number; // 返回内存地址
  free(address: number): void;
  readMemory<T>(address: number, type: Type): T;
  writeMemory<T>(address: number, value: T): void;
  
  // 垃圾回收
  runGC(): GarbageCollectionResult;
  getGCMetrics(): GCMetrics;
  
  // 异常处理
  throwException(exception: Exception): void;
  catchException(handler: ExceptionHandler): void;
  
  // 系统调用
  registerSyscall(name: string, handler: SyscallHandler): void;
  invokeSyscall(name: string, args: any[]): any;
}
```

## 开发指南

### 项目结构

```
nyar-interpreter/
├── src/
│   ├── interpreter/   # 解释器核心
│   ├── bytecode/     # 字节码系统
│   ├── runtime/      # 运行时环境
│   ├── memory/       # 内存管理
│   ├── gc/          # 垃圾回收
│   ├── debugging/    # 调试工具
│   ├── syscalls/     # 系统调用
│   └── utils/        # 工具函数
├── tests/           # 测试文件
├── package.json
└── README.md
```

### 添加新的字节码指令

```typescript
// 定义新的字节码操作码
enum Opcode {
  // ... 现有操作码
  NEW_INSTRUCTION = 0x42
}

// 在解释器中实现指令处理
class BytecodeInterpreter {
  private executeNewInstruction(operands: number[]): void {
    const [operand1, operand2] = operands;
    
    // 实现指令逻辑
    const result = this.registers[operand1] + this.registers[operand2];
    
    // 设置结果寄存器
    this.registers[0] = result; // 假设寄存器0用于结果
    
    // 更新程序计数器
    this.pc += 3; // 操作码 + 两个操作数
  }
  
  private dispatchInstruction(opcode: Opcode, operands: number[]): void {
    switch (opcode) {
      // ... 现有指令
      case Opcode.NEW_INSTRUCTION:
        this.executeNewInstruction(operands);
        break;
      default:
        throw new Error(`未知操作码: ${opcode}`);
    }
  }
}
```

## 集成示例

### 与虚拟机集成

```typescript
import { BytecodeInterpreter } from '@nyar/interpreter';
import { VirtualMachine } from '@nyar/vm';

class NyarVM {
  private interpreter: BytecodeInterpreter;
  private vm: VirtualMachine;
  
  constructor() {
    this.interpreter = new BytecodeInterpreter();
    this.vm = new VirtualMachine();
  }
  
  executeProgram(bytecode: Bytecode): ExecutionResult {
    // 配置虚拟机环境
    this.vm.initialize({
      memorySize: 1024 * 1024,
      stackSize: 64 * 1024,
      enableJIT: true
    });
    
    // 执行字节码
    return this.interpreter.execute(bytecode, {
      memoryLimit: 1024 * 1024,
      stackSize: 64 * 1024,
      enableGC: true,
      vm: this.vm // 传递虚拟机实例
    });
  }
}
```

## 路线图

### 短期目标 (v0.2.0)
- [ ] 完善字节码指令集
- [ ] 实现基本运行时功能
- [ ] 添加内存管理
- [ ] 提高测试覆盖率

### 中期目标 (v0.5.0)
- [ ] 实现垃圾回收
- [ ] 添加JIT编译支持
- [ ] 完善调试工具
- [ ] 性能优化

### 长期目标 (v1.0.0)
- [ ] 生产环境就绪
- [ ] 完整的运行时功能
- [ ] 高级优化特性
- [ ] 企业级支持

## 贡献

欢迎贡献代码、测试用例和文档！请参考：

- [贡献指南](../nyar-document/CONTRIBUTING.md)
- [代码规范](../nyar-document/development/coding-standards.md)

## 许可证

MIT License - 详见 [LICENSE](../../LICENSE.md)

## 相关项目

- [nyar-vm](../nyar-vm/) - 虚拟机运行时
- [nyar-lir](../nyar-lir/) - 低级中间表示
- [nyar-compiler](../nyar-compiler/) - 编译器基础设施
- [nyar-diagnostics](../nyar-diagnostics/) - 诊断工具
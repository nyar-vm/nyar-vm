# Nyar VM - 虚拟机运行时环境

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![项目状态](https://img.shields.io/badge/状态-活跃开发中-orange.svg)](https://github.com/nyar-lang/nyar-vm/tree/main/projects/nyar-vm)

## 项目概述

Nyar VM 是Nyar框架的虚拟机运行时环境，提供完整的字节码执行、内存管理、垃圾回收和系统服务支持。

## 项目状态

- **开发阶段**: 🔧 活跃开发中
- **版本**: 0.1.0
- **稳定性**: 实验性
- **测试覆盖率**: 进行中

## 核心特性

### 虚拟机核心
- 高性能字节码执行引擎
- 寄存器式虚拟机架构
- 即时编译优化

### 内存管理系统
- 自动内存分配和回收
- 分代垃圾回收算法
- 内存使用监控

### 运行时服务
- 异常处理机制
- 线程和并发支持
- 系统调用接口

## 技术栈

- **语言**: TypeScript
- **构建工具**: Vite
- **测试框架**: Vitest
- **代码质量**: ESLint

## 快速开始

### 安装

```bash
cd projects/nyar-vm
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
import { VirtualMachine, VMConfig } from '@nyar/vm';

// 创建虚拟机配置
const config: VMConfig = {
  memorySize: 1024 * 1024,     // 1MB内存
  stackSize: 64 * 1024,        // 64KB栈
  heapSize: 512 * 1024,         // 512KB堆
  enableJIT: true,              // 启用JIT
  gcThreshold: 0.75,            // GC阈值
  debugMode: false              // 调试模式
};

// 创建虚拟机实例
const vm = new VirtualMachine(config);

// 加载字节码程序
const bytecode = loadBytecode('program.nybc');
vm.loadProgram(bytecode);

// 执行程序
const result = vm.execute();

// 检查执行结果
if (result.success) {
  console.log('程序执行成功');
  console.log('返回值:', result.returnValue);
  console.log('执行时间:', result.executionTime, 'ms');
} else {
  console.error('执行失败:', result.error);
}

// 获取运行时统计信息
const stats = vm.getRuntimeStats();
console.log('内存使用:', stats.memoryUsage);
console.log('GC次数:', stats.gcCount);
console.log('指令计数:', stats.instructionCount);
```

## 核心API

### 虚拟机类

```typescript
class VirtualMachine {
  constructor(config: VMConfig);
  
  // 程序管理
  loadProgram(bytecode: Bytecode): void;
  unloadProgram(): void;
  execute(options?: ExecutionOptions): ExecutionResult;
  
  // 运行时控制
  pause(): void;
  resume(): void;
  stop(): void;
  reset(): void;
  
  // 状态查询
  getState(): VMState;
  getRuntimeStats(): RuntimeStats;
  getMemoryInfo(): MemoryInfo;
  
  // 调试支持
  setBreakpoint(address: number): void;
  clearBreakpoint(address: number): void;
  step(): ExecutionResult;
  getCallStack(): CallFrame[];
}
```

### 内存管理器

```typescript
class MemoryManager {
  // 内存分配
  allocate(size: number, type?: AllocationType): number;
  reallocate(ptr: number, newSize: number): number;
  free(ptr: number): void;
  
  // 内存访问
  read<T>(ptr: number, type: Type): T;
  write<T>(ptr: number, value: T): void;
  copy(src: number, dest: number, size: number): void;
  
  // 内存统计
  getUsage(): MemoryUsage;
  getFragmentation(): FragmentationInfo;
  
  // 垃圾回收
  collectGarbage(): GCResult;
  enableGC(): void;
  disableGC(): void;
}
```

### 运行时环境

```typescript
class RuntimeEnvironment {
  // 系统服务
  registerService(name: string, service: RuntimeService): void;
  getService<T>(name: string): T;
  
  // 事件系统
  on(event: string, handler: EventHandler): void;
  emit(event: string, data?: any): void;
  
  // 配置管理
  getConfig(): VMConfig;
  updateConfig(updates: Partial<VMConfig>): void;
  
  // 插件系统
  loadPlugin(plugin: VMPlugin): void;
  unloadPlugin(pluginName: string): void;
}
```

## 开发指南

### 项目结构

```
nyar-vm/
├── src/
│   ├── vm/           # 虚拟机核心
│   ├── memory/       # 内存管理
│   ├── gc/          # 垃圾回收
│   ├── runtime/     # 运行时服务
│   ├── jit/         # JIT编译器
│   ├── threading/   # 线程支持
│   ├── debugging/   # 调试工具
│   ├── plugins/     # 插件系统
│   └── utils/       # 工具函数
├── tests/           # 测试文件
├── package.json
└── README.md
```

### 添加新的运行时服务

```typescript
// 定义新的运行时服务
class FileSystemService implements RuntimeService {
  name = 'filesystem';
  
  constructor(private vm: VirtualMachine) {}
  
  // 实现服务方法
  async readFile(path: string): Promise<Uint8Array> {
    // 实现文件读取逻辑
    const content = await fs.promises.readFile(path);
    return new Uint8Array(content);
  }
  
  async writeFile(path: string, data: Uint8Array): Promise<void> {
    // 实现文件写入逻辑
    await fs.promises.writeFile(path, data);
  }
  
  // 服务生命周期
  async initialize(): Promise<void> {
    console.log('文件系统服务初始化完成');
  }
  
  async shutdown(): Promise<void> {
    console.log('文件系统服务关闭');
  }
}

// 注册服务到虚拟机
const vm = new VirtualMachine(config);
vm.runtime.registerService('filesystem', new FileSystemService(vm));

// 使用服务
const fsService = vm.runtime.getService<FileSystemService>('filesystem');
const fileContent = await fsService.readFile('/path/to/file.txt');
```

## 集成示例

### 与解释器集成

```typescript
import { VirtualMachine } from '@nyar/vm';
import { BytecodeInterpreter } from '@nyar/interpreter';

class NyarRuntime {
  private vm: VirtualMachine;
  private interpreter: BytecodeInterpreter;
  
  constructor(config: VMConfig) {
    this.vm = new VirtualMachine(config);
    this.interpreter = new BytecodeInterpreter();
    
    // 设置解释器使用虚拟机的内存管理
    this.interpreter.setMemoryManager(this.vm.memory);
  }
  
  async executeProgram(bytecode: Bytecode): Promise<ExecutionResult> {
    // 加载程序到虚拟机
    this.vm.loadProgram(bytecode);
    
    // 执行程序
    const result = this.vm.execute({
      timeout: 5000, // 5秒超时
      memoryLimit: 1024 * 1024
    });
    
    return result;
  }
  
  getRuntimeInfo(): RuntimeInfo {
    return {
      vmState: this.vm.getState(),
      memoryInfo: this.vm.getMemoryInfo(),
      stats: this.vm.getRuntimeStats()
    };
  }
}
```

## 路线图

### 短期目标 (v0.2.0)
- [ ] 完善虚拟机核心功能
- [ ] 实现基本内存管理
- [ ] 添加垃圾回收支持
- [ ] 提高测试覆盖率

### 中期目标 (v0.5.0)
- [ ] 实现JIT编译优化
- [ ] 添加线程和并发支持
- [ ] 完善调试工具
- [ ] 性能基准测试

### 长期目标 (v1.0.0)
- [ ] 生产环境就绪
- [ ] 完整的运行时生态系统
- [ ] 高级优化特性
- [ ] 企业级支持

## 贡献

欢迎贡献代码、测试用例和文档！请参考：

- [贡献指南](../nyar-document/CONTRIBUTING.md)
- [代码规范](../nyar-document/development/coding-standards.md)

## 许可证

MIT License - 详见 [LICENSE](../../LICENSE.md)

## 相关项目

- [nyar-interpreter](../nyar-interpreter/) - 字节码解释器
- [nyar-compiler](../nyar-compiler/) - 编译器基础设施
- [nyar-lir](../nyar-lir/) - 低级中间表示
- [nyar-diagnostics](../nyar-diagnostics/) - 诊断工具
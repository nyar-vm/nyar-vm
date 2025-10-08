# Nyar LIR - 低级中间表示

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![项目状态](https://img.shields.io/badge/状态-活跃开发中-orange.svg)](https://github.com/nyar-lang/nyar-vm/tree/main/projects/nyar-lir)

## 项目概述

Nyar LIR (Low-level Intermediate Representation) 是Nyar编译器框架的低级中间表示层，提供接近机器代码的指令集表示和代码生成基础设施。

## 项目状态

- **开发阶段**: 🔧 活跃开发中
- **版本**: 0.1.0
- **稳定性**: 实验性
- **测试覆盖率**: 进行中

## 核心特性

### 指令集架构
- 类汇编指令表示
- 寄存器分配支持
- 控制流指令

### 代码生成
- 多目标代码生成
- PE文件格式支持
- IL后端集成

### 优化通道
- 指令选择优化
- 寄存器分配优化
- 窥孔优化

## 技术栈

- **语言**: TypeScript
- **构建工具**: Vite
- **测试框架**: Vitest
- **代码质量**: ESLint

## 快速开始

### 安装

```bash
cd projects/nyar-lir
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
import { LIRBuilder, PEWriter } from '@nyar/lir';

// 创建LIR构建器
const builder = new LIRBuilder();

// 构建函数
const functionBuilder = builder.createFunction('add');

// 添加参数
const paramA = functionBuilder.addParameter('a', 'i32');
const paramB = functionBuilder.addParameter('b', 'i32');

// 生成指令
functionBuilder.addInstruction('add', paramA, paramB);
functionBuilder.addInstruction('ret', functionBuilder.getLastResult());

// 获取LIR函数
const lirFunction = functionBuilder.build();

// 生成PE文件
const peWriter = new PEWriter();
const peBytes = peWriter.writeFunction(lirFunction);
```

## 核心API

### 指令集

```typescript
// 基础指令类型
type Instruction = {
  opcode: Opcode;
  operands: Operand[];
  result?: Register;
  location?: SourceLocation;
};

// 操作码枚举
enum Opcode {
  // 算术指令
  ADD = 'add',
  SUB = 'sub',
  MUL = 'mul',
  DIV = 'div',
  
  // 逻辑指令
  AND = 'and',
  OR = 'or',
  XOR = 'xor',
  NOT = 'not',
  
  // 控制流指令
  JMP = 'jmp',
  JZ = 'jz',
  JNZ = 'jnz',
  CALL = 'call',
  RET = 'ret',
  
  // 内存指令
  LOAD = 'load',
  STORE = 'store',
  ALLOCA = 'alloca'
}
```

### 寄存器系统

```typescript
class Register {
  id: number;
  type: Type;
  isVirtual: boolean;
  isAllocated: boolean;
  
  allocate(physicalReg: PhysicalRegister): void;
  free(): void;
}

class RegisterAllocator {
  allocateRegisters(function: LIRFunction): AllocationResult;
  getPhysicalRegister(virtualReg: Register): PhysicalRegister | null;
}
```

### 函数表示

```typescript
class LIRFunction {
  name: string;
  parameters: Parameter[];
  basicBlocks: BasicBlock[];
  returnType: Type;
  
  addBasicBlock(name: string): BasicBlock;
  getEntryBlock(): BasicBlock;
  validate(): ValidationResult;
}

class BasicBlock {
  name: string;
  instructions: Instruction[];
  predecessors: BasicBlock[];
  successors: BasicBlock[];
  
  addInstruction(instruction: Instruction): void;
  insertInstruction(index: number, instruction: Instruction): void;
  removeInstruction(instruction: Instruction): boolean;
}
```

## 开发指南

### 项目结构

```
nyar-lir/
├── src/
│   ├── instructions/  # 指令系统
│   ├── registers/    # 寄存器管理
│   ├── functions/    # 函数表示
│   ├── pe-writer/   # PE文件生成
│   ├── il-backend/  # IL后端
│   ├── optimizations/ # 优化通道
│   └── utils/       # 工具函数
├── tests/           # 测试文件
├── examples/        # 示例代码
├── package.json
└── README.md
```

### 添加新的指令

```typescript
// 定义新的指令操作码
enum Opcode {
  // ... 现有操作码
  NEW_INSTRUCTION = 'new_instruction'
}

// 在指令构建器中添加方法
class LIRBuilder {
  createNewInstruction(operand1: Operand, operand2: Operand): Instruction {
    return {
      opcode: Opcode.NEW_INSTRUCTION,
      operands: [operand1, operand2],
      result: this.allocateRegister()
    };
  }
}

// 在代码生成器中添加支持
class CodeGenerator {
  generateNewInstruction(instruction: Instruction): string[] {
    const [op1, op2] = instruction.operands;
    const result = instruction.result!;
    
    return [
      `; ${instruction.opcode}`,
      `mov ${result}, ${op1}`,
      `; 新指令的具体实现`
    ];
  }
}
```

## 集成示例

### 与MIR集成

```typescript
import { MIRToLIRConverter } from '@nyar/lir';
import { MIRFunction } from '@nyar/mir';

class CompilerBackend {
  private converter: MIRToLIRConverter;
  private peWriter: PEWriter;
  
  constructor() {
    this.converter = new MIRToLIRConverter();
    this.peWriter = new PEWriter();
  }
  
  compileToNative(mirFunction: MIRFunction): Uint8Array {
    // 转换为LIR
    const lirFunction = this.converter.convert(mirFunction);
    
    // 寄存器分配
    const allocator = new RegisterAllocator();
    allocator.allocateRegisters(lirFunction);
    
    // 生成PE文件
    return this.peWriter.writeFunction(lirFunction);
  }
}
```

## 路线图

### 短期目标 (v0.2.0)
- [ ] 完善指令集
- [ ] 实现寄存器分配
- [ ] 添加基本优化
- [ ] 提高测试覆盖率

### 中期目标 (v0.5.0)
- [ ] 支持多目标代码生成
- [ ] 实现高级优化通道
- [ ] 添加调试信息支持
- [ ] 性能基准测试

### 长期目标 (v1.0.0)
- [ ] 生产环境就绪
- [ ] 完整的优化套件
- [ ] 企业级功能
- [ ] 社区生态建设

## 贡献

欢迎贡献代码、测试用例和文档！请参考：

- [贡献指南](../nyar-document/CONTRIBUTING.md)
- [代码规范](../nyar-document/development/coding-standards.md)

## 许可证

MIT License - 详见 [LICENSE](../../LICENSE.md)

## 相关项目

- [nyar-mir](../nyar-mir/) - 中级中间表示
- [nyar-hir](../nyar-hir/) - 高级中间表示
- [nyar-interpreter](../nyar-interpreter/) - 字节码解释器
- [nyar-vm](../nyar-vm/) - 虚拟机运行时
# Nyar Diagnostics - 诊断工具集

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![项目状态](https://img.shields.io/badge/状态-稳定可用-brightgreen.svg)](https://github.com/nyar-lang/nyar-vm/tree/main/projects/nyar-diagnostics)

## 项目概述

Nyar Diagnostics 提供源代码位置追踪、错误报告和诊断工具，为Nyar编译器框架提供完整的诊断基础设施。

## 项目状态

- **开发阶段**: ✅ 稳定可用
- **版本**: 0.1.0
- **稳定性**: 生产就绪
- **测试覆盖率**: 高

## 核心特性

### 源代码位置追踪
- 精确的源代码位置信息
- 多文件位置追踪
- 位置信息序列化

### 错误报告系统
- 结构化的错误信息
- 多级别错误分类
- 友好的错误消息格式

### 诊断工具
- 编译时诊断
- 运行时诊断
- 性能分析工具

## 技术栈

- **语言**: TypeScript
- **构建工具**: Vite
- **测试框架**: Vitest
- **代码质量**: ESLint

## 快速开始

### 安装

```bash
cd projects/nyar-diagnostics
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
import { Diagnostic, SourceLocation, ErrorReporter } from '@nyar/diagnostics';

// 创建源代码位置
const location = new SourceLocation({
  file: 'example.ny',
  line: 10,
  column: 5,
  length: 15
});

// 创建诊断信息
const diagnostic = new Diagnostic({
  level: 'error',
  message: '变量未定义',
  code: 'E001',
  location: location
});

// 报告错误
const reporter = new ErrorReporter();
reporter.report(diagnostic);

// 获取所有诊断信息
const diagnostics = reporter.getDiagnostics();
```

## 核心API

### SourceLocation 类

```typescript
class SourceLocation {
  constructor(options: LocationOptions);
  
  // 位置信息
  file: string;
  line: number;
  column: number;
  length: number;
  
  // 方法
  toString(): string;
  toJSON(): object;
  contains(other: SourceLocation): boolean;
}
```

### Diagnostic 类

```typescript
class Diagnostic {
  constructor(options: DiagnosticOptions);
  
  // 诊断信息
  level: 'error' | 'warning' | 'info';
  message: string;
  code: string;
  location?: SourceLocation;
  
  // 方法
  format(): string;
  isError(): boolean;
}
```

### ErrorReporter 类

```typescript
class ErrorReporter {
  // 报告诊断信息
  report(diagnostic: Diagnostic): void;
  
  // 获取诊断信息
  getDiagnostics(): Diagnostic[];
  getErrors(): Diagnostic[];
  getWarnings(): Diagnostic[];
  
  // 清空诊断信息
  clear(): void;
  
  // 检查状态
  hasErrors(): boolean;
  hasWarnings(): boolean;
}
```

## 开发指南

### 项目结构

```
nyar-diagnostics/
├── src/
│   ├── diagnostics/  # 诊断核心
│   ├── locations/    # 位置追踪
│   ├── reporting/    # 错误报告
│   ├── utils/        # 工具函数
│   └── types/        # 类型定义
├── tests/           # 测试文件
├── package.json
└── README.md
```

### 添加新的诊断代码

```typescript
// 定义诊断代码常量
export const DiagnosticCodes = {
  // 语法错误
  SYNTAX_ERROR: 'E001',
  
  // 类型错误
  TYPE_MISMATCH: 'E002',
  UNDEFINED_VARIABLE: 'E003',
  
  // 语义错误
  DUPLICATE_DEFINITION: 'E004',
  
  // 警告
  UNUSED_VARIABLE: 'W001',
  DEPRECATED_FEATURE: 'W002'
} as const;

// 创建诊断信息工厂
function createSyntaxError(location: SourceLocation, message: string): Diagnostic {
  return new Diagnostic({
    level: 'error',
    code: DiagnosticCodes.SYNTAX_ERROR,
    message,
    location
  });
}
```

## 集成指南

### 与编译器集成

```typescript
import { ErrorReporter } from '@nyar/diagnostics';
import { Parser } from '@nyar/compiler';

class Compiler {
  private reporter: ErrorReporter;
  
  constructor() {
    this.reporter = new ErrorReporter();
  }
  
  compile(source: string): CompilationResult {
    try {
      const ast = this.parse(source);
      // ... 编译过程
      
      return {
        success: !this.reporter.hasErrors(),
        diagnostics: this.reporter.getDiagnostics(),
        // ... 其他结果
      };
    } catch (error) {
      this.reporter.report(createInternalError(error));
      return {
        success: false,
        diagnostics: this.reporter.getDiagnostics()
      };
    }
  }
}
```

## 路线图

### 已完成
- [x] 基础诊断系统
- [x] 源代码位置追踪
- [x] 错误报告框架
- [x] 测试覆盖

### 计划功能
- [ ] 多语言错误消息
- [ ] 诊断信息持久化
- [ ] 实时诊断支持
- [ ] 性能分析集成

## 贡献

欢迎贡献代码、测试用例和文档！请参考：

- [贡献指南](../nyar-document/CONTRIBUTING.md)
- [代码规范](../nyar-document/development/coding-standards.md)

## 许可证

MIT License - 详见 [LICENSE](../../LICENSE.md)

## 相关项目

- [nyar-compiler](../nyar-compiler/) - 编译器基础设施
- [nyar-document](../nyar-document/) - 文档系统
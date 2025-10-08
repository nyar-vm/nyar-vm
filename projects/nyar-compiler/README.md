# Nyar Compiler - E-graph编译器基础设施

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![项目状态](https://img.shields.io/badge/状态-活跃开发中-orange.svg)](https://github.com/nyar-lang/nyar-vm/tree/main/projects/nyar-compiler)

## 项目概述

Nyar Compiler 是Nyar框架的核心编译器基础设施，基于E-graph技术构建，提供现代化的编译器优化管道和中间表示系统。

## 项目状态

- **开发阶段**: 🔧 活跃开发中
- **版本**: 0.1.0
- **稳定性**: 实验性
- **测试覆盖率**: 进行中

## 核心特性

### E-graph优化
- 基于E-graph的优化框架
- 等式推理和重写规则
- 多目标优化策略

### 中间表示系统
- 多级中间表示支持
- 类型安全的IR转换
- 可扩展的优化通道

### 编译器基础设施
- 模块化编译器架构
- 插件化优化通道
- 多语言前端支持

## 技术栈

- **语言**: TypeScript
- **构建工具**: Vite
- **测试框架**: Vitest
- **代码质量**: ESLint

## 快速开始

### 安装

```bash
cd projects/nyar-compiler
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

### 开发模式

```bash
npm run dev
```

## 架构设计

### 核心模块

```typescript
// 编译器管道
CompilerPipeline
├── Frontend (前端解析)
├── HIRGenerator (HIR生成)
├── MIRTransformer (MIR转换)
├── LIROptimizer (LIR优化)
└── CodeGenerator (代码生成)

// E-graph系统
EGraph
├── EClass (等价类)
├── ENode (表达式节点)
├── RewriteRules (重写规则)
└── Saturation (饱和化)
```

### 使用示例

```typescript
import { CompilerPipeline } from '@nyar/compiler';

// 创建编译器实例
const compiler = new CompilerPipeline();

// 编译源代码
const result = compiler.compile(sourceCode, {
  target: 'bytecode',
  optimizationLevel: 'high'
});

// 获取优化后的字节码
const bytecode = result.bytecode;
```

## 开发指南

### 项目结构

```
nyar-compiler/
├── src/
│   ├── compiler/     # 编译器核心
│   ├── egraph/       # E-graph系统
│   ├── ir/          # 中间表示
│   ├── optimizations/ # 优化通道
│   └── utils/       # 工具函数
├── tests/           # 测试文件
├── package.json
└── README.md
```

### 添加新的优化规则

```typescript
// 定义重写规则
const rewriteRule: RewriteRule = {
  name: 'constant-folding',
  pattern: (node) => {
    // 匹配常量折叠模式
    return node.type === 'binary' && 
           node.left.type === 'literal' && 
           node.right.type === 'literal';
  },
  replacement: (node) => {
    // 计算常量表达式
    const result = evalBinaryOperation(node.left.value, node.operator, node.right.value);
    return createLiteralNode(result);
  }
};

// 注册到优化器
optimizer.addRewriteRule(rewriteRule);
```

## 路线图

### 短期目标 (v0.2.0)
- [ ] 完善E-graph优化系统
- [ ] 添加更多优化通道
- [ ] 提高测试覆盖率
- [ ] 性能基准测试

### 中期目标 (v0.5.0)
- [ ] 支持多语言前端
- [ ] 实现JIT编译
- [ ] 添加调试支持
- [ ] 文档完善

### 长期目标 (v1.0.0)
- [ ] 生产环境就绪
- [ ] 完整的优化套件
- [ ] 企业级功能
- [ ] 社区生态建设

## 贡献

欢迎贡献代码、文档和测试用例！请参考：

- [贡献指南](../nyar-document/CONTRIBUTING.md)
- [代码规范](../nyar-document/development/coding-standards.md)

## 许可证

MIT License - 详见 [LICENSE](../../LICENSE.md)

## 相关项目

- [nyar-hir](../nyar-hir/) - 高级中间表示
- [nyar-mir](../nyar-mir/) - 中级中间表示  
- [nyar-lir](../nyar-lir/) - 低级中间表示
- [nyar-interpreter](../nyar-interpreter/) - 字节码解释器
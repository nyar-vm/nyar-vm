# Nyar MIR - 中级中间表示

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![项目状态](https://img.shields.io/badge/状态-活跃开发中-orange.svg)](https://github.com/nyar-lang/nyar-vm/tree/main/projects/nyar-mir)

## 项目概述

Nyar MIR (Mid-level Intermediate Representation) 是Nyar编译器框架的中级中间表示层，基于E-graph技术构建，提供强大的优化能力和语义保持的中间表示。

## 项目状态

- **开发阶段**: 🔧 活跃开发中
- **版本**: 0.1.0
- **稳定性**: 实验性
- **测试覆盖率**: 进行中

## 核心特性

### E-graph优化
- 基于E-graph的等式推理
- 重写规则系统
- 饱和化优化算法

### 语义保持
- 类型安全的转换
- 控制流保持
- 副作用分析

### 优化框架
- 模块化优化通道
- 可配置优化策略
- 性能分析工具

## 技术栈

- **语言**: TypeScript
- **构建工具**: Vite
- **测试框架**: Vitest
- **代码质量**: ESLint

## 快速开始

### 安装

```bash
cd projects/nyar-mir
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
import { EGraph, RewriteRule, MIRTransformer } from '@nyar/mir';

// 创建E-graph
const egraph = new EGraph();

// 添加表达式到E-graph
const expr1 = egraph.addExpression('(a + b) * c');
const expr2 = egraph.addExpression('a * c + b * c');

// 定义重写规则
const distributiveRule: RewriteRule = {
  name: 'distributive-law',
  pattern: '(x + y) * z',
  replacement: 'x * z + y * z'
};

// 应用重写规则
egraph.addRewriteRule(distributiveRule);

// 运行饱和化
egraph.saturate();

// 检查等价性
const areEquivalent = egraph.areEquivalent(expr1, expr2);
console.log('表达式等价:', areEquivalent);

// 提取最优表达式
const optimalExpr = egraph.extractOptimal(expr1);
```

## 核心API

### E-graph系统

```typescript
class EGraph {
  // 表达式管理
  addExpression(expr: string | Expression): EClassId;
  getExpression(id: EClassId): Expression;
  
  // 等价类管理
  findEClass(expr: Expression): EClassId;
  mergeEClasses(id1: EClassId, id2: EClassId): void;
  
  // 重写规则
  addRewriteRule(rule: RewriteRule): void;
  applyRewriteRules(): boolean;
  
  // 优化
  saturate(maxIterations?: number): SaturationResult;
  extractOptimal(id: EClassId, costFunction?: CostFunction): Expression;
}
```

### 重写规则

```typescript
type RewriteRule = {
  name: string;
  pattern: string | PatternMatcher;
  replacement: string | ExpressionBuilder;
  conditions?: Condition[];
  priority?: number;
};

class PatternMatcher {
  constructor(pattern: string);
  match(expr: Expression): MatchResult | null;
}

class ExpressionBuilder {
  constructor(template: string);
  build(match: MatchResult): Expression;
}
```

### MIR转换器

```typescript
class MIRTransformer {
  // HIR到MIR转换
  hirToMir(hirProgram: HIRProgram): MIRProgram;
  
  // MIR优化
  optimizeMir(mirProgram: MIRProgram, options?: OptimizationOptions): MIRProgram;
  
  // MIR到LIR转换
  mirToLir(mirProgram: MIRProgram): LIRProgram;
  
  // 分析结果
  getAnalysisResults(): AnalysisResults;
  getOptimizationReport(): OptimizationReport;
}
```

## 开发指南

### 项目结构

```
nyar-mir/
├── src/
│   ├── egraph/        # E-graph核心
│   ├── expressions/   # 表达式系统
│   ├── rewrite/      # 重写规则
│   ├── optimization/  # 优化通道
│   ├── analysis/     # 分析工具
│   ├── transformers/ # 转换器
│   └── utils/        # 工具函数
├── tests/           # 测试文件
├── package.json
└── README.md
```

### 添加新的重写规则

```typescript
// 定义新的重写规则
const constantFoldingRule: RewriteRule = {
  name: 'constant-folding',
  pattern: '(literal x) OP (literal y)',
  replacement: (match) => {
    const x = match.vars.x as number;
    const y = match.vars.y as number;
    const op = match.vars.OP as string;
    
    let result: number;
    switch (op) {
      case '+': result = x + y; break;
      case '-': result = x - y; break;
      case '*': result = x * y; break;
      case '/': result = x / y; break;
      default: throw new Error(`未知操作符: ${op}`);
    }
    
    return createLiteral(result);
  },
  conditions: [
    // 确保操作符是算术操作符
    (match) => ['+', '-', '*', '/'].includes(match.vars.OP as string)
  ],
  priority: 10 // 高优先级
};

// 注册规则到优化器
optimizer.addRule(constantFoldingRule);
```

## 集成示例

### 与编译器优化管道集成

```typescript
import { HIRToMIRTransformer, MIROptimizer, MIRToLIRTransformer } from '@nyar/mir';
import { HIRProgram } from '@nyar/hir';
import { LIRProgram } from '@nyar/lir';

class CompilerOptimizationPipeline {
  private hirToMir: HIRToMIRTransformer;
  private mirOptimizer: MIROptimizer;
  private mirToLir: MIRToLIRTransformer;
  
  constructor() {
    this.hirToMir = new HIRToMIRTransformer();
    this.mirOptimizer = new MIROptimizer();
    this.mirToLir = new MIRToLIRTransformer();
  }
  
  optimize(hirProgram: HIRProgram): LIRProgram {
    // HIR到MIR转换
    const mirProgram = this.hirToMir.transform(hirProgram);
    
    // MIR优化
    const optimizedMir = this.mirOptimizer.optimize(mirProgram, {
      level: 'aggressive',
      rules: ['constant-folding', 'common-subexpression', 'dead-code-elimination']
    });
    
    // MIR到LIR转换
    return this.mirToLir.transform(optimizedMir);
  }
}
```

## 路线图

### 短期目标 (v0.2.0)
- [ ] 完善E-graph系统
- [ ] 添加更多重写规则
- [ ] 实现基本优化通道
- [ ] 提高测试覆盖率

### 中期目标 (v0.5.0)
- [ ] 支持高级优化策略
- [ ] 实现性能分析工具
- [ ] 添加调试支持
- [ ] 文档完善

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

- [nyar-hir](../nyar-hir/) - 高级中间表示
- [nyar-lir](../nyar-lir/) - 低级中间表示
- [nyar-compiler](../nyar-compiler/) - 编译器基础设施
- [nyar-interpreter](../nyar-interpreter/) - 字节码解释器
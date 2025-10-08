# Nyar HIR - 高级中间表示

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![项目状态](https://img.shields.io/badge/状态-活跃开发中-orange.svg)](https://github.com/nyar-lang/nyar-vm/tree/main/projects/nyar-hir)

## 项目概述

Nyar HIR (High-level Intermediate Representation) 是Nyar编译器框架的高级中间表示层，提供类型安全的抽象语法树表示和语义分析基础设施。

## 项目状态

- **开发阶段**: 🔧 活跃开发中
- **版本**: 0.1.0
- **稳定性**: 实验性
- **测试覆盖率**: 进行中

## 核心特性

### 类型系统
- 强大的类型推断
- 多态类型支持
- 类型约束求解

### 抽象语法树
- 丰富的AST节点类型
- 源代码位置信息
- 语义注释支持

### 语义分析
- 作用域分析
- 类型检查
- 控制流分析

## 技术栈

- **语言**: TypeScript
- **构建工具**: Vite
- **测试框架**: Vitest
- **代码质量**: ESLint

## 快速开始

### 安装

```bash
cd projects/nyar-hir
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
import { HIRFactory, TypeChecker } from '@nyar/hir';

// 创建HIR工厂
const factory = new HIRFactory();

// 构建AST节点
const functionDecl = factory.createFunctionDeclaration({
  name: 'add',
  parameters: [
    factory.createParameter('a', factory.createTypeReference('number')),
    factory.createParameter('b', factory.createTypeReference('number'))
  ],
  returnType: factory.createTypeReference('number'),
  body: factory.createBlockStatement([
    factory.createReturnStatement(
      factory.createBinaryExpression(
        factory.createIdentifier('a'),
        '+',
        factory.createIdentifier('b')
      )
    )
  ])
});

// 类型检查
const typeChecker = new TypeChecker();
typeChecker.checkFunction(functionDecl);

// 获取类型信息
const functionType = typeChecker.getType(functionDecl);
```

## 核心API

### AST节点类型

```typescript
// 声明节点
type Declaration = 
  | FunctionDeclaration
  | VariableDeclaration
  | ClassDeclaration
  | InterfaceDeclaration;

// 语句节点
type Statement =
  | ExpressionStatement
  | BlockStatement
  | ReturnStatement
  | IfStatement
  | WhileStatement;

// 表达式节点
type Expression =
  | Identifier
  | Literal
  | BinaryExpression
  | CallExpression
  | MemberExpression;
```

### 类型系统

```typescript
// 基础类型
class Type {
  kind: TypeKind;
  name: string;
  
  isAssignableTo(other: Type): boolean;
  toString(): string;
}

// 类型种类
enum TypeKind {
  Primitive,    // 基本类型
  Function,     // 函数类型
  Object,       // 对象类型
  Union,        // 联合类型
  Intersection, // 交叉类型
  Generic       // 泛型类型
}
```

### 语义分析器

```typescript
class SemanticAnalyzer {
  // 分析程序
  analyze(program: Program): AnalysisResult;
  
  // 获取符号表
  getSymbolTable(): SymbolTable;
  
  // 获取类型信息
  getType(node: ASTNode): Type;
  
  // 获取诊断信息
  getDiagnostics(): Diagnostic[];
}
```

## 开发指南

### 项目结构

```
nyar-hir/
├── src/
│   ├── ast/          # AST节点定义
│   ├── types/        # 类型系统
│   ├── analysis/     # 语义分析
│   ├── visitors/     # AST访问者
│   ├── factory/      # 节点工厂
│   └── utils/        # 工具函数
├── tests/           # 测试文件
├── package.json
└── README.md
```

### 添加新的AST节点

```typescript
// 定义新的AST节点
class ForStatement extends Statement {
  constructor(
    public init: VariableDeclaration | Expression | null,
    public test: Expression | null,
    public update: Expression | null,
    public body: Statement,
    public location?: SourceLocation
  ) {
    super('ForStatement', location);
  }
  
  accept<T>(visitor: Visitor<T>): T {
    return visitor.visitForStatement(this);
  }
}

// 在访问者接口中添加方法
interface Visitor<T> {
  visitForStatement(node: ForStatement): T;
}

// 在工厂中添加创建方法
class HIRFactory {
  createForStatement(
    init: VariableDeclaration | Expression | null,
    test: Expression | null,
    update: Expression | null,
    body: Statement,
    location?: SourceLocation
  ): ForStatement {
    return new ForStatement(init, test, update, body, location);
  }
}
```

## 集成示例

### 与编译器前端集成

```typescript
import { Parser } from '@nyar/compiler';
import { HIRFactory, SemanticAnalyzer } from '@nyar/hir';

class CompilerFrontend {
  private parser: Parser;
  private factory: HIRFactory;
  private analyzer: SemanticAnalyzer;
  
  constructor() {
    this.parser = new Parser();
    this.factory = new HIRFactory();
    this.analyzer = new SemanticAnalyzer();
  }
  
  compile(source: string): HIRProgram {
    // 解析源代码
    const ast = this.parser.parse(source);
    
    // 转换为HIR
    const hirProgram = this.factory.createProgram(ast);
    
    // 语义分析
    const analysisResult = this.analyzer.analyze(hirProgram);
    
    if (analysisResult.hasErrors) {
      throw new CompilationError('语义分析失败', analysisResult.diagnostics);
    }
    
    return hirProgram;
  }
}
```

## 路线图

### 短期目标 (v0.2.0)
- [ ] 完善类型系统
- [ ] 添加更多AST节点
- [ ] 提高测试覆盖率
- [ ] 性能优化

### 中期目标 (v0.5.0)
- [ ] 支持泛型编程
- [ ] 添加模式匹配
- [ ] 实现模块系统
- [ ] 文档完善

### 长期目标 (v1.0.0)
- [ ] 生产环境就绪
- [ ] 完整的语言特性支持
- [ ] 高级优化集成
- [ ] 工具链支持

## 贡献

欢迎贡献代码、测试用例和文档！请参考：

- [贡献指南](../nyar-document/CONTRIBUTING.md)
- [代码规范](../nyar-document/development/coding-standards.md)

## 许可证

MIT License - 详见 [LICENSE](../../LICENSE.md)

## 相关项目

- [nyar-compiler](../nyar-compiler/) - 编译器基础设施
- [nyar-mir](../nyar-mir/) - 中级中间表示
- [nyar-lir](../nyar-lir/) - 低级中间表示
- [nyar-diagnostics](../nyar-diagnostics/) - 诊断工具
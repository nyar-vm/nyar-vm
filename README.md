# Nyar Framework - 现代编程语言框架

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![项目状态](https://img.shields.io/badge/状态-活跃开发中-brightgreen.svg)](https://github.com/nyar-lang/nyar-vm)

## 项目概述

Nyar Framework 是一个现代化的编程语言框架，提供完整的编译器基础设施和虚拟机运行时环境。框架基于E-graph技术构建，支持多级中间表示和高级优化。

## 项目状态

| 项目 | 状态 | 描述 | 版本 |
|------|------|------|------|
| nyar-compiler | 🔧 开发中 | 编译器基础设施 | 0.1.0 |
| nyar-diagnostics | ✅ 稳定 | 诊断工具集 | 0.1.0 |
| nyar-document | 📚 完善 | 文档系统 | 0.1.0 |
| nyar-hir | 🔧 开发中 | 高级中间表示 | 0.1.0 |
| nyar-lir | 🔧 开发中 | 低级中间表示 | 0.1.0 |
| nyar-mir | 🔧 开发中 | 中级中间表示 | 0.1.0 |
| nyar-interpreter | 🔧 开发中 | 字节码解释器 | 0.1.0 |
| nyar-vm | 🔧 开发中 | 虚拟机运行时 | 0.1.0 |

## 架构设计

```
源代码 → 前端解析 → HIR → MIR → LIR → 字节码 → 虚拟机执行
```

### 核心组件

- **nyar-compiler**: E-graph编译器基础设施
- **nyar-diagnostics**: 源代码位置、错误报告和诊断工具
- **nyar-hir**: 高级中间表示（抽象语法树级别）
- **nyar-mir**: 中级中间表示（E-graph优化级别）
- **nyar-lir**: 低级中间表示（字节码生成级别）
- **nyar-interpreter**: 字节码解释器
- **nyar-vm**: 虚拟机运行时环境

## 快速开始

### 安装依赖

```bash
# 克隆项目
git clone https://github.com/nyar-lang/nyar-vm.git
cd nyar-vm

# 安装依赖
npm install
```

### 构建项目

```bash
# 构建所有项目
npm run build

# 运行测试
npm test
```

## 开发指南

### 项目结构

```
nyar-vm/
├── projects/           # 所有子项目
│   ├── nyar-compiler/     # 编译器基础设施
│   ├── nyar-diagnostics/ # 诊断工具
│   ├── nyar-document/    # 文档系统
│   ├── nyar-hir/         # 高级IR
│   ├── nyar-lir/         # 低级IR
│   ├── nyar-mir/         # 中级IR
│   ├── nyar-interpreter/ # 解释器
│   └── nyar-vm/          # 虚拟机
└── README.md
```

### 开发环境设置

1. 确保安装 Node.js 18+ 和 npm
2. 克隆项目仓库
3. 运行 `npm install` 安装依赖
4. 使用 `npm run dev` 启动开发服务器

## 贡献指南

我们欢迎社区贡献！请参考：

- [贡献指南](projects/nyar-document/CONTRIBUTING.md)
- [代码规范](projects/nyar-document/development/coding-standards.md)
- [问题报告](https://github.com/nyar-lang/nyar-vm/issues)

## 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE.md) 文件。

## 联系方式

- 💬 [Discord 社区](https://discord.gg/nyar-vm)
- 🐛 [问题追踪](https://github.com/nyar-lang/nyar-vm/issues)
- 📧 [邮件列表](https://groups.google.com/g/nyar-vm)

---

**准备好探索现代编程语言开发了吗？** [开始使用 Nyar Framework！](projects/nyar-document/guide/getting-started.md)
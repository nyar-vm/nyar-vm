# Nyar Framework - 文档系统

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![项目状态](https://img.shields.io/badge/状态-完善可用-brightgreen.svg)](https://github.com/nyar-lang/nyar-vm/tree/main/projects/nyar-document)

## 项目概述

Nyar Document 是Nyar框架的完整文档系统，为编程语言和编译器框架提供全面的文档支持，包括用户指南、开发文档和维护手册。

## 项目状态

- **开发阶段**: 📚 完善可用
- **版本**: 0.1.0
- **稳定性**: 生产就绪
- **文档完整性**: 高

## 核心特性

### 多层级文档
- 用户指南和教程
- 开发者文档
- 维护手册
- API参考文档

### 现代化文档工具
- 基于VitePress构建
- 响应式设计
- 搜索功能支持
- 多语言支持

### 内容管理系统
- 结构化文档组织
- 版本控制集成
- 自动构建部署
- 社区贡献支持

## 技术栈

- **文档引擎**: VitePress
- **构建工具**: Vite
- **部署平台**: GitHub Pages / Netlify
- **代码质量**: ESLint + Prettier

## 快速开始

### 本地开发

```bash
cd projects/nyar-document
npm install

# 启动开发服务器
npm run dev

# 构建静态网站
npm run build

# 预览构建结果
npm run preview
```

### 内容贡献

```bash
# 创建新的文档页面
npm run new:page -- "新页面标题"

# 检查文档链接
npm run check:links

# 格式化文档
npm run format
```

## 文档结构

### 面向语言用户
- 📚 [语言指南](guide/) - Valkyrie编程语言使用指南
- ❓ [常见问题](faq.md) - 关于Valkyrie的常见问题解答
- 📖 [示例代码](examples/) - 代码示例和教程

### 面向语言实现者
- 🔧 [开发指南](development/) - 如何实现面向Nyar的语言
- 🏗️ [前端实现](development/valkyrie-frontend.md) - 构建语言前端
- 📦 [后端集成](development/javascript-backend.md) - 与Nyar后端集成

### 面向平台维护者
- ⚙️ [维护指南](maintenance/) - Nyar平台内部维护
- 🔬 [虚拟机内部](maintenance/rust-backend.md) - 虚拟机实现深入解析
- 📊 [语言表示](language/) - IR设计和实现

## 架构概述

```
源代码 → 前端解析 → HIR → MIR → LIR → 字节码 → 虚拟机执行
```

### 文档层次结构

```
nyar-document/
├── guide/           # 用户指南
├── development/     # 开发文档
├── maintenance/     # 维护手册
├── language/        # 语言规范
├── api/            # API参考
└── examples/       # 示例代码
```

## 平台优势

### 对于应用开发者
- 🎯 **表达性语言**: 使用Valkyrie的现代特性如代数效应
- 🚀 **高性能**: 受益于Nyar的高级优化
- 🌐 **随处部署**: 单一代码库可在Web、服务器和桌面运行
- 🛠️ **强大工具**: 丰富的IDE支持和调试工具

### 对于语言设计者
- 🏗️ **坚实基础**: 基于成熟的虚拟机技术构建
- ⚡ **性能优势**: 免费获得JIT编译和优化
- 🔧 **多目标支持**: 自动支持多种部署目标
- 📊 **分析工具**: 内置的性能分析和剖析支持

### 对于平台工程师
- 🔬 **研究平台**: 实验新的语言特性
- 📈 **优化能力**: 基于IR的高级优化管道
- 🧪 **可扩展性**: 自定义后端的插件架构
- 📚 **完善文档**: 全面的文档和示例

## 社区

- 💬 [Discord社区](https://discord.gg/nyar-vm)
- 🐛 [问题追踪](https://github.com/nyar-lang/nyar-vm/issues)
- 💡 [讨论区](https://github.com/nyar-lang/nyar-vm/discussions)
- 📧 [邮件列表](https://groups.google.com/g/nyar-vm)

## 贡献

我们欢迎对Nyar平台和Valkyrie语言的贡献！详情请参考：

- [贡献指南](CONTRIBUTING.md)
- [代码规范](development/coding-standards.md)
- [文档编写指南](development/writing-guide.md)

### 开发领域
- 🔧 虚拟机优化和性能改进
- 🌐 新的编译目标和后端
- 📚 文档和教育内容
- 🛠️ 开发者工具和IDE集成
- 🧪 测试、基准测试和质量保证

## 许可证

本项目采用MIT许可证 - 详见 [LICENSE](LICENSE) 文件。

## 致谢

- 灵感来源于LLVM、JVM和其他成功的虚拟机平台
- 使用Rust构建以确保内存安全和性能
- 为下一代编程语言设计

---

**准备好探索高性能语言实现了吗？** [开始使用Nyar！](guide/getting-started.md)
# Nyar 文档总览

Nyar 是多语言前端与多后端编译平台。语言前端拥有自己的解析、名字解析与语义闭合；平台接收前端交付的已闭合合同，完成语言无关的规划、目标降低与产物装配。

本仓同时维护两个虚拟机运行时：

- **OdinVM**（`projects/nyar.rs/runtimes/odin-vm`）：执行 `odin-bytecode` 模块，提供解释执行、托管堆、JIT、代数效应与协程。
- **LegendVM**（`projects/nyar.rs/runtimes/legend-vm`）：面向宿主脚本语言的 PE / Futamura 解释基底，负责 guest 注册、解释分发与原生产物特化。

## 文档地图

| 目录 | 内容 |
|------|------|
| [guide/](./guide/index.md) | Valkyrie 语言入门、语言特性、动态语言与插件机制 |
| [frontends/](./frontends/index.md) | 各语言前端的设计与现状 |
| [backends/](./backends/index.md) | OVM、WASM、Native、JVM、CLR 后端 |
| [maintenance/](./maintenance/index.md) | OdinVM、LegendVM、GC、JIT 与字节码等组件的维护说明 |
| [faq.md](./faq.md) | 常见问题 |

## 组件归属

| 组件 | 路径 | 职责 |
|------|------|------|
| `nyar` | `projects/nyar.rs/compilers/nyar` | 语言无关的规划、分区与后端选择 |
| `nyar-analyzer` | `projects/nyar.rs/compilers/nyar-analyzer` | 前端事实到 `ProgramFacts` |
| `nyar-optimizer` | `projects/nyar.rs/compilers/nyar-optimizer` | 对象代数与 E-Graph 优化 |
| `nyar-emitter` | `projects/nyar.rs/compilers/nyar-emitter` | 各后端的目标降低与产物装配，AOT 产物由此产出 |
| `nyar-types` | `projects/nyar.rs/compilers/nyar-types` | 跨前端/后端的中性身份与合同类型 |
| `odin-bytecode` | `projects/nyar.rs/formats/odin-bytecode` | `.nyar` 磁盘字节码格式 |
| `odin-vm` | `projects/nyar.rs/runtimes/odin-vm` | OVM 字节码加载、校验与解释器 |
| `legend-vm` | `projects/nyar.rs/runtimes/legend-vm` | 宿主脚本语言的解释与特化基底 |
| `nyar-gc` | `projects/nyar.rs/runtimes/nyar-gc` | 托管堆与分代标记-清除回收 |
| `nyar-jit` | `projects/nyar.rs/runtimes/nyar-jit` | 即时编译、栈图与去优化 |
| `rusty-valkyrie` | `projects/nyar.rs/languages/rusty-valkyrie` | Valkyrie 前端 HIR / MIR 与产品模块 |

## 阅读约定

- 本目录的文档直接以 Markdown 阅读，不依赖静态站点生成。
- 标记为历史设计的页面仅用于追溯，当前实现以源码、测试与各 crate 的 `readme.md` 为准。

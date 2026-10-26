# compiler / input

**不可变编译输入**与 Legion 快照之间的类型边界。

## 内容

- 再导出 `nyar_types::compiler_input` 中的 `CompilerSourceFile`、`CompilerSourceGroup`、`CompilerSourceInput`、`GeneratedSourceGroup` 等。
- 输入类型记录修订身份、依赖可见性和配置摘要；`legion.von` 的解析由 `legion::workspace` 完成。

## 数据流

```mermaid
flowchart LR
  legion[legion::workspace 快照]
  input[compiler::input]
  compiler[driver / syntax]

  legion --> input --> compiler
```

Legion 负责工程文件与依赖的发现，Compiler 从 `input` 接收结构化的源码视图。

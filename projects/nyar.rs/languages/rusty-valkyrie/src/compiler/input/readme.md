# compiler / input

**不可变编译输入**与 Legion 快照之间的类型边界。

## 内容

- 再导出 `nyar_types::compiler_input` 中的 `CompilerSourceFile`、`CompilerSourceGroup`、`CompilerSourceInput`、`GeneratedSourceGroup` 等。
- 准入规则（修订身份、依赖可见性、配置哈希）将在此扩展；**不** 解析 `legion.von`（那是 `legion::workspace`）。

## 数据流

```mermaid
flowchart LR
  legion[legion::workspace 快照]
  input[compiler::input]
  compiler[driver / syntax]

  legion --> input --> compiler
```

Legion 负责「工程上提交了哪些文件与依赖」；Compiler 从 `input` 起只处理已准入的输入视图。

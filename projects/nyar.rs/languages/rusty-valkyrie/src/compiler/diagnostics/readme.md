# compiler / diagnostics

**跨编译阶段的诊断关联与展示辅助**：把各层生产者给出的错误种类、span 与 related 信息挂到统一呈现模型。

## 原则

- **错误种类** 由产生该事实的模块定义（HIR、semantic、emitter 等），不在此目录堆积巨型 `enum` 收容全编译器。
- 本目录负责：severity 映射、标签合并、与 `utilities::diagnostic`（boost 投影）的衔接点。

## 与 utilities 的分工

- `compiler`：编译期事实（`ValkyrieError`、`DiagnosticRow` 等）。
- `utilities::report`：CLI/N-API **出口** 一行用户可见文案，不参与库内 `Result` 别名设计。

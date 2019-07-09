# Miette 对话式错误处理

## 概述

Miette 是 Nyar 编译器的统一错误处理和诊断框架。它将传统的冰冷、难以理解的编译器错误信息转变为友好的、上下文相关的对话式体验，帮助开发者快速理解和解决问题。

## 设计理念

### 诊断即对话 (Diagnostics as Dialogue)

编译器的错误和警告信息不应是冰冷的、令人费解的指令。Nyar 将编译器视为开发者的智能助手，其诊断信息是与开发者之间的一场友好的、旨在解决问题的对话。

### IDE 级别的诊断体验

每个诊断都力求包含：
- 精确的源码位置
- 上下文相关的标签
- 解释错误的根本原因
- 自动修复的建议（在可能的情况下）

## 核心组件

### `nyar-error` Crate

统一的错误类型定义，所有编译器组件都使用这些类型：

```rust
use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// 编译器的根错误类型
#[derive(Error, Diagnostic, Debug)]
pub enum CompilerError {
    #[error("解析错误")]
    Parse(#[from] ParseError),
    
    #[error("类型错误")]
    Type(#[from] TypeError),
    
    #[error("代码生成错误")]
    CodeGen(#[from] CodeGenError),
}

/// 解析阶段的错误
#[derive(Error, Diagnostic, Debug)]
pub enum ParseError {
    #[error("意外的标记")]
    #[diagnostic(
        code(nyar::parse::unexpected_token),
        help("期望 '{expected}'，但找到了 '{found}'"),
        url("https://nyar-lang.org/docs/syntax-errors#unexpected-token")
    )]
    UnexpectedToken {
        expected: String,
        found: String,
        #[label("意外的标记")]
        span: SourceSpan,
    },
    
    #[error("未闭合的字符串字面量")]
    #[diagnostic(
        code(nyar::parse::unclosed_string),
        help("字符串字面量必须以引号结尾")
    )]
    UnclosedString {
        #[label("字符串开始于此")]
        start: SourceSpan,
        #[label("期望在此处找到闭合引号")]
        expected_end: SourceSpan,
    },
}

/// 类型检查错误
#[derive(Error, Diagnostic, Debug)]
pub enum TypeError {
    #[error("类型不匹配")]
    #[diagnostic(
        code(nyar::type::mismatch),
        help("尝试将 {found} 类型的值赋给 {expected} 类型的变量")
    )]
    TypeMismatch {
        expected: String,
        found: String,
        #[label("期望 {expected} 类型")]
        expected_span: SourceSpan,
        #[label("但这里是 {found} 类型")]
        found_span: SourceSpan,
    },
    
    #[error("未定义的变量")]
    #[diagnostic(
        code(nyar::type::undefined_variable),
        help("变量 '{name}' 在此作用域中未定义")
    )]
    UndefinedVariable {
        name: String,
        #[label("未定义的变量")]
        span: SourceSpan,
        #[related]
        similar_names: Vec<SimilarName>,
    },
}

/// 相似名称建议
#[derive(Error, Diagnostic, Debug)]
#[error("你是否想要使用 '{name}'?")]
#[diagnostic(code(nyar::suggestion::similar_name))]
pub struct SimilarName {
    pub name: String,
    #[label("定义于此")]
    pub span: SourceSpan,
}
```

### `thiserror` 宏的使用

`thiserror` 提供了声明式的错误定义方式：

```rust
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("配置文件不存在: {path}")]
    FileNotFound { path: String },
    
    #[error("配置格式错误")]
    InvalidFormat(#[from] toml::de::Error),
    
    #[error("IO 错误")]
    Io(#[from] std::io::Error),
}
```

## 错误上下文的携带

### 源码位置追踪

所有错误都携带精确的源码位置信息：

```rust
use miette::{NamedSource, SourceSpan};

/// 错误报告上下文
pub struct ErrorContext {
    /// 源文件内容
    pub source: NamedSource<String>,
    /// 文件路径
    pub file_path: PathBuf,
}

impl ErrorContext {
    pub fn new(file_path: PathBuf, content: String) -> Self {
        Self {
            source: NamedSource::new(file_path.display().to_string(), content),
            file_path,
        }
    }
    
    /// 创建带有源码上下文的错误
    pub fn error_with_span<E>(&self, error: E, span: SourceSpan) -> miette::Report
    where
        E: Into<Box<dyn Diagnostic + Send + Sync>>,
    {
        miette::Report::new(error)
            .with_source_code(self.source.clone())
    }
}
```

### 多级错误链

复杂的错误可以包含多个相关的子错误：

```rust
#[derive(Error, Diagnostic, Debug)]
#[error("函数调用失败")]
#[diagnostic(code(nyar::call::failed))]
pub struct CallError {
    pub function_name: String,
    #[label("函数调用")]
    pub call_span: SourceSpan,
    #[related]
    pub argument_errors: Vec<ArgumentError>,
}

#[derive(Error, Diagnostic, Debug)]
#[error("参数 {index} 类型错误")]
#[diagnostic(code(nyar::call::argument_type))]
pub struct ArgumentError {
    pub index: usize,
    #[label("类型不匹配")]
    pub span: SourceSpan,
}
```

## 错误报告的方式

### 命令行界面

在 CLI 中，错误以彩色、格式化的方式显示：

```rust
use miette::{IntoDiagnostic, Result};

fn main() -> Result<()> {
    let result = compile_file("example.ny")
        .into_diagnostic()
        .wrap_err("编译失败");
    
    match result {
        Ok(output) => {
            println!("编译成功: {}", output.len());
        }
        Err(err) => {
            // miette 会自动格式化错误信息
            eprintln!("{:?}", err);
            std::process::exit(1);
        }
    }
    
    Ok(())
}
```

### 语言服务器集成

在 LSP 中，错误被转换为标准的诊断信息：

```rust
use lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

/// 将 miette 错误转换为 LSP 诊断
pub fn miette_to_lsp_diagnostic(
    error: &miette::Report,
    source: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    
    if let Some(span) = error.source_code() {
        let range = span_to_lsp_range(span, source);
        
        diagnostics.push(Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: error.code().map(|c| c.to_string().into()),
            message: error.to_string(),
            source: Some("nyar".to_string()),
            ..Default::default()
        });
    }
    
    diagnostics
}

fn span_to_lsp_range(span: &SourceSpan, source: &str) -> Range {
    let start_offset = span.offset();
    let end_offset = start_offset + span.len();
    
    Range {
        start: offset_to_position(start_offset, source),
        end: offset_to_position(end_offset, source),
    }
}
```

## 高级特性

### 自动修复建议

某些错误可以提供自动修复建议：

```rust
#[derive(Error, Diagnostic, Debug)]
#[error("缺少分号")]
#[diagnostic(
    code(nyar::syntax::missing_semicolon),
    help("在语句末尾添加分号")
)]
pub struct MissingSemicolon {
    #[label("期望在此处添加分号")]
    pub span: SourceSpan,
    #[suggestion("添加分号", code = ";")]
    pub suggestion_span: SourceSpan,
}
```

### 错误恢复策略

编译器可以从错误中恢复，继续分析后续代码：

```rust
pub struct Parser {
    errors: Vec<ParseError>,
    // ... 其他字段
}

impl Parser {
    /// 解析时遇到错误，记录并尝试恢复
    fn recover_from_error(&mut self, error: ParseError) {
        self.errors.push(error);
        
        // 跳过到下一个可能的恢复点
        self.skip_to_recovery_point();
    }
    
    fn skip_to_recovery_point(&mut self) {
        // 跳过到分号、右括号等恢复点
        while !self.is_at_recovery_point() && !self.is_at_end() {
            self.advance();
        }
    }
}
```

### 国际化支持

错误信息支持多语言：

```rust
use fluent::{FluentBundle, FluentResource};

#[derive(Error, Diagnostic, Debug)]
#[error("{}", self.localized_message())]
pub struct LocalizedError {
    pub code: &'static str,
    pub args: HashMap<String, String>,
    #[label("{}", self.localized_label())]
    pub span: SourceSpan,
}

impl LocalizedError {
    fn localized_message(&self) -> String {
        // 从 fluent 资源中获取本地化消息
        get_localized_message(self.code, &self.args)
    }
    
    fn localized_label(&self) -> String {
        get_localized_label(self.code, &self.args)
    }
}
```

## 性能考虑

### 延迟错误构建

错误信息的构建可能很昂贵，使用延迟构建来优化性能：

```rust
pub struct LazyError {
    builder: Box<dyn Fn() -> miette::Report + Send + Sync>,
}

impl LazyError {
    pub fn new<F>(builder: F) -> Self
    where
        F: Fn() -> miette::Report + Send + Sync + 'static,
    {
        Self {
            builder: Box::new(builder),
        }
    }
    
    pub fn build(self) -> miette::Report {
        (self.builder)()
    }
}
```

### 错误批处理

收集多个错误后一次性报告：

```rust
pub struct ErrorCollector {
    errors: Vec<miette::Report>,
}

impl ErrorCollector {
    pub fn add_error<E>(&mut self, error: E)
    where
        E: Into<miette::Report>,
    {
        self.errors.push(error.into());
    }
    
    pub fn into_result<T>(self, value: T) -> Result<T, Vec<miette::Report>> {
        if self.errors.is_empty() {
            Ok(value)
        } else {
            Err(self.errors)
        }
    }
}
```

## 测试支持

### 错误快照测试

使用 `insta` 进行错误信息的快照测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;
    
    #[test]
    fn test_type_mismatch_error() {
        let source = "let x: i32 = \"hello\";".to_string();
        let error = TypeError::TypeMismatch {
            expected: "i32".to_string(),
            found: "string".to_string(),
            expected_span: SourceSpan::new(7.into(), 3),
            found_span: SourceSpan::new(13.into(), 7),
        };
        
        let report = miette::Report::new(error)
            .with_source_code(NamedSource::new("test.ny", source));
        
        assert_snapshot!(format!("{:?}", report));
    }
}
```

## 最佳实践

1. **一致的错误代码**: 使用统一的错误代码命名规范
2. **有用的帮助信息**: 提供具体的解决建议，而不是重复错误描述
3. **精确的位置信息**: 确保错误位置准确指向问题所在
4. **相关错误链接**: 使用 `#[related]` 属性链接相关错误
5. **文档链接**: 在适当的地方提供文档链接

## 与 Salsa 的集成

错误处理与增量计算的结合：

```rust
#[salsa::query_group(ErrorQueries)]
pub trait ErrorQueries {
    /// 收集文件中的所有错误
    fn file_errors(&self, file_id: FileId) -> Arc<Vec<miette::Report>>;
    
    /// 收集项目中的所有错误
    fn project_errors(&self) -> Arc<Vec<miette::Report>>;
}

fn file_errors(db: &dyn ErrorQueries, file_id: FileId) -> Arc<Vec<miette::Report>> {
    let mut errors = Vec::new();
    
    // 收集解析错误
    if let Err(parse_errors) = db.parse_file(file_id) {
        errors.extend(parse_errors);
    }
    
    // 收集类型错误
    if let Err(type_errors) = db.type_check_file(file_id) {
        errors.extend(type_errors);
    }
    
    Arc::new(errors)
}
```

## 总结

Miette 错误处理系统将 Nyar 编译器的诊断体验提升到了新的高度。通过提供上下文丰富、用户友好的错误信息，它不仅帮助开发者快速定位和解决问题，更重要的是，它将编译器从一个冰冷的工具转变为开发者的智能助手，显著提升了整体的开发体验。
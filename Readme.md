# Nyar Virtual Machine Platform

Nyar is a high-performance virtual machine, optimization platform, and interpreter designed for modern programming languages. It provides a unified compilation target and runtime environment that enables language designers to focus on language features while leveraging Nyar's advanced optimization capabilities.

## What is Nyar?

Nyar is **not** a programming language - it's a virtual machine platform that serves as:

- **Virtual Machine**: Executes bytecode with high performance
- **Optimization Platform**: Provides advanced optimization passes and JIT compilation
- **Interpreter**: Supports both interpreted and compiled execution modes
- **Language Runtime**: Offers runtime services for memory management, garbage collection, and effect handling

## Architecture Overview

```
Source Language (e.g., Valkyrie) → Frontend → AST → HIR → MIR → LIR → Nyar VM
                                                                    ↓
                                                            Native Code / JS / WASM
```

## Key Features

### 🚀 **High Performance**
- Advanced JIT compilation with multiple optimization tiers
- Efficient bytecode interpretation
- Smart memory management and garbage collection
- SIMD and vectorization support

### 🔧 **Multi-Target Compilation**
- Native code generation (x86_64, ARM64)
- JavaScript compilation for web deployment
- WebAssembly output for portable performance
- LLVM backend integration

### 🎭 **Advanced Language Features**
- Native algebraic effects support
- Efficient closure and continuation handling
- Pattern matching optimization
- Tail call optimization

### 🛠️ **Developer Experience**
- Rich debugging and profiling tools
- Hot code reloading
- Comprehensive error reporting
- IDE integration support

## Supported Frontends

- **Valkyrie**: Modern functional programming language with algebraic effects
- **Custom Languages**: Extensible frontend architecture for new language implementations

## Sub Projects

### Core VM Components
- **nyar-hir**: High-level Intermediate Representation
- **nyar-wasm**: WebAssembly backend
- **nyar-error**: Error handling and diagnostics

### Language Support
- **valkyrie-parser**: Valkyrie language frontend
- **nyar-document**: Documentation and guides

## Getting Started

### Prerequisites
- Rust 1.70+
- LLVM 15+ (for native compilation)
- Node.js 16+ (for JavaScript backend)

### Building from Source

```bash
# Clone the repository
git clone https://github.com/nyar-lang/nyar-vm.git
cd nyar-vm

# Build the VM
cargo build --release

# Run tests
cargo test
```

### Using Nyar VM

```bash
# Compile Valkyrie source to Nyar bytecode
nyar compile input.vk -o output.nyar

# Execute bytecode
nyar run output.nyar

# Compile to JavaScript
nyar compile input.vk --target js -o output.js

# Compile to WebAssembly
nyar compile input.vk --target wasm -o output.wasm
```

## Benefits for Language Implementers

### 🎯 **Focus on Language Design**
- No need to implement complex optimization passes
- Built-in support for modern language features
- Automatic memory management
- Cross-platform deployment

### ⚡ **Performance Out of the Box**
- JIT compilation with adaptive optimization
- Efficient garbage collection
- SIMD and vectorization
- Profile-guided optimization

### 🌐 **Multi-Target Support**
- Single IR compiles to multiple targets
- Consistent semantics across platforms
- Optimized code generation for each target

### 🔍 **Rich Tooling**
- Built-in debugger and profiler
- Memory usage analysis
- Performance monitoring
- IDE integration APIs

## Documentation

- [Architecture Guide](projects/nyar-document/guide/)
- [Language Implementation Guide](projects/nyar-document/development/)
- [VM Internals](projects/nyar-document/maintenance/)
- [API Reference](projects/nyar-document/api/)

## Contributing

We welcome contributions to the Nyar VM platform! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Development Areas
- 🔧 VM optimization and performance
- 🌐 New compilation targets
- 📚 Documentation and examples
- 🛠️ Developer tooling
- 🧪 Testing and benchmarking

## License

This project is licensed under the MIT License - see the [LICENSE](License.md) file for details.

## Acknowledgments

- Inspired by LLVM, JVM, and modern language runtimes
- Built with Rust for safety and performance
- Community-driven development

---

**Ready to build the next generation of programming languages?** Start with Nyar VM!
# nyar-jit

`nyar-jit` 负责将热点代码段从解释执行转换为本机机器码执行，以实现接近原生的性能。

## 核心职责

- **热点识别**: 监控字节码执行频率。
- **动态编译**: 驱动 `ProjectChomsky` 进行实时优化。
- **机器码管理**: 安全地管理可执行内存（W^X 策略）。

## 编译流水线

1.  **Extract (提取)**: 从运行中的 VM 提取 `IKun` 意图流（或直接从字节码反推）。
2.  **Optimize (优化)**: 利用 `UniversalOptimizer` 进行等价饱和（Equality Saturation）优化。
3.  **Lowering (降级)**: 将优化后的 `IKunTree` 转换为特定架构（如 x86_64, ARM64）的指令。
4.  **Execute (执行)**: 将机器码写入 `JitMemory`，通过函数指针跳转执行。

## 与 Gaia 引擎的集成

`nyar-jit` 深度集成了 `gaia-jit` 库：
- **JitMemory**: 处理跨平台的内存页权限管理（Alloc -> Write -> Make Executable）。
- **UniversalOptimizer**: 提供跨语言的公共优化 Pass。

## 开发调试

在调试 JIT 生成的代码时，可以使用以下环境变量：
- `NYAR_LOG_JIT=1`: 打印 JIT 编译过程的详细日志。
- `NYAR_DUMP_ASM=1`: 将生成的机器码反汇编并保存到本地。

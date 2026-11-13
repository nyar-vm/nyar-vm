# `nyar-backend-native`

`nyar-backend-native` 提供原生目标的最小后端实现；该冻结目标不消费 Semantic MIR，正式支持前必须接入完整
TargetExecutionPlan。

输入是已编码镜像字节：本后端只做镜像格式识别、目标架构校验与包装落盘，不承担 lowering。
`classify_native_image` 区分可执行文件与共享库（PE DLL / ELF `ET_DYN`，x86-64 或 AArch64）；
共享库扩展名（`.node` / `.pyd` / `.so` / `.dll` / `.dylib`）由
`NativeBinaryBackendInput::artifact_extension` 指定，可执行镜像不得声明共享库扩展名，
产物架构必须与请求目标一致。

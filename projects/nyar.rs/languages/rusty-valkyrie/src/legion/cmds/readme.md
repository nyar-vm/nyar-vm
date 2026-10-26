# Legion 工程操作

本目录实现构建、检查、运行、测试、文档、产物分析和包管理操作。操作入口将项目、目标和配置传入 workspace 规划与编译流程，并组织执行结果。

`build` 包含构建计划与编译器的连接；`test` 和 `test_engine` 处理测试发现及执行；`run` 选择产物与 runner；[spy](spy/readme.md) 检查目标产物；`bootstrap` 组织多代构建。

阅读某项操作时，可从对应的参数类型和 `run` 入口开始，追踪到 workspace、Compiler、缓存与报告层。工程库入口见 [Legion](../readme.md)。

# 目标代码服务

Wasm、CLR、JVM、OVM 和 native 是并列目标。每个目标拥有计划、LIR、验证、表示、调用约定、运行时包装与产物格式。语言到目标适配器消费语言自己的 IR 或残余程序，不引入共同语义总 IR。

- [Wasm](wasm.md)
- [CLR](clr.md)
- [JVM](jvm.md)
- [OVM 与 OdinVM](nyar-vm.md)
- [Native 与扩展 ABI](native.md)

[架构合同](../architecture.md)定义共同边界。目标目录或 emitter 存在只证明局部实现存在，不证明所有前端或宿主组合均已支持。

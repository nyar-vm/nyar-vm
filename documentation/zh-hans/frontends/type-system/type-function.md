# 类型函数 (Type Function)

类型函数是 Valkyrie 语言中用于在类型层面进行计算的强大特性。通过 `mezzo` 关键字定义，类型函数允许在编译时对类型进行操作和变换。

## 基本语法

```valkyrie
mezzo FunctionName(param: Type) -> ReturnType {
    $ 类型函数体
}
```

## 示例

### 判断偶数类型

```valkyrie
mezzo IsEven(z: Type) -> bool {
    $ 检查类型 z 是否表示偶数
    match z {
        i32 if z % 2 == 0 => true,
        _ => false
    }
}
```

### 类型映射

```valkyrie
mezzo MapType⟨T⟩(input: T) -> Type {
    $ 对输入类型进行映射变换
    match input {
        i32 => i64,
        f32 => f64,
        _ => input
    }
}
```

### 条件类型选择

```valkyrie
mezzo ConditionalType⟨T, U⟩(condition: bool) -> Type {
    $ 根据条件选择类型
    if condition {
        T
    } else {
        U
    }
}
```

## 特性

- **编译时执行**: 类型函数在编译时执行，不会产生运行时开销
- **类型安全**: 所有类型操作都经过编译器验证
- **递归支持**: 支持递归类型函数定义
- **模式匹配**: 可以对类型进行模式匹配

## 使用场景

1. **类型验证**: 在编译时验证类型是否满足特定条件
2. **类型转换**: 自动推导和转换相关类型
3. **泛型约束**: 为泛型参数添加复杂的类型约束
4. **元编程**: 实现高级的编译时代码生成

## 注意事项

- 类型函数必须是纯函数，不能有副作用
- 所有分支都必须返回有效的类型
- 递归深度有限制，防止无限递归
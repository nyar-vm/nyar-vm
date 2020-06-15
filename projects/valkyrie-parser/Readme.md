Valkyrie Language Specification
-------------------------------

## Let Binding

```ts
let a; /// 声明不变量 a
let mut lazy ref a; ///可以叠多个修饰词
```

这里必须加 `;`, 否则会无限捕捉 token

### 类型声明

```ts
let (mut a, b, c): (int, int, int)
```

### 初始化

```ts
let (mut a, b, c) = (1, 2, 3)
```

### 其他

```ts
let mut a, b, c {
    return (1, 2, 3)
}
```

`let` 后面 tuple 的括号可以省略不写;

可以用 block 开启新的 scope

```ts
let mut a, _, _ = (1,2,3)
```

有时候表达式有多个返回值, 但是如果不需要可以用 `_` 抑制

## Def Binding

```py
def a() {
    pass
}
```


```py
def final private lazy function(a, b:int?=1) {
    pass
}
```


## Lambda

其实 let 和 def 没有多大区别.

只是习惯上用 let 声明变量, def 声明函数.

## Control Flow



### If Statement

```py
if a {

}
else if b {
    pass
}
else if c {
    pass
}
else {

}
```

### While Statement

```py
while(true){
    pass
}
```


### For-in Statement

```py
for v in iter {
   pass
}
```

```py
for k, v in iters {
   pass
}
```

同理 for 后面的括号可以省略

in 后面如果不是 Iterator 会尝试转化.

## Pattern Match

### Match Statement

undefined

## Apply Call

- apply: `var<type_expr>(input_expr, key:value_expr)`

## Uniform Function Call Syntax

UFCS 是指 `.操作符` 调用时先搜索实例方法, 如果没有, 那么继续寻找函数.

最终效果使得 `a.map(f)` <=> `map(a,f)` 在语义上就是等价的.

由于这个特性的存在, 静态方法和类方法不能再直接使用 `.操作符`

也就是说不能写成 `"42".i64.from()`, 而要使用 `::操作符`, 也就是说要写 `"42".i64::from()`

### Called Without Parentheses

CWP 是指如果函数不写参数可以省略括号

`"42".i64::from` => `"42".i64::from()` => `i64::from("42")`

## Index Call

- index: `var[num]` or `var[str]` or `var[slice]` or `var[list]`


| Short                | Full                 | Result |
| :------------------- | :------------------- | :----- |
| `a["key"]`           | `a.key`              | value  |
| `a[1,2,3]`           | `a.1.2.3`            | value  |
| `a[[1,2,3]]`         | `[a.1,a.2,a.3]`      | list   |
| `a[1:3:-1]`          | `a[[3,2,1]]`         | list   |
| `a[1:3:-1,[1,3,-1]]` | `a[1:3:-1][[1,2,3]]` | list   |



## Literal

### String Literal

字符串能使用 `"` 或者 `'` 定义

`""` 是标准字符串, 默认支持转义, 也可以使用 r 禁止 `r"\n"`

r 是字符串表达式.

### Number Literal

2147483647i32

173.5cm

这里 `i32` 和 `cm` 是数字表达式, 实际输入是字符串.

### Other Literal

- true => Boolean::True
- false => Boolean::False
- null => Optional::None


----
----
----


## Data

data

## Type

type


## Class

无继承

```ts
class A(int,int) {
    // 类似 rust 的 Tuple Structs
}
```

```ts
class C: T1, T2 {
    a : int; /// 必须写类型
    b : int = 2 /// defalut 值
    c = false;/// 不支持 let tuple 模式匹配
    C() -> C {} ///构造函数
    f()->Unit { /// 必须写返回类型
    }
}
```

构造函数未定义的话, `C()` 默认会空初始化, 也就是对所有字段赋 default.

## Trait

```rust
trait T {
    f(){}
}
```

### Class Extend

```ts
extend C {
    f() {}
}
extend C with T {
    f(){}
}
def C::f() {

}
```



## Overload

function Adhoc, undefined

---

op dispatch

```py
@operator::infix  //中缀
@operator::prefix //前缀
@operator::suffix //后缀
@operator::number //数字
@operator::string //字符
```

Does not support new operators, nor does it support changing the precedence and associativity of operators

```py
import operator.infix

@infix("+")
def add_string_and_int(s:str,i:int)->Unit{
    pass
}
```

All types must be explicitly marked

can be set in trait

shorten of __infix_add

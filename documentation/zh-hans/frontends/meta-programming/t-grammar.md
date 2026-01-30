# 模板语法 (T-Grammar)

在理解了 [V-Grammar](../../examples/web-development/v-grammar.md) 的对象构建和 [X-Grammar](../../examples/web-development/x-grammar.md) 的视觉投影后，**T-Grammar** 提供了对**代码生成**和**文本处理**的极致支持。

T-Grammar 专门为 [quote](./macro.md) 元编程设计，允许在生成代码时无缝嵌入复杂的逻辑控制流。

## 1. 基础语法

T-Grammar 通常作为**模板字符串字面量**存在，使用 `quote" "` 或 `t" "` 前缀包裹。在某些上下文中（如返回值为 `TokenStream` 的宏），最后一个字符串字面量会被自动解读为 T-Grammar。

T-Grammar 使用 `<%` 和 `%>` 作为界定符。

- **表达式插值 `<% expr %>`**：将表达式的结果转换为字符串并插入。
- **逻辑控制 `<% statement %>`**：执行 Valkyrie 的控制流语句。
- **注释 `<# comment #>`**：模板内部注释，不会输出到结果中。

```valkyrie
let template = quote"
<% let name = "Valkyrie" %>
Hello, <% name %>!
<# 这是一个注释，不会出现在生成的代码中 #>
"
```

## 2. 逻辑控制

T-Grammar 完美继承了 Valkyrie 的控制流语法，但将其包裹在 `<% %>` 中。逻辑标签（如 `<% for %>`）可以独立存在于模板中，但在普通的 Valkyrie 代码中直接嵌入标签（如 `class <% name %>`）是非法的。

### 条件分支 (`if`)

```valkyrie
quote"
<% if condition %>
    # 当 condition 为真时生成的代码
<% else if other_condition %>
    # 当 other_condition 为真时生成的代码
<% else %>
    # 默认生成的代码
<% end if %>
"
```

### 循环迭代 (`for`)

```valkyrie
quote"
<% for i in j %>
    <% expr(i) %>
<% end for %>
"
```

### 模式匹配 (`match`)

```valkyrie
quote"
<% match status %>
    <% case Loading %>
        print("Loading...")
    <% case Success { data } %>
        print("Data: <% data %>")
    <% case Error { err } %>
        print("Error: <% err %>")
<% end match %>
"
```

## 3. 宏与 TokenStream

在编写过程宏时，如果函数声明返回 `TokenStream`，Valkyrie 会自动将函数末尾的字符串字面量视为 `quote` 模板。

```valkyrie
@macro
micro generate_struct(name: string, fields: [Field]) -> TokenStream {
    # 这里的字符串被自动解读为 quote"..."
    """
    class <% name %> {
        <% for f in fields %>
            <% f.name %>: <% f.type %>,
        <% end for %>
    }
    """
}
```

## 4. 空白符控制

为了让生成的代码整洁美观，T-Grammar 提供了空白符修剪功能：

- `<%_` : 移除标签左侧的所有空白（包括换行）。
- `_%>` : 移除标签右侧的所有空白。

```valkyrie
<ul>
<%_ for i in 1..3 _%>
    <li><% i %></li>
<%_ end for _%>
</ul>
```

生成的 HTML 将是紧凑的：
```html
<ul>
    <li>1</li>
    <li>2</li>
    <li>3</li>
</ul>
```

## 5. 语法对比

| 特性 | T-Grammar | X-Grammar | V-Grammar |
| :--- | :--- | :--- | :--- |
| **定位** | 文本/代码生成 | UI 视觉投影 | 对象 DSL 构建 |
| **界定符** | `<% ... %>` | `<tag> ... </tag>` | `{ ... }` |
| **插值** | `<% expr %>` | `(expr)` | `${expr}` 或 `expr` |
| **逻辑** | `<% for ... %>` | `<for ...>` | `for ... { ... }` |

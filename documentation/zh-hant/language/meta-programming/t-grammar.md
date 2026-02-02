# 模板語法 (T-Grammar)

在理解了 [V-Grammar](../../examples/web-development/v-grammar.md) 的對象構建和 [X-Grammar](../../examples/web-development/x-grammar.md) 的視覺投影後，**T-Grammar** 提供了對**代碼生成**和**文本處理**的極致支持。

T-Grammar 專門為 [quote](./macro.md) 元編程設計，允許在生成代碼時無縫嵌入複雜的邏輯控制流。

## 1. 基礎語法

T-Grammar 通常作為**模板字符串字面量**存在，使用 `quote" "` 或 `t" "` 前綴包裹。在某些上下文中（如返回值為 `TokenStream` 的宏），最後一個字符串字面量會被自動解讀為 T-Grammar。

T-Grammar 使用 `<%` 和 `%>` 作為界定符。

- 表達式插值 `{ expr }`：將表達式的結果轉換為字串並插入。
- 邏輯控制 `<% statement %>`：執行 Valkyrie 的控制流語句。
- 註釋 `<# comment #>`：模板內部註釋，不會輸出到結果中。

```valkyrie
let template = quote"
<% let name = "Valkyrie" %>
Hello, {name}!
<# 這是一個註釋，不會出現在生成的代碼中 #>
"
```

## 2. 邏輯控制

T-Grammar 完美繼承了 Valkyrie 的控制流語法，但將其包裹在 `<% %>` 中。邏輯標籤（如 `<% for %>`）可以獨立存在於模板中，但在普通的 Valkyrie 代碼中直接嵌入標籤（如 `class <% name %>`）是非法的。

### 條件分支 (`if`)

```valkyrie
quote"
<% if condition %>
    # 當 condition 為真時生成的代碼
<% else if other_condition %>
    # 當 other_condition 為真時生成的代碼
<% else %>
    # 默認生成的代碼
<% end if %>
"
```

### 循環迭代 (`for`)

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

## 3. 宏與 TokenStream

在編寫過程宏時，如果函數聲明返回 `TokenStream`，Valkyrie 會自動將函數末尾的字符串字面量視為 `quote` 模板。

```valkyrie
@macro
micro generate_struct(name: string, fields: [Field]) -> TokenStream {
    # 這裡的字符串被自動解讀為 quote"..."
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

為了讓生成的代碼整潔美觀，T-Grammar 提供了空白符修剪功能：

- `<%_` : 移除標籤左側的所有空白（包括換行）。
- `_%>` : 移除標籤右側的所有空白。

```valkyrie
<ul>
<%_ for i in 1..3 _%>
    <li><% i %></li>
<%_ end for _%>
</ul>
```

生成的 HTML 將是緊湊的：
```html
<ul>
    <li>1</li>
    <li>2</li>
    <li>3</li>
</ul>
```

## 5. 語法對比

| 特性 | T-Grammar | X-Grammar | V-Grammar |
| :--- | :--- | :--- | :--- |
| **定位** | 文本/代碼生成 | UI 視覺投影 | 對象 DSL 構建 |
| **界定符** | `<% ... %>` | `<tag> ... </tag>` | `{ ... }` |
| **插值** | `{ expr }` | `(expr)` | `${expr}` 或 `expr` |
| **邏輯** | `<% for ... %>` | `<for ...>` | `for ... { ... }` |

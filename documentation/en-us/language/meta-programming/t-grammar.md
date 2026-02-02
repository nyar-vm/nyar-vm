# Template Grammar (T-Grammar)

After understanding [V-Grammar](../../examples/web-development/v-grammar.md) for object construction and [X-Grammar](../../examples/web-development/x-grammar.md) for visual projection, **T-Grammar** provides ultimate support for **code generation** and **text processing**.

T-Grammar is specifically designed for [quote](./macro.md) metaprogramming, allowing seamless embedding of complex logic control flow when generating code.

## 1. Basic Syntax

T-Grammar usually exists as a **template string literal**, wrapped with the `quote" "` or `t" "` prefix. In certain contexts (such as macros returning a `TokenStream`), the last string literal is automatically interpreted as T-Grammar.

T-Grammar uses `<%` and `%>` as delimiters.

- **Expression Interpolation `{ expr }`**: Converts the result of an expression to a string and inserts it.
- **Logic Control `<% statement %>`**: Executes Valkyrie control flow statements.
- **Comments `<# comment #>`**: Internal template comments that will not be output to the result.

```valkyrie
let template = quote"
<% let name = "Valkyrie" %>
Hello, {name}!
<# This is a comment and will not appear in the generated code #>
"
```

## 2. Logic Control

T-Grammar perfectly inherits Valkyrie's control flow syntax but wraps it in `<% %>`. Logic tags (such as `<% for %>`) can exist independently within a template, but embedding tags directly in normal Valkyrie code (e.g., `class <% name %>`) is illegal.

### Conditional Branching (`if`)

```valkyrie
quote"
<% if condition %>
    # Code generated when condition is true
<% else if other_condition %>
    # Code generated when other_condition is true
<% else %>
    # Default generated code
<% end if %>
"
```

### Iteration (`for`)

```valkyrie
quote"
<% for i in j %>
    <% expr(i) %>
<% end for %>
"
```

### Pattern Matching (`match`)

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

## 3. Macros and TokenStream

When writing procedural macros, if a function declaration returns a `TokenStream`, Valkyrie automatically treats the string literal at the end of the function as a `quote` template.

```valkyrie
@macro
micro generate_struct(name: string, fields: [Field]) -> TokenStream {
    # The string here is automatically interpreted as quote"..."
    """
    class <% name %> {
        <% for f in fields %>
            <% f.name %>: <% f.type %>,
        <% end for %>
    }
    """
}
```

## 4. Whitespace Control

To make the generated code neat and beautiful, T-Grammar provides whitespace trimming features:

- `<%_` : Removes all whitespace (including newlines) to the left of the tag.
- `_%>` : Removes all whitespace to the right of the tag.

```valkyrie
<ul>
<%_ for i in 1..3 _%>
    <li><% i %></li>
<%_ end for _%>
</ul>
```

The generated HTML will be compact:
```html
<ul>
    <li>1</li>
    <li>2</li>
    <li>3</li>
</ul>
```

## 5. Syntax Comparison

| Feature | T-Grammar | X-Grammar | V-Grammar |
| :--- | :--- | :--- | :--- |
| **Positioning** | Text/Code Generation | UI Visual Projection | Object DSL Construction |
| **Delimiters** | `<% ... %>` | `<tag> ... </tag>` | `{ ... }` |
| **Interpolation** | `{ expr }` | `(expr)` | `${expr}` or `expr` |
| **Logic** | `<% for ... %>` | `<for ...>` | `for ... { ... }` |

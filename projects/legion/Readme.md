# Nyar High-Level Intermediate Representation

## Task

- From text to AST
- Macro expand
- Fill missing lex level info

TEXT -> AST -> CSR -> CPS

### Stage Abstract Syntax Tree

Remove all whitespace and comments (except documentation comments)

### Stage Class Scope Resolver

Eliminate control flow keywords like

`break`, `continue`, `fallthrough`
`yield`, `yield_from`

Transform such non-local exit to function call.

Identify tail calls and tail recursion.

Common subexpression elimination.



### Stage Continuation Passing Style


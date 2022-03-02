| Symbol | Example | Comment | 
|--------|---------|---------|

| `%`    | `$var_1` | local variable |
| `#`    | `#a`     | input argument |

```vk


def add2(a: Integer, b: Integer) {
    a + b
}


#repl::ONE = 1;


function repl::add2
(
    %a: std::math::Integer,
    %b: std::math::Integer,
    %__fsm: small_int,
    ^return: auto,
)
{
    %tmp1 = binary(+, %a, %b);
    ^return(%tmp1);
}
```


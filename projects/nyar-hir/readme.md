- AST

```scala
def fibonacci(n: Integer) {
    fibonacci_helper(n, 0, 1)
}

def fibonacci_helper(n, previous, current)
{
    if n == 0 { previous }
    if n == 1 { current }
    fibonacci_helper(n - 1, current, previous + current);
}
```

- HIR

```scala
function repl::fibonacci
(
    state: small_int,
    %n: auto,
    ^return: auto,
)
{
0: 
   %tmp1 = call repl::fibonacci_helper (%n, 0, 1);
}


function fibonacci_helper
(
    [named]
    state: small_int,
    %n: auto,
    %previous: auto,
    %current: auto,
    ^return: auto,
)
{
function.begin:
    %var1 = binary == (%n, 0);
    jump 1 if %var1;
    jump 2;
if1.begin: 
    ^return(%previous);
if1.end:
    %var2 = binary == (%n, 1);
    jump 3 if %var2;
    jump 4;
if2.begin:
    ^return(%current);
if2.end:
    %tmp3 = binary - (%n, 1);
    %tmp4 = binary + (%previous, %current);
    %tmp5 = %n - 1;
    %tmp6 = %current;
    %tmp7 = ninary + (%previous, %current);
    %n = %tmp5;
    %previous = %tmp6;
    %current = %tmp7;
    jump 0;
}
```


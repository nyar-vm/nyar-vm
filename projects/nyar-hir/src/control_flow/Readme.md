


## For statement

```markdown
for `pattern` in `iterator` if `conditional` {
    `[body]`
}
otherwise {
    `[otherwise]`
}
```


```markdown
i = iterator.into_iterator()
let mut not_run = false;
loop {
    let next = i.next();
    if next.is_none() {
        goto@tail
    }
    let pattern = next.unwrap();
    body
    goto@begin
}
if not_run {
    `[otherwise]`
    goto@tail
}

for `pattern` in `iterator` if `conditional` {
    `[body]`
}
otherwise {
    `[otherwise]`
}
```

## Loop statement


```markdown
@head 10
yield n
@tail 11
```

```markdown
@label 10
state = 11
return(n)
@label 11
```


## While statement

```markdown
while `condition` {
    `[body]`
}
else {
    `[otherwise]`
}
```
## While Statement

```scala
while cond {
    body1
}
```


```scala
loop {
    if cond {
        body1
    }
    else {
        break
    }
}
```

## While Else Statement


```scala
while cond {
    body1
}
else {
    body2
}
```


```scala
if cond {
    loop {
        if cond {
            body1
        }
    }
}
else {
    body2
}
```



```scala
for i in iter if cond {
    body1
}
else {
    body2
}
```


```scala
for i in iter if cond {
    body1
}
else {
    body2
}
```
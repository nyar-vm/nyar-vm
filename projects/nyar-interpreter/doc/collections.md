


异质容器


|      | 同质   | 异质  |
| :--- | :----- | :---- |
| 定长 | Array  | Tuple |
| 变长 | Vector | List  |




## 映射类

HashMap
RBTreeMap
AVLTreeMap
OrderedMap
LRUMap

## 映射类

HashSet
BTreeSet
OrderedSet
LRUSet


class HashSet<T> {
    inner: HashMap<T, ()>
}

@[public, native]

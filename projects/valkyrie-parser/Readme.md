# Renormalization Notations

- a is a scalar
- A is a vector

| Unicode | Usage   | Expand          |
|:--------|:--------|:----------------|
| U+22C8  | `A ⋈ B` | `A.concat(B)`   |             |
| U+2A1D  | `A ⨝ B` | `A.concat(B)`   |             |
| U+002D  | `-a`    | `A.negative(a)` |
| U+002D  | `a - b` | `A.negative(a)` |
| U+2212  | `-a`    | `A.minus(a)`    |
| U+2212  | `a − b` | `A.minus(a)`    |

## Product Operators

| Unicode | Usage   | Expand                   | Comment     |
|:--------|:--------|:-------------------------|-------------|
| U+22C6  | `a ⋆ b` | `A.broadcast_product(a)` |             |
| U+2217  | `a ∗ b` | `A.broadcast_product(a)` |             |
| U+002A  | `a * b` | `A.broadcast_product(a)` | recommended |
| U+00D7  | `a × b` | `A.cross_product(a)`     |             |
| U+2A2F  | `a ⨯ b` | `A.cross_product(a)`     | recommended |
| U+2A09  | `a ⨉ b` | `A.cross_product(a)`     |             |
| U+2297  | `a ⊗ b` | `A.tensor_product(a)`    |             |
| U+2A02  | `a ⨂ b` | `A.tensor_product(a)`    |             |
| U+29BB  | `a ⦻ b` | `A.tensor_product(a)`    |             |
| U+22C5  | `a ⋅ b` | `A.dot_product(a)`       |             |
| U+2317  | `A ⌗ B` | `A.cartesian_product(B)` | recommended |
| U+22C4  | `a ⊙ b` |                          | reserved    |
| U+2A00  | `a ⨀ b` |                          | reserved    |
|
| U+22A0  | `A ⊠ B` |                          | reserved    |
| U+22C9  | `A ⋉ B` |                          | reserved    |
| U+22CA  | `A ⋊ B` |                          | reserved    |
| U+2A33  | `A ⨳ B` |                          | reserved    |

### Broadcast Product

A.cartesian_product(a).wait

A.filter { $x != 0 }.wait

### Tensor Product

Also named `Kronecker Product` in matrix form.

⊙

### Cartesian Product

A ⋈ B ⋈ C

[1, 2, 3] ⋈ [1, 2, 3]

let `⋈` be cartesian product

so "abc" ⋈ "def" equals

### Division Product

[2, 5, 7] ⩩ 9 =

## Not Operator

| Unicode | Usage | Expand    |
|:--------|:------|:----------|
| U+00AC  | `¬a`  | `A.not()` |
| U+0021  | `!a`  | `A.not()` |

## Superscript

| Unicode | Usage | Expand        |
|:--------|:------|:--------------|
| U+2070  | `a⁰`  | `A.power(0)`  |
| U+00B9  | `a¹`  | `A.power(1)`  |
| U+00B2  | `a²`  | `A.power(2)`  |
| U+00B3  | `a³`  | `A.power(3)`  |
| U+2074  | `a⁴`  | `A.power(4)`  |
| U+2075  | `a⁵`  | `A.power(5)`  |
| U+2076  | `a⁶`  | `A.power(6)`  |
| U+2077  | `a⁷`  | `A.power(7)`  |
| U+2078  | `a⁸`  | `A.power(8)`  |
| U+2079  | `a⁹`  | `A.power(9)`  |
| U+2079  | `a¹⁰` | `A.power(10)` |

## Set Operators

$±$

⋇

| U+22C3 | `a ⋃ b` | `A.union(a)`             |
| U+22C2 | `a ⋂ b` | `A.intersection(a)`      |

| Unicode | Usage   | Expand                   |
|:--------|:--------|:-------------------------|
| U+2218  | `a ∘ b` | `A.broadcast_product(a)` |

| Unicode | Usage   | Expand           |
|:--------|:--------|:-----------------|
| U+220D  | `A ∍ a` | `A.contains(a)`  |
| U+220B  | `A ∋ a` | `A.contains(a)`  |
| U+220C  | `A ∌ a` | `!A.contains(a)` |
| U+220A  | `a ∊ A` | `A.contains(a)`  |
| U+2208  | `a ∈ A` | `A.contains(a)`  |
| U+2209  | `a ∉ A` | `!A.contains(a)` |

## Degree

| Unicode | Usage | Expand       |
|:--------|:------|:-------------|
| U+00B0  | `a°`  | `A.degree()` |
| U+2032  | `a′`  | `A.minute()` |
| U+2033  | `a″`  | `A.second()` |
| U+2034  | `a‴`  | `A.third()`  |
| U+2057  | `a⁗`  | `A.fourth()` |

## Matrix

```vk
[
    1, 2; 
    3, 4;
]
a.[b]
¶, §, ⸿, ፠, ๛

≛



⸿name.space
¶
¶module::S()
```

# Percent

| Unicode | Usage | Expand               |
|:--------|:------|:---------------------|
| U+2205  | `a%`  | `A.percent()`        |
| U+2030  | `a‰`  | `A.perthousand()`    |
| U+2031  | `a‱`  | `A.pertenthousand()` |

## Temperature

| Unicode | Usage | Expand          |
|:--------|:------|:----------------|
| U+2103  | `n℃`  | `Celsius(n)`    |
| U+2109  | `n℉`  | `Fahrenheit(n)` |

## Logic

| Unicode | Usage   | Expand      |   Comment   |
|:--------|:--------|:------------|:-----------:|
| U+      | `!a`    | `a.not()`   |             |
| U+00AC  | `¬a`    | `a.not()`   | recommended |
| U+2228  | `a ∨ b` | `a.or(b)`   |             |
| U+22BD  | `a ⊽ b` | `a.nor(b)`  |  `¬p ∧ ¬q`  |
| U+22BB  | `a ⊻ b` | `a.xor(b)`  |             |
| U+2227  | `a ∧ b` | `a.and(b)`  |             |
| U+22BC  | `a ⊼ b` | `a.nand(b)` |  `¬p ∨ ¬q`  |
| U+2A5F  | `a ⩟ b` | `a.xand(b)` |             |

Logic gate can be called by `p.logic_gate(q, mask)`

| No. | Mask | Expression |  
|----:|:-----|:----------:|
|   1 | 0000 |  `false`   | 
|   2 | 0001 |  `p ∧ q`   |  
|   3 | 0010 |  `p ∧ ¬q`  |  
|   4 | 0011 |    `p`     |  
|   5 | 0100 |  `¬p ∧ q`  | 
|   6 | 0101 |    `q`     | 
|   7 | 0110 |  `p ⊻ q`   | 
|   8 | 0111 |  `p ∨ q`   | 
|   9 | 1000 | `¬p ∧ ¬q`  | 
|  10 | 1001 | `p === q`  | 
|  11 | 1010 |    `¬q`    |  
|  12 | 1011 |  `p ∨ ¬q`  | 
|  13 | 1100 |    `¬p`    |   
|  14 | 1101 |  `¬p ∨ q`  | 
|  15 | 1110 | `¬p ∨ ¬q`  | 
|  16 | 1111 |   `true`   |  

### Comparison

| Unicode | Usage   | Expand    |
|:--------|:--------|:----------|
| U+2260  | `a ≠ b` | `a.ne(b)` |
| U+2264  | `a ≤ b` | `a.le(b)` |
| U+2265  | `a ≥ b` | `a.ge(b)` |

### Empty set

| Unicode | Usage | Expand |
|:--------|:------|:-------|
| U+2205  | `∅`   | `[]`   |

with

case A
when A

switch if {
a == b {
}
}
k <- a + b
a <~ b
a <- b
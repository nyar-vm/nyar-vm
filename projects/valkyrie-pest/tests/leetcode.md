
```vk
def twoSum(self, nums: Vector[int], target: int) -> Vector[int] {
    let map: HashMap[int, int] = []
    for i, num in nums.enumerate() {
        if target - num in map {
            return [map[target - num], i]
        }
        map[nums[i]] = i
    }
    return []
}
```


## Prefix Expression

| Operator |     Name |
| -------: | -------: |
|      `+` | Positive |
|      `-` | Negative |
|      `!` |      Not |

## Postfix Expression

| Operator |      Name |
| -------: | --------: |
|     `++` | Increment |
|     `--` | Decrement |
|      `!` | Unchecked |
|      `?` |     Raise |
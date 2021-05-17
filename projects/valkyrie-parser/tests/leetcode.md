
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

using std;
using numpy as npy 
using {
    ~ wooooooow
    torch as tf
    tensorflow as th
}
using "script/path" {
    self as script, other
}
using mod.*
using mod::*
using mod as z
using mod as _
using mod.{
	a as b
	c as d
	e::f::{
        g as h
	}
}
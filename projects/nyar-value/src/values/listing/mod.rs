use std::collections::{linked_list::IntoIter, LinkedList};

use shredder::Scan;

use crate::{NyarCast, NyarResult, NyarValue};

#[derive(Clone, Debug, Scan)]
pub struct AnyList {
    inner: LinkedList<NyarValue>,
}

impl AnyList {
    pub fn length(&self) -> NyarValue {
        todo!()
    }
    pub fn first(&self) -> Option<NyarValue> {
        self.inner.iter().nth(0).cloned()
    }
    pub fn last(&self) -> Option<NyarValue> {
        self.inner.iter().rev().nth(0).cloned()
    }
    /// 给出去掉最后一个元素的 expr.
    pub fn most(&self) -> AnyList {
        let mut out = LinkedList::new();
        if self.inner.len() <= 1 {
            return AnyList { inner: out };
        }
        let mut iter = self.inner.iter();
        AnyList { inner: iter.take(self.inner.len() - 1).cloned().collect() }
    }
    /// 给出去掉第一个元素的 expr.
    pub fn rest(&self) -> AnyList {
        let mut out = LinkedList::new();
        if self.inner.len() <= 1 {
            return AnyList { inner: out };
        }
        let mut iter = self.inner.iter();
        AnyList { inner: iter.take(self.inner.len() - 1).cloned().collect() }
    }
    pub fn flatten(&self) -> AnyList {
        todo!()
    }
    pub fn take_range(&self, a: usize, b: usize) -> NyarResult {
        todo!()
    }
    pub fn map(&self) -> AnyList {
        todo!()
    }
    pub fn sort(&self) -> AnyList {
        todo!()
    }
    pub fn contains(&self) -> bool {
        todo!()
    }
    pub fn riffle(&self) -> AnyList {
        todo!()
    }
    // Union[list1,list2,…]
    // 给出 listi 中不同元素的列表
    // Intersection[list1,list2,…]
    // 给出 listi 中共有的元素的列表
    // Complement[universal,list1,…]
    // 给出在 universal 中，但不在 listi 中的元素的列表
    // Subsets[list]
    // 给出 list 中元素的所有子集的列表
    // DeleteDuplicates[list]
    // 从 list 中删除所有重复元素
    // Union[list]
    // 整理元素，删除重复元素
    // Reverse[list]
    // 对 list 的元素进行反向排序
    // RotateLeft[list,n]
    // 把列表 list 元素向左轮换 n 个位置
    // RotateRight[list,n]
    // 把列表元素向右轮换 n 个位置
    // PadLeft[list,len,x]
    // 在 list 的左边用 x 进行填充，产生一个长度为 len 的列表
    // PadRight[list,len,x]
    // 在 list 的右边进行填充
    // Partition[list,n]
    // 把 list 分成 n 个元素一组
    // Partition[list,n,d]
    // 使用偏移 d 进行逐次分组
    // Split[list]
    // 把 list 按邻接的相同元素进行分组
    // SplitBy[list,f]
    // 当应用 f 时，将 list 分为具有相同值得参数运行
    // Gather[list]
    // 将 list 的参数收集到相同参数的子列表中
    // GatherBy[list,f]
    // 当应用 f 时，将列表的参数收集到具有相同值的子列表中

    // Min
    // Max
    // Sort[list]
    // 把 list 中的元素按顺序排列
    // Ordering[list]
    // Sort[list] 中 list 元素的位置
    // Ordering[list,n]
    // Ordering[list] 的前 n 个元素
    // Ordering[list,-n]
    // Ordering[list] 的最后 n 个元素
    // Permutations[list]
    // list 的所有可能排序
    // AllTrue ▪  AnyTrue ▪  NoneTrue
    pub fn all_true(&self) -> bool {
        for i in self.inner {
            if i.is_false() {
                return false;
            }
        }
        return true;
    }
    pub fn any_true(&self) -> bool {
        for i in self.inner {
            if i.is_true() {
                return true;
            }
        }
        return false;
    }
    pub fn none_true(&self) -> bool {
        for i in self.inner {
            if i.is_true() {
                return false;
            }
        }
        return true;
    }
}

/// Mutable methods
impl AnyList {
    pub fn extend(&mut self, new: &AnyList) {
        self.inner.extend(new.inner.iter().cloned())
    }
    pub fn push(&mut self, new: NyarValue) {
        self.inner.push_front(new)
    }
    pub fn pop(&mut self) -> Option<NyarValue> {
        self.inner.pop_back()
    }
    pub fn pop_front(&mut self) -> Option<NyarValue> {
        self.inner.pop_front()
    }
    pub fn push_front(&mut self, new: NyarValue) {
        self.inner.push_front(new)
    }
    pub fn extend_front(&mut self, new: &AnyList) {
        let mut out = new.clone();
        out.inner.extend(self.inner.iter().cloned());
        *self = out
    }
    pub fn drop(&mut self) -> AnyList {
        todo!()
    }
    pub fn flatten_inplace(&mut self) {
        todo!()
    }
    pub fn map_inplace(&mut self) {
        todo!()
    }
    pub fn sort_inplace(&mut self) {
        todo!()
    }
}

impl IntoIterator for AnyList {
    type Item = NyarValue;
    type IntoIter = IntoIter<NyarValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

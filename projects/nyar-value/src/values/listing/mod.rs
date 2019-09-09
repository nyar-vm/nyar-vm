use std::collections::{
    linked_list::{IntoIter, IterMut},
    LinkedList,
};

use shredder::Scan;

use crate::{NyarResult, NyarValue};

#[derive(Clone, Debug, Scan)]
pub struct NyarList {
    inner: LinkedList<NyarValue>,
}

impl NyarList {
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
    pub fn most(&self) -> NyarList {
        let mut out = LinkedList::new();
        if self.inner.len() <= 1 {
            return NyarList { inner: out };
        }
        let mut iter = self.inner.iter();
        NyarList { inner: iter.take(self.inner.len() - 1).cloned().collect() }
    }
    /// 给出去掉第一个元素的 expr.
    pub fn rest(&self) -> NyarList {
        let mut out = LinkedList::new();
        if self.inner.len() <= 1 {
            return NyarList { inner: out };
        }
        let mut iter = self.inner.iter();
        NyarList { inner: iter.take(self.inner.len() - 1).cloned().collect() }
    }
    pub fn flatten(&self) -> NyarList {
        todo!()
    }
    pub fn take_range(&self, a: NyarValue, b: NyarValue) -> NyarResult {
        todo!()
    }
    pub fn map(&self) -> NyarList {
        todo!()
    }
    pub fn reduce(&self) -> NyarList {
        todo!()
    }
    pub fn filter(&self) -> NyarList {
        todo!()
    }
    pub fn discard(&self) -> NyarList {
        todo!()
    }
    pub fn filter_discard(&self) -> NyarList {
        todo!()
    }
    pub fn riffle(&self) -> NyarList {
        todo!()
    }
    pub fn reverse(&self) -> NyarList {
        NyarList { inner: self.inner.iter().rev().cloned().collect() }
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
    // Ordering[list,n]
    // Ordering[list] 的前 n 个元素
    // Ordering[list,-n]
    // Ordering[list] 的最后 n 个元素
    // Permutations[list]
    // list 的所有可能排序
    // AllTrue ▪  AnyTrue ▪  NoneTrue
    pub fn all_true(&self) -> bool {
        for i in &self.inner {
            if i.is_false() {
                return false;
            }
        }
        return true;
    }
    pub fn any_true(&self) -> bool {
        for i in &self.inner {
            if i.is_true() {
                return true;
            }
        }
        return false;
    }
    pub fn none_true(&self) -> bool {
        for i in &self.inner {
            if i.is_true() {
                return false;
            }
        }
        return true;
    }
}

/// Mutable methods
impl NyarList {
    pub fn extend(&mut self, new: &NyarList) {
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
    pub fn extend_front(&mut self, new: &NyarList) {
        let mut out = new.clone();
        out.inner.extend(self.inner.iter().cloned());
        *self = out
    }
    pub fn drop(&mut self) -> NyarList {
        todo!()
    }
    pub fn flatten_inplace(&mut self) {
        todo!()
    }
    pub fn map_inplace(&mut self) {
        todo!()
    }
    // 即 `iter_mut`
    pub fn traverse(&mut self) -> IterMut<'_, NyarValue> {
        self.inner.iter_mut()
    }
}

impl IntoIterator for NyarList {
    type Item = NyarValue;
    type IntoIter = IntoIter<NyarValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

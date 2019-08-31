use std::collections::LinkedList;

use shredder::Scan;

use crate::NyarValue;

#[derive(Debug, Scan)]
pub struct AnyList {
    inner: LinkedList<NyarValue>,
}

impl AnyList {
    pub fn length(&self) -> NyarValue {
        todo!()
    }
    pub fn first(&self) -> Option<&NyarValue> {
        self.inner.iter().nth(0)
    }
    pub fn last(&self) -> Option<&NyarValue> {
        self.inner.iter().rev().nth(0)
    }
    /// 给出去掉最后一个元素的 expr.
    pub fn most(&self) -> AnyList {
        let mut out = LinkedList::new();
        if self.inner.len() <= 1 {
            return AnyList { inner: out };
        }
        let mut iter = self.inner.iter();
        AnyList { inner: iter.take(self.inner.len() - 1).collect() }
    }
    /// 给出去掉第一个元素的 expr.
    pub fn rest(&self) -> AnyList {
        let mut out = LinkedList::new();
        if self.inner.len() <= 1 {
            return AnyList { inner: out };
        }
        let mut iter = self.inner.iter();
        AnyList { inner: iter.take(self.inner.len() - 1).collect() }
    }
    pub fn flatten(&self) -> AnyList {
        todo!()
    }
}

impl AnyList {
    pub fn push_front(&mut self, new: NyarValue) {
        self.inner.push_front(new)
    }
    pub fn pop_front(&mut self) -> Option<NyarValue> {
        self.inner.pop_front()
    }
    pub fn push_back(&mut self, new: NyarValue) {
        self.push_front(new)
    }
}

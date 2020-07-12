mod attributes;
mod prototype;

use self::{attributes::NyarFunctionAttributes, prototype::NyarFunction};
use crate::Result;
use indexmap::IndexMap;
use std::{collections::HashMap, lazy::Lazy, rc::Rc};

#[derive(Debug, Clone)]
pub struct EffectHandler {
    effects: HashMap<String, String>,
}
#[derive(Debug, Clone)]
pub struct Argument;
#[derive(Debug, Clone)]
pub struct Statement;
#[derive(Debug, Clone)]
pub struct Typing;
#[derive(Debug, Clone)]
pub enum Value {}

#[derive(Debug, Clone)]
struct FunctionInstance {
    prototype: Rc<NyarFunction>,
    args: Vec<Value>,
    kvs: IndexMap<String, Value>,
}

impl FunctionInstance {
    pub fn new(f: NyarFunction) -> Self {
        Self { prototype: Rc::new(f), args: vec![], kvs: IndexMap::new() }
    }
    pub fn fill_arguments(&mut self, args: Vec<Value>) -> Result<()> {
        self.check_valid()?;
        // The non-curried function must fill all parameters at once!
        if !self.is_currying() {
            self.check_ready()?
        }
        Ok(())
    }
    pub fn fill_named_arguments(&mut self) -> Result<()> {
        self.check_valid()?;
        // The non-curried function must fill all parameters at once!
        if !self.is_currying() {
            self.check_ready()?
        }
        Ok(())
    }

    pub fn check_valid(&self) -> Result<()> {
        Ok(())
    }
    pub fn check_ready(&self) -> Result<()> {
        Ok(())
    }
    pub fn evaluate(&self) -> Result<Value> {
        self.check_ready()?;
        unimplemented!()
    }
}

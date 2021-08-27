use std::{collections::HashMap, rc::Rc};

use indexmap::IndexMap;

use crate::{
    value::{error::ErrorLevels, Value},
    NyarError3, Result,
};

pub use self::{attributes::FunctionAttributes, prototype::FunctionPrototype};

mod attributes;
mod instances;
mod prototype;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct EffectHandler {
    effects: HashMap<String, String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Statement;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FunctionInstance {
    prototype: Rc<FunctionPrototype>,
    args: Vec<Value>,
    kvs: IndexMap<String, Value>,
}

impl FunctionInstance {
    pub fn new(f: Rc<FunctionPrototype>) -> Self {
        Self { prototype: f, args: vec![], kvs: IndexMap::new() }
    }
    pub fn fill_arguments(&mut self, args: Vec<Value>) -> Result<()> {
        self.args.extend(args);
        self.check_valid()?;
        // The non-curried function must fill all parameters at once!
        if !self.is_currying() {
            self.check_ready()?
        }
        Ok(())
    }
    pub fn fill_named_arguments(&mut self, args: HashMap<String, Value>) -> Result<()> {
        match self.allow_override_keywords() {
            ErrorLevels::Allow => self.kvs.extend(args),
            ErrorLevels::Warning => {
                for (k, v) in args.into_iter() {
                    if self.kvs.contains_key(k.as_str()) {
                        println!("noooop!")
                    }
                    self.kvs.insert(k, v);
                }
            }
            ErrorLevels::Deny => {
                for (k, v) in args.into_iter() {
                    if self.kvs.contains_key(k.as_str()) {
                        return Err(NyarError3::msg("GG"));
                    }
                    self.kvs.insert(k, v);
                }
            }
        }
        self.check_valid()?;
        // The non-curried function must fill all parameters at once!
        if !self.is_currying() {
            self.check_ready()?
        }
        Ok(())
    }

    pub fn check_valid(&self) -> Result<()> {
        match self.allow_extra_arguments() {
            ErrorLevels::Allow => {}
            ErrorLevels::Warning => {}
            ErrorLevels::Deny => {}
        }
        match self.allow_extra_keywords() {
            ErrorLevels::Allow => {}
            ErrorLevels::Warning => {}
            ErrorLevels::Deny => {}
        }
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
